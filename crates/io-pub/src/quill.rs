//! The Quill text stream (`Quill/QuillSub/CONTENTS`): a chunk directory followed by the text of all stories and
//! the tables that describe them. See FORMAT-NOTES.md.
//!
//! Chunks used: `TEXT` (all stories, UTF-16LE), `STRS` (story lengths), `SYID` (story identifiers), `FDPC` and
//! `FDPP` (character and paragraph runs), `STSH` (style sheet; the second one holds the default styles), `FONT`,
//! `PL  ` (text colours) and `TCD ` (table cell ends).

use crate::blocks::{Block, blocks, children, find, value_of};
use crate::bytes::{u16_at, u32_at, utf16_z};
use crate::ir::{CharFmt, ParaFmt, QStory, Rgb, Text, TextColor};
use std::collections::HashMap;

/// How colours are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Publisher 2002 and later: indices into the `PL  ` table.
    V2002,
    /// Publisher 98 and 2000: colour references in the formatting records.
    V2000,
}

#[derive(Clone, Debug)]
struct Chunk {
    tag: [u8; 4],
    id: u16,
    off: usize,
    len: usize,
}

const MAX_LISTS: usize = 64;
const MAX_CHUNKS: usize = 4096;
const MAX_STORIES: usize = 100_000;

fn directory(b: &[u8]) -> Option<Vec<Chunk>> {
    if !b.starts_with(b"CHNKINK ") {
        return None;
    }
    let mut out = Vec::new();
    let mut list = 0x18usize;
    let mut seen = Vec::new();
    for _ in 0..MAX_LISTS {
        if seen.contains(&list) {
            break;
        }
        seen.push(list);
        let count = usize::from(u16_at(b, list + 2)?);
        let next = u32_at(b, list + 4)?;
        let mut p = list + 8;
        for _ in 0..count {
            let tag: [u8; 4] = b.get(p + 2..p + 6)?.try_into().ok()?;
            let id = u16_at(b, p + 6)?;
            let off = u32_at(b, p + 16)? as usize;
            let len = u32_at(b, p + 20)? as usize;
            if off.checked_add(len).is_some_and(|e| e <= b.len()) && out.len() < MAX_CHUNKS {
                out.push(Chunk { tag, id, off, len });
            }
            p += 24;
        }
        if next == 0xffff_ffff {
            break;
        }
        list = next as usize;
    }
    Some(out)
}

/// Signed value of a fixed-size block, by its size.
fn signed(blk: &Block) -> i64 {
    match blk.len {
        1 => i64::from(blk.value as u8 as i8),
        2 => i64::from(blk.value as u16 as i16),
        _ => i64::from(blk.value as i32),
    }
}

fn blocks_of_record(q: &[u8], p: usize) -> Vec<Block> {
    let Some(len) = u32_at(q, p) else { return Vec::new() };
    blocks(q, p + 4, p.saturating_add(len as usize).min(q.len()))
}

fn char_fmt(q: &[u8], p: usize, mode: Mode) -> CharFmt {
    let list = blocks_of_record(q, p);
    let mut f = CharFmt::default();
    for b in &list {
        let fixed = !b.is_container();
        match b.id {
            0x02 => f.bold = true,
            0x03 => f.italic = true,
            0x1e if fixed => f.underline = Some(b.value as u8).filter(|&u| u != 0),
            0x0c if fixed => f.size_emu = Some(i64::from(b.value)),
            0x0f if fixed => f.script = b.value as u8,
            0x04 => f.outline = true,
            0x05 => f.shadow = true,
            0x13 => f.small_caps = true,
            0x14 => f.all_caps = true,
            0x16 => f.emboss = true,
            0x17 => f.engrave = true,
            0x20 if fixed => f.scale = Some(b.value),
            0x12 if fixed => f.lcid = Some(b.value),
            0x2e if fixed => f.color = Some(bare_color(b.value, mode)),
            0x44 if !fixed => {
                if let Some(v) = value_of(&children(q, b), 0) {
                    f.color = Some(TextColor::Table(v as usize));
                }
            }
            0x24 if !fixed => {
                // Font container: one sub-container per script; the first one is the Latin font.
                let first = children(q, b).into_iter().find(|k| k.is_container());
                if let Some(k) = first
                    && let Some(v) = children(q, &k).first().filter(|x| !x.is_container())
                {
                    f.font = Some(v.value as usize);
                }
            }
            _ => {}
        }
    }
    f
}

fn bare_color(v: u32, mode: Mode) -> TextColor {
    match mode {
        Mode::V2002 => TextColor::Table(v as usize),
        Mode::V2000 => TextColor::Ref(crate::legacy::translate_ref(v).unwrap_or(0)),
    }
}

