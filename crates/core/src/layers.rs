//! Layer ordering and deletion (LY-02); also `draw_order`.

use crate::{Applied, Command, CoreError, Document, Id};

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::MoveLayer { layer, to } => {
            let from = doc.layers.iter().position(|l| l.id == *layer).ok_or(CoreError::NoSuchObject(*layer))?;
            let l = doc.layers.remove(from);
            let to = (*to).min(doc.layers.len());
            doc.layers.insert(to, l);
            Ok(Applied::default())
        }
        Command::DeleteLayer { layer } => {
            let pos = doc.layers.iter().position(|l| l.id == *layer).ok_or(CoreError::NoSuchObject(*layer))?;
            doc.layers.remove(pos);
            for o in doc.objects.values_mut() {
                if o.layer == Some(*layer) {
                    o.layer = None;
                }
            }
            Ok(Applied::default())
        }
        other => Err(CoreError::Unsupported(format!("{other:?} is not a layer command"))),
    }
}

impl Document {
    /// Position of an object's layer in `self.layers`, if it is on a layer that exists.
    fn layer_position(&self, id: Id) -> Option<usize> {
        let layer = self.objects.get(&id)?.layer?;
        self.layers.iter().position(|l| l.id == layer)
    }

    /// True when the object is locked itself or sits on a locked layer.
    pub(crate) fn object_locked(&self, id: Id) -> bool {
        let Some(o) = self.objects.get(&id) else {
            return false;
        };
        o.locked || o.layer.and_then(|layer| self.layers.iter().find(|l| l.id == layer)).is_some_and(|l| l.locked)
    }

    /// Objects of page `index` in drawing order (back to front): objects with no layer first, then each
    /// layer bottom to top, each in page z-order. Hidden layers are still listed; the renderer skips them.
    pub fn draw_order(&self, index: usize) -> Vec<Id> {
        let Some(page) = self.pages.get(index) else {
            return Vec::new();
        };
        // Rank 0 is "no layer"; rank i + 1 is the layer at position i. The sort is stable, so page z-order holds within a rank.
        let mut ids = page.objects.clone();
        ids.sort_by_key(|id| self.layer_position(*id).map_or(0, |p| p + 1));
        ids
    }
}
