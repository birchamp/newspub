//! Windows metafile import (IM-11): EMF and WMF drawings converted to SVG, written from the published
//! MS-EMF and MS-WMF record layouts. The common drawing records are supported: rectangles, rounded
//! rectangles, ellipses, polygons, polylines, poly-polygons, Bézier curves, paths (begin/end/fill/stroke),
//! move/line, pens and brushes (solid and null), text, window/viewport mapping, world transforms (EMF),
//! saved device contexts and fill modes. Bitmaps, regions, clipping and gradients are skipped.

mod emf;
mod svg;
mod wmf;

#[derive(Debug, thiserror::Error)]
pub enum MetafileError {
    #[error("not a Windows metafile")]
    NotMetafile,
    #[error("damaged metafile: {0}")]
    Damaged(String),
}

/// Is this an EMF (starts with an EMR_HEADER record carrying the " EMF" signature)?
pub fn is_emf(b: &[u8]) -> bool {
    b.len() >= 44 && b[0..4] == [1, 0, 0, 0] && b[40..44] == [0x20, 0x45, 0x4D, 0x46]
}

/// Is this a WMF (placeable header key, or a plain META header)?
pub fn is_wmf(b: &[u8]) -> bool {
    b.len() >= 18
        && (b[0..4] == [0xD7, 0xCD, 0xC6, 0x9A] || (b[0..6] == [1, 0, 9, 0, 0, 3]) || (b[0..6] == [2, 0, 9, 0, 0, 3]))
}

/// Converts an EMF or WMF file to an SVG document (sized in millimetres or inches from the file's frame).
pub fn to_svg(bytes: &[u8]) -> Result<String, MetafileError> {
    if is_emf(bytes) {
        emf::convert(bytes)
    } else if is_wmf(bytes) {
        wmf::convert(bytes)
    } else {
        Err(MetafileError::NotMetafile)
    }
}
