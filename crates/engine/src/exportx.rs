//! EPUB and XPS export (EX-06).

use crate::{EngineError, Outcome, Session, SessionAction};

impl Session {
    pub(crate) fn exportx_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::ExportEpub { path } => {
                let path = self.resolve(path);
                let layout = self.layout();
                let doc = self.view.as_deref().unwrap_or(&self.doc);
                newpub_io_html::epub::export_epub(doc, &layout, &self.fonts, &path)
                    .map_err(|e| EngineError::Other(e.to_string()))?;
                Ok(Outcome::default())
            }
            SessionAction::ExportXps { path } => {
                let path = self.resolve(path);
                let layout = self.layout();
                let doc = self.view.as_deref().unwrap_or(&self.doc);
                newpub_io_xps::export(doc, &layout, &self.fonts, &path)
                    .map_err(|e| EngineError::Other(e.to_string()))?;
                Ok(Outcome::default())
            }
            other => Err(EngineError::Other(format!("{other:?} is not an EPUB/XPS action"))),
        }
    }
}
