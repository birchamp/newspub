//! User dictionary commands (SP-02).

use crate::{Applied, Command, CoreError, Document};

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::AddToDictionary { word } => {
            let w = word.trim();
            if !w.is_empty() && !doc.custom_words.iter().any(|c| c == w) {
                doc.custom_words.push(w.to_string());
                doc.custom_words.sort();
            }
            Ok(Applied::default())
        }
        Command::RemoveFromDictionary { word } => {
            let w = word.trim();
            doc.custom_words.retain(|c| c != w);
            Ok(Applied::default())
        }
        other => Err(CoreError::Unsupported(format!("{other:?}"))),
    }
}
