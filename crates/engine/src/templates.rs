//! Templates: save, new from file, built-ins (BB-01, BB-02).
//! Owner: Batch 2 task TEMPLATES. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn templates_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn templates_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
