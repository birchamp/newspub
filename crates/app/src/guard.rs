//! Unsaved changes are never lost silently (UI-16): closing the window, New, Open and opening a file first ask
//! "Save changes?" when the publication has unsaved edits. Files dropped on the window open or are placed (UI-17).

use crate::{Dialog, NewpubApp, files, widgets};
use newpub_engine::SessionAction;
use std::path::PathBuf;

/// What to do once the unsaved publication is saved or discarded.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pending {
    Close,
    New,
    OpenDialog,
    OpenFile(PathBuf),
}

/// Files that open as publications when dropped; anything else is tried as a picture.
fn is_publication(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["npub", "newspub", "pub"].contains(&e.to_ascii_lowercase().as_str()))
}

impl NewpubApp {
    /// Runs `p` now, or first asks about unsaved changes.
    pub(crate) fn guard(&mut self, p: Pending) {
        if self.session.dirty {
            self.unsaved = Some(p);
        } else {
            self.proceed(p);
        }
    }

    fn proceed(&mut self, p: Pending) {
        match p {
            Pending::Close => self.close_window = true,
            Pending::New => self.open_picker(),
            Pending::OpenDialog => self.dialog = Dialog::Open { path: String::new() },
            Pending::OpenFile(path) => {
                self.open_file(&path);
            }
        }
    }

    /// Called by the Save dialog: after a successful save the interrupted action goes on; a cancelled save drops it.
    pub(crate) fn save_finished(&mut self, saved: bool) {
        if let Some(p) = self.after_save.take()
            && saved
        {
            self.proceed(p);
        }
    }

    /// The window's close button, and files dropped on the window.
    pub(crate) fn window_events(&mut self, ctx: &egui::Context) {
        // A clean publication lets the window close; otherwise the close waits for the prompt's answer.
        // `ViewportCommand::Close` comes back as a close request: once the user chose to close, it goes through.
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_confirmed {
            if self.session.dirty {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.unsaved = Some(Pending::Close);
            } else {
                self.window_closes += 1;
            }
        }
        let (dropped, pointer) = ctx.input(|i| (i.raw.dropped_files.clone(), i.pointer.hover_pos()));
        for file in dropped {
            let path = file.path().to_path_buf();
            if is_publication(&path) {
                self.guard(Pending::OpenFile(path));
                continue;
            }
            // A picture: its top-left corner goes where it was dropped on the page, else it is centred.
            let at = pointer.filter(|p| self.canvas_rect.contains(*p)).map(|p| self.screen_to_page(p));
            let a = SessionAction::InsertPicture {
                path: path.to_string_lossy().to_string(),
                page: Some(self.page),
                x: at.map(|a| newpub_engine::core::Length(a.0)),
                y: at.map(|a| newpub_engine::core::Length(a.1)),
                width: None,
                height: None,
                into: None,
                link: false,
            };
            if let Some(o) = self.act(a) {
                self.selection = o.created.first().copied().into_iter().collect();
                self.caret = None;
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.status = format!("Placed {name}");
            }
        }
        if std::mem::take(&mut self.close_window) {
            self.close_confirmed = true;
            self.window_closes += 1;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The "Save changes?" prompt.
    pub(crate) fn unsaved_prompt(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.unsaved.clone() else { return };
        let title = self.doc_title();
        let mut choice = None;
        files::dialog_window("Unsaved changes").show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.label(egui::RichText::new(format!("Save changes to {title}?")).font(crate::theme::semibold(15.0)));
            widgets::hint(ui, "Your changes will be lost if you don't save them.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, "Save Changes").clicked() {
                    choice = Some(true);
                }
                if widgets::secondary_button(ui, "Don't Save").clicked() {
                    choice = Some(false);
                }
                if widgets::secondary_button(ui, "Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.unsaved = None;
                }
            });
        });
        match choice {
            Some(false) => {
                self.unsaved = None;
                self.recovery_discard();
                self.proceed(pending);
            }
            Some(true) => {
                self.unsaved = None;
                // A publication saved before is saved in place; an untitled one (or an imported .pub) gets a name.
                let native =
                    self.session.path.clone().filter(|p| !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pub")));
                match native {
                    Some(path) => {
                        if self.act(SessionAction::Save { path: path.to_string_lossy().to_string() }).is_some() {
                            self.proceed(pending);
                        }
                    }
                    None => {
                        self.after_save = Some(pending);
                        self.open_save();
                    }
                }
            }
            None => {}
        }
    }
}
