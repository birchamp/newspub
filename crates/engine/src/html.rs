//! HTML export (EX-04).

use crate::{EngineError, Outcome, Session, SessionAction};

impl Session {
    pub(crate) fn html_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::ExportHtml { path } => {
                let dir = self.resolve(path);
                let layout = self.layout();
                let doc = self.view.as_deref().unwrap_or(&self.doc);
                newpub_io_html::export(doc, &layout, &self.fonts, &dir)
                    .map_err(|e| EngineError::Other(e.to_string()))?;
                Ok(Outcome::default())
            }
            other => Err(EngineError::Other(format!("{other:?} is not an HTML action"))),
        }
    }
}
