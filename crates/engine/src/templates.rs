//! Templates: save, new from file, built-ins (BB-01, BB-02).

mod builtin;

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::{Document, History, ObjectKind};
use serde_json::{Value, json};
use std::collections::BTreeSet;

impl Session {
    pub(crate) fn templates_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::SaveTemplate { path, name, keep_text, keep_images } => {
                let doc = template_copy(&self.doc, name, *keep_text, *keep_images);
                newpub_io_native::save(&doc, &self.resolve(path))?;
                Ok(Outcome::default())
            }
            SessionAction::NewFromTemplate { path } => {
                let doc = newpub_io_native::open(&self.resolve(path))?;
                self.install_fresh(doc);
                Ok(Outcome::default())
            }
            SessionAction::NewFromBuiltin { id } => {
                let doc = builtin::build(id)?;
                self.install_fresh(doc);
                Ok(Outcome::default())
            }
            other => Err(EngineError::Other(format!("{other:?} is not a template action"))),
        }
    }

    pub(crate) fn templates_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::BuiltinTemplates => Ok(Value::Array(
                builtin::LIST.iter().map(|(id, name, pages)| json!({"id": id, "name": name, "pages": pages})).collect(),
            )),
            other => Err(EngineError::Other(format!("query {other:?} is not a template query"))),
        }
    }

    /// Replaces the session's publication with an untitled one: no path, no history, not dirty.
    fn install_fresh(&mut self, mut doc: Document) {
        doc.meta.title.clear();
        self.doc = doc;
        self.history = History::default();
        self.group = None;
        self.typing = None;
        self.path = None;
        self.raster.clear_images();
        self.changed();
        self.dirty = false;
    }
}

/// A copy of `doc` prepared for saving as a template called `name`.
fn template_copy(doc: &Document, name: &str, keep_text: bool, keep_images: bool) -> Document {
    let mut d = doc.clone();
    d.meta.title = name.to_string();
    if !keep_text {
        for story in d.stories.values_mut() {
            let first = story.paras.first().cloned().unwrap_or_default();
            story.text.clear();
            story.chars.clear();
            story.paras = vec![first];
        }
    }
    if !keep_images {
        for o in d.objects.values_mut() {
            if let ObjectKind::Image(img) = &mut o.kind {
                img.asset = None;
            }
        }
    }
    let used: BTreeSet<_> = d
        .objects
        .values()
        .filter_map(|o| match &o.kind {
            ObjectKind::Image(img) => img.asset,
            _ => None,
        })
        .collect();
    d.assets.retain(|id, _| used.contains(id));
    d
}
