//! OLE container access: stream listing, version detection, stream reads.

use crate::PubError;
use crate::bytes::{u8_at, u16_at, u32_at};
use crate::ir::Generation;
use cfb::CompoundFile;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

/// Largest stream read into memory.
const MAX_STREAM: u64 = 128 * 1024 * 1024;

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

/// File format generation from the `Contents` header: signature `E8 AC`, then the format number.
pub(crate) fn generation(contents: &[u8]) -> Option<Generation> {
    if u8_at(contents, 0)? != 0xE8 || u8_at(contents, 1)? != 0xAC {
        return None;
    }
    match u16_at(contents, 2)? {
        0x2C => Some(Generation::V2002),
        // Publisher 97, 98 and 2000 share format number 0x22; the Quill stream tells 97 apart.
        0x22 => Some(Generation::V2000),
        _ => None,
    }
}

/// Application major version from the "Version" property of the document summary (PIDDSI_VERSION).
fn app_major_version(cf: &mut CompoundFile<File>) -> Option<u32> {
    let b = read_stream(cf, "\u{5}DocumentSummaryInformation").ok()??;
    let sections = u32_at(&b, 24)? as usize;
    for s in 0..sections.min(4) {
        let off = u32_at(&b, 44 + s * 20)? as usize;
        let count = u32_at(&b, off + 4)? as usize;
        for i in 0..count.min(64) {
            let pid = u32_at(&b, off + 8 + i * 8)?;
            let at = off.checked_add(u32_at(&b, off + 12 + i * 8)? as usize)?;
            if pid == 0x17 && u32_at(&b, at)? == 3 {
                return u32_at(&b, at + 4).map(|v| v >> 16);
            }
        }
    }
    None
}

/// Product name for a format generation and application version.
fn version_name(contents: &[u8], major: Option<u32>) -> String {
    match generation(contents) {
        Some(Generation::V2002) => match major {
            Some(10) => "Publisher 2002".into(),
            Some(11) => "Publisher 2003".into(),
            Some(12) => "Publisher 2007".into(),
            Some(14) => "Publisher 2010".into(),
            Some(15) => "Publisher 2013".into(),
            Some(16..) => "Publisher 2016 or later".into(),
            _ => "Publisher 2002 or later".into(),
        },
        Some(Generation::V2000) | Some(Generation::V97) => "Publisher 98 or 2000".into(),
        None => "Publisher (unknown version)".into(),
    }
}

pub(crate) fn report(cf: &mut CompoundFile<File>) -> Result<Report, PubError> {
    let mut streams: Vec<String> = cf.walk().filter(|e| e.is_stream()).map(|e| path_string(e.path())).collect();
    streams.sort();
    if !streams.iter().any(|s| s == "Contents") {
        return Err(PubError::NotPublisher("the OLE document has no Publisher streams".into()));
    }
    let contents = read_stream(cf, "Contents")?.unwrap_or_default();
    if contents.get(0..2) != Some(&[0xE8, 0xAC]) {
        return Err(PubError::NotPublisher("the OLE document has no Publisher streams".into()));
    }
    let version = version_name(&contents, app_major_version(cf));
    // Streams inside storages are also listed by their short name, so callers can look up `EscherStm` as well as
    // `Escher/EscherStm`. Full paths come first.
    let leaves: Vec<String> = streams.iter().filter_map(|s| s.rsplit_once('/').map(|(_, l)| l.to_string())).collect();
    for leaf in leaves {
        if !streams.contains(&leaf) {
            streams.push(leaf);
        }
    }
    Ok(Report { is_publisher: true, version, streams })
}
