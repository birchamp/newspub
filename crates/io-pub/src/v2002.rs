//! Publisher 2002 and later: shapes from `Escher/EscherStm` joined to the chunks of `Contents` by sequence number,
//! text from the Quill stream.

use crate::contents::{self, Contents, ShapeRec};
use crate::escher::{self, ENode, ESp, Escher, FLAG_FLIP_H, FLAG_FLIP_V, Opts};
use crate::ir::{Blip, Dash, Geom, LineSpec, Node, PageIr, Pub, Rgb, Shp, TableBind, TextBind, resolve_color};
use crate::quill::{self, Mode};
use std::collections::{HashMap, HashSet};

/// Sizes of the shape property ids used here (raw ids, flag bits included).
mod pid {
    pub const ROTATION: u16 = 0x0004;
    pub const TEXT_LEFT: u16 = 0x0081;
    pub const TEXT_TOP: u16 = 0x0082;
    pub const TEXT_RIGHT: u16 = 0x0083;
    pub const TEXT_BOTTOM: u16 = 0x0084;
    pub const NUM_COLUMNS: u16 = 0x008c;
    pub const COLUMN_SPACING: u16 = 0x008d;
    pub const ADJUST1: u16 = 0x0147;
    pub const FILL_TYPE: u16 = 0x0180;
    pub const FILL_COLOR: u16 = 0x0181;
    pub const FILL_STYLE_BOOL: u16 = 0x01bf;
    pub const LINE_COLOR: u16 = 0x01c0;
    pub const LINE_WIDTH: u16 = 0x01cb;
    pub const LINE_DASHING: u16 = 0x01ce;
    pub const LINE_BOOL: u16 = 0x01ff;
    pub const GEOM_BOOL: u16 = 0x017f;
    pub const PIB: u16 = 0x4104;
    pub const CROP_TOP: u16 = 0x0100;
    pub const CROP_BOTTOM: u16 = 0x0101;
    pub const CROP_LEFT: u16 = 0x0102;
    pub const CROP_RIGHT: u16 = 0x0103;
    /// Per-side line colours and widths (tertiary property table): top, right, bottom, left.
    pub const SIDE_COLOR: [u16; 4] = [0x0580, 0x05c0, 0x0600, 0x0540];
    pub const SIDE_WIDTH: [u16; 4] = [0x058b, 0x05cb, 0x060b, 0x054b];
    pub const SIDE_BOOL: [u16; 4] = [0x05bf, 0x05ff, 0x063f, 0x057f];
}

const DEFAULT_LINE_WIDTH: i64 = 9525;

struct Ctx<'a> {
    palette: &'a [Rgb],
    contents: &'a Contents,
}

/// Line exists unless a "use" flag says it was explicitly turned off.
fn line_exists(flags: Option<u32>, geom: Option<u32>) -> bool {
    const USE_LINE: u32 = 1 << 19;
    const LINE: u32 = 1 << 3;
    const GEOM_USE_LINE_OK: u32 = 1 << 12;
    const GEOM_LINE_OK: u32 = 1 << 28;
    let Some(f) = flags else { return false };
    if f & USE_LINE != 0 && f & LINE == 0 {
        return false;
    }
    match geom {
        Some(g) if g & GEOM_USE_LINE_OK != 0 => g & GEOM_LINE_OK != 0,
        _ => true,
    }
}

fn dash_of(v: Option<u32>) -> Dash {
    match v.unwrap_or(0) {
        1 | 6 => Dash::Dashed,
        2 | 5 => Dash::Dotted,
        3 | 4 | 8 => Dash::Mixed,
        7 | 9 | 10 => Dash::LongDashed,
        _ => Dash::Solid,
    }
}

fn fixed16(v: u32) -> f64 {
    f64::from(v as i32) / 65536.0
}

