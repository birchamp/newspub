//! newpub-engine: the session that the app and the journey runner both drive.
//! Every user-visible operation goes through [`Session::run`]; every observation through
//! [`Session::query`] or exported files.

pub mod action;

pub use action::{Action, Query, SessionAction};
pub use newpub_core as core;
pub use newpub_io_pdf::{Imposition, PdfOptions};
pub use newpub_layout as layout;
pub use newpub_render as render;

use newpub_core::*;
use newpub_layout::{DocLayout, FontStore};
use newpub_render::Rasterizer;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Native(#[from] newpub_io_native::NativeError),
    #[error(transparent)]
    Pdf(#[from] newpub_io_pdf::PdfError),
    #[error("image: {0}")]
    Image(String),
    #[error("nothing to {0}")]
    Nothing(&'static str),
    #[error("{0}")]
    Other(String),
}

/// Result of running an action.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Outcome {
    /// Ids created, primary first.
    pub created: Vec<Id>,
}

pub struct Session {
    doc: Document,
    history: History<Document>,
    fonts: Arc<FontStore>,
    layout: Option<Arc<DocLayout>>,
    raster: Rasterizer,
    /// Snapshot taken at `begin_group`, plus nesting depth.
    group: Option<(Document, usize)>,
    /// Where typing coalesces: (story, next insertion index).
    typing: Option<(Id, usize)>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// Directory relative paths are resolved against.
    pub base_dir: PathBuf,
}

impl Session {
    pub fn new(fonts: Arc<FontStore>) -> Session {
        Session {
            doc: Document::default(),
            history: History::default(),
            fonts,
            layout: None,
            raster: Rasterizer::new(),
            group: None,
            typing: None,
            path: None,
            dirty: false,
            base_dir: PathBuf::from("."),
        }
    }

    /// A session that only uses bundled fonts (deterministic; used by journeys).
    pub fn bundled() -> Session {
        Session::new(Arc::new(FontStore::bundled()))
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn fonts(&self) -> &FontStore {
        &self.fonts
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { self.base_dir.join(p) }
    }

    /// Current layout (cached until the document changes).
    pub fn layout(&mut self) -> Arc<DocLayout> {
        if self.layout.is_none() {
            self.layout = Some(Arc::new(newpub_layout::layout_document(&self.doc, &self.fonts)));
        }
        self.layout.clone().expect("layout computed")
    }

    fn changed(&mut self) {
        self.layout = None;
        self.dirty = true;
    }

    /// Replaces the document wholesale as one undoable step.
    fn commit(&mut self, new: Document, coalesce: Option<String>) {
        let old = std::mem::replace(&mut self.doc, new);
        if self.group.is_none() {
            self.history.record(old, coalesce);
        }
        self.changed();
    }

    fn apply_cmd(&mut self, cmd: &Command) -> Result<Applied, EngineError> {
        let mut d = self.doc.clone();
        let a = d.apply(cmd)?;
        self.commit(d, None);
        Ok(a)
    }

    /// Runs one action.
    pub fn run(&mut self, action: &Action) -> Result<Outcome, EngineError> {
        if !matches!(action, Action::Session(SessionAction::TypeText { .. })) {
            self.typing = None;
            self.history.seal();
        }
        match action {
            Action::Doc(cmd) => Ok(Outcome { created: self.apply_cmd(cmd)?.created }),
            Action::Session(s) => self.run_session(s),
        }
    }

    fn run_session(&mut self, s: &SessionAction) -> Result<Outcome, EngineError> {
        use SessionAction::*;
        match s {
            NewDocument { width, height, margins, facing, pages, bleed } => {
                let setup = PageSetup {
                    width: *width,
                    height: *height,
                    margins: margins.unwrap_or(Insets::uniform(36.0)),
                    facing: *facing,
                    bleed: bleed.unwrap_or(Length(0.0)),
                };
                if width.0 <= 0.0 || height.0 <= 0.0 {
                    return Err(CoreError::Invalid("page size must be positive".into()).into());
                }
                self.doc = Document::new(setup, *pages);
                self.history = History::default();
                self.group = None;
                self.path = None;
                self.raster.clear_images();
                self.changed();
                self.dirty = false;
                Ok(Outcome { created: self.doc.pages.iter().map(|p| p.id).collect() })
            }
            Undo => {
                if self.group.is_some() {
                    return Err(EngineError::Other("cannot undo inside an undo group".into()));
                }
                let cur = self.doc.clone();
                let prev = self.history.undo(cur).ok_or(EngineError::Nothing("undo"))?;
                self.doc = prev;
                self.raster.clear_images();
                self.changed();
                Ok(Outcome::default())
            }
            Redo => {
                let cur = self.doc.clone();
                let next = self.history.redo(cur).ok_or(EngineError::Nothing("redo"))?;
                self.doc = next;
                self.raster.clear_images();
                self.changed();
                Ok(Outcome::default())
            }
            BeginGroup => {
                match &mut self.group {
                    Some((_, depth)) => *depth += 1,
                    None => self.group = Some((self.doc.clone(), 1)),
                }
                Ok(Outcome::default())
            }
            EndGroup => {
                let Some((snap, depth)) = self.group.take() else {
                    return Err(EngineError::Other("end_group without begin_group".into()));
                };
                if depth > 1 {
                    self.group = Some((snap, depth - 1));
                } else if snap != self.doc {
                    self.history.record(snap, None);
                }
                Ok(Outcome::default())
            }
            TypeText { target, at, text } => {
                let sid = self.doc.story_of(*target)?;
                let len = self.doc.story(sid)?.len();
                let at = at.unwrap_or(len);
                let key = match self.typing {
                    Some((s, next)) if s == sid && next == at && !text.contains(PARA_SEP) => {
                        Some(format!("type:{}", sid.0))
                    }
                    _ => None,
                };
                let mut d = self.doc.clone();
                d.apply(&Command::InsertText { target: sid, at: Some(at), text: text.clone(), attrs: None })?;
                let coalesce = key.or_else(|| Some(format!("type:{}", sid.0)));
                if self.typing.map(|(s, n)| s != sid || n != at).unwrap_or(true) {
                    self.history.seal();
                }
                self.commit(d, coalesce);
                self.typing = Some((sid, at + text.chars().count()));
                if text.contains(PARA_SEP) {
                    // A new paragraph ends the coalesced typing run (Publisher-like word/para granularity).
                    self.history.seal();
                }
                Ok(Outcome::default())
            }
            InsertPicture { path, page, x, y, width, height, into, link } => {
                self.insert_picture(path, *page, *x, *y, *width, *height, *into, *link)
            }
            ImportText { target, path, at } => {
                let p = self.resolve(path);
                let bytes = std::fs::read(&p)?;
                let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                self.apply_cmd(&Command::InsertText { target: *target, at: *at, text, attrs: None })?;
                Ok(Outcome::default())
            }
            Autoflow { frame } => self.autoflow(*frame),
            Save { path } => {
                let p = self.resolve(path);
                newpub_io_native::save(&self.doc, &p)?;
                self.path = Some(p);
                self.dirty = false;
                Ok(Outcome::default())
            }
            Open { path } => {
                let p = self.resolve(path);
                let d = newpub_io_native::open(&p)?;
                self.doc = d;
                self.history = History::default();
                self.group = None;
                self.path = Some(p);
                self.raster.clear_images();
                self.changed();
                self.dirty = false;
                Ok(Outcome::default())
            }
            ExportPdf { path, options } => {
                let bytes = self.pdf_bytes(options)?;
                std::fs::write(self.resolve(path), bytes)?;
                Ok(Outcome::default())
            }
            ExportPng { path, page, dpi } => {
                let png = self.page_png(*page, *dpi)?;
                std::fs::write(self.resolve(path), png)?;
                Ok(Outcome::default())
            }
        }
    }

    pub fn pdf_bytes(&mut self, options: &PdfOptions) -> Result<Vec<u8>, EngineError> {
        let layout = self.layout();
        Ok(newpub_io_pdf::export_pdf(&self.doc, &layout, &self.fonts, options)?)
    }

    /// Renders a page to a pixmap at `dpi`.
    pub fn render_page(&mut self, page: usize, dpi: f64) -> Result<newpub_render::Pixmap, EngineError> {
        if page >= self.doc.pages.len() {
            return Err(CoreError::NoSuchPage(page).into());
        }
        let layout = self.layout();
        let disp = newpub_render::page_display(&self.doc, &layout, page);
        newpub_render::render_page(&mut self.raster, &self.doc, &self.fonts, &disp, dpi)
            .ok_or_else(|| EngineError::Other("render failed".into()))
    }

    pub fn page_png(&mut self, page: usize, dpi: f64) -> Result<Vec<u8>, EngineError> {
        let pm = self.render_page(page, dpi)?;
        pm.encode_png().map_err(|e| EngineError::Image(e.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_picture(
        &mut self,
        path: &str,
        page: Option<usize>,
        x: Option<Length>,
        y: Option<Length>,
        width: Option<Length>,
        height: Option<Length>,
        into: Option<Id>,
        link: bool,
    ) -> Result<Outcome, EngineError> {
        let p = self.resolve(path);
        let bytes = std::fs::read(&p)?;
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| EngineError::Image(e.to_string()))?;
        let mime = match reader.format() {
            Some(image::ImageFormat::Png) => "image/png",
            Some(image::ImageFormat::Jpeg) => "image/jpeg",
            other => return Err(EngineError::Image(format!("unsupported picture format {other:?}"))),
        };
        let (pw, ph) = reader.into_dimensions().map_err(|e| EngineError::Image(e.to_string()))?;
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut d = self.doc.clone();
        let aid = d.add_asset(&name, mime, Arc::from(bytes), pw, ph);
        if link
            && let Some(a) = d.assets.get_mut(&aid) {
                a.link = Some(p.to_string_lossy().to_string());
            }
        if let Some(frame) = into {
            d.apply(&Command::SetImage {
                id: frame,
                patch: ImagePatch { asset: Some(aid), fit: Some(Fit::Fill), ..Default::default() },
            })?;
            self.commit(d, None);
            return Ok(Outcome { created: vec![frame, aid] });
        }
        let page = page.unwrap_or(0);
        if page >= d.pages.len() {
            return Err(CoreError::NoSuchPage(page).into());
        }
        let aspect = pw.max(1) as f64 / ph.max(1) as f64;
        let (w, h) = match (width, height) {
            (Some(w), Some(h)) => (w.0, h.0),
            (Some(w), None) => (w.0, w.0 / aspect),
            (None, Some(h)) => (h.0 * aspect, h.0),
            (None, None) => {
                let (mut w, mut h) = (pw as f64 * 0.75, ph as f64 * 0.75);
                let (t, b, l, r) = d.page_margins(page);
                let (aw, ah) = (d.setup.width.0 - l - r, d.setup.height.0 - t - b);
                let k = (aw / w).min(ah / h).min(1.0);
                w *= k;
                h *= k;
                (w, h)
            }
        };
        let (t, _, l, _) = d.page_margins(page);
        let rx = x.map(|v| v.0).unwrap_or(l + ((d.setup.width.0 - l - d.page_margins(page).3) - w) / 2.0);
        let ry = y.map(|v| v.0).unwrap_or(t);
        let a = d.apply(&Command::AddImage {
            page: Some(page),
            master: None,
            rect: Rect::new(rx, ry, w, h),
            asset: Some(aid),
        })?;
        self.commit(d, None);
        let mut created = a.created;
        created.push(aid);
        Ok(Outcome { created })
    }

    fn autoflow(&mut self, frame: Id) -> Result<Outcome, EngineError> {
        let sid = self.doc.story_of(frame)?;
        let template = self.doc.object(frame)?.clone();
        let ObjectKind::Text(tf) = &template.kind else { return Err(CoreError::NotText(frame).into()) };
        let tf = tf.clone();
        let mut d = self.doc.clone();
        let mut created = vec![];
        for _ in 0..2000 {
            let layout = newpub_layout::layout_document(&d, &self.fonts);
            if layout.stories.get(&sid).and_then(|s| s.overflow_at).is_none() {
                break;
            }
            let last = *d.story(sid)?.frames.last().ok_or(CoreError::NotText(frame))?;
            let last_page = d.page_of(last).unwrap_or(d.pages.len() - 1);
            let at = last_page + 1;
            d.apply(&Command::InsertPages { at: Some(at), count: 1, master: None })?;
            let r = d.apply(&Command::AddTextFrame {
                page: Some(at),
                master: None,
                rect: template.rect,
                columns: Some(tf.columns),
                gutter: Some(tf.gutter),
            })?;
            let nf = r.created[0];
            d.apply(&Command::SetTextFrame {
                id: nf,
                patch: TextFramePatch { insets: Some(tf.insets), valign: Some(tf.valign), ..Default::default() },
            })?;
            d.apply(&Command::LinkFrames { from: last, to: nf })?;
            created.push(d.pages[at].id);
            created.push(nf);
        }
        self.commit(d, None);
        Ok(Outcome { created })
    }

    /// Answers a query.
    pub fn query(&mut self, q: &Query) -> Result<Value, EngineError> {
        use Query::*;
        let to = |v: &dyn erased::Ser| v.to_json();
        Ok(match q {
            Document => serde_json::to_value(&self.doc).map_err(|e| EngineError::Other(e.to_string()))?,
            PageCount => json!(self.doc.pages.len()),
            Page { page } => {
                let p = self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?;
                to(p)
            }
            Object { id } => to(self.doc.object(*id)?),
            ObjectCount { page } => json!(self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.objects.len()),
            PageObjects { page } => to(&self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.objects),
            StoryText { target } => {
                let sid = self.doc.story_of(*target)?;
                json!(self.doc.story(sid)?.text)
            }
            StoryLength { target } => {
                let sid = self.doc.story_of(*target)?;
                json!(self.doc.story(sid)?.len())
            }
            StoryFrames { target } => {
                let sid = self.doc.story_of(*target)?;
                to(&self.doc.story(sid)?.frames)
            }
            FrameText { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                json!(l.frames.get(frame).map(|f| f.text()).unwrap_or_default())
            }
            FrameLines { frame } => {
                self.doc.object(*frame)?;
                let l = self.layout();
                let lines: Vec<Value> = l
                    .frames
                    .get(frame)
                    .map(|f| {
                        f.lines
                            .iter()
                            .map(|ln| {
                                let one = newpub_layout::FrameLayout { lines: vec![ln.clone()], ..Default::default() };
                                let first_x = ln
                                    .runs
                                    .iter()
                                    .flat_map(|r| r.glyphs.first())
                                    .map(|g| g.x)
                                    .fold(f64::INFINITY, f64::min);
                                // Ink excludes whitespace glyphs (e.g. the trailing space of a justified line).
                                let last_x = ln
                                    .runs
                                    .iter()
                                    .flat_map(|r| r.glyphs.iter().map(move |g| (r, g)))
                                    .filter(|(r, g)| {
                                        !r.text
                                            .get(g.text_range.clone())
                                            .map(|t| !t.is_empty() && t.trim().is_empty())
                                            .unwrap_or(false)
                                    })
                                    .map(|(_, g)| g.x + g.advance)
                                    .fold(f64::NEG_INFINITY, f64::max);
                                json!({
                                    "text": one.text(),
                                    "x": ln.x, "width": ln.width, "top": ln.top, "height": ln.height,
                                    "baseline": ln.baseline, "column": ln.column, "hyphenated": ln.hyphenated,
                                    "start": ln.char_range.start, "end": ln.char_range.end,
                                    "ink_left": if first_x.is_finite() { json!(first_x) } else { Value::Null },
                                    "ink_right": if last_x.is_finite() { json!(last_x) } else { Value::Null },
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Value::Array(lines)
            }
            Overflow { target } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                json!(l.stories.get(&sid).and_then(|s| s.overflow_at).is_some())
            }
            OverflowAt { target } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                json!(l.stories.get(&sid).and_then(|s| s.overflow_at))
            }
            CharPage { target, at } | CharFrame { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let l = self.layout();
                let frame = self.doc.story(sid)?.frames.iter().copied().find(|f| {
                    l.frames
                        .get(f)
                        .map(|fl| {
                            fl.lines.iter().any(|ln| {
                                ln.char_range.contains(at) || (ln.char_range.end == *at && ln.char_range.start < *at)
                            })
                        })
                        .unwrap_or(false)
                });
                match (q, frame) {
                    (CharFrame { .. }, Some(f)) => json!(f),
                    (CharPage { .. }, Some(f)) => json!(self.doc.page_of(f)),
                    _ => Value::Null,
                }
            }
            CharAttrs { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let st = self.doc.story(sid)?;
                let pi = st.para_index_at(*at);
                to(&self.doc.resolve_char(&st.paras[pi], &st.span_attrs_at(*at)))
            }
            ParaAttrs { target, at } => {
                let sid = self.doc.story_of(*target)?;
                let st = self.doc.story(sid)?;
                let pi = st.para_index_at(*at);
                let mut v = to(&self.doc.resolve_para(&st.paras[pi]));
                let style = st.paras[pi].style.and_then(|s| self.doc.styles.para.get(&s)).map(|s| s.name.clone());
                v["style"] = json!(style);
                v
            }
            ObjectPage { id } => {
                self.doc.object(*id)?;
                json!(self.doc.page_of(*id))
            }
            Styles => json!({
                "para": self.doc.styles.para.values().map(|s| s.name.clone()).collect::<Vec<_>>(),
                "chars": self.doc.styles.chars.values().map(|s| s.name.clone()).collect::<Vec<_>>(),
            }),
            Masters => Value::Array(
                self.doc.masters.iter().map(|m| json!({"id": m.id, "name": m.name, "objects": m.objects})).collect(),
            ),
            History => json!({"undo": self.history.undo_len(), "redo": self.history.redo_len()}),
            TextBounds { frame } => {
                let o = self.doc.object(*frame)?.clone();
                let l = self.layout();
                let Some(fl) = l.frames.get(frame) else { return Ok(Value::Null) };
                let mut b: Option<Rect> = None;
                for ln in &fl.lines {
                    for r in &ln.runs {
                        for g in &r.glyphs {
                            let gr = Rect::new(o.rect.x + g.x, o.rect.y + ln.top, g.advance.max(0.0), ln.height);
                            b = Some(b.map_or(gr, |x| x.union(&gr)));
                        }
                    }
                }
                b.map(|r| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h})).unwrap_or(Value::Null)
            }
        })
    }
}

mod erased {
    pub trait Ser {
        fn to_json(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
        }
    }
}
