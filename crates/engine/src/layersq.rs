//! Layer queries (LY-02).

use crate::{EngineError, Query, Session};
use serde_json::{Value, json};

impl Session {
    pub(crate) fn layers_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::Layers => Ok(json!(
                self.doc
                    .layers
                    .iter()
                    .map(|l| json!({"id": l.id, "name": l.name, "visible": l.visible, "locked": l.locked}))
                    .collect::<Vec<_>>()
            )),
            other => Err(EngineError::Other(format!("query {other:?} is not a layer query"))),
        }
    }
}
