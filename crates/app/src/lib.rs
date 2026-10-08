//! newpub-app: the egui desktop application. A thin view over [`Session`]: every gesture
//! becomes an [`Action`], every displayed fact comes from the session.

mod dup;
mod picker;
mod print;
mod recent;

use egui::{Color32, Pos2, Rect as ERect, Sense, Stroke, TextureHandle, Vec2};
use newpub_engine::core::{
    self as core, Align, CharAttrs, Command, Id, Length, ObjectKind, ObjectPatch, ParaAttrs, Rect, ShapeKind,
    TextFramePatch, ZOp,
};
use newpub_engine::{Action, PdfOptions, Session, SessionAction};
use std::time::{Duration, Instant};

/// Zoom stops for Cmd+= / Cmd+- (25 ... 400 %).
const ZOOM_STOPS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
/// Edits of the same kind within this window are one undo step.
const COALESCE: Duration = Duration::from_millis(600);
/// Half-size of a selection handle's hit area, in screen points.
const HANDLE_HIT: f32 = 8.0;

/// Drawing tools on the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    TextBox,
    Rectangle,
    Ellipse,
    Line,
}

impl Tool {
    pub const ALL: [(Tool, &'static str); 5] = [
        (Tool::Select, "Select"),
        (Tool::TextBox, "Text Box"),
        (Tool::Rectangle, "Rectangle"),
        (Tool::Ellipse, "Ellipse"),
        (Tool::Line, "Line"),
    ];
}

/// In-app dialogs (egui windows, so they are reachable through AccessKit and in UI journeys).
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    None,
    Picker(picker::PickerState),
    Print(print::PrintState),
    ExportPdf { path: String, crop_marks: bool, booklet: bool },
    Save { path: String },
    Open { path: String },
    InsertPicture { path: String },
}

pub const PRESETS: [(&str, &str, &str); 4] =
    [("Letter", "8.5in", "11in"), ("A4", "210mm", "297mm"), ("A5", "148mm", "210mm"), ("Custom", "", "")];

pub struct NewpubApp {
    pub session: Session,
    pub page: usize,
    pub tool: Tool,
    pub selection: Vec<Id>,
    /// Zoom: screen points per document point.
    pub zoom: f32,
    pub dialog: Dialog,
    pub status: String,
    texture: Option<(TextureHandle, u64, usize, u32)>,
    /// Canvas page origin on screen (top-left of the page) from the last frame.
    page_origin: Pos2,
    drag_start: Option<Pos2>,
    drag_now: Option<Pos2>,
    /// Selected object being moved: total offset in document points.
    moving: Option<Vec2>,
    /// The zoom was set by "fit" and has not been changed since.
    pub fitted: bool,
    /// Size of the canvas viewport from the last frame (for "fit").
    canvas_size: Vec2,
    /// Corner handle being dragged: object, corner index (0 TL, 1 TR, 2 BL, 3 BR), original rect.
    resizing: Option<(Id, usize, Rect)>,
    /// The last coalescable edit: key, session revision right after it, time.
    run: Option<(String, u64, Instant)>,
    /// Accumulated nudge of the current run, in points.
    run_delta: (f64, f64),
    fields: Fields,
    recent: recent::Recent,
    /// Show the template picker before the first frame (UI-08; the desktop binary sets it).
    pub startup_picker: bool,
    /// When set, print jobs are written here as `job-<n>.pdf` instead of going to the OS print
    /// system (PR-07; journeys use it, since CI has no printer).
    pub print_spool: Option<std::path::PathBuf>,
    print_jobs: usize,
    last_print_job: Option<serde_json::Value>,
}

/// Text buffers of the object and format panels.
#[derive(Default)]
struct Fields {
    owner: Option<Id>,
    size: String,
    x: String,
    y: String,
    w: String,
    h: String,
    columns: String,
}

fn parse_len(s: &str) -> Option<Length> {
    core::units::parse_length(s).filter(|v| *v > 0.0).map(Length)
}

impl NewpubApp {
    pub fn new(session: Session) -> NewpubApp {
        NewpubApp {
            session,
            page: 0,
            tool: Tool::Select,
            selection: vec![],
            zoom: 0.9,
            dialog: Dialog::None,
            status: String::new(),
            texture: None,
            page_origin: Pos2::ZERO,
            drag_start: None,
            drag_now: None,
            moving: None,
            fitted: false,
            canvas_size: Vec2::ZERO,
            resizing: None,
            run: None,
            run_delta: (0.0, 0.0),
            fields: Fields::default(),
            recent: recent::Recent::default(),
            startup_picker: false,
            print_spool: None,
            print_jobs: 0,
            last_print_job: None,
        }
    }

