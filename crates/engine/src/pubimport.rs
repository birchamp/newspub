//! .pub import (PI-01..PI-04).
//! Owner: Batch 2 task PUB. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{Action, EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

/// Session state owned by this module.
#[allow(dead_code)] // fields are used once the task lands
#[derive(Default)]
pub struct State {
    pub last_report: Option<Value>,
}

impl Session {
    pub(crate) fn pub_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn pub_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
