//! Character and paragraph attributes. Every field is optional: `None` means inherit.
//! The same struct serves as a style definition, a run override, and a command patch.

use crate::Id;
use crate::color::Color;
use crate::units::Length;
use serde::{Deserialize, Serialize};

macro_rules! overlay {
    ($self:ident, $other:ident; $($f:ident),* $(,)?) => {
        $( if $other.$f.is_some() { $self.$f = $other.$f.clone(); } )*
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Baseline {
    Normal,
    Superscript,
    Subscript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Caps {
    Normal,
    SmallCaps,
    AllCaps,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CharAttrs {
    /// Character style applied to the run (only meaningful on runs, not inside style definitions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Tracking in 1/1000 em (positive = looser).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<f64>,
    /// Horizontal glyph scaling in percent (100 = normal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    /// Font kerning (OpenType `kern`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerning: Option<bool>,
    /// Standard ligatures (OpenType `liga`, `clig`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ligatures: Option<bool>,
    /// Discretionary ligatures (OpenType `dlig`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dlig: Option<bool>,
    /// Additional OpenType feature tags, e.g. `["ss01", "onum"]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Baseline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<Caps>,
    /// BCP-47 language tag for spelling and hyphenation ("zxx" = do not proof).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Hyperlink on this run (EX-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    /// This char is a field (it must be `field::FIELD_CHAR`); see core::field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<crate::field::Field>,
    /// Text effects (TY-18). `Some(empty)` removes inherited effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<TextEffects>,
}

/// Text effects drawn with the glyphs (TY-18). Lengths in points.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextEffects {
    /// Copy of the glyphs drawn behind them, offset by (dx, dy), optionally blurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
    /// Stroke around each glyph outline (centred on the outline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<TextOutline>,
    /// Soft halo around the glyphs: opacity 0.8 at the outline falling to 0 at `radius`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<TextGlow>,
    /// Mirror image below the baseline (gap at most 1 pt) whose opacity starts at this value (0–1)
    /// and fades to 0 over the glyph height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<f64>,
    /// Raised look: a 1 pt dark copy offset down-right and a light copy offset up-left behind the glyphs.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub emboss: bool,
    /// Sunken look: the emboss copies swapped (light down-right, dark up-left).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub engrave: bool,
}

impl TextEffects {
    pub fn is_empty(&self) -> bool {
        *self == TextEffects::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextShadow {
    pub dx: f64,
    pub dy: f64,
    #[serde(default)]
    pub blur: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextOutline {
    pub width: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextGlow {
    pub radius: f64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Url(String),
    /// Target page id.
    Page(crate::Id),
}

impl CharAttrs {
    /// Fields set in `other` replace fields in `self`.
    pub fn overlay(&mut self, other: &CharAttrs) {
        overlay!(self, other; style, font, size, bold, italic, underline, strike, color, tracking,
            scale, kerning, ligatures, dlig, features, baseline, caps, lang, link, field, effects);
    }
    pub fn is_empty(&self) -> bool {
        *self == CharAttrs::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
    /// Justify including the last line.
    JustifyAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineSpacing {
    /// Multiple of the font's natural line height (1.0 = single).
    Multiple(f64),
    /// Exact baseline-to-baseline distance.
    Exactly(Length),
    /// At least this distance.
    AtLeast(Length),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabAlign {
    Left,
    Center,
    Right,
    Decimal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabStop {
    pub pos: Length,
    #[serde(default = "tab_left")]
    pub align: TabAlign,
    /// Leader character, e.g. "." or "_".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leader: Option<char>,
}
fn tab_left() -> TabAlign {
    TabAlign::Left
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ListStyle {
    None,
    Bullet {
        #[serde(default = "bullet_char")]
        bullet: char,
        #[serde(default)]
        indent: Length,
    },
    Numbered {
        #[serde(default = "decimal")]
        format: NumberFormat,
        #[serde(default = "one_u32")]
        start: u32,
        /// Text after the number, e.g. "." or ")".
        #[serde(default = "dot")]
        suffix: String,
        #[serde(default)]
        indent: Length,
    },
}
fn bullet_char() -> char {
    '•'
}
fn decimal() -> NumberFormat {
    NumberFormat::Decimal
}
fn one_u32() -> u32 {
    1
}
fn dot() -> String {
    ".".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DropCap {
    /// Number of lines the drop cap spans.
    pub lines: u32,
    /// Number of characters dropped.
    #[serde(default = "one_u32")]
    pub chars: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParaAttrs {
    /// Paragraph style (only meaningful on paragraphs, not inside style definitions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<LineSpacing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_left: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_right: Option<Length>,
    /// First-line indent relative to `indent_left`; negative for hanging indents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent_first: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabStop>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<ListStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_cap: Option<DropCap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphenate: Option<bool>,
    /// Hyphenation zone: only hyphenate if the line would otherwise be this much short.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyphen_zone: Option<Length>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_with_next: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_together: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widow_control: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_to_baseline: Option<bool>,
    /// Right-to-left base direction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
}

impl ParaAttrs {
    pub fn overlay(&mut self, other: &ParaAttrs) {
        overlay!(self, other; style, align, line_spacing, space_before, space_after, indent_left,
            indent_right, indent_first, tabs, list, drop_cap, hyphenate, hyphen_zone,
            keep_with_next, keep_together, widow_control, align_to_baseline, rtl);
    }
}

/// Fully resolved character formatting.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedChar {
    pub font: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Color,
    pub tracking: f64,
    pub scale: f64,
    pub kerning: bool,
    pub ligatures: bool,
    pub dlig: bool,
    pub features: Vec<String>,
    pub baseline: Baseline,
    pub caps: Caps,
    pub lang: String,
    pub effects: TextEffects,
}

impl ResolvedChar {
    pub fn from_attrs(a: &CharAttrs) -> ResolvedChar {
        ResolvedChar {
            font: a.font.clone().unwrap_or_else(|| crate::DEFAULT_FONT.to_string()),
            size: a.size.map(|l| l.0).unwrap_or(crate::DEFAULT_SIZE),
            bold: a.bold.unwrap_or(false),
            italic: a.italic.unwrap_or(false),
            underline: a.underline.unwrap_or(false),
            strike: a.strike.unwrap_or(false),
            color: a.color.clone().unwrap_or(Color::BLACK),
            tracking: a.tracking.unwrap_or(0.0),
            scale: a.scale.unwrap_or(100.0),
            kerning: a.kerning.unwrap_or(true),
            ligatures: a.ligatures.unwrap_or(true),
            dlig: a.dlig.unwrap_or(false),
            features: a.features.clone().unwrap_or_default(),
            baseline: a.baseline.unwrap_or(Baseline::Normal),
            caps: a.caps.unwrap_or(Caps::Normal),
            lang: a.lang.clone().unwrap_or_else(|| "en-US".into()),
            effects: a.effects.clone().unwrap_or_default(),
        }
    }
}

/// Fully resolved paragraph formatting.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedPara {
    pub align: Align,
    pub line_spacing: LineSpacing,
    pub space_before: f64,
    pub space_after: f64,
    pub indent_left: f64,
    pub indent_right: f64,
    pub indent_first: f64,
    pub tabs: Vec<TabStop>,
    pub list: ListStyle,
    pub drop_cap: Option<DropCap>,
    pub hyphenate: bool,
    pub hyphen_zone: f64,
    pub keep_with_next: bool,
    pub keep_together: bool,
    pub widow_control: bool,
    pub align_to_baseline: bool,
    pub rtl: bool,
}

impl ResolvedPara {
    pub fn from_attrs(a: &ParaAttrs) -> ResolvedPara {
        ResolvedPara {
            align: a.align.unwrap_or(Align::Left),
            line_spacing: a.line_spacing.unwrap_or(LineSpacing::Multiple(1.0)),
            space_before: a.space_before.map(|l| l.0).unwrap_or(0.0),
            space_after: a.space_after.map(|l| l.0).unwrap_or(0.0),
            indent_left: a.indent_left.map(|l| l.0).unwrap_or(0.0),
            indent_right: a.indent_right.map(|l| l.0).unwrap_or(0.0),
            indent_first: a.indent_first.map(|l| l.0).unwrap_or(0.0),
            tabs: a.tabs.clone().unwrap_or_default(),
            list: a.list.clone().unwrap_or(ListStyle::None),
            drop_cap: a.drop_cap.clone(),
            hyphenate: a.hyphenate.unwrap_or(false),
            hyphen_zone: a.hyphen_zone.map(|l| l.0).unwrap_or(18.0),
            keep_with_next: a.keep_with_next.unwrap_or(false),
            keep_together: a.keep_together.unwrap_or(false),
            widow_control: a.widow_control.unwrap_or(true),
            align_to_baseline: a.align_to_baseline.unwrap_or(false),
            rtl: a.rtl.unwrap_or(false),
        }
    }
}
