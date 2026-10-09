//! The Insert ribbon tab: tools, tables, WordArt, shapes, fields, building blocks, links and symbols.

use crate::widgets::{self, group, ribbon_button};
use crate::{Dialog, NewpubApp, Tool, icons as ic, labeled_field};
use newpub_engine::core::{
    self as core, Color, Command, Dash, Id, Length, LineCap, LineJoin, ObjectKind, Rect, ShapeKind, SpecialChar,
    field::Field,
};
use newpub_engine::{Query, SessionAction};

/// Which Insert window is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Open {
    #[default]
    None,
    Table,
    WordArt,
    Shapes,
    Blocks,
    Link,
    Symbols,
}

/// State of the Insert tab's windows.
#[derive(Default)]
pub(crate) struct InsertState {
    open: Open,
    rows: String,
    cols: String,
    wordart_text: String,
    wordart_styles: Vec<String>,
    wordart_style: usize,
    /// `(name, category)` of the building blocks, loaded when the window opens.
    blocks: Vec<(String, String)>,
    shapes: Vec<String>,
    url: String,
}

/// Friendly name of a shape kind for the gallery, or `None` for kinds not offered here.
fn shape_label(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "round_rect" => "Rounded Rectangle",
        "triangle" => "Triangle",
        "star" => "Star",
        "polygon" => "Polygon",
        "arrow" => "Arrow",
        "callout" => "Callout",
        _ => return None,
    })
}

fn shape_kind(kind: &str) -> Option<ShapeKind> {
    Some(match kind {
        "round_rect" => ShapeKind::RoundRect { radius: Length(18.0) },
        "triangle" => ShapeKind::Triangle,
        "star" => ShapeKind::Star { points: 5, inner: 0.5 },
        "polygon" => ShapeKind::Polygon { sides: 6 },
        "arrow" => ShapeKind::Arrow,
        "callout" => ShapeKind::Callout { tail: [0.2, 1.3] },
        _ => return None,
    })
}

const SPECIALS: [(&str, SpecialChar); 7] = [
    ("Em Dash", SpecialChar::EmDash),
    ("En Dash", SpecialChar::EnDash),
    ("Non-breaking Space", SpecialChar::NonBreakingSpace),
    ("Non-breaking Hyphen", SpecialChar::NonBreakingHyphen),
    ("Optional Hyphen", SpecialChar::OptionalHyphen),
    ("Line Break", SpecialChar::LineBreak),
    ("Tab Character", SpecialChar::Tab),
];

/// Strings of a JSON array result.
fn strings(v: Option<serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()))
        .unwrap_or_default()
}

impl NewpubApp {
    /// The selected text box, when the first selected object is one.
    fn selected_frame(&self) -> Option<Id> {
        let id = *self.selection.first()?;
        matches!(self.session.doc().objects.get(&id)?.kind, ObjectKind::Text(_)).then_some(id)
    }

    /// The selected picture, if any.
    fn selected_picture(&self) -> Option<Id> {
        let id = *self.selection.first()?;
        matches!(self.session.doc().objects.get(&id)?.kind, ObjectKind::Image(_)).then_some(id)
    }

    /// The page area inside the margins.
    fn margin_rect(&self) -> Rect {
        let s = &self.session.doc().setup;
        let m = &s.margins;
        Rect::new(
            m.left.0,
            m.top.0,
            (s.width.0 - m.left.0 - m.right.0).max(36.0),
            (s.height.0 - m.top.0 - m.bottom.0).max(36.0),
        )
    }

    fn select_created(&mut self, out: Option<newpub_engine::Outcome>) -> bool {
        match out.and_then(|o| o.created.first().copied()) {
            Some(id) => {
                self.selection = vec![id];
                true
            }
            None => false,
        }
    }

    fn open_insert_window(&mut self, w: Open) {
        self.insert_ui.open = w;
        match w {
            Open::Table => {
                self.insert_ui.rows = "3".into();
                self.insert_ui.cols = "3".into();
            }
            Open::WordArt => {
                self.insert_ui.wordart_text.clear();
                self.insert_ui.wordart_style = 0;
                self.insert_ui.wordart_styles = strings(self.session.query(&Query::WordArtStyles).ok());
            }
            Open::Shapes => {
                self.insert_ui.shapes = strings(self.session.query(&Query::ShapeKinds).ok());
            }
            Open::Blocks => {
                let v = self.session.query(&Query::BuildingBlocks).ok();
                self.insert_ui.blocks = v
                    .and_then(|v| {
                        v.as_array().map(|a| {
                            a.iter()
                                .filter_map(|b| {
                                    let name = b.get("name")?.as_str()?.to_string();
                                    let cat = b.get("category")?.as_str()?.to_string();
                                    Some((name, cat))
                                })
                                .collect()
                        })
                    })
                    .unwrap_or_default();
            }
            _ => {}
        }
    }

