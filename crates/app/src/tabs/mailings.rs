//! The Mailings ribbon tab.

use crate::widgets::{self, group, ribbon_button, small_button};
use crate::{NewpubApp, icons as ic, labeled_field};
use newpub_engine::core::{Command, FilterOp, Id, ObjectKind};
use newpub_engine::{Query, SessionAction};

/// State of the Mailings tab's windows.
#[derive(Default)]
pub(crate) struct MailingsState {
    recipients_open: bool,
    fields_open: bool,
    merge_pdf_open: bool,
    options_open: bool,
    source: String,
    sheet: String,
    pdf: String,
    filter_field: String,
    filter_op: usize,
    filter_value: String,
    sort_field: String,
    sort_desc: bool,
}

/// What the data source query reports.
struct Source {
    fields: Vec<String>,
    records: usize,
    preview: Option<usize>,
}

const OPS: [(&str, FilterOp); 5] = [
    ("equals", FilterOp::Equals),
    ("does not equal", FilterOp::NotEquals),
    ("contains", FilterOp::Contains),
    ("is blank", FilterOp::IsBlank),
    ("is not blank", FilterOp::IsNotBlank),
];

impl NewpubApp {
    fn data_source(&mut self) -> Option<Source> {
        let v = self.session.query(&Query::DataSource).ok()?;
        if v.is_null() {
            return None;
        }
        Some(Source {
            fields: v["fields"].as_array()?.iter().filter_map(|f| f.as_str().map(String::from)).collect(),
            records: v["records"].as_u64().unwrap_or(0) as usize,
            preview: v["preview"].as_u64().map(|p| p as usize),
        })
    }

    /// The selected text frame, if any.
    fn mail_text_frame(&self) -> Option<Id> {
        let d = self.session.doc();
        self.selection
            .iter()
            .copied()
            .find(|id| matches!(d.objects.get(id).map(|o| &o.kind), Some(ObjectKind::Text(_))))
    }

