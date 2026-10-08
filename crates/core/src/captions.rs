//! Picture captions (IM-10).
//! Owner: Batch 4 task OBJECTS. Placeholder until that task lands.

use crate::{Applied, Command, CoreError, Document};

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}
