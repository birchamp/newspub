//! The Review ribbon tab.

use crate::NewpubApp;

/// State of the Review tab's windows.
#[derive(Default)]
pub(crate) struct ReviewState {}

impl NewpubApp {
    pub(crate) fn review_tab(&mut self, _ui: &mut egui::Ui) {}

    pub(crate) fn review_windows(&mut self, _ctx: &egui::Context) {}
}
