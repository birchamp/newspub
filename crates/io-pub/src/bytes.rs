//! Bounds-checked little-endian reads. Every accessor returns `None` instead of panicking.

pub(crate) fn u8_at(b: &[u8], p: usize) -> Option<u8> {
    b.get(p).copied()
}

pub(crate) fn u16_at(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(p..p.checked_add(2)?)?.try_into().ok()?))
}

pub(crate) fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))
}

pub(crate) fn i32_at(b: &[u8], p: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(p..p.checked_add(4)?)?.try_into().ok()?))
}

/// Decodes UTF-16LE up to the first NUL; unpaired surrogates become U+FFFD.
pub(crate) fn utf16_z(b: &[u8]) -> String {
    let units = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0);
    char::decode_utf16(units).map(|r| r.unwrap_or('\u{fffd}')).collect()
}
