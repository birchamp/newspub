//! Colour separations (PR-08).
//! Owner: Batch 4 task SEPARATIONS. Placeholder until that task lands.

use crate::{PdfError, PdfOptions};
use newpub_core::Document;
use newpub_layout::{DocLayout, FontStore};

/// Plate names in output order: "Cyan", "Magenta", "Yellow", "Black", then the spot colours used (by name).
pub fn plates(_doc: &Document) -> Vec<String> {
    vec![]
}

/// One PDF page per plate per exported page (pages outer, plates inner).
pub fn export_separations(
    _doc: &Document,
    _layout: &DocLayout,
    _fonts: &FontStore,
    _opts: &PdfOptions,
    _indices: &[usize],
) -> Result<Vec<u8>, PdfError> {
    Err(PdfError::Unsupported("colour separations are not implemented yet".into()))
}