    /// Keeps the recent-files list in the OS config dir (the desktop binary does this; journeys stay in memory).
    pub fn with_persistent_recent(mut self) -> NewpubApp {
        self.recent = recent::Recent::persistent();
        self
    }

    /// Recent files, newest first.
    pub fn recent_files(&self) -> &[std::path::PathBuf] {
        self.recent.files()
    }

    fn remember_recent(&mut self) {
        if let Some(p) = self.session.path.clone() {
            self.recent.add(&p);
        }
    }

    /// Prepares an edit that replaces the previous one of the same `key` when it was made within 600 ms and
    /// nothing else changed the document since: undoes that edit. Returns whether it continues a run.
    fn begin_run(&mut self, key: &str) -> bool {
        let continues = matches!(&self.run, Some((k, rev, at))
            if k == key && *rev == self.session.revision() && at.elapsed() < COALESCE);
        if continues && self.session.can_undo() {
            self.act(SessionAction::Undo);
            true
        } else {
            self.run_delta = (0.0, 0.0);
            false
        }
    }

    fn end_run(&mut self, key: String, ok: bool) {
        self.run = ok.then(|| (key, self.session.revision(), Instant::now()));
    }

    /// Runs `cmd` as part of the coalescing run `key`.
    fn act_run(&mut self, key: String, cmd: Command) -> Option<newpub_engine::Outcome> {
        self.begin_run(&key);
        let out = self.act(cmd);
        self.end_run(key, out.is_some());
        out
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        let dir = ((dx > 0.0) as i8 - (dx < 0.0) as i8, (dy > 0.0) as i8 - (dy < 0.0) as i8);
        let key = format!("nudge:{ids:?}:{dir:?}");
        let continued = self.begin_run(&key);
        if !continued {
            self.run_delta = (0.0, 0.0);
        }
        let total = (self.run_delta.0 + dx, self.run_delta.1 + dy);
        let out = self.act(Command::MoveObjects { ids, dx: Length(total.0), dy: Length(total.1) });
        if out.is_some() {
            self.run_delta = total;
        }
        self.end_run(key, out.is_some());
    }

    fn zoom_step(&mut self, dir: i32) {
        let z = self.zoom;
        let next = if dir > 0 {
            ZOOM_STOPS.iter().copied().find(|s| *s > z + 1e-4).unwrap_or(ZOOM_STOPS[ZOOM_STOPS.len() - 1])
        } else {
            ZOOM_STOPS.iter().copied().rev().find(|s| *s < z - 1e-4).unwrap_or(ZOOM_STOPS[0])
        };
        self.zoom = next;
        self.fitted = false;
    }

    /// Zooms so the whole page fits the canvas.
    pub fn fit_page(&mut self) {
        let doc = self.session.doc();
        let (w, h) = (doc.setup.width.0 as f32, doc.setup.height.0 as f32);
        let avail = if self.canvas_size.x > 80.0 && self.canvas_size.y > 80.0 {
            self.canvas_size - Vec2::splat(40.0)
        } else {
            Vec2::new(800.0, 700.0)
        };
        if w > 0.0 && h > 0.0 {
            self.zoom = (avail.x / w).min(avail.y / h).clamp(0.1, 8.0);
        }
        self.fitted = true;
    }

    /// Runs an action and reports errors in the status bar.
    pub fn act(&mut self, a: impl Into<Action>) -> Option<newpub_engine::Outcome> {
        match self.session.run(&a.into()) {
            Ok(o) => {
                self.page = self.page.min(self.session.doc().pages.len().saturating_sub(1));
                self.selection.retain(|id| self.session.doc().objects.contains_key(id));
                Some(o)
            }
            Err(e) => {
                self.status = format!("Error: {e}");
                None
            }
        }
    }

    /// View state for UI journeys (`{q: view}`): zoom, whether the view is fitted, whether the zoom
    /// is one of the preset stops, selection, current page, tool.
    pub fn view_state(&self) -> serde_json::Value {
        serde_json::json!({
            "zoom": self.zoom,
            "fit": self.fitted,
            "zoom_stop": ZOOM_STOPS.iter().any(|z| (z - self.zoom).abs() < 1e-4),
            "selection": self.selection,
            "page": self.page,
            "tool": format!("{:?}", self.tool),
            "print_jobs": self.print_jobs,
            "last_print_job": self.last_print_job,
        })
    }

    /// Screen position of a document point on the current page (used by UI journeys).
    pub fn page_to_screen(&self, x: f64, y: f64) -> Pos2 {
        self.page_origin + Vec2::new(x as f32 * self.zoom, y as f32 * self.zoom)
    }

