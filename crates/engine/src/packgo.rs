//! Pack and Go (PR-09).
//! Owner: Batch 4 task PACKGO.
//!
//! Collects a publication into one folder: the `.newspub` file, a copy of every linked picture (`links/`),
//! the font faces the text uses (`fonts/`), optionally a PDF, and a `pack-report.txt` that lists what was written.

use crate::{EngineError, Outcome, PdfOptions, Session, SessionAction};
use newpub_core::Document;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

/// Characters that are not allowed in file names on the common desktop systems.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// OS/2 `fsType` bit for a restricted licence: the font must not be embedded.
const FS_RESTRICTED: u16 = 0x0002;

/// The publication name used for files: the title without forbidden characters, or "publication".
fn sanitise_name(title: &str) -> String {
    let cleaned: String = title.chars().filter(|c| !FORBIDDEN.contains(c) && !c.is_control()).collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() { "publication".to_string() } else { trimmed.to_string() }
}

/// `file` if it is free in `taken`, otherwise `name-2.ext`, `name-3.ext`, ... Names compare case-insensitively.
fn unique_name(taken: &mut BTreeSet<String>, file: &str) -> String {
    let (stem, ext) = match file.rfind('.') {
        Some(i) if i > 0 => (&file[..i], &file[i..]),
        _ => (file, ""),
    };
    let mut candidate = file.to_string();
    let mut n = 2;
    while taken.contains(&candidate.to_lowercase()) {
        candidate = format!("{stem}-{n}{ext}");
        n += 1;
    }
    taken.insert(candidate.to_lowercase());
    candidate
}

/// Reads a big-endian `u16` at `at`.
fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}

/// Reads a big-endian `u32` at `at` as an offset.
fn be32(bytes: &[u8], at: usize) -> Option<usize> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

/// The OS/2 `fsType` field of the font with `index` in a font file (a collection holds several).
fn fs_type(bytes: &[u8], index: u32) -> Option<u16> {
    let dir = if bytes.get(0..4)? == &b"ttcf"[..] { be32(bytes, 12 + 4 * index as usize)? } else { 0 };
    let tables = be16(bytes, dir + 4)? as usize;
    for i in 0..tables {
        let rec = dir + 12 + 16 * i;
        if bytes.get(rec..rec + 4)? == &b"OS/2"[..] {
            let table = be32(bytes, rec + 8)?;
            return be16(bytes, table + 8);
        }
    }
    None
}

