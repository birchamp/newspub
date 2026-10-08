//! newpub-app: the egui desktop application. A thin view over [`Session`]: every gesture
//! becomes an [`Action`], every displayed fact comes from the session.

use egui::{Color32, Pos2, Rect as ERect, Sense, Stroke, TextureHandle, Vec2};
use newpub_engine::core::{
    self as core, Align, CharAttrs, Command, Id, Insets, Length, ObjectKind, ParaAttrs, Rect, ShapeKind,
};
use newpub_engine::{Action, PdfOptions, Session, SessionAction};

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
    NewPublication { preset: usize, width: String, height: String },
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
        }
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
        const STOPS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
        serde_json::json!({
            "zoom": self.zoom,
            "fit": self.fitted,
            "zoom_stop": STOPS.iter().any(|z| (z - self.zoom).abs() < 1e-4),
            "selection": self.selection,
            "page": self.page,
            "tool": format!("{:?}", self.tool),
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
        self.shortcuts(&ctx);
        egui::Panel::top("ribbon").show(ui, |ui| self.ribbon(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("pages").resizable(false).default_size(90.0).show(ui, |ui| self.page_navigator(ui));
        egui::Panel::right("format").resizable(false).default_size(190.0).show(ui, |ui| self.format_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.dialogs(&ctx);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing_in_widget = ctx.egui_wants_keyboard_input();
        let (undo, redo) = ctx.input(|i| {
            (
                i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                i.modifiers.command
                    && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))),
            )
        });
        if !typing_in_widget {
            if undo {
                self.act(SessionAction::Undo);
            }
            if redo {
                self.act(SessionAction::Redo);
            }
        }
        // Typing into the selected text frame.
        if typing_in_widget || self.dialog != Dialog::None {
            return;
        }
        let Some(frame) = self.selected_text_frame() else { return };
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Text(t) => {
                    self.act(SessionAction::TypeText { target: frame, at: None, text: t });
                }
                egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => {
                    self.act(SessionAction::TypeText { target: frame, at: None, text: "\n".into() });
                }
                egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
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
                    }
                }
                egui::Event::Key { key: egui::Key::Delete, pressed: true, modifiers, .. } if modifiers.command => {
                    let ids = self.selection.clone();
                    self.act(Command::DeleteObjects { ids });
                }
                _ => {}
            }
        }
    }

    fn ribbon(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("New").clicked() {
                self.dialog = Dialog::NewPublication { preset: 0, width: "8.5in".into(), height: "11in".into() };
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
            }
            if ui.button("Zoom Out").clicked() {
                self.zoom = (self.zoom / 1.25).max(0.1);
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
        let Some(frame) = self.selected_text_frame() else {
            if let Some(id) = self.selection.first().copied() {
                ui.label(format!("Object {id} selected"));
            } else {
                ui.label("Select a text box to format text.");
            }
            return;
        };
        let doc = self.session.doc();
        let Ok(sid) = doc.story_of(frame) else { return };
        let Ok(story) = doc.story(sid) else { return };
        let rc = doc.resolve_char(&story.paras[0], &story.span_attrs_at(0));
        let mut font = rc.font.clone();
        let mut size = rc.size;
        let families = self.session.fonts().families();
        let font_label = ui.label("Font");
        egui::ComboBox::from_id_salt("font")
            .selected_text(&font)
            .show_ui(ui, |ui| {
                for f in &families {
                    ui.selectable_value(&mut font, f.clone(), f);
                }
            })
            .response
            .labelled_by(font_label.id);
        let size_label = ui.label("Font size");
        ui.add(egui::DragValue::new(&mut size).range(1.0..=999.0).speed(0.5).suffix(" pt")).labelled_by(size_label.id);
        let mut patch = CharAttrs::default();
        if font != rc.font {
            patch.font = Some(font);
        }
        if (size - rc.size).abs() > 1e-9 {
            patch.size = Some(Length(size));
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
        for id in &self.selection {
            if let Some(o) = doc.objects.get(id) {
                let rr = ERect::from_min_max(
                    self.page_to_screen(o.rect.x, o.rect.y),
                    self.page_to_screen(o.rect.right(), o.rect.bottom()),
                )
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

    fn handle_canvas_input(&mut self, resp: &egui::Response) {
        if resp.drag_started() {
            // egui reports a drag once the pointer has moved; the gesture began at the press origin.
            self.drag_start = resp.ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            if self.tool == Tool::Select
                && let Some(p) = self.drag_start
            {
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
        if resp.dragged() {
            self.drag_now = resp.interact_pointer_pos();
            if let (Some(m), Some(a), Some(b)) = (self.moving.as_mut(), self.drag_start, self.drag_now) {
                *m = (b - a) / self.zoom;
            }
        }
        if resp.drag_stopped() {
            let (a, b) = (self.drag_start.take(), self.drag_now.take().or(resp.interact_pointer_pos()));
            if let Some(m) = self.moving.take() {
                if m.length() > 0.5 && !self.selection.is_empty() {
                    let ids = self.selection.clone();
                    self.act(Command::MoveObjects { ids, dx: Length(m.x as f64), dy: Length(m.y as f64) });
                }
            } else if let (Some(a), Some(b)) = (a, b) {
                self.create_from_drag(a, b);
            }
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
            Dialog::NewPublication { preset, width, height } => {
                egui::Window::new("New Publication").collapsible(false).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for (i, (name, w, h)) in PRESETS.iter().enumerate() {
                            if ui.selectable_label(*preset == i, *name).clicked() {
                                *preset = i;
                                if !w.is_empty() {
                                    *width = w.to_string();
                                    *height = h.to_string();
                                }
                            }
                        }
                    });
                    labeled_field(ui, "Page width", width);
                    labeled_field(ui, "Page height", height);
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            match (parse_len(width), parse_len(height)) {
                                (Some(w), Some(h)) => {
                                    let a = SessionAction::NewDocument {
                                        width: w,
                                        height: h,
                                        margins: Some(Insets::uniform(36.0)),
                                        facing: false,
                                        pages: 1,
                                        bleed: None,
                                    };
                                    if self.act(a).is_some() {
                                        self.page = 0;
                                        self.selection.clear();
                                        close = true;
                                    }
                                }
                                _ => self.status = "Enter a page size such as 8.5in or 210mm".into(),
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
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
                    ui.horizontal(|ui| {
                        if ui.button("Open File").clicked()
                            && self.act(SessionAction::Open { path: path.clone() }).is_some()
                        {
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
