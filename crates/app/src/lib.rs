//! newpub-app: the egui desktop application. A thin view over [`Session`]: every gesture
//! becomes an [`Action`], every displayed fact comes from the session.

mod a11y;
mod dup;
mod files;
mod freeform;
mod icons;
mod inspector;
mod pane;
mod picker;
mod print;
mod recent;
mod shell;
mod tabs;
pub mod theme;
mod view;
mod widgets;

use egui::{Color32, Pos2, Rect as ERect, Stroke, TextureHandle, Vec2};
use newpub_engine::core::{self as core, Command, Id, Length, ObjectKind, ObjectPatch, Rect, ShapeKind, ZOp};
use newpub_engine::{Action, Session, SessionAction};
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
    Freeform,
}

impl Tool {
    pub const ALL: [(Tool, &'static str); 6] = [
        (Tool::Select, "Select"),
        (Tool::TextBox, "Text Box"),
        (Tool::Rectangle, "Rectangle"),
        (Tool::Ellipse, "Ellipse"),
        (Tool::Line, "Line"),
        (Tool::Freeform, "Freeform"),
    ];
}

/// In-app dialogs (egui windows, so they are reachable through AccessKit and in UI journeys).
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    None,
    Picker(picker::PickerState),
    Print(print::PrintState),
    ExportPdf {
        path: String,
        crop_marks: bool,
        booklet: bool,
    },
    Save {
        path: String,
    },
    Open {
        path: String,
    },
    InsertPicture {
        path: String,
    },
    /// The export dialog for every format (files.rs).
    Export(files::ExportState),
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
    pub(crate) texture: Option<(TextureHandle, u64, usize, u32)>,
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
    /// Scroll, spread, units and keyboard-editing state (see view.rs).
    view: view::ViewState,
    /// Selection pane (see pane.rs).
    pane: pane::PaneState,
    /// Freeform tool and point editing (see freeform.rs).
    freeform: freeform::FreeformState,
    /// Active ribbon tab.
    ribbon_tab: shell::RibbonTab,
    /// Fonts and styles installed into the egui context.
    themed: bool,
    pub(crate) insert_ui: tabs::insert::InsertState,
    pub(crate) design_ui: tabs::design::DesignState,
    pub(crate) mailings_ui: tabs::mailings::MailingsState,
    pub(crate) review_ui: tabs::review::ReviewState,
}

/// Text buffers of the object and format panels.
#[derive(Default)]
pub(crate) struct Fields {
    owner: Option<Id>,
    size: String,
    x: String,
    y: String,
    w: String,
    h: String,
    columns: String,
    pub(crate) more: inspector::InspectorFields,
}

