//! File dialogs: open, save, export, insert picture.

use crate::{Dialog, NewpubApp, labeled_field, picker, print, widgets};
use newpub_engine::{PdfOptions, SessionAction};

/// State of the export dialog.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportState {}

impl NewpubApp {
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
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
                        if widgets::primary_button(ui, "Export").clicked() {
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
                                separations: false,
                            };
                            if self.act(SessionAction::ExportPdf { path: path.clone(), options }).is_some() {
                                self.status = format!("Exported {path}");
                                close = true;
                            }
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Save { path } => {
                egui::Window::new("Save Publication").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "File name", path);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Save File").clicked()
                            && self.act(SessionAction::Save { path: path.clone() }).is_some()
                        {
                            self.remember_recent();
                            self.status = format!("Saved {path}");
                            close = true;
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
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
                        if widgets::primary_button(ui, "Open File").clicked()
                            && self.act(SessionAction::Open { path: path.clone() }).is_some()
                        {
                            self.remember_recent();
                            self.page = 0;
                            self.selection.clear();
                            close = true;
                        }
                        if widgets::secondary_button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Export(_st) => close = true,
            Dialog::InsertPicture { path } => {
                egui::Window::new("Insert Picture").collapsible(false).show(ctx, |ui| {
                    labeled_field(ui, "Picture file", path);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, "Place Picture").clicked() {
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
                        if widgets::secondary_button(ui, "Cancel").clicked() {
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