fn geom_of(t: u16) -> Geom {
    match t {
        1 => Geom::Rect,
        2 => Geom::RoundRect,
        3 => Geom::Ellipse,
        4 => Geom::Diamond,
        5 => Geom::Triangle,
        6 => Geom::RightTriangle,
        9 => Geom::Polygon(6),
        10 => Geom::Polygon(8),
        12 => Geom::Star(5),
        13 => Geom::Arrow,
        20 | 32 => Geom::Line,
        56 => Geom::Polygon(5),
        58 => Geom::Star(8),
        59 => Geom::Star(16),
        60 => Geom::Star(32),
        92 => Geom::Star(24),
        75 => Geom::Picture,
        136..=190 => Geom::WordArt,
        202 => Geom::TextBox,
        other => Geom::Unsupported(other),
    }
}

/// Line of a shape from the primary table, else from the first existing side of the tertiary table.
fn line_of(sp: &ESp, palette: &[Rgb]) -> Option<LineSpec> {
    let o = &sp.opts;
    if line_exists(o.get(pid::LINE_BOOL), o.get(pid::GEOM_BOOL)) {
        let color = o.get(pid::LINE_COLOR).and_then(|c| resolve_color(c, palette));
        if let Some(color) = color {
            let width_emu = o.get(pid::LINE_WIDTH).map_or(DEFAULT_LINE_WIDTH, i64::from);
            return Some(LineSpec { color, width_emu, dash: dash_of(o.get(pid::LINE_DASHING)) });
        }
        // No colour in the primary table: the sides carry their own.
        let t = &sp.tertiary;
        if line_exists(t.get(pid::LINE_BOOL), None) {
            for side in 0..4 {
                if !line_exists(t.get(pid::SIDE_BOOL[side]), None) {
                    continue;
                }
                if let Some(color) = t.get(pid::SIDE_COLOR[side]).and_then(|c| resolve_color(c, palette)) {
                    let width_emu = t.get(pid::SIDE_WIDTH[side]).map_or(DEFAULT_LINE_WIDTH, i64::from);
                    return Some(LineSpec { color, width_emu, dash: dash_of(o.get(pid::LINE_DASHING)) });
                }
            }
        }
    }
    None
}

fn fill_of(o: &Opts, palette: &[Rgb]) -> Option<Rgb> {
    // Only solid fills are read; gradients, patterns and picture fills are left unfilled.
    if o.get(pid::FILL_TYPE).unwrap_or(0) != 0 {
        return None;
    }
    if o.get(pid::FILL_STYLE_BOOL).is_some_and(|f| f & 0xf0 == 0) {
        return None;
    }
    o.get(pid::FILL_COLOR).and_then(|c| resolve_color(c, palette))
}

/// Maps a child anchor through the group's coordinate systems to page coordinates.
#[derive(Clone, Copy)]
struct GroupMap {
    cs: [i64; 4],
    abs: [i64; 4],
}

impl GroupMap {
    fn apply(&self, a: [i32; 4]) -> [i64; 4] {
        let cw = (self.cs[2] - self.cs[0]) as f64;
        let ch = (self.cs[3] - self.cs[1]) as f64;
        let (cw, ch) = (if cw == 0.0 { 1.0 } else { cw }, if ch == 0.0 { 1.0 } else { ch });
        let sx = (self.abs[2] - self.abs[0]) as f64 / cw;
        let sy = (self.abs[3] - self.abs[1]) as f64 / ch;
        let x = |v: i32| ((f64::from(v) - self.cs[0] as f64) * sx) as i64 + self.abs[0];
        let y = |v: i32| ((f64::from(v) - self.cs[1] as f64) * sy) as i64 + self.abs[1];
        [x(a[0]), y(a[1]), x(a[2]), y(a[3])]
    }
}

fn normalise(r: [i64; 4]) -> [i64; 4] {
    [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])]
}

fn anchor_rect(sp: &ESp, group: Option<GroupMap>) -> Option<[i64; 4]> {
    if let Some(a) = sp.client_anchor {
        return Some(normalise(a.map(i64::from)));
    }
    let a = sp.child_anchor?;
    Some(normalise(group?.apply(a)))
}

