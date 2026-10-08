//! newpub-io-native: the `.npub` format — a zip with `document.json` and `media/<id>`.

use newpub_core::{Document, FORMAT_VERSION};
use std::io::{Read, Seek, Write};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("file was written by a newer newpub (format {0}, this build reads up to {FORMAT_VERSION})")]
    TooNew(u32),
    #[error("missing media entry {0}")]
    MissingMedia(String),
}

pub fn write<W: Write + Seek>(doc: &Document, w: W) -> Result<(), NativeError> {
    let mut z = zip::ZipWriter::new(w);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file(
        "mimetype",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    z.write_all(b"application/x-newpub")?;
    z.start_file("document.json", opts)?;
    z.write_all(&serde_json::to_vec_pretty(doc)?)?;
    for a in doc.assets.values() {
        if a.link.is_some() && a.bytes.is_empty() {
            continue;
        }
        z.start_file(
            format!("media/{}", a.id.0),
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )?;
        z.write_all(&a.bytes)?;
    }
    z.finish()?;
    Ok(())
}

pub fn read<R: Read + Seek>(r: R) -> Result<Document, NativeError> {
    let mut z = zip::ZipArchive::new(r)?;
    let mut json = vec![];
    z.by_name("document.json")?.read_to_end(&mut json)?;
    let probe: serde_json::Value = serde_json::from_slice(&json)?;
    let v = probe.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    if v > FORMAT_VERSION {
        return Err(NativeError::TooNew(v));
    }
    let mut doc: Document = serde_json::from_value(migrate(probe, v))?;
    for a in doc.assets.values_mut() {
        let name = format!("media/{}", a.id.0);
        match z.by_name(&name) {
            Ok(mut f) => {
                let mut b = vec![];
                f.read_to_end(&mut b)?;
                a.bytes = Arc::from(b);
            }
            Err(_) if a.link.is_some() => {
                if let Some(bytes) = a.link.as_ref().and_then(|p| std::fs::read(p).ok()) {
                    a.bytes = Arc::from(bytes);
                }
            }
            Err(_) => return Err(NativeError::MissingMedia(name)),
        }
    }
    doc.version = FORMAT_VERSION;
    Ok(doc)
}

/// Upgrades older document JSON to the current format version.
fn migrate(v: serde_json::Value, _from: u32) -> serde_json::Value {
    v
}

pub fn save(doc: &Document, path: &std::path::Path) -> Result<(), NativeError> {
    // Write to a temp file then rename, so a crash never leaves a half-written file.
    let tmp = path.with_extension("npub.tmp");
    {
        let f = std::fs::File::create(&tmp)?;
        write(doc, std::io::BufWriter::new(f))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn open(path: &std::path::Path) -> Result<Document, NativeError> {
    read(std::io::BufReader::new(std::fs::File::open(path)?))
}
