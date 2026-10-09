//! The "Choose a template" window: blank publication with a page size, or a built-in template (UI-08).

use crate::{NewpubApp, PRESETS, labeled_field, parse_len};
use newpub_engine::core::Insets;
use newpub_engine::{Query, SessionAction};

/// What the picker window has selected.
#[derive(Clone, Debug, PartialEq)]
pub struct PickerState {
    /// `None` = blank publication, otherwise the built-in template id.
    choice: Option<String>,
    preset: usize,
    width: String,
    height: String,
    /// Built-in templates (id, name, pages), read once when the picker opens.
    templates: Vec<(String, String, u64)>,
}

impl PickerState {
    pub fn new(app: &mut NewpubApp) -> PickerState {
        let templates = app
            .session
            .query(&Query::BuiltinTemplates)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|t| {
                Some((
                    t.get("id")?.as_str()?.to_string(),
                    t.get("name")?.as_str()?.to_string(),
                    t.get("pages").and_then(|p| p.as_u64()).unwrap_or(0),
                ))
            })
            .collect();
        PickerState { choice: None, preset: 0, width: PRESETS[0].1.into(), height: PRESETS[0].2.into(), templates }
    }
}

/// Draws the picker; returns true when it should close.
pub fn show(app: &mut NewpubApp, ctx: &egui::Context, st: &mut PickerState) -> bool {
    let mut close = false;
    egui::Window::new("Choose a template").collapsible(false).show(ctx, |ui| {
        if ui.selectable_label(st.choice.is_none(), "Blank publication").clicked() {
            st.choice = None;
        }
        ui.horizontal(|ui| {
            for (i, (name, w, h)) in PRESETS.iter().enumerate() {
                if ui.selectable_label(st.choice.is_none() && st.preset == i, *name).clicked() {
                    st.choice = None;
                    st.preset = i;
                    if !w.is_empty() {
                        st.width = w.to_string();
                        st.height = h.to_string();
                    }
                }
            }
        });
        labeled_field(ui, "Page width", &mut st.width);
        labeled_field(ui, "Page height", &mut st.height);
        ui.separator();
        for (id, name, pages) in &st.templates {
            let selected = st.choice.as_deref() == Some(id.as_str());
            if ui.selectable_label(selected, name).clicked() {
                st.choice = Some(id.clone());
            }
            ui.weak(format!("{pages} pages"));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if crate::widgets::primary_button(ui, "Create").clicked() {
                close = create(app, st);
            }
            if crate::widgets::secondary_button(ui, "Cancel").clicked() {
                close = true;
            }
        });
    });
    close
}

fn create(app: &mut NewpubApp, st: &PickerState) -> bool {
    let action = match &st.choice {
        Some(id) => SessionAction::NewFromBuiltin { id: id.clone() },
        None => match (parse_len(&st.width), parse_len(&st.height)) {
            (Some(width), Some(height)) => SessionAction::NewDocument {
                width,
                height,
                margins: Some(Insets::uniform(36.0)),
                facing: false,
                pages: 1,
                bleed: None,
            },
            _ => {
                app.status = "Enter a page size such as 8.5in or 210mm".into();
                return false;
            }
        },
    };
    if app.act(action).is_none() {
        return false;
    }
    app.page = 0;
    app.selection.clear();
    app.fit_page();
    true
}
