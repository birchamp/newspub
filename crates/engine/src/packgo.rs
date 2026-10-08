//! Pack and Go (PR-09).
//! Owner: Batch 4 task PACKGO. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn packgo_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
}
