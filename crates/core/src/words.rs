//! User dictionary commands (SP-02).
//! Owner: Batch 2 task SPELL. Placeholder until that task lands.

use crate::{Applied, Command, CoreError, Document};

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}
