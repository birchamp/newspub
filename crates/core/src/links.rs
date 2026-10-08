//! Hyperlinks, bookmarks, reading order (EX-05, AX-03).
//! Owner: Batch 2 task PDF. Placeholder until that task lands.

use crate::{Applied, Command, CoreError, Document};

pub(crate) fn apply(_doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    Err(CoreError::Unsupported(format!("{cmd:?} is not implemented yet")))
}
