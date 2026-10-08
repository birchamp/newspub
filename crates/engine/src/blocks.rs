//! Building blocks: user library and built-in blocks (BB-03).
//! Owner: Batch 3 task BLOCKS. Placeholder until that task lands.

#![allow(unused_imports)]
use crate::{EngineError, Outcome, Query, Session, SessionAction};
use serde_json::Value;

impl Session {
    pub(crate) fn blocks_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        Err(EngineError::Other(format!("{a:?} is not implemented yet")))
    }
    pub(crate) fn blocks_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Err(EngineError::Other(format!("query {q:?} is not implemented yet")))
    }
}