pub(crate) fn parse_len(s: &str) -> Option<Length> {
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
            view: view::ViewState::default(),
            pane: pane::PaneState::default(),
            freeform: freeform::FreeformState::default(),
            ribbon_tab: shell::RibbonTab::default(),
            themed: false,
            insert_ui: Default::default(),
            design_ui: Default::default(),
            mailings_ui: Default::default(),
            review_ui: Default::default(),
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

    pub(crate) fn zoom_step(&mut self, dir: i32) {
        let z = self.zoom;
        let next = if dir > 0 {
            ZOOM_STOPS.iter().copied().find(|s| *s > z + 1e-4).unwrap_or(ZOOM_STOPS[ZOOM_STOPS.len() - 1])
        } else {
            ZOOM_STOPS.iter().copied().rev().find(|s| *s < z - 1e-4).unwrap_or(ZOOM_STOPS[0])
        };
        self.zoom = next;
        self.fitted = false;
        self.reset_scroll();
    }

    /// Zooms so the whole page fits the canvas.
    pub fn fit_page(&mut self) {
        let (w, h) = self.content_pts();
        let avail = if self.canvas_size.x > 80.0 && self.canvas_size.y > 80.0 {
            self.canvas_size - Vec2::splat(40.0)
        } else {
            Vec2::new(800.0, 700.0)
        };
        if w > 0.0 && h > 0.0 {
            self.zoom = (avail.x / w).min(avail.y / h).clamp(0.1, 8.0);
        }
        self.fitted = true;
        self.reset_scroll();
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
        let mut v = serde_json::json!({
            "zoom": self.zoom,
            "fit": self.fitted,
            "zoom_stop": ZOOM_STOPS.iter().any(|z| (z - self.zoom).abs() < 1e-4),
            "selection": self.selection,
            "page": self.page,
            "tool": format!("{:?}", self.tool),
            "print_jobs": self.print_jobs,
            "last_print_job": self.last_print_job,
        });
        self.view_extra(&mut v);
        self.freeform_view(&mut v);
        v
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
        if !std::mem::replace(&mut self.themed, true) {
            // Fonts registered now are usable from the next frame on.
            theme::install(&ctx);
            ctx.set_theme(egui::Theme::Light);
            ctx.request_repaint();
            return;
        }
        if std::mem::take(&mut self.startup_picker) {
            self.open_picker();
        }
        self.sync_units();
        self.shortcuts(&ctx);
        let p = theme::palette(&ctx);
        let bar = |fill| egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, p.border));
        egui::Panel::top("header").exact_size(48.0).frame(egui::Frame::NONE).show(ui, |ui| self.header(ui));
        egui::Panel::top("ribbon")
            .frame(bar(p.surface).inner_margin(egui::Margin { left: 4, right: 4, top: 2, bottom: 4 }))
            .show(ui, |ui| self.ribbon(ui));
        egui::Panel::bottom("status").exact_size(30.0).frame(bar(p.surface)).show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("pages")
            .resizable(false)
            .exact_size(132.0)
            .frame(bar(p.surface).inner_margin(egui::Margin::symmetric(10, 10)))
            .show(ui, |ui| self.page_navigator(ui));
        egui::Panel::right("format")
            .resizable(false)
            .exact_size(272.0)
            .frame(bar(p.app_bg).inner_margin(egui::Margin::symmetric(10, 10)))
            .show(ui, |ui| self.format_panel(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.pasteboard)).show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
        self.tab_windows(&ctx);
        self.selection_pane(&ctx);
    }

    pub(crate) fn open_picker(&mut self) {
        self.dialog = Dialog::Picker(picker::PickerState::new(self));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let typing_in_widget = ctx.egui_wants_keyboard_input() && !self.canvas_focused(ctx);
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
                Key::Escape if self.freeform_escape() => {}
                Key::Escape => self.escape(),
                Key::Enter => self.freeform_enter(),
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

    pub(crate) fn reorder(&mut self, op: ZOp) {
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
        let full = ui.available_rect_before_wrap();
        self.canvas_view(ui);
        self.freeform_overlay(ui, ERect::from_min_max(full.max - self.canvas_size, full.max));
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
        if self.tool == Tool::Freeform {
            self.freeform_input(resp);
            return;
        }
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
            Tool::Select | Tool::Freeform => None,
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
}

/// A line's rect may have negative size; store a positive rect plus flips.
fn normalize_line(r: Rect) -> (Rect, bool, bool) {
    let fh = r.w < 0.0;
    let fv = r.h < 0.0;
    (Rect::new(r.x.min(r.x + r.w), r.y.min(r.y + r.h), r.w.abs(), r.h.abs()), fh, fv)
}

/// A labelled single-line field that reports (edited this frame, has keyboard focus).
pub(crate) fn labeled_field_state(ui: &mut egui::Ui, label: &str, value: &mut String) -> (bool, bool) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        let r = ui.add(egui::TextEdit::singleline(value).desired_width(70.0)).labelled_by(l.id);
        (r.changed(), r.has_focus())
    })
    .inner
}

/// A number without trailing zeros.
pub(crate) fn fmt_num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A length in points shown in inches.
pub(crate) fn fmt_len(pt: f64) -> String {
    format!("{}in", fmt_num(pt / 72.0))
}

pub(crate) fn labeled_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
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