fn build_shape(cx: &mut Ctx, sp: &ESp, rec: Option<&ShapeRec>, rect: [i64; 4]) -> Option<Shp> {
    let seq = sp.seq?;
    let o = &sp.opts;
    let mut rect = rect;
    let mut geom = geom_of(sp.shape_type);
    let rotation = o.get(pid::ROTATION).map_or(0.0, |r| fixed16(r).rem_euclid(360.0));
    // A shape turned about 90 degrees is anchored by its turned bounds: recover the unturned box.
    if (45.0..135.0).contains(&rotation) || (225.0..315.0).contains(&rotation) {
        let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
        let (cxm, cym) = ((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2);
        rect = [cxm - h / 2, cym - w / 2, cxm - h / 2 + h, cym - w / 2 + w];
    }
    let table = rec.filter(|r| r.ty == 0x10).and_then(|r| r.table.as_ref());
    if table.is_some() {
        geom = Geom::Table;
    } else if o.get(pid::PIB).is_some_and(|p| p > 0) && !matches!(geom, Geom::WordArt) {
        geom = Geom::Picture;
    } else if let Some(r) = rec
        && r.text_id.is_some()
        && matches!(geom, Geom::Rect)
        && sp.shape_type == 1
    {
        // A plain rectangle that holds text: keep it as a shape with text (handled by the builder).
    }
    let mut s = Shp::new(seq, rect, geom.clone());
    s.rotation = rotation;
    s.flip_h = sp.flags & FLAG_FLIP_H != 0;
    s.flip_v = sp.flags & FLAG_FLIP_V != 0;
    s.fill = fill_of(o, cx.palette);
    s.line = line_of(sp, cx.palette);
    let ins = |id| o.get(id).map(i64::from);
    s.insets = [
        ins(pid::TEXT_LEFT).unwrap_or(36576),
        ins(pid::TEXT_TOP).unwrap_or(36576),
        ins(pid::TEXT_RIGHT).unwrap_or(36576),
        ins(pid::TEXT_BOTTOM).unwrap_or(36576),
    ];
    s.columns = sp.tertiary.get(pid::NUM_COLUMNS).unwrap_or(1).clamp(1, 20);
    s.column_gap = sp.tertiary.get(pid::COLUMN_SPACING).map_or(0, i64::from);
    if let Some(r) = rec {
        s.valign = r.valign.unwrap_or(0).min(2);
        if let Some(id) = r.text_id {
            s.text = Some(TextBind { text_id: id, chain_pos: r.chain_pos });
        }
    }
    if let Some(a) = o.get(pid::ADJUST1)
        && matches!(geom, Geom::RoundRect)
    {
        s.corner = (f64::from(a as i32) / 21600.0).clamp(0.0, 0.5);
    }
    if matches!(geom, Geom::Picture) {
        s.image = o.get(pid::PIB).map(|p| p as usize);
        let c = |id| fixed16(o.get(id).unwrap_or(0)).clamp(0.0, 0.95);
        s.crop = [c(pid::CROP_LEFT), c(pid::CROP_TOP), c(pid::CROP_RIGHT), c(pid::CROP_BOTTOM)];
        s.fill = None;
    }
    if let (Some(t), Some(r)) = (table, rec) {
        let cells = cx.contents.cells.get(&t.cells_seq).cloned().unwrap_or_default();
        s.table = Some(TableBind {
            rows: t.rows,
            cols: t.cols,
            col_widths: t.col_widths.clone(),
            row_heights: t.row_heights.clone(),
            cells: cells.iter().map(|c| (c.row0, c.row1, c.col0, c.col1)).collect(),
        });
        let _ = r;
    }
    Some(s)
}

fn build_node(cx: &mut Ctx, node: &ENode, group: Option<GroupMap>) -> Option<Node> {
    match node {
        ENode::Sp(sp) => {
            let rect = anchor_rect(sp, group)?;
            let rec = sp.seq.and_then(|s| cx.contents.shapes.get(&s));
            build_shape(cx, sp, rec, rect).map(|s| Node::Shape(Box::new(s)))
        }
        ENode::Group(g) => {
            let leader = g.leader.as_ref()?;
            let seq = leader.seq?;
            let abs = anchor_rect(leader, group)?;
            let cs = leader.group_coords.map_or(abs, |c| c.map(i64::from));
            let map = GroupMap { cs, abs };
            let kids: Vec<Node> = g.kids.iter().filter_map(|k| build_node(cx, k, Some(map))).collect();
            Some(Node::Group { seq, kids })
        }
    }
}

fn node_seq(n: &Node) -> u32 {
    match n {
        Node::Shape(s) => s.seq,
        Node::Group { seq, .. } => *seq,
    }
}

pub(crate) struct Streams<'a> {
    pub contents: &'a [u8],
    pub escher: &'a [u8],
    pub delay: Option<&'a [u8]>,
    pub quill: &'a [u8],
}

