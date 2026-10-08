//! newpub-core: document model and command layer. No IO, fonts, or UI.

pub mod attrs;
pub mod color;
pub mod command;
pub mod field;
pub mod fragment;
pub mod guides;
pub mod history;
pub mod layers;
pub mod links;
pub mod merge;
pub mod model;
pub mod schemes;
pub mod story;
pub mod table;
pub mod textops;
pub mod units;
pub mod words;

pub use attrs::*;
pub use color::Color;
pub use command::*;
pub use history::History;
pub use model::*;
pub use story::{CharSpan, LINE_SEP, PARA_SEP, Story};
pub use units::{Affine, Insets, Length, Rect};

use serde::{Deserialize, Serialize};

/// Default body font (Carlito is metric-compatible with Publisher's default, Calibri).
pub const DEFAULT_FONT: &str = "Carlito";
pub const DEFAULT_SIZE: f64 = 11.0;

/// Identifier of any document entity (page, master, object, story, style, asset, layer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub u64);

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CoreError {
    #[error("no such page index {0}")]
    NoSuchPage(usize),
    #[error("no such master {0}")]
    NoSuchMaster(Id),
    #[error("no such object {0}")]
    NoSuchObject(Id),
    #[error("no such story {0}")]
    NoSuchStory(Id),
    #[error("no such asset {0}")]
    NoSuchAsset(Id),
    #[error("no such style {0:?}")]
    NoSuchStyle(String),
    #[error("{0} is not a text frame or story")]
    NotText(Id),
    #[error("{0} is not a {1}")]
    WrongKind(Id, &'static str),
    #[error("{0} is locked")]
    Locked(Id),
    #[error("bad text range {start}..{end} (story length {len})")]
    BadRange { start: usize, end: usize, len: usize },
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
