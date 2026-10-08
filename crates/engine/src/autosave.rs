//! Autosave and recovery (FI-03).
//! Owner: Batch 2 task MISC. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

/// Session state owned by this module.
#[allow(dead_code)] // fields are used once the task lands
#[derive(Default)]
pub struct State {
    pub dir: Option<std::path::PathBuf>,
    pub every: u32,
    pub count: u32,
}

impl Session {
    pub(crate) fn autosave_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn autosave_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
    /// Called after every document change. Placeholder: does nothing until FI-03 lands.
    pub(crate) fn autosave_tick(&mut self) {}
}