pub(crate) fn read(s: &Streams) -> Result<Pub, String> {
    let contents = contents::parse(s.contents).ok_or("the Contents stream is damaged or has no document chunk")?;
    let text = quill::parse(s.quill, Mode::V2002)?;
    let esc: Escher = escher::parse(s.escher);

    let mut out = Pub { width_emu: contents.width, height_emu: contents.height, text, ..Default::default() };
    out.text.default_chars.truncate(256);

    // Pictures: store index (1-based) to data.
    out.blips = esc
        .store
        .iter()
        .map(|e| {
            let from_delay = s.delay.and_then(|d| escher::blip_at(d, e.delay_offset as usize));
            from_delay.or_else(|| e.inline.and_then(|(at, _)| escher::blip_at(s.escher, at)))
        })
        .collect::<Vec<Option<Blip>>>();

    let palette = contents_palette(&contents);
    out.palette = palette.clone();
    let order: Vec<Node> = {
        let mut cx = Ctx { palette: &palette, contents: &contents };
        esc.top.iter().filter_map(|n| build_node(&mut cx, n, None)).collect()
    };
    let by_seq: HashMap<u32, usize> = order.iter().enumerate().map(|(i, n)| (node_seq(n), i)).collect();

    // Pages in document order; master pages are kept apart.
    let listed: Vec<&contents::PageRec> = contents.page_list.iter().filter_map(|seq| contents.pages.get(seq)).collect();
    let mut real: Vec<&contents::PageRec> = listed.iter().copied().filter(|p| !p.is_master && p.has_content).collect();
    if real.is_empty() {
        real = listed.iter().copied().filter(|p| !p.is_master).collect();
    }
    if real.is_empty() {
        return Err("the document lists no pages".into());
    }
    let masters: Vec<&contents::PageRec> = listed.iter().copied().filter(|p| p.is_master).collect();

    let nodes_for = |p: &contents::PageRec| -> Vec<Node> {
        let want: HashSet<u32> = p.shapes.iter().copied().collect();
        order.iter().filter(|n| want.contains(&node_seq(n))).cloned().collect()
    };
    for m in &masters {
        out.masters.push(PageIr { nodes: nodes_for(m), master: None });
    }
    for p in &real {
        let master = p.master.and_then(|ms| masters.iter().position(|m| m.seq == ms));
        out.pages.push(PageIr { nodes: nodes_for(p), master });
    }
    // Shapes that no page lists are not shown by Publisher either (they belong to hidden layouts).
    let placed: HashSet<u32> = real.iter().chain(masters.iter()).flat_map(|p| p.shapes.iter().copied()).collect();
    let unplaced = by_seq.keys().filter(|k| !placed.contains(k)).count();
    if unplaced > 0 {
        out.skipped.insert("shapes that belong to no page", unplaced);
    }
    if contents.embedded_fonts > 0 {
        out.skipped.insert("embedded fonts", contents.embedded_fonts);
    }
    Ok(out)
}

/// Palette used by colour references: the publication palette, padded with black up to the eight scheme entries.
fn contents_palette(c: &Contents) -> Vec<Rgb> {
    let mut p = c.palette.clone();
    if p.len() < 8 {
        p.insert(0, Rgb(0, 0, 0));
    }
    p
}
