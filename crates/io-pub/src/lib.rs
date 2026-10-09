//! newpub-io-pub: Microsoft Publisher (.pub) import (ARCHITECTURE.md §9).
//!
//! The parser is our own, written from the public format descriptions listed in FORMAT-NOTES.md (chiefly the
//! libmspub project, which is also the reference the journeys were measured against) and from inspection of sample
//! files. It reads the OLE container, the `Contents` object directory, the Escher drawing records and the Quill text
//! stream of Publisher 2002 and later, and the older fixed-layout `Contents` of Publisher 98 and 2000.
//!
//! Imported: page size and page count, text boxes with their geometry and linked-box chains, character and
//! paragraph formatting, pictures, basic shapes with fill and line, groups, tables and master pages. What is not
//! imported is listed in [`ImportReport::warnings`].

mod blocks;
mod build;
mod bytes;
mod container;
mod contents;
mod escher;
mod ir;
mod legacy;
mod quill;
mod v2002;

use newpub_core::Document;
use serde::Serialize;
use std::path::Path;

pub use container::Report;

#[derive(Debug, thiserror::Error)]
pub enum PubError {
    #[error("not a Publisher file: {0}")]
    NotPublisher(String),
    #[error("damaged Publisher file: {0}")]
    Damaged(String),
    #[error("unsupported Publisher file: {0}")]
    Unsupported(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Doc(#[from] newpub_core::CoreError),
}

/// What an import did and what it could not decode.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ImportReport {
    /// Product that wrote the file, as far as the file says.
    pub version: String,
    pub pages: usize,
    pub frames_placed: usize,
    pub fonts: Vec<String>,
    pub warnings: Vec<String>,
}

/// A decoded picture ready to become a document asset.
#[derive(Clone, Debug)]
pub struct Picture {
    /// `image/png`, `image/jpeg` or `image/svg+xml`.
    pub mime: String,
    pub bytes: Vec<u8>,
    pub px_w: u32,
    pub px_h: u32,
}

/// Turns picture file bytes (PNG, JPEG, GIF, BMP, TIFF, EMF, WMF) into an asset, or `None` when it cannot.
pub type PictureDecoder<'a> = &'a dyn Fn(&[u8]) -> Option<Picture>;

/// Opens the container and reports what Publisher streams it holds.
pub fn inspect(path: &Path) -> Result<Report, PubError> {
    let mut cf = container::open(path)?;
    container::report(&mut cf)
}

/// Imports a Publisher file as a new document. Only PNG and JPEG pictures are kept; use [`import_with`] to
/// supply a decoder for the other picture formats.
pub fn import(path: &Path) -> Result<(Document, ImportReport), PubError> {
    import_with(path, &basic_decoder)
}

/// Imports a Publisher file, decoding pictures with `decode`.
pub fn import_with(path: &Path, decode: PictureDecoder) -> Result<(Document, ImportReport), PubError> {
    let mut cf = container::open(path)?;
    let rep = container::report(&mut cf)?;
    let read = |cf: &mut _, name: &str| container::read_stream(cf, name);
    let contents = read(&mut cf, "Contents")?.ok_or_else(|| PubError::Damaged("no Contents stream".into()))?;
    let quill = read(&mut cf, "Quill/QuillSub/CONTENTS")?;
    let escher_stream = read(&mut cf, "Escher/EscherStm")?;
    let delay = read(&mut cf, "Escher/EscherDelayStm")?;

    let generation = container::generation(&contents)
        .ok_or_else(|| PubError::Unsupported(format!("unknown Publisher format ({})", rep.version)))?;
    let mut parsed = match generation {
        ir::Generation::V2002 => {
            let q = quill.ok_or_else(|| PubError::Damaged("no Quill text stream".into()))?;
            let e = escher_stream.ok_or_else(|| PubError::Damaged("no Escher drawing stream".into()))?;
            v2002::read(&v2002::Streams { contents: &contents, escher: &e, delay: delay.as_deref(), quill: &q })
        }
        ir::Generation::V2000 | ir::Generation::V97 => legacy::read(&contents, quill.as_deref(), generation),
    }
    .map_err(PubError::Damaged)?;

    let mut report = ImportReport { version: rep.version.clone(), pages: parsed.pages.len(), ..Default::default() };
    report.warnings.append(&mut parsed.warnings);
    let doc = build::build(&parsed, decode, &mut report).map_err(PubError::Damaged)?;
    Ok((doc, report))
}

/// PNG and JPEG pictures, with their pixel size read from the file header.
fn basic_decoder(b: &[u8]) -> Option<Picture> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
        return Some(Picture { mime: "image/png".into(), bytes: b.to_vec(), px_w: w, px_h: h });
    }
    if b.starts_with(&[0xff, 0xd8]) {
        let mut p = 2;
        while p + 9 < b.len() {
            if b[p] != 0xff {
                p += 1;
                continue;
            }
            let marker = b[p + 1];
            let len = usize::from(u16::from_be_bytes([b[p + 2], b[p + 3]]));
            if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                let h = u32::from(u16::from_be_bytes([b[p + 5], b[p + 6]]));
                let w = u32::from(u16::from_be_bytes([b[p + 7], b[p + 8]]));
                return Some(Picture { mime: "image/jpeg".into(), bytes: b.to_vec(), px_w: w, px_h: h });
            }
            p += 2 + len.max(2);
        }
    }
    None
}
