//! Business information sets (BB-05): the user library of sets (lead).
//! Library file: `<library>/business-info.json`, a JSON array of sets (replaced by name).

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::{BusinessInfo, Command};
use serde_json::{Value, json};

impl Session {
    fn bizinfo_file(&self) -> std::path::PathBuf {
        self.library_path().join("business-info.json")
    }

    fn bizinfo_sets(&self) -> Result<Vec<BusinessInfo>, EngineError> {
        let p = self.bizinfo_file();
        if !p.exists() {
            return Ok(vec![]);
        }
        let text = std::fs::read_to_string(&p)?;
        serde_json::from_str(&text).map_err(|e| EngineError::Other(format!("{}: {e}", p.display())))
    }

    pub(crate) fn bizinfo_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::SaveBusinessInfoSet {} => {
                let info = self
                    .doc
                    .business_info
                    .clone()
                    .ok_or_else(|| EngineError::Other("the publication has no business information".into()))?;
                let mut sets = self.bizinfo_sets()?;
                sets.retain(|s| s.name != info.name);
                sets.push(info);
                sets.sort_by_key(|a| a.name.to_lowercase());
                let p = self.bizinfo_file();
                if let Some(dir) = p.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                let tmp = p.with_extension("json.tmp");
                std::fs::write(&tmp, serde_json::to_vec_pretty(&sets).map_err(|e| EngineError::Other(e.to_string()))?)?;
                std::fs::rename(&tmp, &p)?;
                Ok(Outcome::default())
            }
            SessionAction::ApplyBusinessInfoSet { name } => {
                let info = self
                    .bizinfo_sets()?
                    .into_iter()
                    .find(|s| s.name == *name)
                    .ok_or_else(|| EngineError::Other(format!("no business information set {name:?}")))?;
                self.apply_cmd(&Command::SetBusinessInfo { info })?;
                Ok(Outcome::default())
            }
            other => Err(EngineError::Other(format!("{other:?} is not a business information action"))),
        }
    }

    pub(crate) fn bizinfo_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        Ok(match q {
            Query::BusinessInfo => {
                serde_json::to_value(&self.doc.business_info).map_err(|e| EngineError::Other(e.to_string()))?
            }
            Query::BusinessInfoSets => json!(self.bizinfo_sets()?.into_iter().map(|s| s.name).collect::<Vec<_>>()),
            other => return Err(EngineError::Other(format!("query {other:?} is not a business information query"))),
        })
    }
}
