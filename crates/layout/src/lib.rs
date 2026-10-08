//! newpub-layout: text layout for stories flowing through chains of frames.
//! See ARCHITECTURE.md §5. Lead-owned.

pub mod fonts;
mod para;
pub mod shape;
mod story;

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

/// Lays out every story in the document.
pub fn layout_document(doc: &Document, fonts: &FontStore) -> DocLayout {
    let mut out = DocLayout::default();
    for story in doc.stories.values() {
        let (sl, frames) = story::layout_story(doc, fonts, story);
        for f in frames {
            out.frames.insert(f.frame, f);
        }
        out.stories.insert(story.id, sl);
    }
    // Text frames without a story layout (should not happen) still get an entry.
    for o in doc.objects.values() {
        if let ObjectKind::Text(_) = o.kind {
            out.frames.entry(o.id).or_insert_with(|| FrameLayout { frame: o.id, ..Default::default() });
        }
    }
    out
}
