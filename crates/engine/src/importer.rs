//! Text import (TF-11): plain UTF-8 text and Word `.docx` text, inserted into a story.
//! The file type is detected from its content, not its extension.

use crate::{EngineError, Outcome, Session};
use newpub_core::{CharAttrs, Command, Id};
use std::io::{Cursor, Read};

/// Largest `word/document.xml` we will read (guards against decompression bombs).
const MAX_XML_BYTES: u64 = 64 * 1024 * 1024;

/// One piece of text to insert, with its formatting.
struct Run {
    text: String,
    attrs: Option<CharAttrs>,
}

/// Formatting flags of a Word run.
#[derive(Clone, Copy, Default)]
struct Fmt {
    bold: bool,
    italic: bool,
    underline: bool,
}

fn unsupported() -> EngineError {
    EngineError::Other("unsupported file type".into())
}

impl Session {
    /// Inserts the text of `path` into `target`'s story at `at` (default: end). One undo step.
    pub(crate) fn import_text(&mut self, target: Id, path: &str, at: Option<usize>) -> Result<Outcome, EngineError> {
        let p = self.resolve(path);
        let bytes = std::fs::read(&p)?;
        let runs = if bytes.starts_with(b"PK\x03\x04") { docx_runs(&bytes)? } else { plain_runs(&bytes)? };
        let sid = self.doc.story_of(target)?;
        let mut pos = match at {
            Some(at) => at,
            None => self.doc.story(sid)?.len(),
        };
        let mut d = self.doc.clone();
        for run in runs {
            if run.text.is_empty() {
                continue;
            }
            let n = run.text.chars().count();
            d.apply(&Command::InsertText { target: sid, at: Some(pos), text: run.text, attrs: run.attrs })?;
            pos += n;
        }
        self.commit(d, None);
        Ok(Outcome::default())
    }
}

/// Plain text: UTF-8 (invalid bytes replaced), CRLF normalised, one final newline dropped.
fn plain_runs(bytes: &[u8]) -> Result<Vec<Run>, EngineError> {
    const BINARY_MAGIC: [&[u8]; 5] = [b"\x89PNG", b"\xFF\xD8\xFF", b"GIF8", b"%PDF-", b"\xD0\xCF\x11\xE0"];
    if bytes.contains(&0) || BINARY_MAGIC.iter().any(|m| bytes.starts_with(m)) {
        return Err(unsupported());
    }
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
    Ok(vec![Run { text, attrs: None }])
}

/// A `.docx` (zip) with a `word/document.xml` part. Each `<w:p>` is a paragraph.
fn docx_runs(bytes: &[u8]) -> Result<Vec<Run>, EngineError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| unsupported())?;
    let entry = zip.by_name("word/document.xml").map_err(|_| unsupported())?;
    let mut xml = Vec::new();
    entry.take(MAX_XML_BYTES).read_to_end(&mut xml)?;
    if xml.len() as u64 >= MAX_XML_BYTES {
        return Err(EngineError::Other("word/document.xml is too large".into()));
    }
    let paragraphs = parse_document(&String::from_utf8_lossy(&xml));
    let mut runs = Vec::new();
    for (i, para) in paragraphs.into_iter().enumerate() {
        if i > 0 {
            runs.push(Run { text: "\n".into(), attrs: Some(CharAttrs::default()) });
        }
        runs.extend(para);
    }
    Ok(runs)
}

/// Scans WordprocessingML for paragraphs of formatted text runs. Tags it does not know are skipped.
fn parse_document(xml: &str) -> Vec<Vec<Run>> {
    let mut paragraphs: Vec<Vec<Run>> = Vec::new();
    let mut para: Vec<Run> = Vec::new();
    // The open run: its formatting and the text collected so far.
    let mut cur: Option<(Fmt, String)> = None;
    let mut in_rpr = false;
    let mut in_t = false;
    let mut i = 0;
    while let Some(off) = xml[i..].find('<') {
        let start = i + off;
        if let Some((_, text)) = cur.as_mut().filter(|_| in_t) {
            text.push_str(&unescape(&xml[i..start]));
        }
        let rest = &xml[start..];
        if rest.starts_with("<!--") {
            i = start + rest.find("-->").map(|e| e + 3).unwrap_or(rest.len());
            continue;
        }
        let Some(end) = rest.find('>') else { break };
        i = start + end + 1;
        let tag = &rest[1..end];
        let closing = tag.starts_with('/');
        let body = tag.trim_start_matches('/');
        let self_closing = body.ends_with('/');
        let body = body.trim_end_matches('/');
        let name = body.split(char::is_whitespace).next().unwrap_or("");
        match (name, closing) {
            ("w:p", false) => {
                flush(&mut cur, &mut para);
                if self_closing {
                    paragraphs.push(Vec::new());
                } else {
                    para.clear();
                }
            }
            ("w:p", true) => {
                flush(&mut cur, &mut para);
                paragraphs.push(std::mem::take(&mut para));
            }
            ("w:r", false) if !self_closing => {
                flush(&mut cur, &mut para);
                cur = Some((Fmt::default(), String::new()));
            }
            ("w:r", true) => flush(&mut cur, &mut para),
            ("w:rPr", false) => in_rpr = !self_closing,
            ("w:rPr", true) => in_rpr = false,
            ("w:b" | "w:i" | "w:u", false) if in_rpr => {
                if let Some((fmt, _)) = cur.as_mut() {
                    let on = !is_off(attr(body, "w:val"));
                    match name {
                        "w:b" => fmt.bold = on,
                        "w:i" => fmt.italic = on,
                        _ => fmt.underline = on,
                    }
                }
            }
            ("w:t", false) => in_t = !self_closing,
            ("w:t", true) => in_t = false,
            ("w:tab", false) => {
                if let Some((_, text)) = cur.as_mut() {
                    text.push('\t');
                }
            }
            ("w:br" | "w:cr", false) => {
                if let Some((_, text)) = cur.as_mut() {
                    text.push(' ');
                }
            }
            _ => {}
        }
    }
    paragraphs
}

/// Moves the open run (if it has text) into the paragraph.
fn flush(cur: &mut Option<(Fmt, String)>, para: &mut Vec<Run>) {
    if let Some((fmt, text)) = cur.take().filter(|(_, text)| !text.is_empty()) {
        let attrs = CharAttrs {
            bold: Some(fmt.bold),
            italic: Some(fmt.italic),
            underline: Some(fmt.underline),
            ..Default::default()
        };
        para.push(Run { text, attrs: Some(attrs) });
    }
}

/// Value of `name="..."` (or single-quoted) in a tag body, if present.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{name}="))? + name.len() + 1;
    let quote = tag[at..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value = &tag[at + 1..];
    Some(&value[..value.find(quote)?])
}

/// OOXML toggle values that mean "off".
fn is_off(val: Option<&str>) -> bool {
    matches!(val, Some("0" | "false" | "off" | "none"))
}

/// Decodes the XML entities that can appear in text (named and numeric).
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        match after.find(';').filter(|&e| e <= 10).and_then(|e| decode_entity(&after[..e]).map(|c| (c, e))) {
            Some((c, e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}
