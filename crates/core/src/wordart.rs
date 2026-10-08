//! WordArt-style text objects (TY-19).
//! Owner: Batch 4 task TEXTART. Model types are fixed (lead); `apply` and `styles` are placeholders.

use crate::color::Color;
use crate::model::{Gradient, Stroke};
use crate::{Applied, Command, CoreError, Document};
use serde::{Deserialize, Serialize};

/// Text drawn as glyph outlines stretched to fill the object's rect, then warped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordArt {
    pub text: String,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    pub fill: Color,
    /// Overrides `fill` when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Stroke>,
    #[serde(default)]
    pub warp: Warp,
}

pub fn default_font() -> String {
    "Liberation Sans".into()
}

/// How the text is bent inside its rect (rect-relative; `h` = rect height).
/// - `none`: one line, glyphs stretched to fill the whole rect.
/// - `arch_up`: glyph height 0.6 h; the baseline is an upward arc — the middle glyphs' tops touch the rect top,
///   the end glyphs' bottoms touch the rect bottom.
/// - `arch_down`: the mirror: end glyphs' tops at the rect top, the middle glyphs' bottoms at the rect bottom.
/// - `wave`: glyph height 0.7 h; the baseline is one sine period over the width (amplitude 0.15 h).
/// - `slant_up` / `slant_down`: glyph height 0.7 h; the baseline rises / falls linearly across the width.
/// - `inflate`: glyph heights grow from 0.5 h at the ends to h in the middle, vertically centred.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warp {
    #[default]
    None,
    ArchUp,
    ArchDown,
    Wave,
    SlantUp,
    SlantDown,
    Inflate,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WordArtPatch {
    pub text: Option<String>,
    pub font: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub fill: Option<Color>,
    pub gradient: Option<Gradient>,
    pub no_gradient: bool,
    pub outline: Option<Stroke>,
    pub no_outline: bool,
    pub warp: Option<Warp>,
}

/// A named preset (original designs): the WordArt it produces for a given text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordArtStyle {
    pub name: String,
    pub art: WordArt,
}

/// Built-in style presets.
pub fn styles() -> Vec<WordArtStyle> {
    vec![]
}

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}