    pub(crate) fn insert_tab(&mut self, ui: &mut egui::Ui) {
        let frame = self.selected_frame();
        let picture = self.selected_picture();
        let open = self.insert_ui.open;
        group(ui, "Objects", |ui| {
            if ribbon_button(ui, ic::TEXT_T, "Text Box", self.tool == Tool::TextBox, true).clicked() {
                self.tool = Tool::TextBox;
            }
            if ribbon_button(ui, ic::IMAGE, "Picture", false, true).clicked() {
                self.dialog = Dialog::InsertPicture { path: String::new() };
            }
            if ribbon_button(ui, ic::FILE_TEXT, "Text File", false, frame.is_some())
                .on_disabled_hover_text("Select a text box first")
                .on_hover_text("Insert the text of a .txt or .docx file into the selected text box")
                .clicked()
                && let Some(f) = frame
            {
                self.dialog = Dialog::InsertText { path: String::new(), target: f };
            }
            if ribbon_button(ui, ic::TABLE, "Table", open == Open::Table, true).clicked() {
                self.open_insert_window(Open::Table);
            }
            if ribbon_button(ui, ic::SHAPES, "Shapes", open == Open::Shapes, true).clicked() {
                self.open_insert_window(Open::Shapes);
            }
            if ribbon_button(ui, ic::TEXT_AA, "WordArt", open == Open::WordArt, true).clicked() {
                self.open_insert_window(Open::WordArt);
            }
        });
        group(ui, "Blocks", |ui| {
            if ribbon_button(ui, ic::PUZZLE_PIECE, "Building Blocks", open == Open::Blocks, true).clicked() {
                self.open_insert_window(Open::Blocks);
            }
        });
        group(ui, "Fields", |ui| {
            let on = frame.is_some();
            let tip = "Select a text box first";
            let mut field = None;
            if ribbon_button(ui, ic::HASH, "Page Number", false, on).on_disabled_hover_text(tip).clicked() {
                field = Some(Some(Field::PageNumber));
            }
            if ribbon_button(ui, ic::FILES, "Page Count", false, on).on_disabled_hover_text(tip).clicked() {
                field = Some(Some(Field::PageCount));
            }
            if ribbon_button(ui, ic::CALENDAR, "Date", false, on).on_disabled_hover_text(tip).clicked() {
                field = Some(Some(Field::Date(newpub_engine::core::field::DateFormat::Long)));
            }
            if let (Some(f), Some(field)) = (frame, field) {
                let at = Some(self.insertion_point(f));
                if let Some(field) = field {
                    self.act(Command::InsertField { target: f, at, field });
                }
            }
        });
        group(ui, "Links & Symbols", |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if widgets::small_button(ui, ic::LINK, "Hyperlink", open == Open::Link, frame.is_some())
                    .on_disabled_hover_text("Select a text box first")
                    .clicked()
                {
                    self.insert_ui.url.clear();
                    self.insert_ui.open = Open::Link;
                }
                if widgets::small_button(ui, ic::PI, "Special Character", open == Open::Symbols, frame.is_some())
                    .on_disabled_hover_text("Select a text box first")
                    .clicked()
                {
                    self.insert_ui.open = Open::Symbols;
                }
            });
            ui.vertical(|ui| {
                if widgets::small_button(ui, ic::SUBTITLES, "Caption", false, picture.is_some())
                    .on_disabled_hover_text("Select a picture first")
                    .clicked()
                    && let Some(p) = picture
                {
                    self.act(Command::AddCaption { picture: p, text: None, position: Default::default() });
                }
            });
        });
    }

    pub(crate) fn insert_windows(&mut self, ctx: &egui::Context) {
        let mut close = false;
        match self.insert_ui.open {
            Open::None => return,
            Open::Table => {
                egui::Window::new("New Table").collapsible(false).resizable(false).show(ctx, |ui| {
                    labeled_field(ui, "Rows", &mut self.insert_ui.rows);
                    labeled_field(ui, "Columns", &mut self.insert_ui.cols);
                    let rows = self.insert_ui.rows.trim().parse::<usize>().ok().filter(|n| (1..=100).contains(n));
                    let cols = self.insert_ui.cols.trim().parse::<usize>().ok().filter(|n| (1..=50).contains(n));
                    if rows.is_none() || cols.is_none() {
                        widgets::hint(ui, "Rows: 1 to 100, columns: 1 to 50.");
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Insert Table").clicked()
                            && let (Some(rows), Some(cols)) = (rows, cols)
                        {
                            let m = self.margin_rect();
                            let rect = Rect::new(m.x, m.y, m.w, (rows as f64 * 24.0).min(m.h));
                            let out =
                                self.act(Command::AddTable { page: Some(self.page), master: None, rect, rows, cols });
                            close = self.select_created(out);
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Open::WordArt => {
                egui::Window::new("New WordArt").collapsible(false).resizable(false).show(ctx, |ui| {
                    labeled_field(ui, "WordArt text", &mut self.insert_ui.wordart_text);
                    let styles = self.insert_ui.wordart_styles.clone();
                    if !styles.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Style");
                            let cur = styles.get(self.insert_ui.wordart_style).cloned().unwrap_or_default();
                            egui::ComboBox::from_id_salt("wordart-style").selected_text(cur).show_ui(ui, |ui| {
                                for (i, s) in styles.iter().enumerate() {
                                    ui.selectable_value(&mut self.insert_ui.wordart_style, i, s);
                                }
                            });
                        });
                    }
                    let text = self.insert_ui.wordart_text.trim().to_string();
                    if text.is_empty() {
                        widgets::hint(ui, "Type the text to shape.");
                    }
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Insert WordArt").clicked() && !text.is_empty() {
                            let m = self.margin_rect();
                            let (w, h) = (m.w.min(288.0), 72.0);
                            let rect = Rect::new(m.x + (m.w - w) / 2.0, m.y + (m.h - h) / 2.0, w, h);
                            let style = styles.get(self.insert_ui.wordart_style).cloned();
                            let out = self.act(Command::AddWordArt {
                                page: Some(self.page),
                                master: None,
                                rect,
                                text,
                                style,
                            });
                            close = self.select_created(out);
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Open::Shapes => {
                egui::Window::new("Shape Gallery").collapsible(false).resizable(false).show(ctx, |ui| {
                    let mut pick = None;
                    for k in &self.insert_ui.shapes {
                        if let Some(label) = shape_label(k)
                            && ui.button(label).clicked()
                        {
                            pick = shape_kind(k);
                        }
                    }
                    if let Some(kind) = pick {
                        let size = 144.0;
                        let s = &self.session.doc().setup;
                        let rect = Rect::new((s.width.0 - size) / 2.0, (s.height.0 - size) / 2.0, size, size);
                        let stroke = Some(core::Stroke {
                            color: Color::BLACK,
                            width: Length(1.0),
                            dash: Dash::Solid,
                            cap: LineCap::Butt,
                            join: LineJoin::Miter,
                        });
                        let out = self.act(Command::AddShape {
                            page: Some(self.page),
                            master: None,
                            rect,
                            kind,
                            fill: Some(Color::rgb(255, 255, 255)),
                            stroke,
                        });
                        close = self.select_created(out);
                    }
                    ui.separator();
                    if widgets::secondary_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Open::Blocks => {
                egui::Window::new("Building Blocks").collapsible(false).show(ctx, |ui| {
                    let mut pick = None;
                    let mut cats: Vec<String> = Vec::new();
                    for (_, c) in &self.insert_ui.blocks {
                        if !cats.contains(c) {
                            cats.push(c.clone());
                        }
                    }
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for cat in &cats {
                            ui.label(egui::RichText::new(cat).strong());
                            for (name, _) in self.insert_ui.blocks.iter().filter(|b| &b.1 == cat) {
                                if ui.button(name).clicked() {
                                    pick = Some(name.clone());
                                }
                            }
                            ui.add_space(4.0);
                        }
                    });
                    if let Some(name) = pick {
                        let m = self.margin_rect();
                        let out = self.act(SessionAction::InsertBuildingBlock {
                            name,
                            page: self.page,
                            x: Length(m.x),
                            y: Length(m.y),
                        });
                        if let Some(o) = out {
                            self.selection = o.created;
                            close = true;
                        }
                    }
                    ui.separator();
                    if widgets::secondary_button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Open::Link => {
                egui::Window::new("Insert Hyperlink").collapsible(false).resizable(false).show(ctx, |ui| {
                    labeled_field(ui, "Link address", &mut self.insert_ui.url);
                    let url = self.insert_ui.url.trim().to_string();
                    let frame = self.selected_frame();
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Apply Link").clicked()
                            && !url.is_empty()
                            && let Some(f) = frame
                        {
                            let end = self.insertion_point(f);
                            let cmd = Command::SetHyperlink { target: f, start: 0, end, url: Some(url), page: None };
                            if self.act(cmd).is_some() {
                                close = true;
                            }
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                    widgets::hint(ui, "Links the whole text box.");
                });
            }
            Open::Symbols => {
                egui::Window::new("Special Characters").collapsible(false).resizable(false).show(ctx, |ui| {
                    let frame = self.selected_frame();
                    for (label, ch) in SPECIALS {
                        if ui.add_enabled(frame.is_some(), egui::Button::new(label)).clicked()
                            && let Some(f) = frame
                        {
                            let at = Some(self.insertion_point(f));
                            self.act(Command::InsertSpecialChar { target: f, at, char: ch });
                        }
                    }
                    ui.separator();
                    if widgets::secondary_button(ui, "Done").clicked() {
                        close = true;
                    }
                });
            }
        }
        if close {
            self.insert_ui.open = Open::None;
        }
    }
}
