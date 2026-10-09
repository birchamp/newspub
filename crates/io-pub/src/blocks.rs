//! The tagged block syntax used by the `Contents` stream (2002+) and by Quill formatting records.
//!
//! A block is `id: u8, type: u8` followed by data whose size depends on the type: a fixed 0, 1, 2, 4, 8, 16 or 24
//! bytes, or (for the container types) a `u32` length that counts itself, followed by the payload. Containers hold
//! further blocks right after the length field.

use crate::bytes::{u8_at, u16_at, u32_at};

/// Longest block list read from one container. A block takes at least two bytes, so this only matters for
/// huge damaged streams.
pub(crate) const MAX_BLOCKS: usize = 200_000;

pub(crate) const T_CONTAINER: u8 = 0x88;
pub(crate) const T_DIRECTORY: u8 = 0x90;
pub(crate) const T_SEQNUM: u8 = 0x70;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Block {
    pub id: u8,
    pub ty: u8,
    /// Value of a 1, 2 or 4 byte block; 0 for other types.
    pub value: u32,
    /// Offset of the data (for variable blocks, of the length field).
    pub off: usize,
    /// Data length (for variable blocks including the length field).
    pub len: usize,
}

/// Data length of a fixed-size block type, or `None` for the variable-length types.
fn fixed_len(ty: u8) -> Option<usize> {
    match ty {
        0x78 | 0x05 | 0x08 | 0x0a => Some(0),
        0x10 | 0x12 | 0x18 | 0x1a | 0x07 => Some(2),
        0x20 | 0x22 | 0x58 | 0x68 | 0x70 | 0xb8 => Some(4),
        0x28 => Some(8),
        0x38 => Some(16),
        0x48 => Some(24),
        0xc0 | 0x80 | 0x82 | 0x88 | 0x8a | 0x90 | 0x98 | 0xa0 => None,
        // Unknown types carry no data, as in the files seen so far.
        _ => Some(0),
    }
}

impl Block {
    pub fn end(&self) -> usize {
        self.off + self.len
    }

    pub fn is_container(&self) -> bool {
        fixed_len(self.ty).is_none()
    }

    /// Payload range of a variable-length block (after the length field).
    pub fn body(&self) -> (usize, usize) {
        (self.off + 4.min(self.len), self.end())
    }
}

/// Reads one block at `p`, requiring it to end by `limit`.
pub(crate) fn read_block(b: &[u8], p: usize, limit: usize) -> Option<Block> {
    let id = u8_at(b, p)?;
    let ty = u8_at(b, p + 1)?;
    let off = p + 2;
    let limit = limit.min(b.len());
    let (len, value) = match fixed_len(ty) {
        Some(n) => {
            let v = match n {
                1 => u32::from(u8_at(b, off)?),
                2 => u32::from(u16_at(b, off)?),
                4 => u32_at(b, off)?,
                _ => 0,
            };
            (n, v)
        }
        None => {
            let n = u32_at(b, off)? as usize;
            if n < 4 {
                return None;
            }
            (n, 0)
        }
    };
    let end = off.checked_add(len)?;
    (end <= limit).then_some(Block { id, ty, value, off, len })
}

/// All blocks in `b[start..end]`; stops at the first block that does not fit.
pub(crate) fn blocks(b: &[u8], start: usize, end: usize) -> Vec<Block> {
    let mut out = Vec::new();
    let mut p = start;
    while p + 2 <= end && out.len() < MAX_BLOCKS {
        let Some(blk) = read_block(b, p, end) else { break };
        p = blk.end();
        out.push(blk);
    }
    out
}

/// The blocks inside a container block.
pub(crate) fn children(b: &[u8], parent: &Block) -> Vec<Block> {
    if !parent.is_container() {
        return Vec::new();
    }
    let (s, e) = parent.body();
    blocks(b, s, e)
}

/// First block with `id` in the list.
pub(crate) fn find(list: &[Block], id: u8) -> Option<&Block> {
    list.iter().find(|x| x.id == id)
}

/// Value of the first fixed-size block with `id`.
pub(crate) fn value_of(list: &[Block], id: u8) -> Option<u32> {
    find(list, id).filter(|x| !x.is_container()).map(|x| x.value)
}
