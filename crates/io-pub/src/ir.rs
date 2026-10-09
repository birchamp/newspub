//! Format-neutral intermediate form: what the version-specific readers produce and `build` turns into a
//! newpub document. Geometry is in EMU (12700 per point) with the origin at the page centre.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Low three bytes of `v` as red, green, blue.
    pub fn from_rgb24(v: u32) -> Rgb {
        Rgb((v & 0xff) as u8, ((v >> 8) & 0xff) as u8, ((v >> 16) & 0xff) as u8)
    }
}

/// Resolves a 2002+ colour reference: type byte 8 indexes the publication palette, anything else is RGB in the
/// low three bytes. `None` for an out-of-range palette index.
pub(crate) fn resolve_color(v: u32, palette: &[Rgb]) -> Option<Rgb> {
    if v >> 24 == 0x08 { palette.get((v & 0x00ff_ffff) as usize).copied() } else { Some(Rgb::from_rgb24(v)) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dash {
    Solid,
    Dashed,
    Dotted,
    /// Dash-dot patterns.
    Mixed,
    LongDashed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineSpec {
    pub color: Rgb,
    pub width_emu: i64,
    pub dash: Dash,
}

/// Geometry of a shape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Geom {
    Rect,
    RoundRect,
    Ellipse,
    Line,
    Triangle,
    RightTriangle,
    Diamond,
    Polygon(u32),
    Star(u32),
    Arrow,
    /// Picture frame: blip index (1-based, as in the shape property).
    Picture,
    TextBox,
    Table,
    /// WordArt (text-effect shapes): not imported.
    WordArt,
    /// An autoshape type this importer cannot draw.
    Unsupported(u16),
}

/// A text box, or the text of a table, tied to a story by its text id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextBind {
    pub text_id: u32,
    /// Position in the chain of linked boxes (0 = first).
    pub chain_pos: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableBind {
    pub rows: usize,
    pub cols: usize,
    pub col_widths: Vec<i64>,
    pub row_heights: Vec<i64>,
    /// Cell rectangles in reading order (row0, row1, col0, col1 inclusive).
    pub cells: Vec<(usize, usize, usize, usize)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Shp {
    pub seq: u32,
    /// xs, ys, xe, ye in EMU relative to the page centre (already normalised so xs <= xe, ys <= ye).
    pub rect: [i64; 4],
    /// Clockwise degrees.
    pub rotation: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub geom: Geom,
    pub fill: Option<Rgb>,
    pub line: Option<LineSpec>,
    pub text: Option<TextBind>,
    pub table: Option<TableBind>,
    /// Blip index (1-based) for a picture.
    pub image: Option<usize>,
    /// Picture crop as fractions of the picture: left, top, right, bottom.
    pub crop: [f64; 4],
    /// Left, top, right, bottom text margins in EMU.
    pub insets: [i64; 4],
    /// 0 top, 1 middle, 2 bottom.
    pub valign: u8,
    pub columns: u32,
    pub column_gap: i64,
    /// Rounded-corner radius as a fraction of the shorter side (0..0.5).
    pub corner: f64,
}

impl Shp {
    pub fn new(seq: u32, rect: [i64; 4], geom: Geom) -> Shp {
        Shp {
            seq,
            rect,
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            geom,
            fill: None,
            line: None,
            text: None,
            table: None,
            image: None,
            crop: [0.0; 4],
            insets: [36576; 4],
            valign: 0,
            columns: 1,
            column_gap: 0,
            corner: 1.0 / 6.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Shape(Box<Shp>),
    Group { seq: u32, kids: Vec<Node> },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PageIr {
    pub nodes: Vec<Node>,
    /// Index into `Pub::masters`.
    pub master: Option<usize>,
}

/// Raw picture data from the BLIP store.
#[derive(Clone, Debug)]
pub(crate) struct Blip {
    pub kind: BlipKind,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlipKind {
    Png,
    Jpeg,
    Emf,
    Wmf,
    Dib,
    Tiff,
    /// Pict or CMYK JPEG: kept for the warning only.
    Other,
}

/// Text colour: an entry of the Quill colour table (2002+), or a reference given in the record (Publisher 98/2000).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextColor {
    Table(usize),
    /// A colour reference in the 2002 form (palette index or RGB), from a Publisher 98/2000 record.
    Ref(u32),
}

/// Character formatting as stored: every field is optional and relative to the default character style.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CharFmt {
    pub bold: bool,
    pub italic: bool,
    pub underline: Option<u8>,
    pub size_emu: Option<i64>,
    pub color: Option<TextColor>,
    pub font: Option<usize>,
    /// 1 superscript, 2 subscript.
    pub script: u8,
    pub outline: bool,
    pub shadow: bool,
    pub small_caps: bool,
    pub all_caps: bool,
    pub emboss: bool,
    pub engrave: bool,
    /// Horizontal scale in tenths of a percent.
    pub scale: Option<u32>,
    pub lcid: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ParaFmt {
    pub align: Option<u8>,
    pub style_index: Option<usize>,
    /// Raw line spacing value (odd: points in 1/8 EMU, 2 bit: lines in 96pt-font EMU).
    pub line_spacing: Option<u32>,
    pub space_before: Option<i64>,
    pub space_after: Option<i64>,
    pub first_indent: Option<i64>,
    pub left_indent: Option<i64>,
    pub right_indent: Option<i64>,
    pub tabs: Vec<i64>,
    pub drop_lines: Option<u32>,
    pub drop_chars: Option<u32>,
}

/// One story: text plus formatting runs. Runs end at an exclusive char index.
#[derive(Clone, Debug, Default)]
pub(crate) struct QStory {
    pub text_id: u32,
    pub chars: Vec<u16>,
    pub char_runs: Vec<(usize, CharFmt)>,
    pub para_runs: Vec<(usize, ParaFmt)>,
    /// Table text: index of each cell's terminating character; the last entry may equal the story length.
    pub cell_ends: Option<Vec<usize>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Text {
    pub stories: Vec<QStory>,
    pub fonts: Vec<String>,
    /// Raw colour references of the Quill colour table.
    pub color_refs: Vec<u32>,
    pub default_chars: Vec<CharFmt>,
    pub default_paras: Vec<ParaFmt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Generation {
    /// Publisher 97 (text in the `Contents` stream).
    V97,
    /// Publisher 98 and 2000.
    V2000,
    /// Publisher 2002 and later.
    V2002,
}

#[derive(Debug, Default)]
pub(crate) struct Pub {
    pub width_emu: i64,
    pub height_emu: i64,
    pub pages: Vec<PageIr>,
    pub masters: Vec<PageIr>,
    pub text: Text,
    /// Publication palette that colour references index.
    pub palette: Vec<Rgb>,
    pub blips: Vec<Option<Blip>>,
    /// Things seen but not imported, as (what, count).
    pub skipped: HashMap<&'static str, usize>,
    pub warnings: Vec<String>,
}

impl Pub {
    pub fn skip(&mut self, what: &'static str) {
        *self.skipped.entry(what).or_insert(0) += 1;
    }
}
