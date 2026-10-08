//! Hyperlink annotations and bookmarks (EX-05). Placeholder until Batch 2 task PDF lands.

use crate::impose::Sheet;
use newpub_core::Document;
use newpub_layout::DocLayout;

/// Adds link annotations for document page `page` drawn at `origin` (points, top-left) on this PDF page.
pub fn annotate_page(
    _pdf_page: &mut krilla::page::Page,
    _doc: &Document,
    _layout: &DocLayout,
    _page: usize,
    _origin: (f64, f64),
) {
}

/// Adds the document outline (bookmarks).
pub fn outline(_kd: &mut krilla::Document, _doc: &Document, _sheets: &[Sheet]) {}
