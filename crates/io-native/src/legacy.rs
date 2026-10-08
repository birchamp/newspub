//! Predecessor NewsPub (Electron) `.newspub` files, converted on open (FI-05; ARCHITECTURE.md §12).
//!
//! The predecessor is a zip with `metadata.json`, `document.json` (`metadata`, `pages[].frames`,
//! `threads`, `assetManifest`) and `assets/<filename>`. Frames refer to threads (stories) by id; a
//! thread's frames are chained in `threadOrder`. Paragraphs are `\n` in run text.

use crate::NativeError;
use newpub_core::{
    Align, Asset, CharAttrs, CharSpan, Color, CropFrac, Document, Fit, Id, ImageAdjust, ImageFrame, ImageMask, Insets,
    Length, Meta, Object, ObjectKind, PageSetup, ParaAttrs, Rect, Story, TextFrame, Wrap, WrapMode,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};
use std::sync::Arc;
use zip::ZipArchive;

const NATIVE_MIME: &[u8] = b"application/x-newpub";
const DEFAULT_PAGE: (f64, f64) = (612.0, 792.0);
/// The predecessor had one fixed margin on every side.
const MARGIN: f64 = 36.0;

static NULL: Value = Value::Null;

/// Converts a predecessor file. `Ok(None)` when the archive is not one (a native file, or anything else).
pub fn try_convert<R: Read + Seek>(z: &mut ZipArchive<R>) -> Result<Option<Document>, NativeError> {
    if read_entry(z, "mimetype")?.is_some_and(|m| m == NATIVE_MIME) {
        return Ok(None);
    }
    if read_entry(z, "metadata.json")?.is_none() {
        return Ok(None);
    }
    let Some(json) = read_entry(z, "document.json")? else {
        return Ok(None);
    };
    let root: Value = serde_json::from_slice(&json)?;
    if root.get("threads").is_none() {
        return Ok(None);
    }
    convert(z, &root).map(Some)
}

fn read_entry<R: Read + Seek>(z: &mut ZipArchive<R>, name: &str) -> Result<Option<Vec<u8>>, NativeError> {
    let Ok(mut f) = z.by_name(name) else {
        return Ok(None);
    };
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;
    Ok(Some(buf))
}

fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k)?.as_str()
}

fn f64_of(v: &Value, k: &str) -> Option<f64> {
    v.get(k)?.as_f64()
}

fn bool_of(v: &Value, k: &str) -> Option<bool> {
    v.get(k)?.as_bool()
}

fn items<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Run or thread style. Missing fields stay `None` and fall back to the thread's default style.
#[derive(Default)]
struct Style {
    font: Option<String>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    color: Option<String>,
    align: Option<String>,
}

fn style(v: Option<&Value>) -> Style {
    let Some(v) = v else {
        return Style::default();
    };
    Style {
        font: str_of(v, "fontFamily").map(str::to_string),
        size: f64_of(v, "fontSize"),
        bold: bool_of(v, "bold"),
        italic: bool_of(v, "italic"),
        color: str_of(v, "color").map(str::to_string),
        align: str_of(v, "alignment").map(str::to_string),
    }
}

fn char_attrs(st: &Style, default: &Style) -> CharAttrs {
    CharAttrs {
        font: st.font.clone().or_else(|| default.font.clone()).filter(|f| !f.is_empty()),
        size: st.size.or(default.size).filter(|s| *s > 0.0).map(Length),
        bold: st.bold.or(default.bold),
        italic: st.italic.or(default.italic),
        color: st.color.as_deref().or(default.color.as_deref()).and_then(Color::parse),
        ..Default::default()
    }
}

fn align_of(s: Option<&str>) -> Option<Align> {
    match s? {
        "left" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" => Some(Align::Right),
        "justify" => Some(Align::Justify),
        _ => None,
    }
}

/// Builds one story from a thread. Each paragraph takes its alignment from the run that holds its first char.
fn build_story(id: Id, thread: Option<&Value>, frames: Vec<Id>) -> Story {
    let default = style(thread.and_then(|t| t.get("defaultStyle")));
    let runs: &[Value] = thread.map(|t| items(t, "runs")).unwrap_or(&[]);
    let mut text = String::new();
    let mut chars: Vec<CharSpan> = vec![];
    let mut paras: Vec<ParaAttrs> = vec![];
    let mut para_start = true;
    for run in runs {
        let run_text = str_of(run, "text").unwrap_or_default().replace("\r\n", "\n").replace('\r', "\n");
        if run_text.is_empty() {
            continue;
        }
        let st = style(run.get("style"));
        let align = align_of(st.align.as_deref().or(default.align.as_deref()));
        for c in run_text.chars() {
            if para_start {
                paras.push(ParaAttrs { align, ..Default::default() });
            }
            para_start = c == '\n';
        }
        chars.push(CharSpan { len: run_text.chars().count(), attrs: char_attrs(&st, &default) });
        text.push_str(&run_text);
    }
    if para_start {
        paras.push(ParaAttrs::default());
    }
    let mut story = Story { id, text, chars, paras, frames };
    story.normalize();
    story
}

fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.get(..8)? != b"\x89PNG\r\n\x1a\n" || b.get(12..16)? != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
    Some((w, h))
}

