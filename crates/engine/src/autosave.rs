//! Autosave and recovery (FI-03).
//!
//! After every `every` document changes the session writes `autosave-<unix-seconds>.newspub` into the
//! configured directory and deletes older autosave files, so one file is kept. A successful save deletes
//! the autosave files. Recovery opens the newest autosave file as an unsaved (dirty) document.

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::History;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PREFIX: &str = "autosave-";
const EXT: &str = ".newspub";

/// Session state owned by this module.
#[derive(Default)]
pub struct State {
    /// Resolved autosave directory; `None` when autosave is off.
    pub dir: Option<PathBuf>,
    /// Write an autosave after this many document changes.
    pub every: u32,
    /// Document changes since the last autosave.
    pub count: u32,
}

impl Session {
    pub(crate) fn autosave_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        use SessionAction::*;
        match a {
            SetAutosave { dir, every_actions } => {
                if *every_actions == 0 {
                    self.autosave = State::default();
                    return Ok(Outcome::default());
                }
                let dir = self.resolve(dir);
                std::fs::create_dir_all(&dir)?;
                self.autosave = State { dir: Some(dir), every: *every_actions, count: 0 };
                Ok(Outcome::default())
            }
            RecoverAutosave { dir } => {
                let dir = self.resolve(dir);
                let (_, name) =
                    autosave_files(&dir).pop().ok_or_else(|| EngineError::Other("no autosave to recover".into()))?;
                let doc = newpub_io_native::open(&dir.join(name))?;
                self.doc = doc;
                self.history = History::default();
                self.group = None;
                self.path = None;
                self.raster.clear_images();
                self.changed();
                Ok(Outcome::default())
            }
            _ => Err(EngineError::Other(format!("{a:?} is not an autosave action"))),
        }
    }

    pub(crate) fn autosave_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::Autosaves { dir } => {
                let dir = self.resolve(dir);
                let names: Vec<String> = autosave_files(&dir).into_iter().map(|(_, name)| name).collect();
                Ok(json!(names))
            }
            _ => Err(EngineError::Other(format!("query {q:?} is not an autosave query"))),
        }
    }

    /// Called after every document change: writes an autosave when the interval is reached.
    /// Failures are ignored; autosave must never fail an edit.
    pub(crate) fn autosave_tick(&mut self) {
        let Some(dir) = self.autosave.dir.clone() else { return };
        self.autosave.count += 1;
        if self.autosave.count < self.autosave.every {
            return;
        }
        self.autosave.count = 0;
        let name = format!("{PREFIX}{}{EXT}", unix_seconds());
        if newpub_io_native::save(&self.doc, &dir.join(&name)).is_err() {
            return;
        }
        for (_, old) in autosave_files(&dir) {
            if old != name {
                let _ = std::fs::remove_file(dir.join(old));
            }
        }
    }

    /// Deletes the autosave files of the configured directory (after a successful save).
    pub(crate) fn autosave_clear(&mut self) {
        let Some(dir) = self.autosave.dir.clone() else { return };
        for (_, name) in autosave_files(&dir) {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Autosave files in `dir`, oldest first, as `(stamp, file name)`. A missing directory has none.
fn autosave_files(dir: &Path) -> Vec<(u64, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(u64, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let stamp = name.strip_prefix(PREFIX)?.strip_suffix(EXT)?;
            if stamp.is_empty() || !stamp.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some((stamp.parse::<u64>().ok()?, name))
        })
        .collect();
    out.sort();
    out
}
