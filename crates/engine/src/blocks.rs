//! Building blocks: user library and built-in blocks (BB-03).
//!
//! The user library is a folder holding `blocks.json` plus `assets/` with the picture bytes (which the
//! document model does not serialize). It is re-read on every use, so separate sessions share it.

mod builtin;

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::Id;
use newpub_core::fragment::Fragment;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Block {
    pub name: String,
    #[serde(default)]
    pub category: String,
    pub fragment: Fragment,
    /// Picture bytes: (asset id, file name inside `assets/`).
    #[serde(default)]
    pub files: Vec<(Id, String)>,
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

fn ext_of(mime: &str) -> &'static str {
    if mime.contains("jpeg") || mime.contains("jpg") { "jpg" } else { "png" }
}

fn read_all(dir: &Path) -> Result<Vec<Block>, EngineError> {
    let file = dir.join("blocks.json");
    if !file.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&file)?;
    let mut blocks: Vec<Block> =
        serde_json::from_str(&text).map_err(|e| EngineError::Other(format!("building block library: {e}")))?;
    for b in &mut blocks {
        for (id, name) in &b.files {
            // Names are written by us; refuse anything that is not a plain file name.
            if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name.as_str()) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(dir.join("assets").join(name))
                && let Some(a) = b.fragment.assets.iter_mut().find(|a| a.id == *id)
            {
                a.bytes = Arc::from(bytes);
            }
        }
    }
    Ok(blocks)
}

fn write_all(dir: &Path, blocks: &[Block]) -> Result<(), EngineError> {
    std::fs::create_dir_all(dir.join("assets"))?;
    for b in blocks {
        for (id, name) in &b.files {
            let path = dir.join("assets").join(name);
            if let Some(a) = b.fragment.assets.iter().find(|a| a.id == *id)
                && !a.bytes.is_empty()
                && !path.exists()
            {
                std::fs::write(&path, &a.bytes[..])?;
            }
        }
    }
    let text = serde_json::to_string_pretty(blocks).map_err(|e| EngineError::Other(e.to_string()))?;
    let tmp = dir.join("blocks.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join("blocks.json"))?;
    Ok(())
}

impl Session {
    pub(crate) fn library_path(&self) -> PathBuf {
        match &self.library_dir {
            Some(p) if p.is_absolute() => p.clone(),
            Some(p) => self.base_dir.join(p),
            None => self.base_dir.join("library"),
        }
    }

    pub(crate) fn blocks_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::SaveBuildingBlock { ids, name, category } => {
                let name = name.trim();
                if name.is_empty() {
                    return Err(EngineError::Other("building block needs a name".into()));
                }
                let fragment = self.doc.extract_fragment(ids)?;
                let files = fragment
                    .assets
                    .iter()
                    .filter(|a| !a.bytes.is_empty())
                    .map(|a| (a.id, format!("{:016x}.{}", fnv(&a.bytes), ext_of(&a.mime))))
                    .collect();
                let dir = self.library_path();
                let mut blocks = read_all(&dir)?;
                blocks.retain(|b| b.name != name);
                blocks.push(Block { name: name.into(), category: category.clone(), fragment, files });
                write_all(&dir, &blocks)?;
                Ok(Outcome::default())
            }
            SessionAction::InsertBuildingBlock { name, page, x, y } => {
                let user = read_all(&self.library_path())?;
                let block = match user.into_iter().find(|b| &b.name == name) {
                    Some(b) => b,
                    None => builtin::builtin_blocks()?
                        .into_iter()
                        .find(|b| &b.name == name)
                        .ok_or_else(|| EngineError::Other(format!("no building block named \"{name}\"")))?,
                };
                let mut d = self.doc.clone();
                let created = d.paste_fragment(&block.fragment, *page, x.pt(), y.pt())?;
                self.commit(d, None);
                Ok(Outcome { created })
            }
            other => Err(EngineError::Other(format!("{other:?} is not a building block action"))),
        }
    }

    /// Copy, cut and paste of objects through the session clipboard.
    pub(crate) fn clipboard_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::CopyObjects { ids } | SessionAction::CutObjects { ids } => {
                let fragment = self.doc.extract_fragment(ids)?;
                let page = ids.iter().find_map(|id| self.doc.page_of(*id)).unwrap_or(0);
                self.clipboard = Some((fragment, page, 0));
                if matches!(a, SessionAction::CutObjects { .. }) {
                    let mut d = self.doc.clone();
                    d.apply(&newpub_core::Command::DeleteObjects { ids: ids.clone() })?;
                    self.commit(d, None);
                }
                Ok(Outcome::default())
            }
            SessionAction::PasteObjects { page, x, y } => {
                let Some((fragment, from, count)) = self.clipboard.as_mut() else {
                    return Err(EngineError::Other("the clipboard is empty".into()));
                };
                let page = page.unwrap_or(*from).min(self.doc.pages.len().saturating_sub(1));
                // In place, cascading by 12 pt per paste so copies do not hide each other.
                *count += 1;
                let step = 12.0 * f64::from(*count);
                let at_x = x.map(|v| v.pt()).unwrap_or(fragment.bounds.x + step);
                let at_y = y.map(|v| v.pt()).unwrap_or(fragment.bounds.y + step);
                let fragment = fragment.clone();
                let mut d = self.doc.clone();
                let created = d.paste_fragment(&fragment, page, at_x, at_y)?;
                self.commit(d, None);
                Ok(Outcome { created })
            }
            other => Err(EngineError::Other(format!("{other:?} is not a clipboard action"))),
        }
    }

    pub(crate) fn blocks_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::BuildingBlocks => {
                let mut user = read_all(&self.library_path())?;
                user.sort_by(|a, b| a.name.cmp(&b.name));
                let list: Vec<Value> = builtin::builtin_blocks()?
                    .iter()
                    .map(|b| json!({"name": b.name, "category": b.category, "user": false}))
                    .chain(user.iter().map(|b| json!({"name": b.name, "category": b.category, "user": true})))
                    .collect();
                Ok(Value::Array(list))
            }
            Query::BuildingBlockLibrary => {
                let dir = self.library_path();
                let n = read_all(&dir)?.len();
                Ok(json!({"path": dir.join("blocks.json").to_string_lossy(), "user_count": n}))
            }
            other => Err(EngineError::Other(format!("query {other:?} is not a building block query"))),
        }
    }
}
