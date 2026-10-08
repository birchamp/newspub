//! PDF/X-4 and PDF/UA-1 output (EX-03, AX-03). Placeholder until those tasks land.

use crate::{PdfError, PdfOptions};
use newpub_core::Document;

/// Final pass over the serialized PDF for the requested standard.
pub fn finish(bytes: Vec<u8>, _doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    match opts.standard {
        None => Ok(bytes),
        Some(s) => Err(PdfError::Unsupported(format!("{s:?} output is not implemented yet"))),
    }
}
