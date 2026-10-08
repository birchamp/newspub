//! HTML export (EX-04).
//! Owner: Batch 2 task HTML. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn html_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
}
