//! The Mailings ribbon tab.

use crate::NewpubApp;

/// State of the Mailings tab's windows.
#[derive(Default)]
pub(crate) struct MailingsState {}

impl NewpubApp {
    pub(crate) fn mailings_tab(&mut self, _ui: &mut egui::Ui) {}

    pub(crate) fn mailings_windows(&mut self, _ctx: &egui::Context) {}
}
