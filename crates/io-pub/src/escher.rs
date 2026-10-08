//! Office Drawing ("Escher") records in `Escher/EscherStm`: only text box anchors are read.

/// Shape type code of a text box in the shape record header.
const SHAPE_TEXT_BOX: u16 = 202;
const REC_SP_CONTAINER: u16 = 0xF004;
const REC_SP: u16 = 0xF00A;
const REC_CLIENT_ANCHOR: u16 = 0xF010;
const MAX_DEPTH: usize = 12;

/// A text box rectangle in EMU, relative to the page centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextBox {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

struct Rec {
    ver: u16,
    inst: u16,
    kind: u16,
    start: usize,
    end: usize,
}

fn header(b: &[u8], p: usize) -> Option<Rec> {
    let h = b.get(p..p.checked_add(8)?)?;
    let vi = u16::from_le_bytes([h[0], h[1]]);
    let kind = u16::from_le_bytes([h[2], h[3]]);
    let len = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
    let start = p + 8;
    let end = start.checked_add(len)?;
    (end <= b.len() && (0xF000..=0xF1FF).contains(&kind)).then_some(Rec {
        ver: vi & 15,
        inst: vi >> 4,
        kind,
        start,
        end,
    })
}

/// Anchor payload: a 4-byte count followed by `(u16 tag, i32 value)` pairs; tags 0x2001..0x2004 are left, top,
/// right, bottom.
fn anchor(data: &[u8]) -> Option<TextBox> {
    let mut v = [None; 4];
    let mut p = 4;
    while p + 6 <= data.len() {
        let tag = u16::from_le_bytes([data[p], data[p + 1]]);
        let val = i32::from_le_bytes([data[p + 2], data[p + 3], data[p + 4], data[p + 5]]);
        if (0x2001..=0x2004).contains(&tag) {
            v[usize::from(tag - 0x2001)] = Some(val);
        }
        p += 6;
    }
    Some(TextBox { left: v[0]?, top: v[1]?, right: v[2]?, bottom: v[3]? })
}

fn walk(b: &[u8], mut p: usize, end: usize, depth: usize, out: &mut Vec<TextBox>) {
    while p + 8 <= end {
        let Some(r) = header(b, p).filter(|r| r.end <= end) else {
            // Top-level containers are separated by a 4-byte field.
            p += 4;
            continue;
        };
        if r.ver == 0xF && depth < MAX_DEPTH {
            if r.kind == REC_SP_CONTAINER
                && let Some(t) = text_box_in(b, &r)
            {
                out.push(t);
            }
            walk(b, r.start, r.end, depth + 1, out);
        }
        p = r.end;
    }
}

fn text_box_in(b: &[u8], sp: &Rec) -> Option<TextBox> {
    let (mut is_text, mut found) = (false, None);
    let mut p = sp.start;
    while let Some(r) = header(b, p).filter(|r| r.end <= sp.end) {
        match r.kind {
            REC_SP if r.inst == SHAPE_TEXT_BOX => is_text = true,
            REC_CLIENT_ANCHOR => found = anchor(&b[r.start..r.end]),
            _ => {}
        }
        p = r.end;
    }
    found.filter(|_| is_text)
}

pub(crate) fn text_boxes(b: &[u8]) -> Vec<TextBox> {
    let mut out = Vec::new();
    walk(b, 0, b.len(), 0, &mut out);
    out
}
