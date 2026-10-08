//! newpub-io-pub: Microsoft Publisher (.pub) import — research track (ARCHITECTURE.md §9).
//!
//! The parser is our own, written from observation of a sample file and public format notes; see FORMAT-NOTES.md.
//! Supported: container inspection (PI-01), story text (PI-02), text box geometry (PI-03, partly) and font
//! names (PI-04). Pictures and character formatting runs are not decoded yet.

mod container;
mod escher;
mod quill;

use newpub_core::{CharAttrs, Command, Document, Insets, Length, PageSetup, Rect};
use serde::Serialize;
use std::path::Path;

pub use container::Report;

#[derive(Debug, thiserror::Error)]
pub enum PubError {
    #[error("not a Publisher file: {0}")]
    NotPublisher(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Doc(#[from] newpub_core::CoreError),
}

/// What an import did and what it could not decode.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ImportReport {
    pub frames_placed: usize,
    pub fonts: Vec<String>,
    pub warnings: Vec<String>,
}

/// Opens the container and reports what Publisher streams it holds.
pub fn inspect(path: &Path) -> Result<Report, PubError> {
    let mut cf = container::open(path)?;
    container::report(&mut cf)
}

/// US Letter and A4 in points; the page size is not decoded, so the first one that holds the frames wins.
const PAGE_CANDIDATES: [(f64, f64); 2] = [(612.0, 792.0), (595.3, 841.9)];
const EMU_PER_PT: f64 = 12700.0;

/// Text box rectangles in points, relative to the page centre.
fn centred_rects(boxes: &[escher::TextBox]) -> Vec<Rect> {
    boxes
        .iter()
        .map(|b| {
            let (x0, y0, x1, y1) = (f64::from(b.left), f64::from(b.top), f64::from(b.right), f64::from(b.bottom));
            Rect::new(
                x0.min(x1) / EMU_PER_PT,
                y0.min(y1) / EMU_PER_PT,
                (x1 - x0).abs() / EMU_PER_PT,
                (y1 - y0).abs() / EMU_PER_PT,
            )
        })
        .collect()
}

/// Picks the first candidate page that holds every box; returns its size and the boxes in page coordinates.
fn fit_page(rects: &[Rect]) -> Option<((f64, f64), Vec<Rect>)> {
    if rects.is_empty() {
        return None;
    }
    PAGE_CANDIDATES.iter().find_map(|&(w, h)| {
        let placed: Vec<Rect> = rects.iter().map(|r| Rect::new(r.x + w / 2.0, r.y + h / 2.0, r.w, r.h)).collect();
        let inside = placed.iter().all(|r| r.x >= 0.0 && r.y >= 0.0 && r.right() <= w && r.bottom() <= h);
        inside.then_some(((w, h), placed))
    })
}

/// Imports a Publisher file as a new document.
pub fn import(path: &Path) -> Result<(Document, ImportReport), PubError> {
    let mut cf = container::open(path)?;
    let rep = container::report(&mut cf)?;
    let mut report = ImportReport::default();

    let quill = match container::read_stream(&mut cf, "Quill/QuillSub/CONTENTS")? {
        Some(bytes) => quill::parse(&bytes, &mut report.warnings),
        None => {
            report.warnings.push("no Quill text stream; the file has no text".into());
            quill::Quill::default()
        }
    };
    report.fonts = quill.fonts.clone();

    let boxes = match container::read_stream(&mut cf, "Escher/EscherStm")? {
        Some(bytes) => escher::text_boxes(&bytes),
        None => Vec::new(),
    };

    let ((width, height), frame_rect) = match fit_page(&centred_rects(&boxes)) {
        Some((size, placed)) => {
            report.warnings.push(format!(
                "page size is not decoded; assumed {} x {} pt from the text box positions",
                size.0, size.1
            ));
            let (x0, y0) = placed.iter().fold((f64::MAX, f64::MAX), |a, r| (a.0.min(r.x), a.1.min(r.y)));
            let (x1, y1) = placed.iter().fold((0.0f64, 0.0f64), |a, r| (a.0.max(r.right()), a.1.max(r.bottom())));
            if placed.len() > 1 {
                report.warnings.push(format!(
                    "{} text boxes found but the text-to-box mapping is not decoded; the story fills their bounding box",
                    placed.len()
                ));
            }
            (size, Rect::new(x0, y0, x1 - x0, y1 - y0))
        }
        None => {
            let (pw, ph) = PAGE_CANDIDATES[0];
            report.warnings.push(if boxes.is_empty() {
                "no text box geometry found; the story is placed within the page margins".to_string()
            } else {
                "text box geometry did not fit a standard page; the story is placed within the page margins".to_string()
            });
            report.warnings.push(format!("page size is not decoded; assumed {pw} x {ph} pt"));
            ((pw, ph), Rect::new(36.0, 36.0, pw - 72.0, ph - 72.0))
        }
    };

    let setup = PageSetup {
        width: Length(width),
        height: Length(height),
        margins: Insets::uniform(36.0),
        facing: false,
        bleed: Length(0.0),
    };
    let mut doc = Document::new(setup, 1);
    if quill.text.is_empty() {
        report.warnings.push("no text found".into());
    } else {
        let (_frame, story) = doc.create_text_frame(Some(0), None, frame_rect)?;
        let attrs = quill.fonts.first().map(|f| CharAttrs { font: Some(f.clone()), ..Default::default() });
        if attrs.is_none() {
            report.warnings.push("no font table found; the default font is used".into());
        }
        doc.apply(&Command::InsertText { target: story, at: Some(0), text: quill.text, attrs })?;
        report.frames_placed = 1;
    }
    if rep.version.contains("unknown") {
        report.warnings.push(format!("unrecognised Publisher version: {}", rep.version));
    }
    report.warnings.push("character and paragraph formatting runs, pictures and shapes are not imported yet".into());
    Ok((doc, report))
}
