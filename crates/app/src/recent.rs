//! Recent files: kept in memory (newest first, at most 8) and optionally persisted as JSON in the OS config dir.

use std::path::{Path, PathBuf};

pub const MAX_RECENT: usize = 8;

#[derive(Clone, Debug, Default)]
pub struct Recent {
    files: Vec<PathBuf>,
    store: Option<PathBuf>,
}

fn config_file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("newpub").join("recent.json"))
}

impl Recent {
    /// The saved theme (true = dark), when the list is persistent and a choice was saved.
    pub fn dark_mode(&self) -> Option<bool> {
        let t = std::fs::read_to_string(self.store.as_ref()?.with_file_name("theme")).ok()?;
        Some(t.trim() == "dark")
    }

    /// Saves the theme choice next to the list (persistent lists only).
    pub fn set_dark_mode(&self, dark: bool) {
        if let Some(store) = &self.store {
            if let Some(dir) = store.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(store.with_file_name("theme"), if dark { "dark" } else { "light" });
        }
    }

    /// Loads (and from now on saves) the list in the OS config dir. Errors are ignored.
    pub fn persistent() -> Recent {
        let store = config_file();
        let files = store
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_file())
            .take(MAX_RECENT)
            .collect();
        Recent { files, store }
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Moves `path` to the front.
    pub fn add(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
        self.files.insert(0, path.to_path_buf());
        self.files.truncate(MAX_RECENT);
        if let Some(store) = &self.store {
            if let Some(dir) = store.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(json) = serde_json::to_string(&self.files) {
                let _ = std::fs::write(store, json);
            }
        }
    }
}
