//! Layer ordering and deletion (LY-02); also `draw_order`.
//! Owner: Batch 2 task LAYERS. Placeholder until that task lands.

use crate::{Applied, Command, CoreError, Document};

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}

impl Document {
    /// Objects of page `index` in drawing order (back to front). Layers will order this (LY-02);
    /// today it is the page's z-order.
    pub fn draw_order(&self, index: usize) -> Vec<crate::Id> {
        self.pages.get(index).map(|p| p.objects.clone()).unwrap_or_default()
    }
}
