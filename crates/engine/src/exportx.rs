//! EPUB and XPS export (EX-06).
//! Owner: Batch 4 task EXPORT. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn exportx_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
}
