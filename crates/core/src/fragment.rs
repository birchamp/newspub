//! Fragments: self-contained copies of objects (with their stories, styles and pictures) that can be
//! pasted into any document — building blocks, clipboard (lead-owned).
//!
//! `Asset.bytes` is not serialized (as in the document model); whoever stores a fragment stores the
//! picture bytes alongside it and puts them back before pasting.

use crate::attrs::{CharAttrs, ParaAttrs};
use crate::model::{Asset, CharStyle, Object, ObjectKind, ParaStyle};
use crate::story::Story;
use crate::units::Rect;
use crate::{CoreError, Document, Id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fragment {
    /// Top-level objects, in z-order.
    pub roots: Vec<Id>,
    /// Every object (roots and group descendants).
    pub objects: Vec<Object>,
    pub stories: Vec<Story>,
    pub assets: Vec<Asset>,
    /// Styles the stories use (matched by name when pasting).
    #[serde(default)]
    pub para_styles: Vec<ParaStyle>,
    #[serde(default)]
    pub char_styles: Vec<CharStyle>,
    /// Union of the roots' rects.
    pub bounds: Rect,
}

impl Document {
    /// Copies objects (and everything they reference) into a fragment.
    pub fn extract_fragment(&self, ids: &[Id]) -> Result<Fragment, CoreError> {
        if ids.is_empty() {
            return Err(CoreError::Invalid("nothing to copy".into()));
        }
        let mut objects = vec![];
        let mut stories = BTreeSet::new();
        let mut assets = BTreeSet::new();
        let mut stack: Vec<Id> = ids.to_vec();
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let o = self.object(id)?.clone();
            match &o.kind {
                ObjectKind::Text(t) => {
                    stories.insert(t.story);
                }
                ObjectKind::Image(im) => assets.extend(im.asset),
                ObjectKind::Group { children } => stack.extend(children.iter().copied()),
                ObjectKind::Table(t) => stories.extend(t.cells.iter().map(|c| c.story)),
                ObjectKind::Shape(s) => stories.extend(s.story),
            }
            objects.push(o);
        }
        let stories: Vec<Story> = stories.into_iter().map(|s| self.story(s).cloned()).collect::<Result<_, _>>()?;
        let (mut ps, mut cs) = (BTreeSet::new(), BTreeSet::new());
        for st in &stories {
            ps.extend(st.paras.iter().filter_map(|p| p.style));
            cs.extend(st.chars.iter().filter_map(|c| c.attrs.style));
        }
        // Include the based-on chains so pasted styles look the same.
        let mut para_styles = vec![];
        let mut todo: Vec<Id> = ps.into_iter().collect();
        let mut done = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if let Some(s) = self.styles.para.get(&id)
                && done.insert(id)
            {
                todo.extend(s.based_on);
                todo.extend(s.next);
                para_styles.push(s.clone());
            }
        }
        let mut char_styles = vec![];
        let mut todo: Vec<Id> = cs.into_iter().collect();
        while let Some(id) = todo.pop() {
            if let Some(s) = self.styles.chars.get(&id)
                && done.insert(id)
            {
                todo.extend(s.based_on);
                char_styles.push(s.clone());
            }
        }
        let rects: Vec<Rect> = ids.iter().map(|i| self.object(*i).map(|o| o.rect)).collect::<Result<_, _>>()?;
        let x0 = rects.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
        let y0 = rects.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
        let x1 = rects.iter().map(|r| r.right()).fold(f64::NEG_INFINITY, f64::max);
        let y1 = rects.iter().map(|r| r.bottom()).fold(f64::NEG_INFINITY, f64::max);
        Ok(Fragment {
            roots: ids.to_vec(),
            objects,
            stories,
            assets: assets.into_iter().filter_map(|a| self.assets.get(&a).cloned()).collect(),
            para_styles,
            char_styles,
            bounds: Rect::new(x0, y0, x1 - x0, y1 - y0),
        })
    }

    /// Pastes a fragment onto `page` with its bounds' top-left at (x, y). Returns the new root ids.
    /// Styles are matched by name (missing ones are added); identical pictures are shared.
    pub fn paste_fragment(&mut self, f: &Fragment, page: usize, x: f64, y: f64) -> Result<Vec<Id>, CoreError> {
        self.pages.get(page).ok_or(CoreError::NoSuchPage(page))?;
        let (dx, dy) = (x - f.bounds.x, y - f.bounds.y);
        let mut map: HashMap<Id, Id> = HashMap::new();
        // Styles by name.
        for s in &f.para_styles {
            let id = match self.styles.para.values().find(|t| t.name == s.name) {
                Some(t) => t.id,
                None => self.alloc(),
            };
            map.insert(s.id, id);
        }
        for s in &f.char_styles {
            let id = match self.styles.chars.values().find(|t| t.name == s.name) {
                Some(t) => t.id,
                None => self.alloc(),
            };
            map.insert(s.id, id);
        }
        let remap = |m: &HashMap<Id, Id>, id: Option<Id>| id.and_then(|i| m.get(&i).copied());
        for s in &f.para_styles {
            let id = map[&s.id];
            if let std::collections::btree_map::Entry::Vacant(e) = self.styles.para.entry(id) {
                let mut n = s.clone();
                n.id = id;
                n.based_on = remap(&map, s.based_on);
                n.next = remap(&map, s.next);
                e.insert(n);
            }
        }
        for s in &f.char_styles {
            let id = map[&s.id];
            if let std::collections::btree_map::Entry::Vacant(e) = self.styles.chars.entry(id) {
                let mut n = s.clone();
                n.id = id;
                n.based_on = remap(&map, s.based_on);
                e.insert(n);
            }
        }
        // Pictures: reuse an identical asset, else add.
        for a in &f.assets {
            let existing = self.assets.values().find(|b| b.mime == a.mime && b.bytes == a.bytes && !a.bytes.is_empty());
            let id = match existing {
                Some(b) => b.id,
                None => {
                    let id = self.alloc();
                    self.assets.insert(id, Asset { id, ..a.clone() });
                    id
                }
            };
            map.insert(a.id, id);
        }
        for o in &f.objects {
            let id = self.alloc();
            map.insert(o.id, id);
        }
        for s in &f.stories {
            let id = self.alloc();
            map.insert(s.id, id);
        }
        let fix_chars = |m: &HashMap<Id, Id>, a: &mut CharAttrs, pages: &BTreeSet<Id>| {
            a.style = remap(m, a.style);
            if let Some(crate::attrs::Link::Page(p)) = &a.link
                && !pages.contains(p)
            {
                a.link = None;
            }
        };
        let pages: BTreeSet<Id> = self.pages.iter().map(|p| p.id).collect();
        for s in &f.stories {
            let mut n = s.clone();
            n.id = map[&s.id];
            n.frames = s.frames.iter().filter_map(|fr| map.get(fr).copied()).collect();
            for span in &mut n.chars {
                fix_chars(&map, &mut span.attrs, &pages);
            }
            if let Some(t) = &mut n.typing_attrs {
                fix_chars(&map, t, &pages);
            }
            for p in &mut n.paras {
                let ParaAttrs { style, .. } = p;
                *style = remap(&map, *style);
            }
            self.stories.insert(n.id, n);
        }
        let layers: BTreeSet<Id> = self.layers.iter().map(|l| l.id).collect();
        for o in &f.objects {
            let mut n = o.clone();
            n.id = map[&o.id];
            n.rect.x += dx;
            n.rect.y += dy;
            n.parent = remap(&map, o.parent);
            n.layer = o.layer.filter(|l| layers.contains(l));
            n.locked = false;
            match &mut n.kind {
                ObjectKind::Text(t) => t.story = map[&t.story],
                ObjectKind::Image(im) => im.asset = remap(&map, im.asset),
                ObjectKind::Group { children } => {
                    *children = children.iter().filter_map(|c| map.get(c).copied()).collect();
                }
                ObjectKind::Table(t) => {
                    for c in &mut t.cells {
                        c.story = map[&c.story];
                    }
                }
                ObjectKind::Shape(s) => s.story = remap(&map, s.story),
            }
            self.objects.insert(n.id, n);
        }
        let roots: Vec<Id> = f.roots.iter().map(|r| map[r]).collect();
        for r in &roots {
            if let Some(o) = self.objects.get_mut(r) {
                o.parent = None;
            }
        }
        self.pages[page].objects.extend(roots.iter().copied());
        Ok(roots)
    }
}