fn para_fmt(q: &[u8], p: usize) -> ParaFmt {
    let list = blocks_of_record(q, p);
    let mut f = ParaFmt::default();
    for b in &list {
        if b.is_container() {
            if b.id == 0x32 {
                // Tab stops: an array of entries, each holding the position as its first block.
                for arr in children(q, b).iter().filter(|a| a.id == 0x28) {
                    for e in children(q, arr) {
                        if let Some(t) = children(q, &e).first().filter(|t| t.id == 0 && !t.is_container())
                            && f.tabs.len() < 64
                        {
                            f.tabs.push(i64::from(t.value));
                        }
                    }
                }
            }
            continue;
        }
        match b.id {
            0x04 => f.align = Some((b.value & 0xff) as u8),
            0x19 => f.style_index = Some(b.value as usize),
            0x34 => f.line_spacing = Some(b.value),
            0x12 => f.space_before = Some(i64::from(b.value)),
            0x13 => f.space_after = Some(i64::from(b.value)),
            0x0c => f.first_indent = Some(signed(b)),
            0x0d => f.left_indent = Some(i64::from(b.value)),
            0x0e => f.right_indent = Some(i64::from(b.value)),
            0x08 => f.drop_lines = Some(b.value),
            0x2d => f.drop_chars = Some(b.value),
            _ => {}
        }
    }
    f
}

/// Run table of an `FDPC` or `FDPP` chunk: `(end offset in the stream, record offset)` pairs.
fn run_table(q: &[u8], c: &Chunk) -> Vec<(usize, usize)> {
    let n = usize::from(u16_at(q, c.off).unwrap_or(0));
    let mut out = Vec::new();
    for i in 0..n {
        let (Some(end), Some(rec)) = (u32_at(q, c.off + 8 + 4 * i), u16_at(q, c.off + 8 + 4 * n + 2 * i)) else {
            break;
        };
        if usize::from(rec) + 4 > c.len {
            break;
        }
        out.push((end as usize, c.off + usize::from(rec)));
    }
    out
}

fn fonts(q: &[u8], c: &Chunk) -> Vec<String> {
    let n = (u32_at(q, c.off + 4).unwrap_or(0) as usize).min(4096);
    let mut p = c.off + 8 + 12 + 4 * n;
    let end = c.off + c.len;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(len) = u16_at(q, p) else { break };
        let name_end = p + 2 + usize::from(len) * 2;
        if name_end + 4 > end {
            break;
        }
        out.push(utf16_z(&q[p + 2..name_end]));
        p = name_end + 4;
    }
    out
}

fn color_refs(q: &[u8], c: &Chunk) -> Vec<u32> {
    let n = (u32_at(q, c.off).unwrap_or(0) as usize).min(4096);
    let mut p = c.off + 12;
    let end = c.off + c.len;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(len) = u32_at(q, p).map(|l| l as usize) else { break };
        if len < 4 || p + len > end {
            break;
        }
        out.push(value_of(&blocks(q, p + 4, p + len), 1).unwrap_or(0));
        p += len;
    }
    out
}

/// Elements of the default style sheet: even entries are character styles, odd ones paragraph styles.
fn default_styles(q: &[u8], c: &Chunk, mode: Mode) -> (Vec<CharFmt>, Vec<ParaFmt>) {
    let n = (u32_at(q, c.off + 4).unwrap_or(0) as usize).min(512);
    let (mut chars, mut paras) = (Vec::new(), Vec::new());
    for i in 0..n {
        let Some(off) = u32_at(q, c.off + 20 + 4 * i) else { break };
        let p = c.off + 20 + off as usize;
        if p + 6 > c.off + c.len {
            break;
        }
        if i % 2 == 0 {
            chars.push(char_fmt(q, p + 2, mode));
        } else {
            paras.push(para_fmt(q, p + 2));
        }
    }
    (chars, paras)
}

/// Story lengths from `STRS`.
fn story_lengths(q: &[u8], c: &Chunk) -> Option<Vec<usize>> {
    let n = u32_at(q, c.off)? as usize;
    let first = c.off.checked_add(4)?.checked_add(u32_at(q, c.off + 4)? as usize)?;
    if n > MAX_STORIES || first.checked_add(n.checked_mul(4)?)? > c.off + c.len {
        return None;
    }
    (0..n).map(|i| u32_at(q, first + 4 * i).map(|v| v as usize)).collect()
}

/// Ids from `SYID`.
fn story_ids(q: &[u8], c: &Chunk) -> Option<Vec<u32>> {
    let n = u32_at(q, c.off + 4)? as usize;
    if n > MAX_STORIES || c.off.checked_add(8)?.checked_add(n.checked_mul(4)?)? > c.off + c.len {
        return None;
    }
    (0..n).map(|i| u32_at(q, c.off + 8 + 4 * i)).collect()
}

