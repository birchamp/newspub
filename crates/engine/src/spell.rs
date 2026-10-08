//! Spell checking (SP-01..SP-03).
//! Owner: Batch 2 task SPELL. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

/// Session state owned by this module.
#[allow(dead_code)] // fields are used once the task lands
#[derive(Default)]
pub struct State {
    pub ignored: Vec<String>,
}

impl Session {
    pub(crate) fn spell_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn spell_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