    pub(crate) fn mailings_tab(&mut self, ui: &mut egui::Ui) {
        let src = self.data_source();
        let has_src = src.is_some();
        let frame = self.mail_text_frame();
        group(ui, "Start Mail Merge", |ui| {
            if ribbon_button(ui, ic::USERS, "Select Recipients", self.mailings_ui.recipients_open, true).clicked() {
                self.mailings_ui.recipients_open = !self.mailings_ui.recipients_open;
            }
            if ribbon_button(ui, ic::FUNNEL, "Recipient Options", self.mailings_ui.options_open, has_src).clicked() {
                self.mailings_ui.options_open = !self.mailings_ui.options_open;
            }
        });
        group(ui, "Write & Insert", |ui| {
            let can = has_src && frame.is_some();
            if ribbon_button(ui, ic::ENVELOPE, "Insert Merge Field", self.mailings_ui.fields_open, can).clicked() {
                self.mailings_ui.fields_open = !self.mailings_ui.fields_open;
            }
        });
        group(ui, "Preview", |ui| {
            let previewing = src.as_ref().is_some_and(|s| s.preview.is_some());
            if ribbon_button(ui, ic::EYE, "Preview Results", previewing, has_src).clicked() {
                let record = if previewing { None } else { Some(0) };
                self.act(SessionAction::SetMergePreview { record });
            }
            let (cur, total) = src.as_ref().map(|s| (s.preview, s.records)).unwrap_or((None, 0));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let on = cur.is_some() && total > 0;
                if small_button(ui, ic::CARET_LEFT, "Previous Record", false, on).clicked() {
                    let i = cur.unwrap_or(0);
                    let record = Some(if i == 0 { total - 1 } else { i - 1 });
                    self.act(SessionAction::SetMergePreview { record });
                }
                if small_button(ui, ic::CARET_RIGHT, "Next Record", false, on).clicked() {
                    let record = Some((cur.unwrap_or(0) + 1) % total);
                    self.act(SessionAction::SetMergePreview { record });
                }
                if let Some(i) = cur {
                    widgets::hint(ui, &format!("Record {} of {}", i + 1, total));
                }
            });
        });
        group(ui, "Finish", |ui| {
            if ribbon_button(ui, ic::FILE_PDF, "Merge to PDF", self.mailings_ui.merge_pdf_open, has_src).clicked() {
                self.mailings_ui.merge_pdf_open = !self.mailings_ui.merge_pdf_open;
                if self.mailings_ui.pdf.is_empty() {
                    self.mailings_ui.pdf = "merged.pdf".into();
                }
            }
            if ribbon_button(ui, ic::ARROWS_MERGE, "Merge to Publication", false, has_src).clicked()
                && self.act(SessionAction::MergeToPublication {}).is_some()
            {
                self.status = "Merged to a new publication".into();
            }
        });
    }

    pub(crate) fn mailings_windows(&mut self, ctx: &egui::Context) {
        let src = self.data_source();
        if self.mailings_ui.recipients_open {
            let mut open = true;
            egui::Window::new("Recipients").default_pos(window_pos(ctx)).open(&mut open).collapsible(false).show(
                ctx,
                |ui| {
                    labeled_field(ui, "Data source file", &mut self.mailings_ui.source);
                    labeled_field(ui, "Sheet (optional)", &mut self.mailings_ui.sheet);
                    if widgets::primary_button(ui, "Attach Data Source").clicked() {
                        let sheet = Some(self.mailings_ui.sheet.trim().to_string()).filter(|s| !s.is_empty());
                        let path = self.mailings_ui.source.clone();
                        if self.act(SessionAction::AttachDataSource { path, sheet }).is_some() {
                            self.status = "Data source attached".into();
                        }
                    }
                    if let Some(s) = self.data_source() {
                        ui.separator();
                        ui.label(format!("{} records", s.records));
                        ui.label(format!("Fields: {}", s.fields.join(", ")));
                    }
                },
            );
            self.mailings_ui.recipients_open = open;
        }
        if self.mailings_ui.fields_open {
            let mut open = true;
            let frame = self.mail_text_frame();
            egui::Window::new("Merge Fields").default_pos(window_pos(ctx)).open(&mut open).collapsible(false).show(
                ctx,
                |ui| {
                    let Some(src) = &src else {
                        ui.label("Select recipients first");
                        return;
                    };
                    for field in &src.fields {
                        if ui.button(field).clicked()
                            && let Some(f) = frame
                        {
                            let at = self.insertion_point(f);
                            self.act(Command::InsertMergeField { target: f, at: Some(at), field: field.clone() });
                        }
                    }
                },
            );
            self.mailings_ui.fields_open = open;
        }
        if self.mailings_ui.options_open {
            let mut open = true;
            egui::Window::new("Recipient Options")
                .default_pos(window_pos(ctx))
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    let Some(src) = &src else { return };
                    let m = &mut self.mailings_ui;
                    let combo = |ui: &mut egui::Ui, id: &str, cur: &mut String| {
                        egui::ComboBox::from_id_salt(id).selected_text(cur.as_str()).show_ui(ui, |ui| {
                            ui.selectable_value(cur, String::new(), "(none)");
                            for f in &src.fields {
                                ui.selectable_value(cur, f.clone(), f);
                            }
                        });
                    };
                    ui.horizontal(|ui| {
                        ui.label("Filter");
                        combo(ui, "mm_filter_field", &mut m.filter_field);
                        egui::ComboBox::from_id_salt("mm_filter_op").selected_text(OPS[m.filter_op].0).show_ui(
                            ui,
                            |ui| {
                                for (i, (n, _)) in OPS.iter().enumerate() {
                                    ui.selectable_value(&mut m.filter_op, i, *n);
                                }
                            },
                        );
                        ui.add(egui::TextEdit::singleline(&mut m.filter_value).desired_width(80.0));
                    });
                    let (ff, fo, fv) = (m.filter_field.clone(), m.filter_op, m.filter_value.clone());
                    ui.horizontal(|ui| {
                        ui.label("Sort by");
                        combo(ui, "mm_sort_field", &mut m.sort_field);
                        ui.checkbox(&mut m.sort_desc, "Descending");
                    });
                    let (sf, sd) = (m.sort_field.clone(), m.sort_desc);
                    if widgets::primary_button(ui, "Apply Options").clicked() {
                        let filter = Some(ff).filter(|f| !f.is_empty());
                        let op = filter.as_ref().map(|_| OPS[fo].1);
                        let value = filter.as_ref().map(|_| fv);
                        self.act(SessionAction::SetMergeFilter { field: filter, op, value });
                        let field = Some(sf).filter(|f| !f.is_empty());
                        self.act(SessionAction::SetMergeSort { field, descending: sd });
                    }
                    if ui.button("Skip blank lines").clicked() {
                        self.act(SessionAction::SetMergeOptions { skip_blank_lines: Some(true) });
                    }
                    if ui.button("Keep blank lines").clicked() {
                        self.act(SessionAction::SetMergeOptions { skip_blank_lines: Some(false) });
                    }
                });
            self.mailings_ui.options_open = open;
        }
        if self.mailings_ui.merge_pdf_open {
            let mut open = true;
            let mut done = false;
            egui::Window::new("Merged PDF").default_pos(window_pos(ctx)).open(&mut open).collapsible(false).show(
                ctx,
                |ui| {
                    labeled_field(ui, "Merged PDF file", &mut self.mailings_ui.pdf);
                    if widgets::primary_button(ui, "Merge").clicked() {
                        let path = self.mailings_ui.pdf.clone();
                        if self.act(SessionAction::MergeToPdf { path: path.clone(), options: None }).is_some() {
                            self.status = format!("Merged to {path}");
                            done = true;
                        }
                    }
                },
            );
            self.mailings_ui.merge_pdf_open = open && !done;
        }
    }
}

/// Default window position: below the ribbon, towards the right, clear of the tab buttons.
fn window_pos(ctx: &egui::Context) -> egui::Pos2 {
    egui::pos2((ctx.content_rect().right() - 340.0).max(20.0), 200.0)
}