    fn screen_to_page(&self, p: Pos2) -> (f64, f64) {
        let d = (p - self.page_origin) / self.zoom;
        (d.x as f64, d.y as f64)
    }

    fn selected_text_frame(&self) -> Option<Id> {
        let d = self.session.doc();
        self.selection
            .iter()
            .copied()
            .find(|id| matches!(d.objects.get(id).map(|o| &o.kind), Some(ObjectKind::Text(_))))
    }

    /// Draws the whole UI into the root `ui`. Called by eframe and by the UI-journey harness.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if std::mem::take(&mut self.startup_picker) {
            self.open_picker();
        }
        self.shortcuts(&ctx);
        egui::Panel::top("ribbon").show(ui, |ui| self.ribbon(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("pages").resizable(false).default_size(90.0).show(ui, |ui| self.page_navigator(ui));
        egui::Panel::right("format").resizable(false).default_size(190.0).show(ui, |ui| self.format_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
    }

    fn open_picker(&mut self) {
        self.dialog = Dialog::Picker(picker::PickerState::new(self));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let typing_in_widget = ctx.egui_wants_keyboard_input();
        let events = ctx.input(|i| i.events.clone());
        let dialog_open = self.dialog != Dialog::None;
        for e in &events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
            let (key, m) = (*key, *modifiers);
            if typing_in_widget {
                continue;
            }
            if m.command {
                match key {
                    Key::Z if !m.shift => {
                        self.act(SessionAction::Undo);
                    }
                    Key::Y | Key::Z => {
                        self.act(SessionAction::Redo);
                    }
                    Key::P => self.dialog = Dialog::Print(print::PrintState::new()),
                    Key::E => {
                        self.dialog =
                            Dialog::ExportPdf { path: "publication.pdf".into(), crop_marks: false, booklet: false }
                    }
                    _ if dialog_open => {}
                    Key::D => self.duplicate_selection(),
                    Key::CloseBracket => self.reorder(ZOp::Front),
                    Key::OpenBracket => self.reorder(ZOp::Back),
                    Key::Equals | Key::Plus => self.zoom_step(1),
                    Key::Minus => self.zoom_step(-1),
                    Key::Num0 => self.fit_page(),
                    _ => {}
                }
                continue;
            }
            if dialog_open {
                continue;
            }
            let step = if m.shift { 10.0 } else { 1.0 };
            match key {
                Key::ArrowLeft => self.nudge(-step, 0.0),
                Key::ArrowRight => self.nudge(step, 0.0),
                Key::ArrowUp => self.nudge(0.0, -step),
                Key::ArrowDown => self.nudge(0.0, step),
                Key::Escape => self.selection.clear(),
                Key::Tab => self.cycle_frame(!m.shift),
                Key::Delete => {
                    let ids = std::mem::take(&mut self.selection);
                    if !ids.is_empty() {
                        self.act(Command::DeleteObjects { ids });
                    }
                }
                Key::Backspace => self.backspace(),
                _ => {}
            }
        }
        // Typing into the selected text frame.
        if typing_in_widget || dialog_open {
            return;
        }
        let Some(frame) = self.selected_text_frame() else { return };
        for e in events {
            match e {
                egui::Event::Text(t) => {
                    self.act(SessionAction::TypeText { target: frame, at: None, text: t });
                }
                egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                    self.act(SessionAction::TypeText { target: frame, at: None, text: "\n".into() });
                }
                _ => {}
            }
        }
    }

    /// Backspace: removes the last character of the selected text frame's story, or deletes the selection
    /// when it is not a text frame (or the frame is empty).
    fn backspace(&mut self) {
        if let Some(frame) = self.selected_text_frame() {
            let len = self
                .session
                .doc()
                .story_of(frame)
                .ok()
                .and_then(|s| self.session.doc().story(s).ok())
                .map(|s| s.len())
                .unwrap_or(0);
            if len > 0 {
                self.act(Command::DeleteText { target: frame, start: len - 1, end: len });
                return;
            }
        }
        let ids = std::mem::take(&mut self.selection);
        if !ids.is_empty() {
            self.act(Command::DeleteObjects { ids });
        }
    }

    fn reorder(&mut self, op: ZOp) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        self.act(SessionAction::BeginGroup);
        for id in ids {
            self.act(Command::SetZ { id, op });
        }
        self.act(SessionAction::EndGroup);
    }

    /// Tab / Shift+Tab: selects the next / previous frame of the selected text frame's story.
    fn cycle_frame(&mut self, forward: bool) {
        let Some(frame) = self.selected_text_frame() else { return };
        let doc = self.session.doc();
        let Some(frames) = doc.story_of(frame).ok().and_then(|s| doc.story(s).ok()).map(|s| s.frames.clone()) else {
            return;
        };
        let Some(i) = frames.iter().position(|f| *f == frame) else { return };
        let n = frames.len();
        let next = frames[if forward { (i + 1) % n } else { (i + n - 1) % n }];
        if let Some(p) = doc.page_of(next) {
            self.page = p;
        }
        self.selection = vec![next];
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                self.open_picker();
            }
            if ui.button("Open").clicked() {
                self.dialog = Dialog::Open { path: String::new() };
            }
            if ui.button("Save").clicked() {
                let path = self
                    .session
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "publication.npub".into());
                self.dialog = Dialog::Save { path };
            }
            if ui.button("Export PDF").clicked() {
                self.dialog = Dialog::ExportPdf { path: "publication.pdf".into(), crop_marks: false, booklet: false };
            }
            if ui.button("Print").clicked() {
                self.dialog = Dialog::Print(print::PrintState::new());
            }
            ui.separator();
            if ui.add_enabled(self.session.can_undo(), egui::Button::new("Undo")).clicked() {
                self.act(SessionAction::Undo);
            }
            if ui.add_enabled(self.session.can_redo(), egui::Button::new("Redo")).clicked() {
                self.act(SessionAction::Redo);
            }
            ui.separator();
            for (tool, label) in Tool::ALL {
                if ui.selectable_label(self.tool == tool, label).clicked() {
                    self.tool = tool;
                }
            }
            if ui.button("Picture").clicked() {
                self.dialog = Dialog::InsertPicture { path: String::new() };
            }
            ui.separator();
            if ui.button("Add Page").clicked() {
                let at = self.page + 1;
                if self.act(Command::InsertPages { at: Some(at), count: 1, master: None }).is_some() {
                    self.page = at;
                }
            }
            if ui.button("Delete Page").clicked() {
                let p = self.page;
                self.act(Command::DeletePage { page: p });
            }
            if !self.selection.is_empty() && ui.button("Delete Object").clicked() {
                let ids = std::mem::take(&mut self.selection);
                self.act(Command::DeleteObjects { ids });
            }
            ui.separator();
            if ui.button("Zoom In").clicked() {
                self.zoom = (self.zoom * 1.25).min(8.0);
                self.fitted = false;
            }
            if ui.button("Zoom Out").clicked() {
                self.zoom = (self.zoom / 1.25).max(0.1);
                self.fitted = false;
            }
        });
    }

    fn page_navigator(&mut self, ui: &mut egui::Ui) {
        ui.heading("Pages");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for i in 0..self.session.doc().pages.len() {
                if ui.selectable_label(self.page == i, format!("Page {}", i + 1)).clicked() {
                    self.page = i;
                    self.selection.clear();
                }
            }
        });
    }

    fn format_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Format");
        let first = self.selection.first().copied();
        if self.fields.owner != first {
            self.fields = Fields { owner: first, ..Default::default() };
        }
        let Some(frame) = self.selected_text_frame() else {
            if first.is_none() {
                ui.label("Select a text box to format text.");
            }
            self.object_panel(ui);
            return;
        };
        let doc = self.session.doc();
        let Ok(sid) = doc.story_of(frame) else { return };
        let Ok(story) = doc.story(sid) else { return };
        let rc = doc.resolve_char(&story.paras[0], &story.span_attrs_at(0));
        let mut font = rc.font.clone();
        let families = self.session.fonts().families();
        let combo = egui::ComboBox::from_id_salt("font").selected_text(&font).show_ui(ui, |ui| {
            for f in &families {
                ui.selectable_value(&mut font, f.clone(), f);
            }
        });
        combo.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Font"));
        let mut patch = CharAttrs::default();
        if font != rc.font {
            patch.font = Some(font);
        }
        ui.horizontal(|ui| {
            if ui.selectable_label(rc.bold, "Bold").clicked() {
                patch.bold = Some(!rc.bold);
            }
            if ui.selectable_label(rc.italic, "Italic").clicked() {
                patch.italic = Some(!rc.italic);
            }
            if ui.selectable_label(rc.underline, "Underline").clicked() {
                patch.underline = Some(!rc.underline);
            }
        });
        if !patch.is_empty() {
            self.act(Command::FormatChars { target: frame, start: None, end: None, attrs: patch });
        }
        // Font size: typing applies.
        let mut size_text = std::mem::take(&mut self.fields.size);
        let (changed, focused) = labeled_field_state(ui, "Font size", &mut size_text);
        if changed {
            if let Some(v) = core::units::parse_length(&size_text).filter(|v| (1.0..=999.0).contains(v))
                && (v - rc.size).abs() > 1e-9
            {
                let attrs = CharAttrs { size: Some(Length(v)), ..Default::default() };
                self.act_run(
                    format!("size:{frame}"),
                    Command::FormatChars { target: frame, start: None, end: None, attrs },
                );
            }
        } else if !focused {
            size_text = fmt_num(rc.size);
        }
        self.fields.size = size_text;
        ui.label("Alignment");
        let rp = self
            .session
            .doc()
            .resolve_para(&self.session.doc().story(sid).map(|s| s.paras[0].clone()).unwrap_or_default());
        ui.horizontal_wrapped(|ui| {
            for (a, label) in [
                (Align::Left, "Align Left"),
                (Align::Center, "Center"),
                (Align::Right, "Align Right"),
                (Align::Justify, "Justify"),
            ] {
                if ui.selectable_label(rp.align == a, label).clicked() {
                    self.act(Command::FormatParas {
                        target: frame,
                        start: None,
                        end: None,
                        attrs: ParaAttrs { align: Some(a), ..Default::default() },
                    });
                }
            }
        });
        ui.separator();
        self.object_panel(ui);
        let overflow = self
            .session
            .query(&newpub_engine::Query::Overflow { target: frame })
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if overflow {
            ui.colored_label(Color32::from_rgb(200, 60, 40), "Text overflow");
        }
    }

    /// Position, size and (for text frames) columns of the first selected object.
    fn object_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selection.first().copied() else { return };
        let Some(obj) = self.session.doc().objects.get(&id) else { return };
        let (rect, columns) = (obj.rect, if let ObjectKind::Text(t) = &obj.kind { Some(t.columns) } else { None });
        ui.label(format!("Object {id}"));
        let mut new_rect = rect;
        let mut edited = false;
        let model = [rect.x, rect.y, rect.w, rect.h];
        let mut bufs = [
            std::mem::take(&mut self.fields.x),
            std::mem::take(&mut self.fields.y),
            std::mem::take(&mut self.fields.w),
            std::mem::take(&mut self.fields.h),
        ];
        for (i, label) in ["X position", "Y position", "Width", "Height"].into_iter().enumerate() {
            let (changed, focused) = labeled_field_state(ui, label, &mut bufs[i]);
            if changed {
                // Any unit is accepted; partial input such as "2i" is ignored until it parses.
                if let Some(v) =
                    core::units::parse_length(&bufs[i]).filter(|v| *v >= if i < 2 { f64::MIN } else { 0.1 })
                {
                    match i {
                        0 => new_rect.x = v,
                        1 => new_rect.y = v,
                        2 => new_rect.w = v,
                        _ => new_rect.h = v,
                    }
                    edited = true;
                }
            } else if !focused {
                bufs[i] = fmt_len(model[i]);
            }
        }
        let [x, y, w, h] = bufs;
        (self.fields.x, self.fields.y, self.fields.w, self.fields.h) = (x, y, w, h);
        if edited && new_rect != rect {
            let patch = ObjectPatch { rect: Some(new_rect), ..Default::default() };
            self.act_run(format!("geometry:{id}"), Command::SetObject { id, patch });
        }
        if let Some(cols) = columns {
            let mut text = std::mem::take(&mut self.fields.columns);
            let (changed, focused) = labeled_field_state(ui, "Columns", &mut text);
            if changed {
                if let Some(n) = text.trim().parse::<u32>().ok().filter(|n| (1..=20).contains(n))
                    && n != cols
                {
                    let patch = TextFramePatch { columns: Some(n), ..Default::default() };
                    self.act_run(format!("columns:{id}"), Command::SetTextFrame { id, patch });
                }
            } else if !focused {
                text = cols.to_string();
            }
            self.fields.columns = text;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Page {} of {}", self.page + 1, self.session.doc().pages.len()));
            ui.separator();
            ui.label(format!("Zoom {:.0}%", self.zoom * 100.0));
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }

    fn page_texture(&mut self, ctx: &egui::Context, ppp: f32) -> Option<TextureHandle> {
        let rev = self.session.revision();
        let scale_key = (self.zoom * ppp * 100.0) as u32;
        if let Some((t, r, p, s)) = &self.texture
            && *r == rev
            && *p == self.page
            && *s == scale_key
        {
            return Some(t.clone());
        }
        let dpi = 72.0 * (self.zoom * ppp) as f64;
        let pm = self.session.render_page(self.page, dpi).ok()?;
        let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
        let tex = ctx.load_texture("page", img, egui::TextureOptions::LINEAR);
        self.texture = Some((tex.clone(), rev, self.page, scale_key));
        Some(tex)
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.session.doc().setup.width.0 as f32, self.session.doc().setup.height.0 as f32);
        let ppp = ui.ctx().pixels_per_point();
        let tex = self.page_texture(ui.ctx(), ppp);
        self.canvas_size = ui.available_size();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let size = Vec2::new(w * self.zoom + 40.0, h * self.zoom + 40.0);
            let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
            let resp = resp.on_hover_text("Page canvas");
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Page canvas"));
            self.page_origin = resp.rect.min + Vec2::splat(20.0);
            let page_rect = ERect::from_min_size(self.page_origin, Vec2::new(w * self.zoom, h * self.zoom));
            painter.rect_filled(page_rect.translate(Vec2::splat(3.0)), 0.0, Color32::from_black_alpha(60));
            if let Some(t) = &tex {
                painter.image(t.id(), page_rect, ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            } else {
                painter.rect_filled(page_rect, 0.0, Color32::WHITE);
            }
            self.draw_guides(&painter);
            self.draw_selection(&painter);
            self.handle_canvas_input(&resp);
            if let (Some(a), Some(b)) = (self.drag_start, self.drag_now)
                && self.tool != Tool::Select
            {
                painter.rect_stroke(
                    ERect::from_two_pos(a, b),
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(40, 120, 220)),
                    egui::StrokeKind::Middle,
                );
            }
        });
    }

    fn draw_guides(&self, painter: &egui::Painter) {
        let doc = self.session.doc();
        let (t, b, l, r) = doc.page_margins(self.page);
        let (w, h) = (doc.setup.width.0, doc.setup.height.0);
        let a = self.page_to_screen(l, t);
        let z = self.page_to_screen(w - r, h - b);
        painter.rect_stroke(
            ERect::from_min_max(a, z),
            0.0,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(90, 140, 220, 120)),
            egui::StrokeKind::Middle,
        );
        // Text frame outlines.
        if let Some(page) = doc.pages.get(self.page) {
            for id in &page.objects {
                if let Some(o) = doc.objects.get(id)
                    && matches!(o.kind, ObjectKind::Text(_))
                {
                    let rr = ERect::from_min_max(
                        self.page_to_screen(o.rect.x, o.rect.y),
                        self.page_to_screen(o.rect.right(), o.rect.bottom()),
                    );
                    painter.rect_stroke(rr, 0.0, Stroke::new(0.5, Color32::from_gray(170)), egui::StrokeKind::Middle);
                }
            }
        }
    }

    fn draw_selection(&self, painter: &egui::Painter) {
        let doc = self.session.doc();
        let off = self.moving.unwrap_or(Vec2::ZERO);
        let preview = self.resize_preview();
        for id in &self.selection {
            if let Some(o) = doc.objects.get(id) {
                let r = match preview {
                    Some((rid, r)) if rid == *id => r,
                    _ => o.rect,
                };
                let rr = ERect::from_min_max(self.page_to_screen(r.x, r.y), self.page_to_screen(r.right(), r.bottom()))
                    .translate(off * self.zoom);
                painter.rect_stroke(
                    rr,
                    0.0,
                    Stroke::new(1.5, Color32::from_rgb(40, 120, 220)),
                    egui::StrokeKind::Outside,
                );
                for c in [rr.left_top(), rr.right_top(), rr.left_bottom(), rr.right_bottom()] {
                    painter.rect_filled(ERect::from_center_size(c, Vec2::splat(6.0)), 0.0, Color32::WHITE);
                    painter.rect_stroke(
                        ERect::from_center_size(c, Vec2::splat(6.0)),
                        0.0,
                        Stroke::new(1.0, Color32::from_rgb(40, 120, 220)),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
    }

    /// Topmost object on the current page under a document point.
    fn hit(&self, x: f64, y: f64) -> Option<Id> {
        let doc = self.session.doc();
        let page = doc.pages.get(self.page)?;
        page.objects
            .iter()
            .rev()
            .copied()
            .find(|id| doc.objects.get(id).map(|o| o.rect.contains(x, y)).unwrap_or(false))
    }

    /// Corner handle (0 TL, 1 TR, 2 BL, 3 BR) of a selected object under a screen position.
    fn handle_at(&self, p: Pos2) -> Option<(Id, usize, Rect)> {
        let doc = self.session.doc();
        for id in &self.selection {
            let Some(o) = doc.objects.get(id) else { continue };
            if o.locked {
                continue;
            }
            let r = o.rect;
            let corners = [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())];
            for (i, (cx, cy)) in corners.into_iter().enumerate() {
                let c = self.page_to_screen(cx, cy);
                if (c.x - p.x).abs() <= HANDLE_HIT && (c.y - p.y).abs() <= HANDLE_HIT {
                    return Some((*id, i, r));
                }
            }
        }
        None
    }

    /// The rect of the object being resized, for the current pointer position.
    fn resize_preview(&self) -> Option<(Id, Rect)> {
        let (id, corner, orig) = self.resizing?;
        let (a, b) = (self.drag_start?, self.drag_now?);
        let d = (b - a) / self.zoom;
        let (d, orig_corner, opposite) = match corner {
            0 => (d, (orig.x, orig.y), (orig.right(), orig.bottom())),
            1 => (d, (orig.right(), orig.y), (orig.x, orig.bottom())),
            2 => (d, (orig.x, orig.bottom()), (orig.right(), orig.y)),
            _ => (d, (orig.right(), orig.bottom()), (orig.x, orig.y)),
        };
        let moved = (orig_corner.0 + d.x as f64, orig_corner.1 + d.y as f64);
        let (x0, x1) = (moved.0.min(opposite.0), moved.0.max(opposite.0));
        let (y0, y1) = (moved.1.min(opposite.1), moved.1.max(opposite.1));
        Some((id, Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))))
    }

    fn handle_canvas_input(&mut self, resp: &egui::Response) {
        if resp.drag_started() {
            // egui reports a drag once the pointer has moved; the gesture began at the press origin.
            self.drag_start = resp.ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            if self.tool == Tool::Select
                && let Some(p) = self.drag_start
            {
                if let Some(h) = self.handle_at(p) {
                    self.resizing = Some(h);
                } else {
                    let (x, y) = self.screen_to_page(p);
                    match self.hit(x, y) {
                        Some(id) => {
                            if !self.selection.contains(&id) {
                                self.selection = vec![id];
                            }
                            self.moving = Some(Vec2::ZERO);
                        }
                        None => self.selection.clear(),
                    }
                }
            }
        }
        if resp.dragged() {
            self.drag_now = resp.interact_pointer_pos();
            if let (Some(m), Some(a), Some(b)) = (self.moving.as_mut(), self.drag_start, self.drag_now) {
                *m = (b - a) / self.zoom;
            }
        }
        if resp.drag_stopped() {
            let (a, b) = (self.drag_start, self.drag_now.or(resp.interact_pointer_pos()));
            self.drag_now = b;
            if let Some((id, r)) = self.resize_preview() {
                self.resizing = None;
                if let Some(o) = self.session.doc().objects.get(&id)
                    && o.rect != r
                {
                    self.act(Command::SetObject { id, patch: ObjectPatch { rect: Some(r), ..Default::default() } });
                }
            } else if let Some(m) = self.moving.take() {
                if m.length() > 0.5 && !self.selection.is_empty() {
                    let ids = self.selection.clone();
                    self.act(Command::MoveObjects { ids, dx: Length(m.x as f64), dy: Length(m.y as f64) });
                }
            } else if let (Some(a), Some(b)) = (a, b) {
                self.create_from_drag(a, b);
            }
            self.resizing = None;
            self.drag_start = None;
            self.drag_now = None;
        }
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let (x, y) = self.screen_to_page(p);
            if self.tool == Tool::Select {
                self.selection = self.hit(x, y).into_iter().collect();
            } else {
                // A click without dragging creates a default-sized object.
                let b = p + Vec2::new(144.0, 72.0) * self.zoom;
                self.create_from_drag(p, b);
            }
        }
    }

    fn create_from_drag(&mut self, a: Pos2, b: Pos2) {
        let (x0, y0) = self.screen_to_page(a);
        let (x1, y1) = self.screen_to_page(b);
        let rect = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs().max(1.0), (y1 - y0).abs().max(1.0));
        let page = Some(self.page);
        let out = match self.tool {
            Tool::Select => None,
            Tool::TextBox => self.act(Command::AddTextFrame { page, master: None, rect, columns: None, gutter: None }),
            Tool::Rectangle | Tool::Ellipse => {
                let kind = if self.tool == Tool::Rectangle { ShapeKind::Rect } else { ShapeKind::Ellipse };
                let stroke = Some(core::Stroke {
                    color: core::Color::BLACK,
                    width: Length(1.0),
                    dash: core::Dash::Solid,
                    cap: core::LineCap::Butt,
                    join: core::LineJoin::Miter,
                });
                self.act(Command::AddShape {
                    page,
                    master: None,
                    rect,
                    kind,
                    fill: Some(core::Color::rgb(255, 255, 255)),
                    stroke,
                })
            }
            Tool::Line => {
                let rect = Rect::new(x0, y0, x1 - x0, y1 - y0);
                let stroke = Some(core::Stroke {
                    color: core::Color::BLACK,
                    width: Length(1.0),
                    dash: core::Dash::Solid,
                    cap: core::LineCap::Butt,
                    join: core::LineJoin::Miter,
                });
                let (rect, fh, fv) = normalize_line(rect);
                let o =
                    self.act(Command::AddShape { page, master: None, rect, kind: ShapeKind::Line, fill: None, stroke });
                if let Some(id) = o.as_ref().and_then(|o| o.created.first().copied())
                    && (fh || fv)
                {
                    self.act(Command::SetObject {
                        id,
                        patch: core::ObjectPatch { flip_h: Some(fh), flip_v: Some(fv), ..Default::default() },
                    });
                }
                o
            }
        };
        if let Some(id) = out.and_then(|o| o.created.first().copied()) {
            self.selection = vec![id];
            self.tool = Tool::Select;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut dialog = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        match &mut dialog {
            Dialog::None => {}
            Dialog::Picker(st) => close = picker::show(self, ctx, st),
            Dialog::Print(st) => close = print::show(self, ctx, st),
            Dialog::ExportPdf { path, crop_marks, booklet } => {
                egui::Window::new("Export PDF").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "PDF file", path);
                    ui.checkbox(crop_marks, "Crop marks");
                    ui.checkbox(booklet, "Booklet");
                    ui.horizontal(|ui| {
                        if ui.button("Export").clicked() {
                            let options = PdfOptions {
                                crop_marks: *crop_marks,
                                bleed: *crop_marks,
                                imposition: if *booklet {
                                    newpub_engine::Imposition::Booklet
                                } else {
                                    newpub_engine::Imposition::None
                                },
                                pages: None,
                                standard: None,
                            };
                            if self.act(SessionAction::ExportPdf { path: path.clone(), options }).is_some() {
                                self.status = format!("Exported {path}");
                                close = true;
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Save { path } => {
                egui::Window::new("Save Publication").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "File name", path);
                    ui.horizontal(|ui| {
                        if ui.button("Save File").clicked()
                            && self.act(SessionAction::Save { path: path.clone() }).is_some()
                        {
                            self.remember_recent();
                            self.status = format!("Saved {path}");
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Open { path } => {
                egui::Window::new("Open Publication").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "File name", path);
                    let recent: Vec<_> = self.recent.files().to_vec();
                    if !recent.is_empty() {
                        ui.label("Recent files");
                    }
                    for file in recent {
                        let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        if ui.button(format!("Recent: {name}")).clicked()
                            && self.act(SessionAction::Open { path: file.to_string_lossy().to_string() }).is_some()
                        {
                            self.remember_recent();
                            self.page = 0;
                            self.selection.clear();
                            close = true;
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Open File").clicked()
                            && self.act(SessionAction::Open { path: path.clone() }).is_some()
                        {
                            self.remember_recent();
                            self.page = 0;
                            self.selection.clear();
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::InsertPicture { path } => {
                egui::Window::new("Insert Picture").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "Picture file", path);
                    ui.horizontal(|ui| {
                        if ui.button("Insert").clicked() {
                            let a = SessionAction::InsertPicture {
                                path: path.clone(),
                                page: Some(self.page),
                                x: None,
                                y: None,
                                width: None,
                                height: None,
                                into: None,
                                link: false,
                            };
                            if let Some(o) = self.act(a) {
                                self.selection = o.created.first().copied().into_iter().collect();
                                close = true;
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
        }
        if !close && self.dialog == Dialog::None {
            self.dialog = dialog;
        }
    }
}

/// A line's rect may have negative size; store a positive rect plus flips.
fn normalize_line(r: Rect) -> (Rect, bool, bool) {
    let fh = r.w < 0.0;
    let fv = r.h < 0.0;
    (Rect::new(r.x.min(r.x + r.w), r.y.min(r.y + r.h), r.w.abs(), r.h.abs()), fh, fv)
}

/// A labelled single-line field that reports (edited this frame, has keyboard focus).
fn labeled_field_state(ui: &mut egui::Ui, label: &str, value: &mut String) -> (bool, bool) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        let r = ui.add(egui::TextEdit::singleline(value).desired_width(70.0)).labelled_by(l.id);
        (r.changed(), r.has_focus())
    })
    .inner
}

/// A number without trailing zeros.
fn fmt_num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A length in points shown in inches.
fn fmt_len(pt: f64) -> String {
    format!("{}in", fmt_num(pt / 72.0))
}

fn labeled_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        ui.text_edit_singleline(value).labelled_by(l.id);
    });
}

impl eframe::App for NewpubApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        NewpubApp::ui(self, ui);
    }
}
