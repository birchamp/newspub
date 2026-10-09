//! Ribbon tabs beyond Home and View. Each tab owns its state and its windows.

pub(crate) mod design;
pub(crate) mod insert;
pub(crate) mod mailings;
pub(crate) mod review;

use crate::NewpubApp;

impl NewpubApp {
    /// Windows opened from the ribbon tabs (called every frame).
    pub(crate) fn tab_windows(&mut self, ctx: &egui::Context) {
        self.insert_windows(ctx);
        self.design_windows(ctx);
        self.mailings_windows(ctx);
        self.review_windows(ctx);
    }
}
