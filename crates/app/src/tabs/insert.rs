//! The Insert ribbon tab.

use crate::NewpubApp;

/// State of the Insert tab's windows.
#[derive(Default)]
pub(crate) struct InsertState {}

impl NewpubApp {
    pub(crate) fn insert_tab(&mut self, _ui: &mut egui::Ui) {}

    pub(crate) fn insert_windows(&mut self, _ctx: &egui::Context) {}
}