/// Cell end offsets of a table story from `TCD `.
fn cell_ends(q: &[u8], c: &Chunk) -> Option<Vec<usize>> {
    let n = (u32_at(q, c.off)? as usize).checked_add(1)?;
    if n > 100_000 || c.off.checked_add(12)?.checked_add(n.checked_mul(4)?)? > c.off + c.len {
        return None;
    }
    (0..n).map(|i| u32_at(q, c.off + 12 + 4 * i).map(|v| v as usize)).collect()
}

/// Splits global runs `(exclusive end, value)` over the story range `s..e` into story-local runs.
fn split_runs<T: Clone>(runs: &[(usize, T)], s: usize, e: usize) -> Vec<(usize, T)> {
    let mut out = Vec::new();
    let mut prev = 0usize;
    for (end, v) in runs {
        let end = (*end).max(prev);
        if end > s && prev < e {
            out.push((end.min(e) - s, v.clone()));
        }
        prev = end;
        if prev >= e {
            break;
        }
    }
    out
}

pub(crate) fn parse(q: &[u8], mode: Mode) -> Result<Text, String> {
    let dir = directory(q).ok_or("the Quill stream has no readable chunk directory")?;
    let by_tag = |tag: &[u8; 4]| dir.iter().filter(|c| &c.tag == tag).collect::<Vec<_>>();
    let missing = |what: &str| format!("the Quill stream has no {what} chunk");
    let text_chunk = *by_tag(b"TEXT").first().ok_or_else(|| missing("TEXT"))?;
    let strs = *by_tag(b"STRS").first().ok_or_else(|| missing("STRS"))?;
    let syid = *by_tag(b"SYID").first().ok_or_else(|| missing("SYID"))?;
    let font_chunk = *by_tag(b"FONT").first().ok_or_else(|| missing("FONT"))?;
    let stsh = by_tag(b"STSH");
    let fdpc = by_tag(b"FDPC");
    let fdpp = by_tag(b"FDPP");
    if fdpc.is_empty() || fdpp.is_empty() {
        return Err(missing("FDPC/FDPP"));
    }

    let lengths = story_lengths(q, strs).ok_or("damaged Quill story length table")?;
    let ids = story_ids(q, syid).ok_or("damaged Quill story id table")?;
    let text = &q[text_chunk.off..text_chunk.off + text_chunk.len];
    let units: Vec<u16> = text.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();

    let mut char_runs: Vec<(usize, CharFmt)> = Vec::new();
    for c in &fdpc {
        for (end, rec) in run_table(q, c) {
            char_runs.push((end.saturating_sub(text_chunk.off) / 2, char_fmt(q, rec, mode)));
        }
    }
    let mut para_runs: Vec<(usize, ParaFmt)> = Vec::new();
    for c in &fdpp {
        for (end, rec) in run_table(q, c) {
            para_runs.push((end.saturating_sub(text_chunk.off) / 2, para_fmt(q, rec)));
        }
    }
    let ends: HashMap<u16, Vec<usize>> =
        by_tag(b"TCD ").into_iter().filter_map(|c| cell_ends(q, c).map(|e| (c.id, e))).collect();

    let mut out = Text { fonts: fonts(q, font_chunk), ..Default::default() };
    if let Some(c) = by_tag(b"PL  ").first() {
        out.color_refs = color_refs(q, c);
    }
    if let Some(c) = stsh.get(1) {
        (out.default_chars, out.default_paras) = default_styles(q, c, mode);
    }
    let mut start = 0usize;
    for (i, (&len, &id)) in lengths.iter().zip(ids.iter()).enumerate() {
        let end = start.checked_add(len).filter(|&e| e <= units.len()).ok_or("a story reaches past the text")?;
        out.stories.push(QStory {
            text_id: id,
            chars: units[start..end].to_vec(),
            char_runs: split_runs(&char_runs, start, end),
            para_runs: split_runs(&para_runs, start, end),
            cell_ends: ends.get(&(i as u16)).cloned(),
        });
        start = end;
    }
    let _ = find; // keep the helper import used by sibling modules in one place
    Ok(out)
}

/// Palette-independent text colour lookup used by the builder.
pub(crate) fn text_color(t: &Text, c: TextColor, palette: &[Rgb]) -> Option<Rgb> {
    match c {
        TextColor::Ref(r) => crate::ir::resolve_color(r, palette),
        TextColor::Table(i) => t.color_refs.get(i).and_then(|&r| crate::ir::resolve_color(r, palette)),
    }
}
