//! The `Contents` stream of Publisher 2002 and later: a directory of numbered chunks (document, pages, shapes,
//! tables, palette, fonts), each a list of tagged blocks. See FORMAT-NOTES.md.

use crate::blocks::{self, Block, T_CONTAINER, T_DIRECTORY, T_SEQNUM, blocks as read_blocks, children, find, value_of};
use crate::bytes::{u32_at, utf16_z};
use crate::ir::Rgb;
use std::collections::HashMap;

/// Chunk type numbers (block id 2 of a directory entry).
const CH_SHAPE: u32 = 0x01;
const CH_TABLE: u32 = 0x10;
const CH_ALTSHAPE: u32 = 0x20;
const CH_GROUP: u32 = 0x30;
const CH_LOGO: u32 = 0x31;
const CH_PAGE: u32 = 0x43;
const CH_DOCUMENT: u32 = 0x44;
const CH_PALETTE: u32 = 0x5c;
const CH_CELLS: u32 = 0x63;
const CH_FONTS: u32 = 0x6c;

/// Most chunks read from one directory.
const MAX_CHUNKS: usize = 100_000;

#[derive(Clone, Debug)]
pub(crate) struct ChunkRef {
    pub seq: u32,
    pub ty: u32,
    pub off: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PageRec {
    pub seq: u32,
    /// Sequence numbers of the page's top-level shapes (empty when the page has no shape list).
    pub shapes: Vec<u32>,
    /// The page has a shape list or a shape count: dummy "hidden" pages have neither.
    pub has_content: bool,
    /// Named page: a master page.
    pub is_master: bool,
    pub master: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TableRec {
    pub rows: usize,
    pub cols: usize,
    pub cells_seq: u32,
    pub col_widths: Vec<i64>,
    pub row_heights: Vec<i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ShapeRec {
    pub ty: u32,
    pub text_id: Option<u32>,
    /// Position within a chain of linked text boxes (0 = first).
    pub chain_pos: u32,
    pub prev: Option<u32>,
    pub next: Option<u32>,
    pub valign: Option<u8>,
    pub table: Option<TableRec>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CellRec {
    pub row0: usize,
    pub row1: usize,
    pub col0: usize,
    pub col1: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Contents {
    pub width: i64,
    pub height: i64,
    pub page_list: Vec<u32>,
    pub pages: HashMap<u32, PageRec>,
    pub shapes: HashMap<u32, ShapeRec>,
    pub palette: Vec<Rgb>,
    pub cells: HashMap<u32, Vec<CellRec>>,
    pub embedded_fonts: usize,
}

/// Reads the directory of chunk references from the trailer.
fn chunk_refs(c: &[u8]) -> Option<Vec<ChunkRef>> {
    let trailer = u32_at(c, 0x1a)? as usize;
    let trailer_len = u32_at(c, trailer)? as usize;
    let limit = trailer.checked_add(4)?.checked_add(trailer_len)?.min(c.len());
    let mut p = trailer + 4;
    let mut refs = Vec::new();
    let mut seq: i64 = -1;
    // Three top-level blocks: two counters and the directory.
    for _ in 0..3 {
        let blk = blocks::read_block(c, p, limit)?;
        if blk.ty == T_DIRECTORY {
            let (s, e) = blk.body();
            for entry in read_blocks(c, s, e) {
                seq += 1;
                if entry.ty != T_CONTAINER {
                    continue;
                }
                let kids = children(c, &entry);
                let (Some(ty), Some(off)) = (value_of(&kids, 2), value_of(&kids, 4)) else { continue };
                if refs.len() >= MAX_CHUNKS {
                    return None;
                }
                refs.push(ChunkRef { seq: seq as u32, ty, off: off as usize });
            }
        }
        p = blk.end();
    }
    Some(refs)
}

/// Blocks of the chunk at `off`: `u32 length` then the block list.
fn chunk_blocks(c: &[u8], off: usize) -> Option<Vec<Block>> {
    let len = u32_at(c, off)? as usize;
    let end = off.checked_add(len)?.min(c.len());
    (end >= off + 4).then(|| read_blocks(c, off + 4, end))
}

fn parse_table(c: &[u8], list: &[Block]) -> Option<TableRec> {
    let rows = value_of(list, 0x66)? as usize;
    let cols = value_of(list, 0x67)? as usize;
    let cells_seq = value_of(list, 0x6b)?;
    let mut t = TableRec { rows, cols, cells_seq, ..Default::default() };
    if rows == 0 || cols == 0 || rows > 500 || cols > 100 {
        return None;
    }
    // Column widths come first in the size array, then row heights.
    let arr = find(list, 0x6d)?;
    let mut sizes = Vec::new();
    for e in children(c, arr) {
        let kids = children(c, &e);
        if let Some(v) = value_of(&kids, 2) {
            sizes.push(i64::from(v));
        }
    }
    if sizes.len() < rows + cols {
        return None;
    }
    t.col_widths = sizes[..cols].to_vec();
    t.row_heights = sizes[cols..cols + rows].to_vec();
    Some(t)
}

fn parse_cells(c: &[u8], list: &[Block]) -> Vec<CellRec> {
    let Some(arr) = find(list, 2) else { return Vec::new() };
    children(c, arr)
        .iter()
        .map(|e| {
            let k = children(c, e);
            let g = |id| value_of(&k, id).unwrap_or(0) as usize;
            CellRec { row0: g(1), row1: g(2), col0: g(3), col1: g(4) }
        })
        .collect()
}

fn parse_palette(c: &[u8], list: &[Block]) -> Vec<Rgb> {
    let mut out = Vec::new();
    for b in list.iter().filter(|b| b.ty == 0xa0) {
        for entry in children(c, b) {
            if out.len() >= 4096 {
                break;
            }
            if entry.ty == T_CONTAINER {
                if let Some(v) = value_of(&children(c, &entry), 1) {
                    out.push(Rgb::from_rgb24(v));
                }
            } else if entry.ty == 0x78 {
                // An unused slot reads as black.
                out.push(Rgb(0, 0, 0));
            }
        }
    }
    out
}

fn parse_shape(c: &[u8], r: &ChunkRef, list: &[Block]) -> ShapeRec {
    let mut s = ShapeRec { ty: r.ty, ..Default::default() };
    s.text_id = value_of(list, 0x27);
    s.chain_pos = value_of(list, 0x28).unwrap_or(0);
    s.prev = value_of(list, 0x36);
    s.next = value_of(list, 0x37);
    s.valign = value_of(list, 0x35).map(|v| v as u8);
    if r.ty == CH_TABLE {
        s.table = parse_table(c, list);
    }
    s
}

pub(crate) fn parse(c: &[u8]) -> Option<Contents> {
    let refs = chunk_refs(c)?;
    let mut out = Contents::default();
    for r in &refs {
        let Some(list) = chunk_blocks(c, r.off) else { continue };
        match r.ty {
            CH_DOCUMENT => {
                if let Some(size) = find(&list, 0x12) {
                    let k = children(c, size);
                    out.width = i64::from(value_of(&k, 1).unwrap_or(0));
                    out.height = i64::from(value_of(&k, 2).unwrap_or(0));
                }
                if let Some(pl) = find(&list, 2) {
                    out.page_list = children(c, pl).iter().filter(|b| b.ty == T_SEQNUM).map(|b| b.value).collect();
                }
            }
            CH_PAGE => {
                let mut p = PageRec { seq: r.seq, ..Default::default() };
                if let Some(sl) = find(&list, 2) {
                    p.has_content = true;
                    p.shapes = children(c, sl).iter().filter(|b| b.ty == T_SEQNUM).map(|b| b.value).collect();
                }
                p.has_content |= find(&list, 1).is_some();
                if let Some(n) = find(&list, 0xe) {
                    let (s, e) = n.body();
                    p.is_master = c.get(s..e).is_some_and(|name| !utf16_z(name).is_empty());
                }
                p.master = value_of(&list, 0xd);
                out.pages.insert(r.seq, p);
            }
            CH_SHAPE | CH_ALTSHAPE | CH_GROUP | CH_LOGO | CH_TABLE => {
                out.shapes.insert(r.seq, parse_shape(c, r, &list));
            }
            CH_CELLS => {
                out.cells.insert(r.seq, parse_cells(c, &list));
            }
            CH_PALETTE => {
                out.palette = parse_palette(c, &list);
            }
            CH_FONTS => {
                if let Some(arr) = find(&list, 2) {
                    out.embedded_fonts += children(c, arr).len();
                }
            }
            _ => {}
        }
    }
    (out.width > 0 && out.height > 0 && !out.page_list.is_empty()).then_some(out)
}
