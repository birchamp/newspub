//! Duplicate (Cmd/Ctrl+D): copies the selected objects, offset by 12 pt, as one undo step.

use super::NewpubApp;
use newpub_engine::core::{Command, Length};

const OFFSET: f64 = 12.0;

impl NewpubApp {
    /// Duplicates the selection and selects the copies.
    pub(crate) fn duplicate_selection(&mut self) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        let cmd = Command::DuplicateObjects { ids, dx: Length(OFFSET), dy: Length(OFFSET) };
        if let Some(out) = self.act(cmd) {
            self.selection = out.created;
        }
    }
}
