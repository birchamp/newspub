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

fn art(fill: Color, gradient: Option<Gradient>, outline: Option<Stroke>, warp: Warp, italic: bool) -> WordArt {
    WordArt { text: String::new(), font: default_font(), bold: true, italic, fill, gradient, outline, warp }
}

fn outline(color: Color, width: f64) -> Stroke {
    Stroke {
        color,
        width: crate::Length(width),
        dash: Default::default(),
        cap: Default::default(),
        join: crate::LineJoin::Round,
    }
}

/// Built-in style presets (original designs).
pub fn styles() -> Vec<WordArtStyle> {
    use crate::model::{GradientKind, GradientStop};
    let rgb = Color::rgb;
    let two = |angle: f64, a: Color, b: Color| Gradient {
        kind: GradientKind::Linear,
        angle,
        stops: vec![GradientStop { at: 0.0, color: a }, GradientStop { at: 1.0, color: b }],
    };
    let v = vec![
        ("Plain", art(Color::BLACK, None, None, Warp::None, false)),
        ("Bold outline", art(Color::WHITE, None, Some(outline(rgb(0x1f, 0x2a, 0x44), 2.0)), Warp::None, false)),
        (
            "Sunset gradient",
            art(
                rgb(0xe0, 0x60, 0x20),
                Some(two(90.0, rgb(0xff, 0xc0, 0x30), rgb(0xc0, 0x20, 0x70))),
                None,
                Warp::None,
                false,
            ),
        ),
        ("Arch", art(rgb(0x1f, 0x3a, 0x7a), None, None, Warp::ArchUp, false)),
        ("Wave", art(rgb(0x10, 0x8a, 0x8a), None, Some(outline(rgb(0x08, 0x40, 0x40), 1.0)), Warp::Wave, false)),
        ("Shadowed", art(rgb(0x2a, 0x2a, 0x2a), None, None, Warp::None, false)),
        ("Inflated", art(rgb(0xb0, 0x20, 0x30), None, Some(outline(rgb(0x50, 0x08, 0x10), 1.5)), Warp::Inflate, false)),
        ("Rising", art(rgb(0x2e, 0x7d, 0x32), None, None, Warp::SlantUp, true)),
    ];
    v.into_iter().map(|(name, art)| WordArtStyle { name: name.into(), art }).collect()
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::AddWordArt { page, master, rect, text, style } => {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                return Err(CoreError::Invalid("WordArt needs a positive size".into()));
            }
            let mut wa = match style {
                None => art(Color::BLACK, None, None, Warp::None, false),
                Some(name) => styles()
                    .into_iter()
                    .find(|s| s.name.eq_ignore_ascii_case(name))
                    .map(|s| s.art)
                    .ok_or_else(|| CoreError::Invalid(format!("no WordArt style named {name:?}")))?,
            };
            wa.text = text.clone();
            // Validate the target before allocating an object.
            if let Some(m) = master {
                doc.master_index(*m).ok_or(CoreError::NoSuchMaster(*m))?;
            } else {
                let p = page.unwrap_or(0);
                if p >= doc.pages.len() {
                    return Err(CoreError::NoSuchPage(p));
                }
            }
            let mut obj = doc.new_object(*rect, crate::ObjectKind::WordArt(wa));
            if style.as_deref().is_some_and(|n| n.eq_ignore_ascii_case("Shadowed")) {
                obj.shadow = Some(crate::model::Shadow {
                    dx: crate::Length(3.0),
                    dy: crate::Length(3.0),
                    blur: crate::Length(2.0),
                    color: Color::rgb(0, 0, 0).with_alpha(0.45),
                });
            }
            let id = doc.place(*page, *master, obj)?;
            Ok(Applied { created: vec![id] })
        }
        Command::SetWordArt { id, patch } => {
            let obj = doc.objects.get_mut(id).ok_or(CoreError::NoSuchObject(*id))?;
            let crate::ObjectKind::WordArt(w) = &mut obj.kind else {
                return Err(CoreError::WrongKind(*id, "WordArt object"));
            };
            if let Some(t) = &patch.text {
                w.text = t.clone();
            }
            if let Some(f) = &patch.font {
                w.font = f.clone();
            }
            if let Some(b) = patch.bold {
                w.bold = b;
            }
            if let Some(i) = patch.italic {
                w.italic = i;
            }
            if let Some(f) = &patch.fill {
                w.fill = f.clone();
            }
            if patch.no_gradient {
                w.gradient = None;
            } else if let Some(g) = &patch.gradient {
                w.gradient = Some(g.clone());
            }
            if patch.no_outline {
                w.outline = None;
            } else if let Some(o) = &patch.outline {
                w.outline = Some(o.clone());
            }
            if let Some(wp) = patch.warp {
                w.warp = wp;
            }
            Ok(Applied::default())
        }
        other => Err(CoreError::Unsupported(format!("{other:?} is not a WordArt command"))),
    }
}
