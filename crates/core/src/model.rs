//! The document model. See ARCHITECTURE.md §3.

use crate::attrs::{CharAttrs, ParaAttrs, ResolvedChar, ResolvedPara};
use crate::color::Color;
use crate::story::Story;
use crate::units::{Insets, Length, Rect};
use crate::{CoreError, Id};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageSetup {
    pub width: Length,
    pub height: Length,
    /// `left`/`right` are inside/outside when `facing` is true.
    pub margins: Insets,
    /// Two-page spreads: odd pages are right-hand pages and margins mirror.
    #[serde(default)]
    pub facing: bool,
    /// Bleed distance beyond the trim on each side.
    #[serde(default)]
    pub bleed: Length,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            width: Length(612.0),
            height: Length(792.0),
            margins: Insets::uniform(36.0),
            facing: false,
            bleed: Length(0.0),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master: Option<Id>,
    #[serde(default)]
    pub ignore_master: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    /// Objects on this page, back to front.
    pub objects: Vec<Id>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Master {
    pub id: Id,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    pub objects: Vec<Id>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Autofit {
    #[default]
    None,
    /// Shrink text size until the story fits.
    ShrinkOnOverflow,
    /// Grow or shrink text to fill the frame.
    BestFit,
    /// Grow the frame height to fit the text.
    GrowFrame,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextFrame {
    pub story: Id,
    #[serde(default = "one")]
    pub columns: u32,
    #[serde(default = "default_gutter")]
    pub gutter: Length,
    #[serde(default = "default_insets")]
    pub insets: Insets,
    #[serde(default)]
    pub valign: VAlign,
    #[serde(default)]
    pub autofit: Autofit,
    /// Font-size scale applied by autofit (1.0 = none). Computed by the engine.
    #[serde(default = "one_f64")]
    pub fit_scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    /// Show "Continued on page …" at the bottom when the story continues elsewhere.
    #[serde(default)]
    pub continued_on: bool,
    /// Show "Continued from page …" at the top when the story starts earlier.
    #[serde(default)]
    pub continued_from: bool,
}
fn one() -> u32 {
    1
}
fn one_f64() -> f64 {
    1.0
}
fn default_gutter() -> Length {
    Length(12.0)
}
fn default_insets() -> Insets {
    Insets::uniform(5.76) // 0.08 in, Publisher's default text box margins
}

impl TextFrame {
    pub fn new(story: Id) -> TextFrame {
        TextFrame {
            story,
            columns: 1,
            gutter: default_gutter(),
            insets: default_insets(),
            valign: VAlign::Top,
            autofit: Autofit::None,
            fit_scale: 1.0,
            fill: None,
            stroke: None,
            continued_on: false,
            continued_from: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dash {
    #[default]
    Solid,
    Dash,
    Dot,
    DashDot,
    LongDash,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrow {
    #[default]
    None,
    Triangle,
    Open,
    Stealth,
    Diamond,
    Oval,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    #[serde(default = "hairline")]
    pub width: Length,
    #[serde(default)]
    pub dash: Dash,
}
fn hairline() -> Length {
    Length(0.75)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    RoundRect {
        radius: Length,
    },
    Ellipse,
    /// Straight line from the rect's top-left to bottom-right (or flipped via `flip_*`).
    Line,
    Triangle,
    Star {
        points: u32,
        inner: f64,
    },
    Polygon {
        sides: u32,
    },
    Arrow,
    /// Free-form path in rect-relative unit coordinates (0..1).
    Path {
        points: Vec<[f64; 2]>,
        closed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
    #[serde(default)]
    pub arrow_start: Arrow,
    #[serde(default)]
    pub arrow_end: Arrow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Image scaled to the frame exactly (the frame keeps the image's aspect on insert).
    #[default]
    Stretch,
    /// Whole image visible, letterboxed.
    Fit,
    /// Frame filled, excess cropped.
    Fill,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageFrame {
    /// `None` for an empty picture placeholder.
    pub asset: Option<Id>,
    /// Crop from each side of the source image, as fractions 0–1 of its size.
    #[serde(default)]
    pub crop: CropFrac,
    #[serde(default)]
    pub fit: Fit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CropFrac {
    #[serde(default)]
    pub left: f64,
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub bottom: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectKind {
    Text(TextFrame),
    Shape(Shape),
    Image(ImageFrame),
    Group { children: Vec<Id> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapMode {
    /// Text ignores the object.
    #[default]
    None,
    /// Text wraps around the bounding box.
    Square,
    /// Text wraps around the object outline.
    Tight,
    /// Text stops above and resumes below the object.
    TopBottom,
    /// Like tight but text may fill open areas inside the shape.
    Through,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wrap {
    #[serde(default)]
    pub mode: WrapMode,
    /// Distance between the object and wrapped text.
    #[serde(default)]
    pub distance: Length,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: Id,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub rect: Rect,
    /// Clockwise degrees around the rect centre.
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    pub kind: ObjectKind,
    #[serde(default)]
    pub wrap: Wrap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub decorative: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Id>,
    /// Group containing this object, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParaStyle {
    pub id: Id,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Id>,
    #[serde(default)]
    pub para: ParaAttrs,
    #[serde(default)]
    pub chars: CharAttrs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharStyle {
    pub id: Id,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<Id>,
    #[serde(default)]
    pub chars: CharAttrs,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Styles {
    pub para: BTreeMap<Id, ParaStyle>,
    pub chars: BTreeMap<Id, CharStyle>,
}

/// Binary asset (image, embedded font). Bytes are stored outside the JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: Id,
    pub name: String,
    pub mime: String,
    /// Pixel size for images (0 for non-images).
    #[serde(default)]
    pub px_w: u32,
    #[serde(default)]
    pub px_h: u32,
    /// Path of a linked (not embedded) file, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip)]
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
}
fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub lang: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    #[serde(default)]
    pub meta: Meta,
    pub setup: PageSetup,
    pub pages: Vec<Page>,
    #[serde(default)]
    pub masters: Vec<Master>,
    pub objects: BTreeMap<Id, Object>,
    pub stories: BTreeMap<Id, Story>,
    #[serde(default)]
    pub styles: Styles,
    #[serde(default)]
    pub assets: BTreeMap<Id, Asset>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    pub next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Document::new(PageSetup::default(), 1)
    }
}

/// Where an object lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Page(usize),
    Master(usize),
    /// Child of a group.
    Group(Id),
}

impl Document {
    pub fn new(setup: PageSetup, pages: usize) -> Document {
        let mut d = Document {
            version: FORMAT_VERSION,
            meta: Meta::default(),
            setup,
            pages: vec![],
            masters: vec![],
            objects: BTreeMap::new(),
            stories: BTreeMap::new(),
            styles: Styles::default(),
            assets: BTreeMap::new(),
            layers: vec![],
            next_id: 1,
        };
        for _ in 0..pages.max(1) {
            let id = d.alloc();
            d.pages.push(Page { id, ..Default::default() });
        }
        d
    }

    pub fn alloc(&mut self) -> Id {
        let id = Id(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn page_index(&self, id: Id) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    pub fn master_index(&self, id: Id) -> Option<usize> {
        self.masters.iter().position(|m| m.id == id)
    }

    pub fn object(&self, id: Id) -> Result<&Object, CoreError> {
        self.objects.get(&id).ok_or(CoreError::NoSuchObject(id))
    }

    pub fn object_mut(&mut self, id: Id) -> Result<&mut Object, CoreError> {
        self.objects.get_mut(&id).ok_or(CoreError::NoSuchObject(id))
    }

    pub fn story(&self, id: Id) -> Result<&Story, CoreError> {
        self.stories.get(&id).ok_or(CoreError::NoSuchStory(id))
    }

    pub fn story_mut(&mut self, id: Id) -> Result<&mut Story, CoreError> {
        self.stories.get_mut(&id).ok_or(CoreError::NoSuchStory(id))
    }

    /// Resolves an id that names either a story or a text frame to the story id.
    pub fn story_of(&self, id: Id) -> Result<Id, CoreError> {
        if self.stories.contains_key(&id) {
            return Ok(id);
        }
        match &self.object(id)?.kind {
            ObjectKind::Text(t) => Ok(t.story),
            _ => Err(CoreError::NotText(id)),
        }
    }

    pub fn owner_of(&self, id: Id) -> Option<Owner> {
        if let Some(p) = self.objects.get(&id).and_then(|o| o.parent) {
            return Some(Owner::Group(p));
        }
        if let Some(i) = self.pages.iter().position(|p| p.objects.contains(&id)) {
            return Some(Owner::Page(i));
        }
        self.masters.iter().position(|m| m.objects.contains(&id)).map(Owner::Master)
    }

    /// The object list (z-order) that contains `id`.
    pub fn z_list_mut(&mut self, id: Id) -> Option<&mut Vec<Id>> {
        match self.owner_of(id)? {
            Owner::Page(i) => Some(&mut self.pages[i].objects),
            Owner::Master(i) => Some(&mut self.masters[i].objects),
            Owner::Group(g) => match &mut self.objects.get_mut(&g)?.kind {
                ObjectKind::Group { children } => Some(children),
                _ => None,
            },
        }
    }

    /// Page index (0-based) showing the frame, if it is on a page (not a master).
    pub fn page_of(&self, id: Id) -> Option<usize> {
        let mut cur = id;
        loop {
            match self.owner_of(cur)? {
                Owner::Page(i) => return Some(i),
                Owner::Master(_) => return None,
                Owner::Group(g) => cur = g,
            }
        }
    }

    /// Master applied to page `i` (respecting `ignore_master`).
    pub fn master_for_page(&self, i: usize) -> Option<&Master> {
        let p = self.pages.get(i)?;
        if p.ignore_master {
            return None;
        }
        p.master.and_then(|m| self.masters.iter().find(|x| x.id == m))
    }

    /// Margins of page `i` as (top, bottom, left, right), mirrored on facing left-hand pages.
    pub fn page_margins(&self, i: usize) -> (f64, f64, f64, f64) {
        let m = &self.setup.margins;
        let (inside, outside) = (m.left.0, m.right.0);
        // Page index 0 is page 1 (a right-hand page): inside is on the left.
        let right_hand = i.is_multiple_of(2);
        let (l, r) = if self.setup.facing && !right_hand { (outside, inside) } else { (inside, outside) };
        (m.top.0, m.bottom.0, l, r)
    }

    /// Effective char attrs: para style chain → char style chain → run overrides.
    pub fn effective_char_attrs(&self, para: &ParaAttrs, run: &CharAttrs) -> CharAttrs {
        let mut out = CharAttrs::default();
        for ps in self.para_style_chain(para.style) {
            out.overlay(&ps.chars);
        }
        for cs in self.char_style_chain(run.style) {
            out.overlay(&cs.chars);
        }
        out.overlay(run);
        out.style = run.style;
        out
    }

    pub fn resolve_char(&self, para: &ParaAttrs, run: &CharAttrs) -> ResolvedChar {
        ResolvedChar::from_attrs(&self.effective_char_attrs(para, run))
    }

    pub fn effective_para_attrs(&self, para: &ParaAttrs) -> ParaAttrs {
        let mut out = ParaAttrs::default();
        for ps in self.para_style_chain(para.style) {
            out.overlay(&ps.para);
        }
        out.overlay(para);
        out
    }

    pub fn resolve_para(&self, para: &ParaAttrs) -> ResolvedPara {
        ResolvedPara::from_attrs(&self.effective_para_attrs(para))
    }

    /// Style chain from the root ancestor down to `id` (cycle-safe).
    fn para_style_chain(&self, id: Option<Id>) -> Vec<&ParaStyle> {
        let mut chain = vec![];
        let mut cur = id;
        while let Some(i) = cur {
            let Some(s) = self.styles.para.get(&i) else { break };
            if chain.iter().any(|c: &&ParaStyle| c.id == s.id) || chain.len() > 32 {
                break;
            }
            chain.push(s);
            cur = s.based_on;
        }
        chain.reverse();
        chain
    }

    fn char_style_chain(&self, id: Option<Id>) -> Vec<&CharStyle> {
        let mut chain = vec![];
        let mut cur = id;
        while let Some(i) = cur {
            let Some(s) = self.styles.chars.get(&i) else { break };
            if chain.iter().any(|c: &&CharStyle| c.id == s.id) || chain.len() > 32 {
                break;
            }
            chain.push(s);
            cur = s.based_on;
        }
        chain.reverse();
        chain
    }

    pub fn find_para_style(&self, name: &str) -> Option<Id> {
        self.styles.para.values().find(|s| s.name == name).map(|s| s.id)
    }

    pub fn find_char_style(&self, name: &str) -> Option<Id> {
        self.styles.chars.values().find(|s| s.name == name).map(|s| s.id)
    }
}
