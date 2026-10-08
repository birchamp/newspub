//! The Quill text stream: a chunk directory followed by chunk data (text, fonts, ...).

/// Text and font names decoded from `Quill/QuillSub/CONTENTS`.
#[derive(Debug, Default)]
pub(crate) struct Quill {
    /// Paragraphs separated by `\n`.
    pub text: String,
    pub fonts: Vec<String>,
}

struct Chunk {
    tag: [u8; 4],
    off: usize,
    len: usize,
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))
}

fn u16_at(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))
}

const DIR_START: usize = 32;
const ENTRY: usize = 24;

fn directory(b: &[u8]) -> Vec<Chunk> {
    let mut out = Vec::new();
    if !b.starts_with(b"CHNKINK ") {
        return out;
    }
    let mut p = DIR_START;
    while u16_at(b, p) == Some(ENTRY as u16) && p + ENTRY <= b.len() {
        let tag: [u8; 4] = b[p + 2..p + 6].try_into().unwrap_or([0; 4]);
        let (Some(off), Some(len)) = (u32_at(b, p + 16), u32_at(b, p + 20)) else { break };
        let (off, len) = (off as usize, len as usize);
        if off.checked_add(len).is_some_and(|end| end <= b.len()) {
            out.push(Chunk { tag, off, len });
        }
        p += ENTRY;
    }
    out
}

/// Converts UTF-16LE story text: `\r` ends a paragraph; other control characters are dropped.
fn decode_text(raw: &[u8]) -> String {
    let units = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
    let mut s = String::new();
    for ch in char::decode_utf16(units) {
        match ch.unwrap_or('\u{fffd}') {
            '\r' => s.push('\n'),
            '\u{b}' => s.push(newpub_core::LINE_SEP),
            '\t' => s.push('\t'),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {}
            c => s.push(c),
        }
    }
    // The text ends with a paragraph mark that does not start a new paragraph.
    if s.ends_with('\n') {
        s.pop();
    }
    s
}

/// The FONT chunk is `u32 size, u32 count, 12 unknown bytes, count x u32 offsets, entries`. Offsets are relative to
/// the start of the offset table; each entry is `u16 length, UTF-16 chars, u32 id`.
fn decode_fonts(chunk: &[u8]) -> Vec<String> {
    const TABLE: usize = 20;
    let mut out: Vec<String> = Vec::new();
    let count = u32_at(chunk, 4).unwrap_or(0) as usize;
    if count == 0 || count > 4096 || TABLE + count * 4 > chunk.len() {
        return out;
    }
    for i in 0..count {
        let Some(off) = u32_at(chunk, TABLE + i * 4) else { break };
        let p = TABLE.saturating_add(off as usize);
        let n = u16_at(chunk, p).unwrap_or(0) as usize;
        let Some(raw) = p.checked_add(2).and_then(|a| chunk.get(a..a.checked_add(n * 2)?)) else { continue };
        let units = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]));
        if let Ok(name) = char::decode_utf16(units).collect::<Result<String, _>>()
            && !name.is_empty()
            && !out.contains(&name)
        {
            out.push(name);
        }
    }
    out
}

pub(crate) fn parse(b: &[u8], warnings: &mut Vec<String>) -> Quill {
    let dir = directory(b);
    if dir.is_empty() {
        warnings.push("the Quill stream has no readable chunk directory".into());
        return Quill::default();
    }
    let find = |tag: &[u8; 4]| dir.iter().find(|c| &c.tag == tag).map(|c| &b[c.off..c.off + c.len]);
    let text = find(b"TEXT").map(decode_text).unwrap_or_default();
    let fonts = find(b"FONT").map(decode_fonts).unwrap_or_default();
    if find(b"TEXT").is_none() {
        warnings.push("the Quill stream has no TEXT chunk".into());
    } else {
        warnings.push("story boundaries are not decoded; all text is imported as one story".into());
    }
    Quill { text, fonts }
}
