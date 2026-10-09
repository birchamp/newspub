//! The Design ribbon tab.

use crate::NewpubApp;

/// State of the Design tab's windows.
#[derive(Default)]
pub(crate) struct DesignState {}

impl NewpubApp {
    pub(crate) fn design_tab(&mut self, _ui: &mut egui::Ui) {}

    pub(crate) fn design_windows(&mut self, _ctx: &egui::Context) {}
}
