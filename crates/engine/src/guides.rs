//! Guides, snapping, units, numeric geometry (GD-01..GD-05).
//! Owner: Batch 2 task GUIDES. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

/// Session state owned by this module.
#[allow(dead_code)] // fields are used once the task lands
#[derive(Default)]
pub struct State {
    /// Snapping enabled? (default true; see Default impl below once implemented)
    pub snapping_off: bool,
    pub units: crate::action::Units,
}

impl Session {
    pub(crate) fn guides_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn guides_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
