//! Selection pane (LY-04): lists the page's objects, selects, renames, hides and shows them.

use crate::NewpubApp;
use newpub_engine::core::{Command, Id, ObjectKind, ObjectPatch, ShapeKind};

/// State of the selection pane.
#[derive(Default)]
pub(crate) struct PaneState {
    pub open: bool,
    /// Rename in progress: the object and the text buffer.
    renaming: Option<(Id, String)>,
}

impl NewpubApp {
    /// The selection pane window, when switched on from the ribbon.
    pub(crate) fn selection_pane(&mut self, ctx: &egui::Context) {
        if !self.pane.open {
            return;
        }
        let rows = self.pane_rows();
        let mut select = None;
        let mut toggle = None;
        let mut start_rename = false;
        let mut commit = None;
        let pos = ctx.content_rect().right_top() + egui::vec2(-560.0, 230.0);
        egui::Window::new("Objects").default_pos(pos).show(ctx, |ui| {
            ui.heading("Objects on this page");
            for (id, label, hidden) in &rows {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.selection.contains(id), label).clicked() {
                        select = Some(*id);
                    }
                    let verb = if *hidden { "Show" } else { "Hide" };
                    if ui.button(format!("{verb} {label}")).clicked() {
                        toggle = Some((*id, !*hidden));
                    }
                });
            }
            if rows.is_empty() {
                ui.label("This page has no objects.");
            }
            ui.separator();
            let has_selection = !self.selection.is_empty();
            if ui.add_enabled(has_selection, egui::Button::new("Rename")).clicked() {
                start_rename = true;
            }
            if let Some((id, buf)) = &mut self.pane.renaming {
                ui.horizontal(|ui| {
                    let l = ui.label("Object name");
                    let r = ui.text_edit_singleline(buf).labelled_by(l.id);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit = Some((*id, buf.clone()));
                    }
                });
            }
        });
        if let Some(id) = select {
            self.selection = vec![id];
        }
        if let Some((id, hidden)) = toggle {
            let patch = ObjectPatch { hidden: Some(hidden), ..Default::default() };
            self.act(Command::SetObject { id, patch });
        }
        if start_rename && let Some(id) = self.selection.first().copied() {
            let current = self.session.doc().objects.get(&id).map(|o| o.name.clone()).unwrap_or_default();
            self.pane.renaming = Some((id, current));
        }
        if let Some((id, name)) = commit {
            self.pane.renaming = None;
            let patch = ObjectPatch { name: Some(name.trim().to_string()), ..Default::default() };
            self.act(Command::SetObject { id, patch });
        }
        // The rename field belongs to the selected object only.
        if let Some((id, _)) = &self.pane.renaming
            && !self.selection.contains(id)
        {
            self.pane.renaming = None;
        }
    }

    /// (id, label, hidden) of the current page's top-level objects, front to back.
    fn pane_rows(&self) -> Vec<(Id, String, bool)> {
        let doc = self.session.doc();
        let Some(page) = doc.pages.get(self.page) else { return vec![] };
        let mut rows = vec![];
        for (i, id) in page.objects.iter().enumerate().rev() {
            let Some(o) = doc.objects.get(id) else { continue };
            let label = if o.name.trim().is_empty() {
                let kind = match &o.kind {
                    ObjectKind::Text(_) => "Text Box",
                    ObjectKind::Image(_) => "Picture",
                    ObjectKind::Shape(s) => match s.kind {
                        ShapeKind::Rect => "Rectangle",
                        ShapeKind::Ellipse => "Ellipse",
                        ShapeKind::Line => "Line",
                        _ => "Shape",
                    },
                    ObjectKind::Table(_) => "Table",
                    ObjectKind::Group { .. } => "Group",
                    ObjectKind::WordArt(_) => "WordArt",
                };
                format!("{kind} {}", i + 1)
            } else {
                o.name.clone()
            };
            rows.push((*id, label, o.hidden));
        }
        rows
    }
}
