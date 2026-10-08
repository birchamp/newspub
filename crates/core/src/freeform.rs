//! Freeform / Bézier shapes and point editing (SH-08).
//! Owner: Batch 4 task FREEFORM. Placeholder until that task lands.

use crate::{Applied, Command, CoreError, Document, Id};
use serde::{Deserialize, Serialize};

/// A path point in page coordinates (query PathNodes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathNodeInfo {
    pub x: f64,
    pub y: f64,
    pub ctrl_in: Option<[f64; 2]>,
    pub ctrl_out: Option<[f64; 2]>,
    pub smooth: bool,
}

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}

impl Document {
    /// Points of a Bézier (or polyline `Path`) shape in page coordinates.
    pub fn path_nodes(&self, id: Id) -> Result<Vec<PathNodeInfo>, CoreError> {
        Err(CoreError::Unsupported(format!("path_nodes({}) is not implemented yet", id.0)))
    }
}
