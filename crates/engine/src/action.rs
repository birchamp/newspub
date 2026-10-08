//! Session-level actions and read-only queries.
//! Interface rule (ARCHITECTURE.md §4): lead-owned; record changes in §11.

use newpub_core::{Command, Id, Insets, Length, Rect};
use newpub_io_pdf::PdfOptions;
use serde::{Deserialize, Serialize};

/// Everything a user (or a journey) can do. Document mutations are [`Command`]s; the rest
/// are session actions. Both share the `cmd` tag namespace.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Action {
    Session(SessionAction),
    Doc(Command),
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(d)?;
        let name = v
            .get("cmd")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| D::Error::custom("action needs a \"cmd\""))?;
        if SessionAction::NAMES.contains(&name.as_str()) {
            serde_json::from_value(v).map(Action::Session).map_err(|e| D::Error::custom(format!("{name}: {e}")))
        } else {
            serde_json::from_value(v).map(Action::Doc).map_err(|e| D::Error::custom(format!("{name}: {e}")))
        }
    }
}

impl From<SessionAction> for Action {
    fn from(s: SessionAction) -> Self {
        Action::Session(s)
    }
}

impl From<Command> for Action {
    fn from(c: Command) -> Self {
        Action::Doc(c)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum SessionAction {
    /// Start a new publication (clears history).
    NewDocument {
        width: Length,
        height: Length,
        #[serde(default)]
        margins: Option<Insets>,
        #[serde(default)]
        facing: bool,
        #[serde(default = "one")]
        pages: usize,
        #[serde(default)]
        bleed: Option<Length>,
    },
    Undo,
    Redo,
    /// Start an undo group: everything until `end_group` undoes as one step.
    BeginGroup,
    EndGroup,
    /// Type text as a user would: consecutive typing at the insertion point coalesces into one undo step.
    TypeText {
        target: Id,
        #[serde(default)]
        at: Option<usize>,
        text: String,
    },
    /// Insert a picture file. Without a size it is placed at its natural size (96 dpi), scaled to fit
    /// the page margins. With only `width` (or `height`) the other side keeps the aspect ratio.
    InsertPicture {
        path: String,
        #[serde(default)]
        page: Option<usize>,
        #[serde(default)]
        x: Option<Length>,
        #[serde(default)]
        y: Option<Length>,
        #[serde(default)]
        width: Option<Length>,
        #[serde(default)]
        height: Option<Length>,
        /// Fill an existing picture frame (placeholder) instead of creating one.
        #[serde(default)]
        into: Option<Id>,
        /// Store a link to the file instead of embedding it.
        #[serde(default)]
        link: bool,
    },
    /// Insert a text file's contents into a story.
    ImportText {
        target: Id,
        path: String,
        #[serde(default)]
        at: Option<usize>,
    },
    /// Flow overflow text: add pages with linked frames (same position as `frame`) until the story fits.
    Autoflow {
        frame: Id,
    },
    Save {
        path: String,
    },
    Open {
        path: String,
    },
    ExportPdf {
        path: String,
        #[serde(default)]
        options: PdfOptions,
    },
    ExportPng {
        path: String,
        page: usize,
        #[serde(default = "dpi")]
        dpi: f64,
    },
}

fn one() -> usize {
    1
}
fn dpi() -> f64 {
    96.0
}

impl SessionAction {
    pub const NAMES: &'static [&'static str] = &[
        "new_document",
        "undo",
        "redo",
        "begin_group",
        "end_group",
        "type_text",
        "insert_picture",
        "import_text",
        "autoflow",
        "save",
        "open",
        "export_pdf",
        "export_png",
    ];
}

/// Read-only questions about the session. Answers are JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "q", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    /// The whole document model as JSON.
    Document,
    PageCount,
    /// `{id, master, ignore_master, background, objects: [ids]}`.
    Page {
        page: usize,
    },
    /// Object JSON (rect, kind, …).
    Object {
        id: Id,
    },
    /// Number of objects on a page (top level).
    ObjectCount {
        page: usize,
    },
    /// Ids of top-level objects on a page, back to front.
    PageObjects {
        page: usize,
    },
    /// Full story text (paragraphs separated by "\n").
    StoryText {
        target: Id,
    },
    StoryLength {
        target: Id,
    },
    /// Ordered frame ids of the story.
    StoryFrames {
        target: Id,
    },
    /// Text displayed in a frame (lines joined with "\n").
    FrameText {
        frame: Id,
    },
    /// Lines in a frame: `[{text, x, y, width, baseline, column, hyphenated}]`.
    FrameLines {
        frame: Id,
    },
    /// Whether the story shown by this frame (or story id) has text that does not fit.
    Overflow {
        target: Id,
    },
    /// First char index that does not fit, or null.
    OverflowAt {
        target: Id,
    },
    /// 0-based page index showing char `at` of the story, or null if it overflows.
    CharPage {
        target: Id,
        at: usize,
    },
    /// Frame id showing char `at`, or null.
    CharFrame {
        target: Id,
        at: usize,
    },
    /// Fully resolved character formatting at char `at`.
    CharAttrs {
        target: Id,
        at: usize,
    },
    /// Fully resolved paragraph formatting of the paragraph containing `at`.
    ParaAttrs {
        target: Id,
        at: usize,
    },
    /// Page index of an object (null when on a master).
    ObjectPage {
        id: Id,
    },
    /// Named styles: `{para: [names], chars: [names]}`.
    Styles,
    /// Masters: `[{id, name, objects}]`.
    Masters,
    /// `{undo, redo}` depths.
    History,
    /// Bounding box of all glyphs in a frame, page coordinates: `{x, y, w, h}` or null.
    TextBounds {
        frame: Id,
    },
}

/// Convenience for building rect values in code.
pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}
