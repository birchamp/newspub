//! Office Drawing ("Escher") records in `Escher/EscherStm`: shapes with their anchors and properties, the picture
//! store, and the pictures in `Escher/EscherDelayStm`. See FORMAT-NOTES.md.

use crate::bytes::{i32_at, u16_at, u32_at};
use crate::ir::{Blip, BlipKind};
use std::collections::HashMap;

const REC_DGG: u16 = 0xF000;
const REC_BSTORE: u16 = 0xF001;
const REC_DG: u16 = 0xF002;
const REC_SPGR: u16 = 0xF003;
const REC_SP: u16 = 0xF004;
const REC_FBSE: u16 = 0xF007;
const REC_FSPGR: u16 = 0xF009;
const REC_FSP: u16 = 0xF00A;
const REC_FOPT: u16 = 0xF00B;
const REC_CHILD_ANCHOR: u16 = 0xF00F;
const REC_CLIENT_ANCHOR: u16 = 0xF010;
const REC_CLIENT_DATA: u16 = 0xF011;
const REC_TERTIARY_OPT: u16 = 0xF122;

/// Shape flag: this shape leads a group.
pub(crate) const FLAG_GROUP: u32 = 1;
pub(crate) const FLAG_FLIP_H: u32 = 1 << 6;
pub(crate) const FLAG_FLIP_V: u32 = 1 << 7;

/// Group nesting beyond this is ignored.
const MAX_DEPTH: usize = 32;
/// Most shapes read from one stream.
const MAX_SHAPES: usize = 50_000;
/// Most properties per record, and most pictures in the store.
const MAX_STORE: usize = 20_000;

#[derive(Clone, Copy)]
struct Rec {
    inst: u16,
    ty: u16,
    start: usize,
    end: usize,
}

fn header(b: &[u8], p: usize, limit: usize) -> Option<Rec> {
    let vi = u16_at(b, p)?;
    let ty = u16_at(b, p + 2)?;
    let len = u32_at(b, p + 4)? as usize;
    let start = p + 8;
    let end = start.checked_add(len)?;
    ((0xF000..=0xF1FF).contains(&ty) && end <= limit.min(b.len())).then_some(Rec { inst: vi >> 4, ty, start, end })
}

/// Records inside `b[start..end]`. A 4-byte filler after drawing containers is skipped like any other bytes that
/// do not form a record header.
fn records(b: &[u8], start: usize, end: usize) -> Vec<Rec> {
    let mut out = Vec::new();
    let mut p = start;
    while p + 8 <= end && out.len() < MAX_SHAPES {
        match header(b, p, end) {
            Some(r) => {
                p = r.end;
                out.push(r);
            }
            None => p += 4,
        }
    }
    out
}

/// Property table: simple values by raw property id (flag bits included), and complex data blobs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {
    pub simple: HashMap<u16, u32>,
    pub complex: HashMap<u16, Vec<u8>>,
}

impl Opts {
    pub fn get(&self, id: u16) -> Option<u32> {
        self.simple.get(&id).copied()
    }
}

fn parse_opts(b: &[u8], r: &Rec) -> Opts {
    let mut o = Opts::default();
    let n = usize::from(r.inst);
    let mut ids = Vec::new();
    for i in 0..n {
        let p = r.start + i * 6;
        let (Some(id), Some(v)) = (u16_at(b, p), u32_at(b, p + 2)) else { break };
        if p + 6 > r.end {
            break;
        }
        o.simple.insert(id, v);
        if id & 0x8000 != 0 {
            ids.push((id, v as usize));
        }
    }
    let mut q = r.start + n * 6;
    for (id, len) in ids {
        let Some(end) = q.checked_add(len).filter(|&e| e <= r.end) else { break };
        o.complex.insert(id, b[q..end].to_vec());
        q = end;
    }
    o
}

/// Anchor-style record body: a 4-byte count then `(u16 id, i32 value)` pairs.
fn pairs(b: &[u8], r: &Rec) -> HashMap<u16, u32> {
    let mut m = HashMap::new();
    let mut p = r.start + 4;
    while p + 6 <= r.end {
        if let (Some(id), Some(v)) = (u16_at(b, p), u32_at(b, p + 2)) {
            m.insert(id, v);
        }
        p += 6;
    }
    m
}

fn four(b: &[u8], p: usize) -> Option<[i32; 4]> {
    Some([i32_at(b, p)?, i32_at(b, p + 4)?, i32_at(b, p + 8)?, i32_at(b, p + 12)?])
}

/// One shape.
#[derive(Clone, Debug, Default)]
pub(crate) struct ESp {
    /// Sequence number of the matching `Contents` chunk (client data property 0x6801).
    pub seq: Option<u32>,
    pub shape_type: u16,
    pub flags: u32,
    pub opts: Opts,
    pub tertiary: Opts,
    pub client_anchor: Option<[i32; 4]>,
    pub child_anchor: Option<[i32; 4]>,
    /// Child coordinate system of a group leader.
    pub group_coords: Option<[i32; 4]>,
}

