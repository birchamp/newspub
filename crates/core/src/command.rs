//! Document commands: every mutation of a [`Document`] is one of these.
//!
//! Interface rule (ARCHITECTURE.md §4): only the lead changes this enum; record changes in §11.
//!
//! Conventions:
//! - `page` fields are **0-based page indices**; `master` fields are master ids.
//! - `target` on text commands may name a story or any text frame of the story.
//! - Text positions are char indices into the story; `start`/`end` default to the whole story.

use crate::attrs::{CharAttrs, ParaAttrs};
use crate::color::Color;
use crate::model::*;
use crate::units::{Insets, Length, Rect};
use crate::{CoreError, Id};
use serde::{Deserialize, Serialize};

/// Reference to a style by id or by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StyleRef {
    Id(Id),
    Name(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZOp {
    Front,
    Back,
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignEdge {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

/// What `align_objects` aligns against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignTo {
    /// The bounding box of the given objects.
    #[default]
    Selection,
    /// The page edges.
    Page,
    /// The page margin guides.
    Margins,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ObjectPatch {
    pub rect: Option<Rect>,
    pub rotation: Option<f64>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
    pub wrap: Option<Wrap>,
    pub alt_text: Option<String>,
    pub decorative: Option<bool>,
    pub locked: Option<bool>,
    pub name: Option<String>,
    pub layer: Option<Id>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextFramePatch {
    pub columns: Option<u32>,
    pub gutter: Option<Length>,
    pub insets: Option<Insets>,
    pub valign: Option<VAlign>,
    pub autofit: Option<Autofit>,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub continued_on: Option<bool>,
    pub continued_from: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShapePatch {
    pub kind: Option<ShapeKind>,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub arrow_start: Option<Arrow>,
    pub arrow_end: Option<Arrow>,
    /// Remove the fill.
    pub no_fill: bool,
    /// Remove the stroke.
    pub no_stroke: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImagePatch {
    pub crop: Option<CropFrac>,
    pub fit: Option<Fit>,
    pub asset: Option<Id>,
    pub stroke: Option<Stroke>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SetupPatch {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub margins: Option<Insets>,
    pub facing: Option<bool>,
    pub bleed: Option<Length>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    // ---- publication & pages ----
    SetPageSetup(SetupPatch),
    /// Insert `count` pages before index `at` (default: append).
    InsertPages {
        #[serde(default)]
        at: Option<usize>,
        #[serde(default = "one")]
        count: usize,
        #[serde(default)]
        master: Option<Id>,
    },
    DeletePage {
        page: usize,
    },
    MovePage {
        from: usize,
        to: usize,
    },
    /// Copies the page and its objects (text frames get copies of their stories) after it.
    DuplicatePage {
        page: usize,
    },
    SetPageBackground {
        page: usize,
        #[serde(default)]
        color: Option<Color>,
    },
    AddMaster {
        name: String,
    },
    RenameMaster {
        master: Id,
        name: String,
    },
    DeleteMaster {
        master: Id,
    },
    /// Apply `master` (or none) to the given pages (default: all pages).
    ApplyMaster {
        #[serde(default)]
        pages: Option<Vec<usize>>,
        master: Option<Id>,
    },
    SetIgnoreMaster {
        page: usize,
        ignore: bool,
    },
    SetMeta {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        author: Option<String>,
        #[serde(default)]
        lang: Option<String>,
    },

    // ---- objects ----
    AddTextFrame {
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        master: Option<Id>,
        rect: Rect,
        #[serde(default)]
        columns: Option<u32>,
        #[serde(default)]
        gutter: Option<Length>,
    },
    AddShape {
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        master: Option<Id>,
        rect: Rect,
        kind: ShapeKind,
        #[serde(default)]
        fill: Option<Color>,
        #[serde(default)]
        stroke: Option<Stroke>,
    },
    /// Picture frame. With `asset: None` this is an empty picture placeholder.
    AddImage {
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        master: Option<Id>,
        rect: Rect,
        #[serde(default)]
        asset: Option<Id>,
    },
    SetObject {
        id: Id,
        patch: ObjectPatch,
    },
    MoveObjects {
        ids: Vec<Id>,
        dx: Length,
        dy: Length,
    },
    DeleteObjects {
        ids: Vec<Id>,
    },
    /// Moves objects to another page (keeping their rects).
    MoveToPage {
        ids: Vec<Id>,
        page: usize,
    },
    SetZ {
        id: Id,
        op: ZOp,
    },
    SetTextFrame {
        id: Id,
        patch: TextFramePatch,
    },
    SetShape {
        id: Id,
        patch: ShapePatch,
    },
    SetImage {
        id: Id,
        patch: ImagePatch,
    },
    /// Align objects' edges or centres. Locked objects are an error.
    AlignObjects {
        ids: Vec<Id>,
        edge: AlignEdge,
        #[serde(default)]
        relative: AlignTo,
    },
    /// Space objects evenly between the outermost two (≥ 3 objects), equal gaps along `axis`.
    DistributeObjects {
        ids: Vec<Id>,
        axis: Axis,
    },
    Group {
        ids: Vec<Id>,
    },
    Ungroup {
        id: Id,
    },

    // ---- text ----
    InsertText {
        target: Id,
        /// Char index; default end of story.
        #[serde(default)]
        at: Option<usize>,
        text: String,
        #[serde(default)]
        attrs: Option<CharAttrs>,
    },
    DeleteText {
        target: Id,
        start: usize,
        end: usize,
    },
    /// Replace chars `start..end` with `text`; the new text takes the formatting of the first replaced char.
    ReplaceText {
        target: Id,
        start: usize,
        end: usize,
        text: String,
    },
    FormatChars {
        target: Id,
        #[serde(default)]
        start: Option<usize>,
        #[serde(default)]
        end: Option<usize>,
        attrs: CharAttrs,
    },
    /// Remove run overrides (keep character style unless `keep_style` is false).
    ClearCharFormat {
        target: Id,
        #[serde(default)]
        start: Option<usize>,
        #[serde(default)]
        end: Option<usize>,
    },
    FormatParas {
        target: Id,
        #[serde(default)]
        start: Option<usize>,
        #[serde(default)]
        end: Option<usize>,
        attrs: ParaAttrs,
    },
    /// Link `to` after `from` in `from`'s chain. `to` must be an empty, unlinked text frame.
    LinkFrames {
        from: Id,
        to: Id,
    },
    /// Break the chain after `frame`. Frames after it get a new, empty story; the text
    /// that was displayed there stays in the story (as overflow of `frame`).
    UnlinkFrame {
        frame: Id,
    },

    // ---- styles ----
    /// Create a paragraph style, or update the one with this name.
    DefineParaStyle {
        name: String,
        #[serde(default)]
        based_on: Option<StyleRef>,
        #[serde(default)]
        next: Option<StyleRef>,
        #[serde(default)]
        para: ParaAttrs,
        #[serde(default)]
        chars: CharAttrs,
    },
    DefineCharStyle {
        name: String,
        #[serde(default)]
        based_on: Option<StyleRef>,
        #[serde(default)]
        chars: CharAttrs,
    },
    DeleteStyle {
        style: StyleRef,
    },
    ApplyParaStyle {
        target: Id,
        #[serde(default)]
        start: Option<usize>,
        #[serde(default)]
        end: Option<usize>,
        style: Option<StyleRef>,
        /// Also clear paragraph and run overrides.
        #[serde(default)]
        clear_overrides: bool,
    },
    ApplyCharStyle {
        target: Id,
        #[serde(default)]
        start: Option<usize>,
        #[serde(default)]
        end: Option<usize>,
        style: Option<StyleRef>,
    },

    // ---- layers ----
    AddLayer {
        name: String,
    },
    SetLayer {
        layer: Id,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        visible: Option<bool>,
        #[serde(default)]
        locked: Option<bool>,
    },
}

fn one() -> usize {
    1
}

/// Result of applying a command.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Applied {
    /// Ids created by the command, primary first (e.g. frame, then its story).
    pub created: Vec<Id>,
}

impl Applied {
    fn ids(ids: Vec<Id>) -> Applied {
        Applied { created: ids }
    }
}

impl Document {
    /// Applies a command. On error the document is unchanged.
    pub fn apply(&mut self, cmd: &Command) -> Result<Applied, CoreError> {
        let mut scratch = self.clone();
        let out = scratch.apply_in_place(cmd)?;
        *self = scratch;
        Ok(out)
    }

    fn resolve_para_style(&self, r: &StyleRef) -> Result<Id, CoreError> {
        match r {
            StyleRef::Id(id) if self.styles.para.contains_key(id) => Ok(*id),
            StyleRef::Name(n) => self.find_para_style(n).ok_or_else(|| CoreError::NoSuchStyle(n.clone())),
            StyleRef::Id(id) => Err(CoreError::NoSuchStyle(id.0.to_string())),
        }
    }

    fn resolve_char_style(&self, r: &StyleRef) -> Result<Id, CoreError> {
        match r {
            StyleRef::Id(id) if self.styles.chars.contains_key(id) => Ok(*id),
            StyleRef::Name(n) => self.find_char_style(n).ok_or_else(|| CoreError::NoSuchStyle(n.clone())),
            StyleRef::Id(id) => Err(CoreError::NoSuchStyle(id.0.to_string())),
        }
    }

    fn check_page(&self, page: usize) -> Result<(), CoreError> {
        if page >= self.pages.len() {
            return Err(CoreError::NoSuchPage(page));
        }
        Ok(())
    }

    /// Adds `obj` to a page or master (front of z-order).
    fn place(&mut self, page: Option<usize>, master: Option<Id>, obj: Object) -> Result<Id, CoreError> {
        let id = obj.id;
        match (page, master) {
            (_, Some(m)) => {
                let mi = self.master_index(m).ok_or(CoreError::NoSuchMaster(m))?;
                self.masters[mi].objects.push(id);
            }
            (p, None) => {
                let p = p.unwrap_or(0);
                self.check_page(p)?;
                self.pages[p].objects.push(id);
            }
        }
        self.objects.insert(id, obj);
        Ok(id)
    }

    fn new_object(&mut self, rect: Rect, kind: ObjectKind) -> Object {
        let id = self.alloc();
        Object {
            id,
            name: String::new(),
            rect,
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            kind,
            wrap: Wrap::default(),
            alt_text: None,
            decorative: false,
            locked: false,
            layer: None,
            parent: None,
        }
    }

    /// Creates a text frame with a new empty story. Returns (frame, story).
    pub fn create_text_frame(
        &mut self,
        page: Option<usize>,
        master: Option<Id>,
        rect: Rect,
    ) -> Result<(Id, Id), CoreError> {
        let sid = self.alloc();
        let mut obj = self.new_object(rect, ObjectKind::Text(TextFrame::new(sid)));
        obj.wrap = Wrap { mode: WrapMode::None, distance: Length(0.0) };
        let fid = obj.id;
        let mut story = crate::story::Story::new(sid);
        story.frames.push(fid);
        self.stories.insert(sid, story);
        self.place(page, master, obj)?;
        Ok((fid, sid))
    }

    /// Adds an image asset. Bytes are not validated here (the engine decodes them).
    pub fn add_asset(&mut self, name: &str, mime: &str, bytes: std::sync::Arc<[u8]>, px_w: u32, px_h: u32) -> Id {
        let id = self.alloc();
        self.assets.insert(id, Asset { id, name: name.into(), mime: mime.into(), px_w, px_h, link: None, bytes });
        id
    }

    fn range_or_all(
        &self,
        story: Id,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Result<std::ops::Range<usize>, CoreError> {
        let len = self.story(story)?.len();
        let (s, e) = (start.unwrap_or(0), end.unwrap_or(len));
        if s > e || e > len {
            return Err(CoreError::BadRange { start: s, end: e, len });
        }
        Ok(s..e)
    }

    /// Removes an object and everything that depends on it (group children, its story when
    /// it was the story's only frame).
    fn remove_object(&mut self, id: Id) -> Result<(), CoreError> {
        let obj = self.objects.get(&id).cloned().ok_or(CoreError::NoSuchObject(id))?;
        if let Some(list) = self.z_list_mut(id) {
            list.retain(|x| *x != id);
        }
        match &obj.kind {
            ObjectKind::Group { children } => {
                for c in children.clone() {
                    if let Some(o) = self.objects.get_mut(&c) {
                        o.parent = None;
                    }
                    self.remove_object(c)?;
                }
            }
            ObjectKind::Text(t) => {
                if let Some(story) = self.stories.get_mut(&t.story) {
                    story.frames.retain(|f| *f != id);
                    if story.frames.is_empty() {
                        self.stories.remove(&t.story);
                    }
                }
            }
            _ => {}
        }
        self.objects.remove(&id);
        Ok(())
    }

    /// Deep-copies an object (and group children / text stories) onto a page.
    fn duplicate_object(
        &mut self,
        id: Id,
        page: usize,
        story_map: &mut std::collections::HashMap<Id, Id>,
    ) -> Result<Id, CoreError> {
        let src = self.object(id)?.clone();
        let nid = self.alloc();
        let mut copy = src.clone();
        copy.id = nid;
        match &mut copy.kind {
            ObjectKind::Text(t) => {
                let old = t.story;
                let new_sid = match story_map.get(&old) {
                    Some(s) => *s,
                    None => {
                        let s = self.alloc();
                        let mut st = self.story(old)?.clone();
                        st.id = s;
                        st.frames.clear();
                        self.stories.insert(s, st);
                        story_map.insert(old, s);
                        s
                    }
                };
                t.story = new_sid;
                self.story_mut(new_sid)?.frames.push(nid);
            }
            ObjectKind::Group { children } => {
                let kids = children.clone();
                let mut new_kids = vec![];
                for k in kids {
                    let nk = self.duplicate_object(k, page, story_map)?;
                    // duplicate_object pushed the child onto the page; move it under the group.
                    self.pages[page].objects.retain(|x| *x != nk);
                    self.object_mut(nk)?.parent = Some(nid);
                    new_kids.push(nk);
                }
                *children = new_kids;
            }
            _ => {}
        }
        copy.parent = None;
        self.objects.insert(nid, copy);
        self.pages[page].objects.push(nid);
        Ok(nid)
    }

    fn apply_in_place(&mut self, cmd: &Command) -> Result<Applied, CoreError> {
        use Command::*;
        match cmd {
            SetPageSetup(p) => {
                let s = &mut self.setup;
                if let Some(v) = p.width {
                    s.width = v;
                }
                if let Some(v) = p.height {
                    s.height = v;
                }
                if let Some(v) = p.margins {
                    s.margins = v;
                }
                if let Some(v) = p.facing {
                    s.facing = v;
                }
                if let Some(v) = p.bleed {
                    s.bleed = v;
                }
                if s.width.0 <= 0.0 || s.height.0 <= 0.0 {
                    return Err(CoreError::Invalid("page size must be positive".into()));
                }
                Ok(Applied::default())
            }
            InsertPages { at, count, master } => {
                let at = at.unwrap_or(self.pages.len());
                if at > self.pages.len() {
                    return Err(CoreError::NoSuchPage(at));
                }
                if let Some(m) = master {
                    self.master_index(*m).ok_or(CoreError::NoSuchMaster(*m))?;
                }
                // New pages inherit the master of the page before them unless one is given.
                let inherit =
                    master.or_else(|| at.checked_sub(1).and_then(|i| self.pages.get(i)).and_then(|p| p.master));
                let mut ids = vec![];
                for k in 0..*count {
                    let id = self.alloc();
                    self.pages.insert(at + k, Page { id, master: inherit, ..Default::default() });
                    ids.push(id);
                }
                Ok(Applied::ids(ids))
            }
            DeletePage { page } => {
                self.check_page(*page)?;
                if self.pages.len() == 1 {
                    return Err(CoreError::Invalid("cannot delete the last page".into()));
                }
                for id in self.pages[*page].objects.clone() {
                    self.remove_object(id)?;
                }
                self.pages.remove(*page);
                Ok(Applied::default())
            }
            MovePage { from, to } => {
                self.check_page(*from)?;
                self.check_page(*to)?;
                let p = self.pages.remove(*from);
                self.pages.insert(*to, p);
                Ok(Applied::default())
            }
            DuplicatePage { page } => {
                self.check_page(*page)?;
                let src = self.pages[*page].clone();
                let id = self.alloc();
                self.pages.insert(page + 1, Page { id, objects: vec![], ..src.clone() });
                let mut map = std::collections::HashMap::new();
                let mut created = vec![id];
                for o in &src.objects {
                    created.push(self.duplicate_object(*o, page + 1, &mut map)?);
                }
                Ok(Applied::ids(created))
            }
            SetPageBackground { page, color } => {
                self.check_page(*page)?;
                self.pages[*page].background = color.clone();
                Ok(Applied::default())
            }
            AddMaster { name } => {
                let id = self.alloc();
                self.masters.push(Master { id, name: name.clone(), background: None, objects: vec![] });
                Ok(Applied::ids(vec![id]))
            }
            RenameMaster { master, name } => {
                let i = self.master_index(*master).ok_or(CoreError::NoSuchMaster(*master))?;
                self.masters[i].name = name.clone();
                Ok(Applied::default())
            }
            DeleteMaster { master } => {
                let i = self.master_index(*master).ok_or(CoreError::NoSuchMaster(*master))?;
                for id in self.masters[i].objects.clone() {
                    self.remove_object(id)?;
                }
                self.masters.remove(i);
                for p in &mut self.pages {
                    if p.master == Some(*master) {
                        p.master = None;
                    }
                }
                Ok(Applied::default())
            }
            ApplyMaster { pages, master } => {
                if let Some(m) = master {
                    self.master_index(*m).ok_or(CoreError::NoSuchMaster(*m))?;
                }
                let targets: Vec<usize> = pages.clone().unwrap_or_else(|| (0..self.pages.len()).collect());
                for p in targets {
                    self.check_page(p)?;
                    self.pages[p].master = *master;
                }
                Ok(Applied::default())
            }
            SetIgnoreMaster { page, ignore } => {
                self.check_page(*page)?;
                self.pages[*page].ignore_master = *ignore;
                Ok(Applied::default())
            }
            SetMeta { title, author, lang } => {
                if let Some(t) = title {
                    self.meta.title = t.clone();
                }
                if let Some(a) = author {
                    self.meta.author = a.clone();
                }
                if let Some(l) = lang {
                    self.meta.lang = l.clone();
                }
                Ok(Applied::default())
            }
            AddTextFrame { page, master, rect, columns, gutter } => {
                let (fid, sid) = self.create_text_frame(*page, *master, *rect)?;
                if let ObjectKind::Text(t) = &mut self.object_mut(fid)?.kind {
                    if let Some(c) = columns {
                        t.columns = (*c).max(1);
                    }
                    if let Some(g) = gutter {
                        t.gutter = *g;
                    }
                }
                Ok(Applied::ids(vec![fid, sid]))
            }
            AddShape { page, master, rect, kind, fill, stroke } => {
                let shape = Shape {
                    kind: kind.clone(),
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                    arrow_start: Arrow::None,
                    arrow_end: Arrow::None,
                };
                let obj = self.new_object(*rect, ObjectKind::Shape(shape));
                let id = self.place(*page, *master, obj)?;
                Ok(Applied::ids(vec![id]))
            }
            AddImage { page, master, rect, asset } => {
                if let Some(a) = asset
                    && !self.assets.contains_key(a)
                {
                    return Err(CoreError::NoSuchAsset(*a));
                }
                let img = ImageFrame { asset: *asset, crop: CropFrac::default(), fit: Fit::Stretch, stroke: None };
                let mut obj = self.new_object(*rect, ObjectKind::Image(img));
                obj.wrap = Wrap { mode: WrapMode::Square, distance: Length(7.2) };
                let id = self.place(*page, *master, obj)?;
                Ok(Applied::ids(vec![id]))
            }
            SetObject { id, patch } => {
                let o = self.object_mut(*id)?;
                let p = patch.clone();
                // A locked object keeps its geometry; locking, naming, wrap etc. stay editable so it can be unlocked.
                let geometry = p.rect.is_some() || p.rotation.is_some() || p.flip_h.is_some() || p.flip_v.is_some();
                if geometry && o.locked {
                    return Err(CoreError::Locked(*id));
                }
                if let Some(v) = p.rect {
                    if v.w < 0.0 || v.h < 0.0 {
                        return Err(CoreError::Invalid("negative size".into()));
                    }
                    o.rect = v;
                }
                if let Some(v) = p.rotation {
                    o.rotation = v.rem_euclid(360.0);
                }
                if let Some(v) = p.flip_h {
                    o.flip_h = v;
                }
                if let Some(v) = p.flip_v {
                    o.flip_v = v;
                }
                if let Some(v) = p.wrap {
                    o.wrap = v;
                }
                if let Some(v) = p.alt_text {
                    o.alt_text = Some(v);
                }
                if let Some(v) = p.decorative {
                    o.decorative = v;
                }
                if let Some(v) = p.locked {
                    o.locked = v;
                }
                if let Some(v) = p.name {
                    o.name = v;
                }
                if let Some(v) = p.layer {
                    o.layer = Some(v);
                }
                Ok(Applied::default())
            }
            MoveObjects { ids, dx, dy } => {
                for id in ids {
                    let o = self.object_mut(*id)?;
                    if o.locked {
                        return Err(CoreError::Locked(*id));
                    }
                    o.rect.x += dx.0;
                    o.rect.y += dy.0;
                    if let ObjectKind::Group { children } = o.kind.clone() {
                        for c in children {
                            let co = self.object_mut(c)?;
                            co.rect.x += dx.0;
                            co.rect.y += dy.0;
                        }
                    }
                }
                Ok(Applied::default())
            }
            DeleteObjects { ids } => {
                for id in ids {
                    if self.object(*id)?.locked {
                        return Err(CoreError::Locked(*id));
                    }
                }
                for id in ids {
                    self.remove_object(*id)?;
                }
                Ok(Applied::default())
            }
            MoveToPage { ids, page } => {
                self.check_page(*page)?;
                for id in ids {
                    self.object(*id)?;
                    if let Some(l) = self.z_list_mut(*id) {
                        l.retain(|x| x != id);
                    }
                    self.pages[*page].objects.push(*id);
                }
                Ok(Applied::default())
            }
            SetZ { id, op } => {
                let list = self.z_list_mut(*id).ok_or(CoreError::NoSuchObject(*id))?;
                let i = list.iter().position(|x| x == id).ok_or(CoreError::NoSuchObject(*id))?;
                let v = list.remove(i);
                let j = match op {
                    ZOp::Front => list.len(),
                    ZOp::Back => 0,
                    ZOp::Forward => (i + 1).min(list.len()),
                    ZOp::Backward => i.saturating_sub(1),
                };
                list.insert(j, v);
                Ok(Applied::default())
            }
            SetTextFrame { id, patch } => {
                let ObjectKind::Text(t) = &mut self.object_mut(*id)?.kind else {
                    return Err(CoreError::NotText(*id));
                };
                let p = patch.clone();
                if let Some(v) = p.columns {
                    t.columns = v.max(1);
                }
                if let Some(v) = p.gutter {
                    t.gutter = v;
                }
                if let Some(v) = p.insets {
                    t.insets = v;
                }
                if let Some(v) = p.valign {
                    t.valign = v;
                }
                if let Some(v) = p.autofit {
                    t.autofit = v;
                    if v == Autofit::None {
                        t.fit_scale = 1.0;
                    }
                }
                if let Some(v) = p.fill {
                    t.fill = Some(v);
                }
                if let Some(v) = p.stroke {
                    t.stroke = Some(v);
                }
                if let Some(v) = p.continued_on {
                    t.continued_on = v;
                }
                if let Some(v) = p.continued_from {
                    t.continued_from = v;
                }
                Ok(Applied::default())
            }
            SetShape { id, patch } => {
                let ObjectKind::Shape(s) = &mut self.object_mut(*id)?.kind else {
                    return Err(CoreError::WrongKind(*id, "shape"));
                };
                let p = patch.clone();
                if let Some(v) = p.kind {
                    s.kind = v;
                }
                if let Some(v) = p.fill {
                    s.fill = Some(v);
                }
                if let Some(v) = p.stroke {
                    s.stroke = Some(v);
                }
                if let Some(v) = p.arrow_start {
                    s.arrow_start = v;
                }
                if let Some(v) = p.arrow_end {
                    s.arrow_end = v;
                }
                if p.no_fill {
                    s.fill = None;
                }
                if p.no_stroke {
                    s.stroke = None;
                }
                Ok(Applied::default())
            }
            SetImage { id, patch } => {
                if let Some(a) = patch.asset
                    && !self.assets.contains_key(&a)
                {
                    return Err(CoreError::NoSuchAsset(a));
                }
                let ObjectKind::Image(im) = &mut self.object_mut(*id)?.kind else {
                    return Err(CoreError::WrongKind(*id, "image"));
                };
                let p = patch.clone();
                if let Some(v) = p.crop {
                    let ok = |x: f64| (0.0..1.0).contains(&x);
                    if !(ok(v.left) && ok(v.right) && ok(v.top) && ok(v.bottom))
                        || v.left + v.right >= 1.0
                        || v.top + v.bottom >= 1.0
                    {
                        return Err(CoreError::Invalid("crop must leave a visible area".into()));
                    }
                    im.crop = v;
                }
                if let Some(v) = p.fit {
                    im.fit = v;
                }
                if let Some(v) = p.asset {
                    im.asset = Some(v);
                }
                if let Some(v) = p.stroke {
                    im.stroke = Some(v);
                }
                Ok(Applied::default())
            }
            AlignObjects { .. } | DistributeObjects { .. } | ReplaceText { .. } => {
                Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
            }
            Group { ids } => {
                if ids.len() < 2 {
                    return Err(CoreError::Invalid("group needs at least two objects".into()));
                }
                let owner = self.owner_of(ids[0]);
                let mut bounds: Option<Rect> = None;
                for id in ids {
                    if self.owner_of(*id) != owner {
                        return Err(CoreError::Invalid("grouped objects must share a page".into()));
                    }
                    let r = self.object(*id)?.rect;
                    bounds = Some(bounds.map_or(r, |b| b.union(&r)));
                }
                let list = self.z_list_mut(ids[0]).ok_or(CoreError::NoSuchObject(ids[0]))?;
                // Keep children in their existing z-order; the group takes the topmost slot.
                let mut ordered: Vec<Id> = list.iter().copied().filter(|x| ids.contains(x)).collect();
                let top = list.iter().rposition(|x| ids.contains(x)).unwrap_or(0);
                let gid = Id(self.next_id);
                let list = self.z_list_mut(ids[0]).ok_or(CoreError::NoSuchObject(ids[0]))?;
                list.insert(top + 1, gid);
                list.retain(|x| !ids.contains(x));
                let obj = self.new_object(bounds.unwrap_or_default(), ObjectKind::Group { children: vec![] });
                debug_assert_eq!(obj.id, gid);
                for c in &ordered {
                    self.object_mut(*c)?.parent = Some(gid);
                }
                let mut obj = obj;
                obj.kind = ObjectKind::Group { children: std::mem::take(&mut ordered) };
                self.objects.insert(gid, obj);
                Ok(Applied::ids(vec![gid]))
            }
            Ungroup { id } => {
                let ObjectKind::Group { children } = self.object(*id)?.kind.clone() else {
                    return Err(CoreError::WrongKind(*id, "group"));
                };
                let list = self.z_list_mut(*id).ok_or(CoreError::NoSuchObject(*id))?;
                let i = list.iter().position(|x| x == id).ok_or(CoreError::NoSuchObject(*id))?;
                list.remove(i);
                for (k, c) in children.iter().enumerate() {
                    list.insert(i + k, *c);
                }
                for c in &children {
                    self.object_mut(*c)?.parent = None;
                }
                self.objects.remove(id);
                Ok(Applied::ids(children))
            }
            InsertText { target, at, text, attrs } => {
                let sid = self.story_of(*target)?;
                let st = self.story_mut(sid)?;
                let at = at.unwrap_or(st.len());
                st.insert(at, text, attrs.clone())?;
                Ok(Applied::default())
            }
            DeleteText { target, start, end } => {
                let sid = self.story_of(*target)?;
                self.story_mut(sid)?.delete(*start..*end)?;
                Ok(Applied::default())
            }
            FormatChars { target, start, end, attrs } => {
                let sid = self.story_of(*target)?;
                let r = self.range_or_all(sid, *start, *end)?;
                if let Some(s) = attrs.style
                    && !self.styles.chars.contains_key(&s)
                {
                    return Err(CoreError::NoSuchStyle(s.0.to_string()));
                }
                self.story_mut(sid)?.format_chars(r, attrs)?;
                Ok(Applied::default())
            }
            ClearCharFormat { target, start, end } => {
                let sid = self.story_of(*target)?;
                let r = self.range_or_all(sid, *start, *end)?;
                self.story_mut(sid)?.set_chars(r, &CharAttrs::default())?;
                Ok(Applied::default())
            }
            FormatParas { target, start, end, attrs } => {
                let sid = self.story_of(*target)?;
                let r = self.range_or_all(sid, *start, *end)?;
                if let Some(s) = attrs.style
                    && !self.styles.para.contains_key(&s)
                {
                    return Err(CoreError::NoSuchStyle(s.0.to_string()));
                }
                self.story_mut(sid)?.format_paras(r, attrs)?;
                Ok(Applied::default())
            }
            LinkFrames { from, to } => {
                let from_story = self.story_of(*from)?;
                let to_story = self.story_of(*to)?;
                if !matches!(self.object(*from)?.kind, ObjectKind::Text(_)) {
                    return Err(CoreError::NotText(*from));
                }
                if from_story == to_story {
                    return Err(CoreError::Invalid("frames are already in the same story".into()));
                }
                let ts = self.story(to_story)?;
                if !ts.is_empty() || ts.frames.len() > 1 {
                    return Err(CoreError::Invalid("the target frame must be empty and unlinked".into()));
                }
                self.stories.remove(&to_story);
                let st = self.story_mut(from_story)?;
                let pos = st.frames.iter().position(|f| f == from).ok_or(CoreError::NotText(*from))?;
                st.frames.insert(pos + 1, *to);
                if let ObjectKind::Text(t) = &mut self.object_mut(*to)?.kind {
                    t.story = from_story;
                }
                Ok(Applied::default())
            }
            UnlinkFrame { frame } => {
                let sid = self.story_of(*frame)?;
                let st = self.story_mut(sid)?;
                let pos = st.frames.iter().position(|f| f == frame).ok_or(CoreError::NotText(*frame))?;
                let tail: Vec<Id> = st.frames.split_off(pos + 1);
                if tail.is_empty() {
                    return Ok(Applied::default());
                }
                let nsid = self.alloc();
                let mut ns = crate::story::Story::new(nsid);
                ns.frames = tail.clone();
                self.stories.insert(nsid, ns);
                for f in tail {
                    if let ObjectKind::Text(t) = &mut self.object_mut(f)?.kind {
                        t.story = nsid;
                    }
                }
                Ok(Applied::ids(vec![nsid]))
            }
            DefineParaStyle { name, based_on, next, para, chars } => {
                let based = based_on.as_ref().map(|r| self.resolve_para_style(r)).transpose()?;
                let nxt = next.as_ref().map(|r| self.resolve_para_style(r)).transpose()?;
                let mut para = para.clone();
                para.style = None;
                let mut chars = chars.clone();
                chars.style = None;
                if let Some(id) = self.find_para_style(name) {
                    if based == Some(id) {
                        return Err(CoreError::Invalid("a style cannot be based on itself".into()));
                    }
                    let s = self.styles.para.get_mut(&id).ok_or(CoreError::NoSuchStyle(name.clone()))?;
                    s.based_on = based.or(s.based_on);
                    s.next = nxt.or(s.next);
                    s.para.overlay(&para);
                    s.chars.overlay(&chars);
                    return Ok(Applied::ids(vec![id]));
                }
                let id = self.alloc();
                self.styles
                    .para
                    .insert(id, ParaStyle { id, name: name.clone(), based_on: based, next: nxt, para, chars });
                Ok(Applied::ids(vec![id]))
            }
            DefineCharStyle { name, based_on, chars } => {
                let based = based_on.as_ref().map(|r| self.resolve_char_style(r)).transpose()?;
                let mut chars = chars.clone();
                chars.style = None;
                if let Some(id) = self.find_char_style(name) {
                    let s = self.styles.chars.get_mut(&id).ok_or(CoreError::NoSuchStyle(name.clone()))?;
                    s.based_on = based.or(s.based_on);
                    s.chars.overlay(&chars);
                    return Ok(Applied::ids(vec![id]));
                }
                let id = self.alloc();
                self.styles.chars.insert(id, CharStyle { id, name: name.clone(), based_on: based, chars });
                Ok(Applied::ids(vec![id]))
            }
            DeleteStyle { style } => {
                if let Ok(id) = self.resolve_para_style(style) {
                    self.styles.para.remove(&id);
                    for s in self.styles.para.values_mut() {
                        if s.based_on == Some(id) {
                            s.based_on = None;
                        }
                        if s.next == Some(id) {
                            s.next = None;
                        }
                    }
                    for st in self.stories.values_mut() {
                        for p in &mut st.paras {
                            if p.style == Some(id) {
                                p.style = None;
                            }
                        }
                    }
                    return Ok(Applied::default());
                }
                let id = self.resolve_char_style(style)?;
                self.styles.chars.remove(&id);
                for s in self.styles.chars.values_mut() {
                    if s.based_on == Some(id) {
                        s.based_on = None;
                    }
                }
                for st in self.stories.values_mut() {
                    for sp in &mut st.chars {
                        if sp.attrs.style == Some(id) {
                            sp.attrs.style = None;
                        }
                    }
                    st.normalize();
                }
                Ok(Applied::default())
            }
            ApplyParaStyle { target, start, end, style, clear_overrides } => {
                let sid = self.story_of(*target)?;
                let r = self.range_or_all(sid, *start, *end)?;
                let id = style.as_ref().map(|s| self.resolve_para_style(s)).transpose()?;
                let st = self.story_mut(sid)?;
                let (a, b) = (st.para_index_at(r.start), st.para_index_at(r.end));
                for p in &mut st.paras[a..=b] {
                    if *clear_overrides {
                        *p = ParaAttrs::default();
                    }
                    p.style = id;
                }
                if *clear_overrides {
                    let ranges = st.para_ranges();
                    let full = ranges[a].start..ranges[b].end;
                    st.set_chars(full, &CharAttrs::default())?;
                }
                Ok(Applied::default())
            }
            ApplyCharStyle { target, start, end, style } => {
                let sid = self.story_of(*target)?;
                let r = self.range_or_all(sid, *start, *end)?;
                let id = style.as_ref().map(|s| self.resolve_char_style(s)).transpose()?;
                let st = self.story_mut(sid)?;
                // Setting `style` to None must clear it, which overlay cannot do; edit directly.
                let mut runs: Vec<(std::ops::Range<usize>, CharAttrs)> = vec![];
                for (rr, a) in st.runs() {
                    let (s0, s1) = (rr.start.max(r.start), rr.end.min(r.end));
                    if s0 < s1 {
                        let mut a = a.clone();
                        a.style = id;
                        runs.push((s0..s1, a));
                    }
                }
                for (rr, a) in runs {
                    st.set_chars(rr, &a)?;
                }
                Ok(Applied::default())
            }
            AddLayer { name } => {
                let id = self.alloc();
                self.layers.push(Layer { id, name: name.clone(), visible: true, locked: false });
                Ok(Applied::ids(vec![id]))
            }
            SetLayer { layer, name, visible, locked } => {
                let l = self.layers.iter_mut().find(|l| l.id == *layer).ok_or(CoreError::NoSuchObject(*layer))?;
                if let Some(n) = name {
                    l.name = n.clone();
                }
                if let Some(v) = visible {
                    l.visible = *v;
                }
                if let Some(v) = locked {
                    l.locked = *v;
                }
                Ok(Applied::default())
            }
        }
    }
}
