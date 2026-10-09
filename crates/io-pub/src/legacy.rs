//! Publisher 98 and 2000 (format number 0x22): shapes are fixed-layout records in the `Contents` stream, text is in
//! the Quill stream (as in later versions). See FORMAT-NOTES.md, "Publisher 98 and 2000".
//!
//! `Contents` holds a chunk trailer (its offset at 0x16): a count, then for each chunk a 10-byte entry with the
//! chunk's id, its parent's id and its offset. The first u16 of a chunk is its type. Shape records keep their
//! rectangle at +6 (four i32 in EMU from the page centre) and their rotation at +4. Publisher 98 stores the fill
//! and the first line two bytes earlier than Publisher 2000; both share the file's format number, so the layout is
//! told apart from the records themselves.

use crate::bytes::{i32_at, u8_at, u16_at, u32_at};
use crate::ir::{Blip, BlipKind, Dash, Generation, Geom, LineSpec, Node, PageIr, Pub, Rgb, Shp, TableBind, TextBind};
use crate::quill::{self, Mode};
use std::collections::{HashMap, HashSet};

const DOCUMENT: u16 = 0x15;
const PAGE: u16 = 0x14;
const PALETTE: u16 = 0x47;
const IMAGE: u16 = 0x02;
const IMAGE_DATA: u16 = 0x21;
const LINE: u16 = 0x04;
const RECT: u16 = 0x05;
const AUTOSHAPE: u16 = 0x06;
const ELLIPSE: u16 = 0x07;
const TEXT: u16 = 0x08;
const TABLE: u16 = 0x0a;
const GROUP: u16 = 0x0f;

/// Page ids Publisher 2000 uses for internal pages that are never shown (and the master page).
const HIDDEN_PAGES: [u16; 6] = [0x116, 0x108, 0x10b, 0x10d, 0x119, 0x109];
const MAX_CHUNKS: usize = 20_000;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug)]
struct Chunk {
    id: u16,
    parent: u16,
    off: usize,
    end: usize,
    ty: u16,
}

/// Field offsets that differ between the two record layouts.
#[derive(Clone, Copy)]
struct Layout {
    fill_color: usize,
    fill_type: usize,
    first_line: usize,
}

const LAYOUT_2000: Layout = Layout { fill_color: 0x22, fill_type: 0x2a, first_line: 0x2c };
const LAYOUT_98: Layout = Layout { fill_color: 0x20, fill_type: 0x28, first_line: 0x2a };

/// The standard colours a Publisher 98/2000 colour index names.
const INDEXED: [(u8, u8, u8); 56] = [
    (0, 0, 0),
    (255, 255, 255),
    (255, 0, 0),
    (0, 255, 0),
    (0, 0, 255),
    (255, 255, 0),
    (0, 255, 255),
    (255, 0, 255),
    (128, 128, 128),
    (192, 192, 192),
    (128, 0, 0),
    (0, 128, 0),
    (0, 0, 128),
    (128, 128, 0),
    (0, 128, 128),
    (128, 0, 128),
    (255, 153, 51),
    (51, 0, 51),
    (0, 0, 153),
    (0, 153, 0),
    (153, 153, 0),
    (204, 102, 0),
    (153, 0, 0),
    (204, 153, 204),
    (102, 102, 255),
    (102, 255, 102),
    (255, 255, 153),
    (255, 204, 153),
    (255, 102, 102),
    (255, 153, 0),
    (0, 102, 255),
    (255, 204, 0),
    (153, 0, 51),
    (102, 51, 0),
    (66, 66, 66),
    (255, 153, 102),
    (153, 51, 0),
    (255, 102, 0),
    (51, 51, 0),
    (153, 204, 0),
    (255, 255, 153),
    (0, 51, 0),
    (51, 153, 102),
    (204, 255, 204),
    (0, 51, 102),
    (51, 204, 204),
    (204, 255, 255),
    (51, 102, 255),
    (0, 204, 255),
    (153, 204, 255),
    (51, 51, 153),
    (102, 102, 153),
    (153, 51, 102),
    (204, 153, 255),
    (51, 51, 51),
    (150, 150, 150),
];

