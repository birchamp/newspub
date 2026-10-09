//! Review > Properties (title, author, language) and Review > Design Checker (problems to fix before printing).

use eframe::egui;
use newpub_engine::Query;
use newpub_engine::core::{Command, Id, ObjectKind};

use crate::{NewpubApp, icons as ic, widgets};

#[derive(Default)]
pub struct CheckerUi {
    pub properties_open: bool,
    pub checker_open: bool,
    /// Edit buffers for the Properties fields.
    title: String,
    author: String,
    lang: String,
}

/// One Design Checker finding: what is wrong, and the object (and page) it is about.
pub struct Problem {
    pub label: String,
    pub object: Option<(Id, usize)>,
}

impl NewpubApp {
    /// Everything the Design Checker reports, page by page, then publication-wide problems.
    pub(crate) fn design_problems(&mut self) -> Vec<Problem> {
        let mut out = vec![];
        let layout = self.session.layout();
        let doc = self.session.doc();
        for (i, page) in doc.pages.iter().enumerate() {
            let n = i + 1;
            for id in &page.objects {
                let Some(o) = doc.objects.get(id) else { continue };
                if let ObjectKind::Text(t) = &o.kind {
                    if layout.frames.get(id).is_some_and(|f| f.overflow) {
                        out.push(Problem { label: format!("Text does not fit (page {n})"), object: Some((*id, i)) });
                    }
                    if doc.story(t.story).map(|s| s.is_empty()).unwrap_or(true) {
                        out.push(Problem { label: format!("Empty text box (page {n})"), object: Some((*id, i)) });
                    }
                }
            }
        }
        let list = |v: Result<serde_json::Value, _>| v.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
        for v in list(self.session.query(&Query::OffPageObjects)) {
            let (Some(id), Some(page)) = (v.get("id").and_then(|x| x.as_u64()), v.get("page").and_then(|x| x.as_u64()))
            else {
                continue;
            };
            out.push(Problem {
                label: format!("Object is off the page (page {})", page + 1),
                object: Some((Id(id), page as usize)),
            });
        }
        for v in list(self.session.query(&Query::MissingFonts)) {
            if let Some(f) = v.as_str() {
                out.push(Problem { label: format!("Font not installed: {f}"), object: None });
            }
        }
        for v in list(self.session.query(&Query::MissingLinks)) {
            let (Some(asset), Some(path)) =
                (v.get("asset").and_then(|x| x.as_u64()), v.get("path").and_then(|x| x.as_str()))
            else {
                continue;
            };
            let file = std::path::Path::new(path).file_name().map(|f| f.to_string_lossy().to_string());
            let file = file.unwrap_or_else(|| path.to_string());
            let doc = self.session.doc();
            let user = doc.pages.iter().enumerate().find_map(|(i, p)| {
                p.objects.iter().find_map(|id| match doc.objects.get(id).map(|o| &o.kind) {
                    Some(ObjectKind::Image(img)) if img.asset == Some(Id(asset)) => Some((*id, i)),
                    _ => None,
                })
            });
            let label = match user {
                Some((_, i)) => format!("Picture not found: {file} (page {})", i + 1),
                None => format!("Picture not found: {file}"),
            };
            out.push(Problem { label, object: user });
        }
        out
    }

    pub(crate) fn checker_windows(&mut self, ctx: &egui::Context) {
        if self.checker_ui.properties_open {
            self.properties_window(ctx);
        }
        if self.checker_ui.checker_open {
            self.design_checker_window(ctx);
        }
    }

    fn properties_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let meta = self.session.doc().meta.clone();
        egui::Window::new("Publication Properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(crate::tabs::review::window_pos(ctx))
            .show(ctx, |ui| {
                widgets::hint(ui, "Saved with the publication and written into exported PDFs.");
                ui.add_space(4.0);
                let ui_state = &mut self.checker_ui;
                let mut set = None;
                egui::Grid::new("props").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    for (label, buf, now, key) in [
                        ("Title", &mut ui_state.title, &meta.title, 0),
                        ("Author", &mut ui_state.author, &meta.author, 1),
                        ("Language", &mut ui_state.lang, &meta.lang, 2),
                    ] {
                        let l = ui.label(label);
                        let r = ui.add(egui::TextEdit::singleline(buf).desired_width(220.0)).labelled_by(l.id);
                        if r.changed() {
                            set = Some((key, buf.clone()));
                        } else if !r.has_focus() {
                            *buf = now.clone();
                        }
                        ui.end_row();
                    }
                });
                widgets::hint(ui, "Language is a code such as en-GB or fr.");
                if let Some((key, v)) = set {
                    let (mut title, mut author, mut lang) = (None, None, None);
                    match key {
                        0 => title = Some(v),
                        1 => author = Some(v),
                        _ => lang = Some(v),
                    }
                    self.act_run(format!("meta:{key}"), Command::SetMeta { title, author, lang });
                }
            });
        self.checker_ui.properties_open &= open;
    }

    fn design_checker_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let problems = self.design_problems();
        egui::Window::new("Design Checker")
            .open(&mut open)
            .collapsible(false)
            .default_width(280.0)
            .default_pos(crate::tabs::review::window_pos(ctx))
            .show(ctx, |ui| {
                if problems.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(ic::CHECK_CIRCLE).font(crate::theme::icon_font(16.0)));
                        ui.label("No problems found");
                    });
                    return;
                }
                widgets::hint(ui, "Click a problem to select what it is about.");
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i, p) in problems.iter().enumerate() {
                        ui.push_id(i, |ui| {
                            let icon = if p.object.is_some() { ic::WARNING } else { ic::INFO };
                            if widgets::small_button(ui, icon, &p.label, false, true).clicked()
                                && let Some((id, page)) = p.object
                            {
                                self.page = page;
                                self.end_text_edit();
                                self.selection = vec![id];
                            }
                        });
                    }
                });
            });
        self.checker_ui.checker_open &= open;
    }
}
