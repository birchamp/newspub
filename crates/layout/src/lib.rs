//! newpub-layout: text layout for stories flowing through chains of frames.
//! See ARCHITECTURE.md §5. Lead-owned.

pub mod fonts;
mod para;
pub mod shape;
mod story;
mod text;

pub use fonts::{Face, FaceId, FontStore};

use newpub_core::{Color, Document, Id, ObjectKind};
use serde::Serialize;
use std::collections::HashMap;
use std::ops::Range;

/// Layout of a whole document.
#[derive(Clone, Debug, Default)]
pub struct DocLayout {
    pub frames: HashMap<Id, FrameLayout>,
    pub stories: HashMap<Id, StoryLayout>,
    /// Per-page layouts of master text frames whose stories contain fields: (frame, page index).
    pub page_frames: HashMap<(Id, usize), FrameLayout>,
}

impl DocLayout {
    /// Hash of every placed glyph, line, decoration and overflow point (positions to 1/1000 pt): equal
    /// fingerprints mean the same layout.
    pub fn fingerprint(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let q = |v: f64| (v * 1000.0).round() as i64;
        let frame = |h: &mut std::collections::hash_map::DefaultHasher, f: &FrameLayout| {
            (f.frame, &f.char_range, f.overflow, f.lines.len()).hash(h);
            for l in &f.lines {
                (l.column, q(l.x), q(l.width), q(l.top), q(l.height), q(l.baseline)).hash(h);
                (&l.char_range, l.para, l.hyphenated, l.runs.len()).hash(h);
                for r in &l.runs {
                    (format!("{:?}", r.face), q(r.size), q(r.x_scale), &r.text, r.glyphs.len()).hash(h);
                    for g in &r.glyphs {
                        (g.id, q(g.x), q(g.y), q(g.advance), &g.text_range, g.char_index, g.generated).hash(h);
                    }
                }
            }
            for d in &f.decorations {
                (d.kind == DecorationKind::Underline, q(d.x0), q(d.x1), q(d.y), q(d.thickness)).hash(h);
            }
        };
        let mut ids: Vec<&Id> = self.frames.keys().collect();
        ids.sort();
        for id in ids {
            frame(&mut h, &self.frames[id]);
        }
        let mut keys: Vec<&(Id, usize)> = self.page_frames.keys().collect();
        keys.sort();
        for k in keys {
            k.hash(&mut h);
            frame(&mut h, &self.page_frames[k]);
        }
        let mut ids: Vec<&Id> = self.stories.keys().collect();
        ids.sort();
        for id in ids {
            let s = &self.stories[id];
            (id, s.overflow_at, &s.frames, q(s.scale)).hash(&mut h);
        }
        format!("{:016x}", h.finish())
    }