/// A colour reference in the 2002 form: type 0x08 indexes the publication palette, anything else is RGB in the low
/// three bytes. `None` for a reference this importer does not understand.
pub(crate) fn translate_ref(v: u32) -> Option<u32> {
    let rgb = |r: u8, g: u8, b: u8| u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16;
    match v >> 24 {
        0xc0 | 0xe0 => Some(0x0800_0000 | (v & 0xff)),
        0x80 | 0x00 => INDEXED.get((v & 0xff) as usize).map(|&(r, g, b)| rgb(r, g, b)),
        0x90 | 0x20 => Some(v & 0x00ff_ffff),
        _ => None,
    }
}

fn color(v: u32, palette: &[Rgb]) -> Option<Rgb> {
    translate_ref(v).and_then(|r| crate::ir::resolve_color(r, palette))
}

/// Line width in EMU from its one-byte code (quarter points, with a compressed range above 0x81).
fn line_width(code: u8) -> i64 {
    let quarters = match code {
        0x81 => 0,
        c if c > 0x81 => {
            let k = i64::from(c - 0x81);
            (k / 3) * 4 + k % 3 + 1
        }
        c => i64::from(c) * 4,
    };
    quarters * 12_700 / 4
}

fn chunks(c: &[u8]) -> Option<Vec<Chunk>> {
    let trailer = u32_at(c, 0x16)? as usize;
    let n = usize::from(u16_at(c, trailer)?).min(MAX_CHUNKS);
    let mut out: Vec<Chunk> = Vec::with_capacity(n);
    for i in 0..n {
        let p = trailer + 2 + i * 10;
        let (id, parent, off) = (u16_at(c, p + 2)?, u16_at(c, p + 4)?, u32_at(c, p + 6)? as usize);
        let Some(ty) = u16_at(c, off) else { continue };
        if let Some(prev) = out.last_mut() {
            prev.end = off.max(prev.off);
        }
        out.push(Chunk { id, parent, off, end: c.len(), ty });
    }
    Some(out)
}

/// Votes over the shape records: Publisher 2000 puts the first line's colour right before the `fe ff` marker at
/// 0x31; Publisher 98 leaves two zero bytes there.
fn layout_of(c: &[u8], list: &[Chunk]) -> Layout {
    let (mut v98, mut v2000) = (0, 0);
    for k in list.iter().filter(|k| matches!(k.ty, RECT | TEXT | TABLE | IMAGE | ELLIPSE | AUTOSHAPE)) {
        let b = |i: usize| u8_at(c, k.off + i).unwrap_or(0);
        if b(0x31) != 0xfe || b(0x32) != 0xff {
            continue;
        }
        if b(0x2f) == 0 && b(0x30) == 0 && b(0x2e) != 0 {
            v98 += 1;
        } else if b(0x30) != 0 {
            v2000 += 1;
        }
    }
    if v98 > v2000 { LAYOUT_98 } else { LAYOUT_2000 }
}

fn rect_of(c: &[u8], k: &Chunk) -> Option<[i64; 4]> {
    let v = |i| i32_at(c, k.off + i).map(i64::from);
    let (xs, ys, xe, ye) = (v(6)?, v(10)?, v(14)?, v(18)?);
    Some([xs.min(xe), ys.min(ye), xs.max(xe), ys.max(ye)])
}

fn geom_of_autoshape(code: u8) -> Geom {
    match code {
        0x01 => Geom::RightTriangle,
        0x03 => Geom::Arrow,
        0x04 => Geom::Star(5),
        0x06 => Geom::Triangle,
        0x0a => Geom::Star(16),
        0x0c => Geom::Diamond,
        0x10 => Geom::Star(24),
        0x12 => Geom::Polygon(5),
        0x18 => Geom::Polygon(6),
        0x1e => Geom::Polygon(8),
        other => Geom::Unsupported(u16::from(other)),
    }
}

struct Reader<'a> {
    c: &'a [u8],
    layout: Layout,
    palette: Vec<Rgb>,
    kids: HashMap<u16, Vec<Chunk>>,
    /// Image data chunks in file order; a picture's blip index is its data chunk's position + 1.
    image_data: Vec<Chunk>,
    out: Pub,
}

