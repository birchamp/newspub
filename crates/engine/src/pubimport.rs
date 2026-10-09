//! .pub import (PI-01..PI-04).

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::History;
use serde_json::Value;

/// Session state owned by this module.
#[derive(Default)]
pub struct State {
    pub last_report: Option<Value>,
}

fn pub_err(e: newpub_io_pub::PubError) -> EngineError {
    EngineError::Other(e.to_string())
}

impl Session {
    pub(crate) fn pub_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        let SessionAction::ImportPub { path } = a else {
            return Err(EngineError::Other(format!("{a:?} is not a .pub action")));
        };
        let decode = |bytes: &[u8]| {
            crate::imgformats::decode_picture(bytes).ok().map(|p| newpub_io_pub::Picture {
                mime: p.mime.to_string(),
                bytes: p.bytes,
                px_w: p.px_w,
                px_h: p.px_h,
            })
        };
        let (mut doc, report) = newpub_io_pub::import_with(&self.resolve(path), &decode).map_err(pub_err)?;
        // Derived geometry (table rows that grow to their text, autofit) as after any other action.
        crate::fixups::run(&mut doc, &self.fonts);
        let json = serde_json::to_value(&report).map_err(|e| EngineError::Other(e.to_string()))?;
        let pages = doc.pages.iter().map(|p| p.id).collect();
        self.doc = doc;
        self.history = History::default();
        self.group = None;
        self.path = None;
        self.raster.clear_images();
        self.changed();
        self.dirty = false;
        self.pub_import.last_report = Some(json);
        Ok(Outcome { created: pages })
    }

    pub(crate) fn pub_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::PubReport { path } => {
                let r = newpub_io_pub::inspect(&self.resolve(path)).map_err(pub_err)?;
                serde_json::to_value(&r).map_err(|e| EngineError::Other(e.to_string()))
            }
            Query::ImportReport => Ok(self.pub_import.last_report.clone().unwrap_or(Value::Null)),
            _ => Err(EngineError::Other(format!("query {q:?} is not a .pub query"))),
        }
    }
}
