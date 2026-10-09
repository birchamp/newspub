//! Colour schemes and font schemes (BB-04, lead-owned).
//!
//! Objects and text may use scheme colours (`Color::Scheme`) and scheme fonts ("+major" for headings,
//! "+minor" for body text). Applying another scheme recolours / re-fonts everything that uses them.
//! The schemes below are original palettes.

use crate::color::Color;
use crate::{Applied, CoreError, Document};
use serde::{Deserialize, Serialize};

/// Font name that stands for the font scheme's heading font.
pub const MAJOR_FONT: &str = "+major";
/// Font name that stands for the font scheme's body font.
pub const MINOR_FONT: &str = "+minor";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemeSlot {
    /// Main (text) colour.
    Main,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Hyperlink,
    FollowedHyperlink,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorScheme {
    pub name: String,
    pub main: Color,
    pub accent1: Color,
    pub accent2: Color,
    pub accent3: Color,
    pub accent4: Color,
    pub accent5: Color,
    pub hyperlink: Color,
    pub followed_hyperlink: Color,
}

impl ColorScheme {
    pub fn slot(&self, s: SchemeSlot) -> &Color {
        match s {
            SchemeSlot::Main => &self.main,
            SchemeSlot::Accent1 => &self.accent1,
            SchemeSlot::Accent2 => &self.accent2,
            SchemeSlot::Accent3 => &self.accent3,
            SchemeSlot::Accent4 => &self.accent4,
            SchemeSlot::Accent5 => &self.accent5,
            SchemeSlot::Hyperlink => &self.hyperlink,
            SchemeSlot::FollowedHyperlink => &self.followed_hyperlink,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontScheme {
    pub name: String,
    /// Headings.
    pub major: String,
    /// Body text.
    pub minor: String,
}

fn hex(s: &str) -> Color {
    Color::parse(s).unwrap_or(Color::BLACK)
}

fn scheme(name: &str, c: [&str; 8]) -> ColorScheme {
    ColorScheme {
        name: name.into(),
        main: hex(c[0]),
        accent1: hex(c[1]),
        accent2: hex(c[2]),
        accent3: hex(c[3]),
        accent4: hex(c[4]),
        accent5: hex(c[5]),
        hyperlink: hex(c[6]),
        followed_hyperlink: hex(c[7]),
    }
}

/// Built-in colour schemes; the first is the default.
pub fn color_schemes() -> Vec<ColorScheme> {
    vec![
        scheme("Default", ["#000000", "#1f4e79", "#c55a11", "#548235", "#7f6000", "#7030a0", "#0563c1", "#954f72"]),
        // The built-in templates' own colours: ink text, navy headings and rules, a pale blue panel.
        scheme("Navy", ["#222222", "#1f3864", "#2e75b6", "#c55a11", "#7f7f7f", "#e8eef7", "#0563c1", "#954f72"]),
        scheme("Harvest", ["#3b2a1a", "#b5651d", "#d9a441", "#8a9a3b", "#7b3f00", "#e8d8b0", "#8a4b08", "#6b4e2e"]),
        scheme("Ocean", ["#0b2233", "#1b6ca8", "#2fa4c7", "#0f8b8d", "#a3d5e0", "#163e64", "#1565c0", "#4a6a8a"]),
        scheme("Meadow", ["#1d2b1a", "#4c8c2b", "#9cc65a", "#f2c14e", "#2e5e4e", "#e6f0d6", "#2e7d32", "#55704a"]),
        scheme("Slate", ["#202124", "#4a5560", "#7d8b99", "#b0bec5", "#c0392b", "#eceff1", "#2c5d8f", "#5f6b7a"]),
        scheme("Berry", ["#2a1029", "#8e2c6e", "#c94f7c", "#5b2a86", "#f3a6c8", "#f7e6ef", "#7b1fa2", "#8e5a7e"]),
        scheme("Sunrise", ["#2b1b17", "#e4572e", "#f3a712", "#ffc857", "#a23b72", "#fdf0d5", "#c0392b", "#8c5a3c"]),
    ]
}

/// Built-in font schemes (bundled fonts only); the first is the default.
pub fn font_schemes() -> Vec<FontScheme> {
    let f = |n: &str, a: &str, b: &str| FontScheme { name: n.into(), major: a.into(), minor: b.into() };
    vec![
        f("Default", "Carlito", "Carlito"),
        f("Classic", "Liberation Serif", "Liberation Serif"),
        f("Modern", "Liberation Sans", "Liberation Sans"),
        f("Contrast", "Liberation Sans", "Liberation Serif"),
        f("Editorial", "Liberation Serif", "Carlito"),
        f("Friendly", "DejaVu Sans", "Carlito"),
    ]
}

/// Colour of a slot in the default scheme (used when no document is at hand).
pub fn default_slot_color(s: SchemeSlot) -> Color {
    color_schemes()[0].slot(s).clone()
}

impl Document {
    pub fn color_scheme(&self) -> ColorScheme {
        self.color_scheme.clone().unwrap_or_else(|| color_schemes().remove(0))
    }

    pub fn font_scheme(&self) -> FontScheme {
        self.font_scheme.clone().unwrap_or_else(|| font_schemes().remove(0))
    }

    /// A concrete colour for `c` (scheme colours resolved, alpha kept).
    pub fn scheme_color(&self, c: &Color) -> Color {
        match c {
            Color::Scheme { slot, a } => self.color_scheme().slot(*slot).clone().with_alpha(*a),
            other => other.clone(),
        }
    }

    /// The real font family for `name` ("+major" / "+minor" resolved).
    pub fn scheme_font(&self, name: &str) -> String {
        match name {
            MAJOR_FONT => self.font_scheme().major,
            MINOR_FONT => self.font_scheme().minor,
            other => other.to_string(),
        }
    }

    /// Replaces scheme colours on objects, table cells and page/master backgrounds with concrete colours
    /// (text is resolved through `resolve_char`). Used to build the displayed document.
    pub fn resolve_object_colors(&mut self) {
        let cs = self.color_scheme();
        let fix = |c: &mut Color| {
            if let Color::Scheme { slot, a } = c {
                *c = cs.slot(*slot).clone().with_alpha(*a);
            }
        };
        let fix_opt = |c: &mut Option<Color>| {
            if let Some(c) = c {
                fix(c);
            }
        };
        for p in &mut self.pages {
            fix_opt(&mut p.background);
        }
        for m in &mut self.masters {
            fix_opt(&mut m.background);
        }
        for o in self.objects.values_mut() {
            if let Some(s) = &mut o.shadow {
                fix(&mut s.color);
            }
            match &mut o.kind {
                crate::ObjectKind::Text(t) => {
                    fix_opt(&mut t.fill);
                    if let Some(s) = &mut t.stroke {
                        fix(&mut s.color);
                    }
                }
                crate::ObjectKind::Shape(s) => {
                    fix_opt(&mut s.fill);
                    if let Some(st) = &mut s.stroke {
                        fix(&mut st.color);
                    }
                    if let Some(g) = &mut s.gradient {
                        for stop in &mut g.stops {
                            fix(&mut stop.color);
                        }
                    }
                }
                crate::ObjectKind::Image(im) => {
                    if let Some(s) = &mut im.stroke {
                        fix(&mut s.color);
                    }
                    fix_opt(&mut im.adjust.recolor);
                }
                crate::ObjectKind::Table(t) => {
                    for c in &mut t.cells {
                        fix_opt(&mut c.fill);
                        let b = &mut c.borders;
                        for s in [&mut b.top, &mut b.bottom, &mut b.left, &mut b.right].into_iter().flatten() {
                            fix(&mut s.color);
                        }
                    }
                }
                crate::ObjectKind::WordArt(w) => {
                    fix(&mut w.fill);
                    if let Some(st) = &mut w.outline {
                        fix(&mut st.color);
                    }
                    if let Some(g) = &mut w.gradient {
                        for stop in &mut g.stops {
                            fix(&mut stop.color);
                        }
                    }
                }
                crate::ObjectKind::Group { .. } => {}
            }
        }
    }

    /// Does anything outside text use a scheme colour? (Text resolves on its own.)
    pub fn uses_object_scheme_colors(&self) -> bool {
        let mut d = self.clone();
        d.resolve_object_colors();
        d.objects != self.objects || d.pages != self.pages || d.masters != self.masters
    }
}

pub(crate) fn apply(doc: &mut Document, cmd: &crate::Command) -> Result<Applied, CoreError> {
    use crate::Command::*;
    match cmd {
        ApplyColorScheme { name } => {
            let s = color_schemes()
                .into_iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| CoreError::Invalid(format!("no colour scheme {name:?}")))?;
            doc.color_scheme = Some(s);
            Ok(Applied::default())
        }
        ApplyFontScheme { name } => {
            let s = font_schemes()
                .into_iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| CoreError::Invalid(format!("no font scheme {name:?}")))?;
            doc.font_scheme = Some(s);
            Ok(Applied::default())
        }
        _ => Err(CoreError::Unsupported(format!("{cmd:?} is not a scheme operation"))),
    }
}