impl Reader<'_> {
    fn line(&self, k: &Chunk, rectangular: bool) -> Option<LineSpec> {
        let side = |at: usize| -> Option<LineSpec> {
            let w = u8_at(self.c, k.off + at)?;
            if w == 0 {
                return None;
            }
            let col = color(u32_at(self.c, k.off + at + 1)?, &self.palette)?;
            Some(LineSpec { color: col, width_emu: line_width(w).max(3175), dash: Dash::Solid })
        };
        // Rectangles store four sides (left first, then top, right, bottom); one outline is kept.
        let first = side(self.layout.first_line);
        if !rectangular {
            return first;
        }
        first.or_else(|| side(0x35)).or_else(|| side(0x3b)).or_else(|| side(0x41))
    }

    fn fill(&self, k: &Chunk) -> Option<Rgb> {
        if u8_at(self.c, k.off + self.layout.fill_type)? != 2 {
            return None;
        }
        color(u32_at(self.c, k.off + self.layout.fill_color)?, &self.palette)
    }

    fn table(&self, k: &Chunk) -> Option<TableBind> {
        let n = usize::from(u16_at(self.c, k.off + 0x74)?).min(400);
        let mut ends = Vec::with_capacity(n);
        for i in 0..n {
            ends.push(i64::from(u32_at(self.c, k.off + 0x7e + 8 * i)?));
        }
        // Column boundaries come first and grow; the row boundaries start again from the top.
        let split = ends.windows(2).position(|w| w[1] <= w[0]).map_or(ends.len(), |i| i + 1);
        let sizes = |e: &[i64]| {
            e.iter().scan(0i64, |prev, &x| Some(std::mem::replace(prev, x)).map(|p| x - p)).collect::<Vec<_>>()
        };
        let (col_widths, row_heights) = (sizes(&ends[..split]), sizes(&ends[split..]));
        let (rows, cols) = (row_heights.len(), col_widths.len());
        if rows == 0 || cols == 0 || col_widths.iter().chain(&row_heights).any(|&v| v <= 0) {
            return None;
        }
        let cells = (0..rows).flat_map(|r| (0..cols).map(move |c| (r, r, c, c))).collect();
        Some(TableBind { rows, cols, col_widths, row_heights, cells })
    }

    fn shape(&mut self, k: &Chunk, depth: usize) -> Option<Node> {
        // Pages also own property chunks; only these record types are drawn.
        if !matches!(k.ty, GROUP | LINE | IMAGE | RECT | ELLIPSE | TEXT | TABLE | AUTOSHAPE) {
            return None;
        }
        let rect = rect_of(self.c, k)?;
        // Zero-size records are Publisher's default shapes, not content (a line may be zero in one direction).
        if rect[2] <= rect[0] && rect[3] <= rect[1] {
            return None;
        }
        if k.ty == GROUP {
            if depth >= MAX_DEPTH {
                return None;
            }
            let kids: Vec<Chunk> = self.kids.get(&k.id).cloned().unwrap_or_default();
            // Members keep page coordinates: groups do not transform their children.
            let nodes: Vec<Node> = kids.iter().filter_map(|kid| self.shape(kid, depth + 1)).collect();
            return Some(Node::Group { seq: u32::from(k.id), kids: nodes });
        }
        let geom = match k.ty {
            LINE => Geom::Line,
            IMAGE => Geom::Picture,
            RECT => Geom::Rect,
            ELLIPSE => Geom::Ellipse,
            TEXT => Geom::TextBox,
            TABLE => Geom::Table,
            _ => geom_of_autoshape(u8_at(self.c, k.off + 0x31)?),
        };
        let mut s = Shp::new(u32::from(k.id), rect, geom.clone());
        let tenths = f64::from(u16_at(self.c, k.off + 4)?);
        if k.ty != LINE && tenths > 0.0 {
            s.rotation = (360.0 - tenths / 10.0).rem_euclid(360.0);
        }
        let flags_at = match k.ty {
            LINE => Some(0x41),
            AUTOSHAPE => Some(0x33),
            _ => None,
        };
        if let Some(at) = flags_at.and_then(|at| u8_at(self.c, k.off + at)) {
            s.flip_v = at & 0x01 != 0;
            s.flip_h = at & 0x12 != 0;
        }
        let rectangular = matches!(k.ty, RECT | TEXT | IMAGE | TABLE);
        s.line = self.line(k, rectangular);
        match k.ty {
            IMAGE => {
                s.image = self.image_data.iter().position(|d| d.parent == k.id).map(|i| i + 1);
                if s.image.is_none() {
                    self.out.skip("pictures without data");
                }
            }
            TEXT | TABLE => {
                let id_at = if k.ty == TEXT { 0x58 } else { 0x66 };
                let text_id = u32::from(u16_at(self.c, k.off + id_at)?);
                s.text = Some(TextBind { text_id, chain_pos: 0 });
                if k.ty == TEXT {
                    // Inner margins in twips: left, top, right, bottom.
                    let m = |i: usize| u16_at(self.c, k.off + 0x46 + 2 * i).map_or(36_576, |v| i64::from(v) * 635);
                    s.insets = [m(0), m(1), m(2), m(3)];
                    s.fill = self.fill(k);
                } else {
                    s.table = self.table(k);
                    if s.table.is_none() {
                        self.out.skip("tables with an unreadable grid");
                        return None;
                    }
                }
            }
            _ => s.fill = self.fill(k),
        }
        if let Geom::Unsupported(_) = geom {
            self.out.skip("autoshapes of an unsupported kind");
        }
        Some(Node::Shape(Box::new(s)))
    }
}

