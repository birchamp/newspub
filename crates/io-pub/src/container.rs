//! OLE container access: stream listing, version detection, stream reads.

use crate::PubError;
use cfb::CompoundFile;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

const QUILL_TEXT: &str = "Quill/QuillSub/CONTENTS";
const MAX_STREAM: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub is_publisher: bool,
    pub version: String,
    /// Full stream paths with `/` separators, followed by the short names of streams inside storages.
    pub streams: Vec<String>,
}

pub(crate) fn open(path: &Path) -> Result<CompoundFile<File>, PubError> {
    let f = File::open(path)?;
    match CompoundFile::open(f) {
        Ok(cf) => Ok(cf),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            Err(PubError::NotPublisher("the file is not an OLE compound document".into()))
        }
        Err(e) => Err(e.into()),
    }
}

fn path_string(p: &Path) -> String {
    let parts: Vec<String> = p
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts.join("/")
}

pub(crate) fn read_stream(cf: &mut CompoundFile<File>, path: &str) -> Result<Option<Vec<u8>>, PubError> {
    let full = format!("/{path}");
    if !cf.is_stream(&full) {
        return Ok(None);
    }
    let s = cf.open_stream(&full)?;
    let mut out = Vec::new();
    s.take(MAX_STREAM).read_to_end(&mut out)?;
    Ok(Some(out))
}

/// Maps the `Contents` header's format number to a product name. The numbers come from a single sample plus
/// public notes, so unknown values are reported as such rather than guessed.
fn version_name(contents: &[u8]) -> String {
    if contents.len() < 4 || contents[0] != 0xE8 || contents[1] != 0xAC {
        return "Publisher (unknown version)".into();
    }
    let n = u16::from_le_bytes([contents[2], contents[3]]);
    match n {
        0x2A => "Publisher 98".into(),
        0x2C => "Publisher 2000".into(),
        _ => format!("Publisher (unknown version, format {n:#x})"),
    }
}

pub(crate) fn report(cf: &mut CompoundFile<File>) -> Result<Report, PubError> {
    let mut streams: Vec<String> = cf.walk().filter(|e| e.is_stream()).map(|e| path_string(e.path())).collect();
    streams.sort();
    let has = |n: &str| streams.iter().any(|s| s == n);
    let is_publisher = has("Contents") && (has(QUILL_TEXT) || has("Escher/EscherStm"));
    if !is_publisher {
        return Err(PubError::NotPublisher("the OLE document has no Publisher streams".into()));
    }
    let mut head = read_stream(cf, "Contents")?.unwrap_or_default();
    head.truncate(4);
    // Streams inside storages are also listed by their short name, so callers can look up `EscherStm` as well as
    // `Escher/EscherStm`. Full paths come first.
    let leaves: Vec<String> = streams.iter().filter_map(|s| s.rsplit_once('/').map(|(_, l)| l.to_string())).collect();
    for leaf in leaves {
        if !streams.contains(&leaf) {
            streams.push(leaf);
        }
    }
    Ok(Report { is_publisher, version: version_name(&head), streams })
}