impl Session {
    pub(crate) fn packgo_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::PackAndGo { dir, fonts, pdf } => self.pack_and_go(dir, *fonts, *pdf),
            _ => Err(EngineError::Other(format!("{a:?} is not a pack-and-go action"))),
        }
    }

    /// Faces used by any text: the bold/italic variant of each run's font, plus a fallback face for each
    /// character the requested face lacks. Whitespace and control characters need no glyph.
    fn packgo_faces(&self) -> BTreeSet<newpub_layout::FaceId> {
        let mut out = BTreeSet::new();
        for st in self.doc.stories.values() {
            let ranges = st.para_ranges();
            let chars: Vec<char> = st.text.chars().collect();
            for (run, attrs) in st.runs() {
                if run.is_empty() {
                    continue;
                }
                for (pi, pr) in ranges.iter().enumerate() {
                    let (lo, hi) = (run.start.max(pr.start), run.end.min(pr.end));
                    if lo >= hi {
                        continue;
                    }
                    let Some(para) = st.paras.get(pi) else { continue };
                    let rc = self.doc.resolve_char(para, attrs);
                    let id = self.fonts.resolve(&rc.font, rc.bold, rc.italic);
                    out.insert(id);
                    let face = self.fonts.face(id);
                    let mut seen = BTreeSet::new();
                    for &c in chars.get(lo..hi).unwrap_or(&[]) {
                        if c.is_whitespace() || c.is_control() || !seen.insert(c) || face.has_glyph(c) {
                            continue;
                        }
                        if let Some(fb) = self.fonts.fallback_for(c, rc.bold, rc.italic) {
                            out.insert(fb);
                        }
                    }
                }
            }
        }
        out
    }

    fn pack_and_go(&mut self, dir: &str, fonts: bool, pdf: bool) -> Result<Outcome, EngineError> {
        let root = self.resolve(dir);
        if root.exists() && !root.is_dir() {
            return Err(EngineError::Other(format!("{} exists and is not a folder", root.display())));
        }
        std::fs::create_dir_all(&root)?;
        let name = sanitise_name(&self.doc.meta.title);
        let mut written: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();

        // Linked pictures are copied into links/ and relinked there. Embedded pictures stay embedded.
        let mut packed: Document = self.doc.clone();
        let mut taken_links = BTreeSet::new();
        let mut copied: HashMap<String, String> = HashMap::new();
        for asset in packed.assets.values_mut() {
            let Some(link) = asset.link.clone() else { continue };
            if let Some(rel) = copied.get(&link) {
                asset.link = Some(rel.clone());
                continue;
            }
            // Bytes are loaded when the file is opened; a link whose file has since gone is read again here.
            let data: Option<Vec<u8>> = if asset.bytes.is_empty() {
                std::fs::read(self.resolve(&link)).ok()
            } else {
                Some(asset.bytes.to_vec())
            };
            let Some(data) = data else {
                skipped.push(format!("link {link}: file not found, the picture stays linked to its old path"));
                continue;
            };
            let file_name = Path::new(&link)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .filter(|f| !f.is_empty())
                .unwrap_or_else(|| asset.name.clone());
            let file = unique_name(&mut taken_links, &sanitise_name(&file_name));
            std::fs::create_dir_all(root.join("links"))?;
            std::fs::write(root.join("links").join(&file), &data)?;
            let rel = format!("links/{file}");
            written.push(rel.clone());
            copied.insert(link, rel.clone());
            asset.link = Some(rel);
        }

        // Fonts: the face bytes, unless the font licence forbids embedding.
        if fonts {
            let mut taken_fonts = BTreeSet::new();
            for id in self.packgo_faces() {
                let face = self.fonts.face(id);
                let style = match (face.bold, face.italic) {
                    (false, false) => "Regular",
                    (true, false) => "Bold",
                    (false, true) => "Italic",
                    (true, true) => "BoldItalic",
                };
                let family: String = face.family.chars().filter(|c| c.is_alphanumeric()).collect();
                let family = if family.is_empty() { "Font".to_string() } else { family };
                let label = format!("{family}-{style}.ttf");
                if fs_type(face.bytes(), face.index).is_some_and(|t| t & FS_RESTRICTED != 0) {
                    skipped.push(format!("fonts/{label}: the font licence does not permit embedding"));
                    continue;
                }
                let file = unique_name(&mut taken_fonts, &label);
                std::fs::create_dir_all(root.join("fonts"))?;
                std::fs::write(root.join("fonts").join(&file), face.bytes())?;
                written.push(format!("fonts/{file}"));
            }
        }

        let doc_file = format!("{name}.newspub");
        newpub_io_native::save(&packed, &root.join(&doc_file))?;
        written.insert(0, doc_file);

        if pdf {
            let bytes = self.pdf_bytes(&PdfOptions::default())?;
            let pdf_file = format!("{name}.pdf");
            std::fs::write(root.join(&pdf_file), bytes)?;
            written.push(pdf_file);
        }

        written.push("pack-report.txt".to_string());
        let mut report = String::new();
        let _ = writeln!(report, "Pack and Go: {name}");
        let _ = writeln!(report, "Folder: {}", root.display());
        let _ = writeln!(report);
        let _ = writeln!(report, "Files written:");
        for f in &written {
            let _ = writeln!(report, "  {f}");
        }
        let _ = writeln!(report);
        if skipped.is_empty() {
            let _ = writeln!(report, "Skipped: none");
        } else {
            let _ = writeln!(report, "Skipped:");
            for s in &skipped {
                let _ = writeln!(report, "  {s}");
            }
        }
        std::fs::write(root.join("pack-report.txt"), report)?;
        Ok(Outcome::default())
    }
}