pub(crate) fn read(contents: &[u8], quill: Option<&[u8]>, g: Generation) -> Result<Pub, String> {
    if g == Generation::V97 {
        return Err("Publisher 97 files are not supported".into());
    }
    let list = chunks(contents).ok_or("the Contents chunk table is damaged")?;
    let doc = list.iter().find(|k| k.ty == DOCUMENT).ok_or("the Contents stream has no document chunk")?;
    let width = i64::from(u32_at(contents, doc.off + 0x14).ok_or("damaged document chunk")?);
    let height = i64::from(u32_at(contents, doc.off + 0x18).ok_or("damaged document chunk")?);
    if !(12_700..=12_700 * 14_400).contains(&width) || !(12_700..=12_700 * 14_400).contains(&height) {
        return Err("the document chunk gives no usable page size".into());
    }
    let q = quill.ok_or("no Quill text stream")?;
    let text = quill::parse(q, Mode::V2000)?;

    let mut palette: Vec<Rgb> = Vec::new();
    if let Some(p) = list.iter().find(|k| k.ty == PALETTE) {
        for i in 0..8 {
            let v = u32_at(contents, p.off + 0xa0 + 4 * i).unwrap_or(0);
            let rgb = translate_ref(v).filter(|r| r >> 24 == 0).map_or(Rgb(0, 0, 0), Rgb::from_rgb24);
            palette.push(rgb);
        }
    }
    let mut kids: HashMap<u16, Vec<Chunk>> = HashMap::new();
    for k in &list {
        kids.entry(k.parent).or_default().push(*k);
    }
    let image_data: Vec<Chunk> = list.iter().copied().filter(|k| k.ty == IMAGE_DATA).collect();
    let blips: Vec<Option<Blip>> = image_data
        .iter()
        .map(|d| {
            let len = u32_at(contents, d.off + 4)? as usize;
            let data = contents.get(d.off + 8..(d.off + 8).checked_add(len)?)?.to_vec();
            Some(Blip { kind: BlipKind::Wmf, data })
        })
        .collect();

    let mut r = Reader {
        c: contents,
        layout: layout_of(contents, &list),
        palette: palette.clone(),
        kids,
        image_data,
        out: Pub { width_emu: width, height_emu: height, text, palette, blips, ..Default::default() },
    };
    r.out.text.default_chars.truncate(256);

    let mut seen = HashSet::new();
    for page in list.iter().filter(|k| k.ty == PAGE && !HIDDEN_PAGES.contains(&k.id)) {
        if !seen.insert(page.id) {
            continue;
        }
        let members: Vec<Chunk> = r.kids.get(&page.id).cloned().unwrap_or_default();
        let nodes: Vec<Node> = members.iter().filter_map(|k| r.shape(k, 0)).collect();
        // Pages without anything drawn on them hold Publisher's defaults, not content.
        if !nodes.is_empty() {
            r.out.pages.push(PageIr { nodes, master: None });
        }
    }
    if r.out.pages.is_empty() {
        return Err("the document has no pages with content".into());
    }
    Ok(r.out)
}
