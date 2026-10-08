//! Publication types: business cards, labels, envelopes (PG-11).
//! Owner: Batch 4 task PRODUCTS. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn products_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn products_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