fn convert<R: Read + Seek>(z: &mut ZipArchive<R>, root: &Value) -> Result<Document, NativeError> {
    let meta = root.get("metadata").unwrap_or(&NULL);
    let size = meta.get("pageSize").unwrap_or(&NULL);
    let (pw, ph) = (f64_of(size, "width").unwrap_or(0.0), f64_of(size, "height").unwrap_or(0.0));
    let (pw, ph) = if pw > 0.0 && ph > 0.0 { (pw, ph) } else { DEFAULT_PAGE };
    let pages = items(root, "pages");
    let setup = PageSetup {
        width: Length(pw),
        height: Length(ph),
        margins: Insets::uniform(MARGIN),
        facing: false,
        bleed: Length(0.0),
    };
    let mut doc = Document::new(setup, pages.len());
    doc.meta = Meta {
        title: str_of(meta, "title").unwrap_or_default().to_string(),
        author: str_of(meta, "author").unwrap_or_default().to_string(),
        lang: String::new(),
    };

    let mut assets: HashMap<&str, Id> = HashMap::new();
    for entry in items(root, "assetManifest") {
        let (Some(key), Some(file)) = (str_of(entry, "id"), str_of(entry, "filename")) else {
            continue;
        };
        let Some(bytes) = read_entry(z, &format!("assets/{file}"))? else {
            continue;
        };
        let id = doc.alloc();
        let (px_w, px_h) = png_size(&bytes).unwrap_or((0, 0));
        let mime = str_of(entry, "mimeType").unwrap_or("application/octet-stream").to_string();
        let name = file.to_string();
        doc.assets.insert(id, Asset { id, name, mime, px_w, px_h, link: None, bytes: Arc::from(bytes) });
        assets.insert(key, id);
    }

    let threads: Option<&Map<String, Value>> = root.get("threads").and_then(Value::as_object);
    // One story per thread, numbered in thread-id order so ids are stable across opens.
    let mut story_ids: BTreeMap<String, Id> = BTreeMap::new();
    if let Some(threads) = threads {
        for key in threads.keys() {
            story_ids.entry(key.clone()).or_insert_with(|| doc.alloc());
        }
    }
    // Text frames per thread: (threadOrder, object id), collected in page order.
    let mut chained: BTreeMap<String, Vec<(f64, Id)>> = BTreeMap::new();

    for (pi, page) in pages.iter().enumerate() {
        for frame in items(page, "frames") {
            let ty = str_of(frame, "type");
            if !matches!(ty, Some("text" | "image")) {
                continue;
            }
            let r = frame.get("rect").unwrap_or(&NULL);
            let rect = Rect::new(
                f64_of(r, "x").unwrap_or(0.0),
                f64_of(r, "y").unwrap_or(0.0),
                f64_of(r, "width").unwrap_or(0.0),
                f64_of(r, "height").unwrap_or(0.0),
            );
            let id = doc.alloc();
            let (kind, wrap) = if ty == Some("text") {
                let key = str_of(frame, "threadId").unwrap_or_default().to_string();
                let story = *story_ids.entry(key.clone()).or_insert_with(|| doc.alloc());
                chained.entry(key).or_default().push((f64_of(frame, "threadOrder").unwrap_or(0.0), id));
                (ObjectKind::Text(TextFrame::new(story)), Wrap::default())
            } else {
                let fit = match str_of(frame, "imageFit") {
                    Some("fit") => Fit::Fit,
                    Some("fill") => Fit::Fill,
                    Some("stretch") => Fit::Stretch,
                    _ => Fit::default(),
                };
                let mode = match str_of(frame, "wrapMode") {
                    Some("rect") => WrapMode::Square,
                    Some("skip") => WrapMode::TopBottom,
                    _ => WrapMode::None,
                };
                let asset = str_of(frame, "imageAssetId").and_then(|k| assets.get(k).copied());
                let image = ImageFrame {
                    asset,
                    crop: CropFrac::default(),
                    fit,
                    stroke: None,
                    adjust: ImageAdjust::default(),
                    mask: ImageMask::default(),
                    soft_edges: Length(0.0),
                    merge_field: None,
                };
                (ObjectKind::Image(image), Wrap { mode, ..Default::default() })
            };
            let name = str_of(frame, "label").unwrap_or_default().to_string();
            doc.objects.insert(
                id,
                Object {
                    id,
                    name,
                    rect,
                    rotation: 0.0,
                    flip_h: false,
                    flip_v: false,
                    kind,
                    wrap,
                    alt_text: None,
                    decorative: false,
                    locked: false,
                    layer: None,
                    parent: None,
                    shadow: None,
                },
            );
            if let Some(p) = doc.pages.get_mut(pi) {
                p.objects.push(id);
            }
        }
    }

    // Every thread a frame names is in `story_ids` (the frame loop adds it), so each story is built here.
    for (key, id) in &story_ids {
        let mut frames = chained.remove(key).unwrap_or_default();
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        let frames: Vec<Id> = frames.into_iter().map(|(_, id)| id).collect();
        let thread = threads.and_then(|t| t.get(key));
        doc.stories.insert(*id, build_story(*id, thread, frames));
    }
    Ok(doc)
}