    /// Layout of `frame` as shown on page `page` (master frames with fields differ per page).
    pub fn frame_on(&self, frame: Id, page: Option<usize>) -> Option<&FrameLayout> {
        page.and_then(|p| self.page_frames.get(&(frame, p))).or_else(|| self.frames.get(&frame))
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct StoryLayout {
    pub story: Id,
    /// First char index that did not fit in the frame chain, if any.
    pub overflow_at: Option<usize>,
    pub frames: Vec<Id>,
    /// Font scale used (autofit), 1.0 normally.
    pub scale: f64,
}

#[derive(Clone, Debug, Default)]
pub struct FrameLayout {
    pub frame: Id,
    pub lines: Vec<Line>,
    /// Story chars displayed in this frame.
    pub char_range: Range<usize>,
    /// True when this is the last frame of a story that overflows.
    pub overflow: bool,
    /// Underlines and strike-throughs, frame-local.
    pub decorations: Vec<Decoration>,
}

/// One laid-out line (or line piece, when wrap splits a line around an object).
/// All coordinates are frame-local points (origin at the frame's top-left, unrotated).
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub column: usize,
    pub x: f64,
    /// Available width of the line slot.
    pub width: f64,
    pub top: f64,
    pub height: f64,
    pub baseline: f64,
    pub char_range: Range<usize>,
    pub para: usize,
    pub runs: Vec<GlyphRun>,
    pub hyphenated: bool,
}

#[derive(Clone, Debug)]
pub struct GlyphRun {
    pub face: FaceId,
    /// Font size in points (after super/subscript and autofit scaling).
    pub size: f64,
    pub color: Color,
    /// Horizontal glyph scale (1.0 = normal).
    pub x_scale: f64,
    pub synthetic_bold: bool,
    pub synthetic_italic: bool,
    /// Text effects (TY-18) drawn by the renderers.
    pub effects: newpub_core::TextEffects,
    /// Source text of the run; glyph `text_range`s index into it.
    pub text: String,
    pub glyphs: Vec<PGlyph>,
}

/// A positioned glyph. `x`/`y` are the frame-local pen position on the baseline.
#[derive(Clone, Debug)]
pub struct PGlyph {
    pub id: u16,
    pub x: f64,
    pub y: f64,
    pub advance: f64,
    pub text_range: Range<usize>,
    pub char_index: usize,
    /// Generated text (list markers): not part of the story, ignored by character queries.
    pub generated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strike,
}

#[derive(Clone, Debug)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
    pub thickness: f64,
    pub color: Color,
}

impl FrameLayout {
    /// Visible text of the frame, lines joined with '\n'.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            for r in &l.runs {
                let mut last: Option<Range<usize>> = None;
                for g in &r.glyphs {
                    if last.as_ref() != Some(&g.text_range) {
                        out.push_str(r.text.get(g.text_range.clone()).unwrap_or(""));
                        last = Some(g.text_range.clone());
                    }
                }
            }
        }
        out
    }
}

/// Lays out one story (no per-page master layouts). Used by the engine's autofit fixups.
pub fn layout_one(doc: &Document, fonts: &FontStore, story: Id) -> Option<(StoryLayout, Vec<FrameLayout>)> {
    let _pass = text::Pass::begin();
    doc.stories.get(&story).map(|s| story::layout_story(doc, fonts, s, None))
}

/// Height a table cell's text needs (content plus the cell's insets), at the cell's current width.
pub fn cell_natural_height(
    doc: &Document,
    fonts: &FontStore,
    table: &newpub_core::table::Table,
    row: usize,
    col: usize,
) -> f64 {
    let _pass = text::Pass::begin();
    let (Some(cell), Some(mut rect)) = (table.cell(row, col), table.cell_rect(row, col)) else { return 0.0 };
    let Some(story) = doc.stories.get(&cell.story) else { return 0.0 };
    rect.h = 1.0e6;
    let (_, frames) = story::layout_cell(doc, fonts, story, cell, rect, None);
    let bottom =
        frames.first().and_then(|f| f.lines.iter().map(|l| l.top + l.height).reduce(f64::max)).unwrap_or(rect.y);
    let used = (bottom - rect.y).max(0.0);
    if story.is_empty() { 0.0 } else { used + cell.insets.bottom.0 }
}

/// Saved story flows that let the next layout of a changed document redo only what the change affects
/// (PF-01): each story reflows from its first changed paragraph and stops once it rejoins its previous flow.
#[derive(Default)]
pub struct LayoutMemo {
    stories: HashMap<Id, story::StoryMemo>,
}

/// Key of the document-wide inputs to text layout.
fn doc_key(doc: &Document, fonts: &FontStore) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", (&doc.styles, &doc.color_scheme, &doc.font_scheme, &doc.baseline_grid)).hash(&mut h);
    (fonts as *const FontStore as usize, fonts.face_count()).hash(&mut h);
    h.finish()
}