#[derive(Clone, Debug)]
pub(crate) enum ENode {
    Sp(Box<ESp>),
    Group(Box<EGroup>),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EGroup {
    pub leader: Option<ESp>,
    pub kids: Vec<ENode>,
}

fn parse_sp(b: &[u8], r: &Rec) -> ESp {
    let mut sp = ESp::default();
    for c in records(b, r.start, r.end) {
        match c.ty {
            REC_FSP => {
                sp.shape_type = c.inst;
                sp.flags = u32_at(b, c.start + 4).unwrap_or(0);
            }
            REC_FSPGR => sp.group_coords = four(b, c.start),
            REC_FOPT => sp.opts = parse_opts(b, &c),
            REC_TERTIARY_OPT => sp.tertiary = parse_opts(b, &c),
            REC_CLIENT_ANCHOR => {
                let m = pairs(b, &c);
                if let (Some(&a), Some(&bb), Some(&cc), Some(&d)) =
                    (m.get(&0x2001), m.get(&0x2002), m.get(&0x2003), m.get(&0x2004))
                {
                    sp.client_anchor = Some([a as i32, bb as i32, cc as i32, d as i32]);
                }
            }
            REC_CHILD_ANCHOR => sp.child_anchor = four(b, c.start),
            REC_CLIENT_DATA => sp.seq = pairs(b, &c).get(&0x6801).copied(),
            _ => {}
        }
    }
    sp
}

fn parse_group(b: &[u8], r: &Rec, depth: usize, budget: &mut usize) -> EGroup {
    let mut g = EGroup::default();
    for c in records(b, r.start, r.end) {
        if *budget == 0 {
            break;
        }
        match c.ty {
            REC_SP => {
                *budget -= 1;
                let sp = parse_sp(b, &c);
                if sp.flags & FLAG_GROUP != 0 && g.leader.is_none() && g.kids.is_empty() {
                    g.leader = Some(sp);
                } else {
                    g.kids.push(ENode::Sp(Box::new(sp)));
                }
            }
            REC_SPGR if depth < MAX_DEPTH => {
                *budget -= 1;
                g.kids.push(ENode::Group(Box::new(parse_group(b, &c, depth + 1, budget))));
            }
            _ => {}
        }
    }
    g
}

/// An entry of the picture store.
#[derive(Clone, Debug)]
pub(crate) struct StoreEntry {
    /// Offset of the picture record in the delay stream.
    pub delay_offset: u32,
    /// The picture record sits inside the store entry itself: its byte range in the Escher stream.
    pub inline: Option<(usize, usize)>,
}

#[derive(Debug, Default)]
pub(crate) struct Escher {
    /// Top-level nodes of every drawing, in drawing order.
    pub top: Vec<ENode>,
    pub store: Vec<StoreEntry>,
}

pub(crate) fn parse(b: &[u8]) -> Escher {
    let mut out = Escher::default();
    let mut budget = MAX_SHAPES;
    for r in records(b, 0, b.len()) {
        match r.ty {
            REC_DGG => {
                for c in records(b, r.start, r.end).into_iter().filter(|c| c.ty == REC_BSTORE) {
                    for f in records(b, c.start, c.end).into_iter().filter(|f| f.ty == REC_FBSE) {
                        if out.store.len() >= MAX_STORE {
                            break;
                        }
                        let body = f.start;
                        let name_len = usize::from(b.get(body + 33).copied().unwrap_or(0));
                        let blip_at = body + 36 + name_len;
                        let inline = (blip_at + 8 <= f.end).then_some((blip_at, f.end));
                        out.store.push(StoreEntry { delay_offset: u32_at(b, body + 28).unwrap_or(0), inline });
                    }
                }
            }
            REC_DG => {
                for s in records(b, r.start, r.end).into_iter().filter(|c| c.ty == REC_SPGR) {
                    let g = parse_group(b, &s, 0, &mut budget);
                    out.top.extend(g.kids);
                }
            }
            _ => {}
        }
    }
    out
}

/// Biggest decompressed metafile accepted.
const MAX_METAFILE: usize = 64 * 1024 * 1024;

/// Reads the picture record at `pos` in `b`.
pub(crate) fn blip_at(b: &[u8], pos: usize) -> Option<Blip> {
    let r = header(b, pos, b.len())?;
    let (kind, metafile) = match r.ty {
        0xF01A => (BlipKind::Emf, true),
        0xF01B => (BlipKind::Wmf, true),
        0xF01C | 0xF02A => (BlipKind::Other, false),
        0xF01D => (BlipKind::Jpeg, false),
        0xF01E => (BlipKind::Png, false),
        0xF01F => (BlipKind::Dib, false),
        0xF029 => (BlipKind::Tiff, false),
        _ => return None,
    };
    // Odd instance numbers carry a second 16-byte identifier.
    let uids = if r.inst & 1 == 1 { 32 } else { 16 };
    if kind == BlipKind::Other {
        return Some(Blip { kind, data: Vec::new() });
    }
    if metafile {
        let h = r.start + uids;
        let compressed_len = u32_at(b, h + 28)? as usize;
        let compression = *b.get(h + 32)?;
        let data = b.get(h + 34..h.checked_add(34)?.checked_add(compressed_len)?.min(r.end))?;
        let data = if compression == 0 {
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, MAX_METAFILE).ok()?
        } else {
            data.to_vec()
        };
        return Some(Blip { kind, data });
    }
    // Bitmaps: identifier(s), one tag byte, then the file.
    let data = b.get(r.start + uids + 1..r.end)?;
    Some(Blip { kind, data: data.to_vec() })
}