/// Like `layout_one`, reusing and updating `memo`.
pub fn layout_one_with(
    doc: &Document,
    fonts: &FontStore,
    story: Id,
    memo: &mut LayoutMemo,
) -> Option<(StoryLayout, Vec<FrameLayout>)> {
    let _pass = text::Pass::begin();
    let s = doc.stories.get(&story)?;
    let mut slot = memo.stories.remove(&story);
    let out = story::layout_story_memo(doc, fonts, s, doc_key(doc, fonts), &mut slot);
    if let Some(m) = slot {
        memo.stories.insert(story, m);
    }
    Some(out)
}

/// Lays out every story in the document.
pub fn layout_document(doc: &Document, fonts: &FontStore) -> DocLayout {
    layout_document_in(doc, fonts, None)
}

/// Lays out every story, reusing and updating `memo` (incremental layout). With the environment variable
/// `NEWPUB_VERIFY_INCREMENTAL` set, every result is checked against a full layout (a testing aid).
pub fn layout_document_with(doc: &Document, fonts: &FontStore, memo: &mut LayoutMemo) -> DocLayout {
    let out = layout_document_in(doc, fonts, Some(memo));
    if std::env::var_os("NEWPUB_VERIFY_INCREMENTAL").is_some() {
        let full = layout_document(doc, fonts);
        assert_eq!(out.fingerprint(), full.fingerprint(), "incremental layout differs from a full layout");
    }
    out
}

fn layout_document_in(doc: &Document, fonts: &FontStore, mut memo: Option<&mut LayoutMemo>) -> DocLayout {
    let _pass = text::Pass::begin();
    let mut out = DocLayout::default();
    let dk = if memo.is_some() { doc_key(doc, fonts) } else { 0 };
    let mut kept = HashMap::new();
    for story in doc.stories.values() {
        let (sl, frames) = match memo.as_deref_mut() {
            Some(m) => {
                let mut slot = m.stories.remove(&story.id);
                let r = story::layout_story_memo(doc, fonts, story, dk, &mut slot);
                if let Some(s) = slot {
                    kept.insert(story.id, s);
                }
                r
            }
            None => story::layout_story(doc, fonts, story, None),
        };
        for f in frames {
            out.frames.insert(f.frame, f);
        }
        out.stories.insert(story.id, sl);
        // Master stories with fields are laid out once per page that shows them.
        let on_master =
            story.frames.first().is_some_and(|f| matches!(doc.owner_of(*f), Some(newpub_core::Owner::Master(_))));
        if on_master && doc.story_has_fields(story.id) {
            let master = story.frames.first().and_then(|f| match doc.owner_of(*f) {
                Some(newpub_core::Owner::Master(mi)) => doc.masters.get(mi).map(|m| m.id),
                _ => None,
            });
            for pi in 0..doc.pages.len() {
                if doc.master_for_page(pi).map(|m| Some(m.id) == master).unwrap_or(false) {
                    let (_, frames) = story::layout_story(doc, fonts, story, Some(pi));
                    for f in frames {
                        out.page_frames.insert((f.frame, pi), f);
                    }
                }
            }
        }
    }
    if let Some(m) = memo {
        m.stories = kept;
    }
    // Table cells: each visible cell's story in its cell box (table-local coordinates).
    for o in doc.objects.values() {
        let ObjectKind::Table(t) = &o.kind else { continue };
        let page = doc.page_of(o.id);
        for r in 0..t.rows() {
            for c in 0..t.cols() {
                let (Some(cell), Some(rect)) = (t.cell(r, c), t.cell_rect(r, c)) else { continue };
                if cell.covered {
                    continue;
                }
                let Some(story) = doc.stories.get(&cell.story) else { continue };
                let (sl, frames) = story::layout_cell(doc, fonts, story, cell, rect, page);
                for f in frames {
                    out.frames.insert(f.frame, f);
                }
                out.stories.insert(story.id, sl);
            }
        }
    }
    // Text frames without a story layout (should not happen) still get an entry.
    for o in doc.objects.values() {
        if let ObjectKind::Text(_) = o.kind {
            out.frames.entry(o.id).or_insert_with(|| FrameLayout { frame: o.id, ..Default::default() });
        }
    }
    out
}
