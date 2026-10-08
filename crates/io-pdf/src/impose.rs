//! Imposition: which document page goes where on each output sheet.

use crate::{Imposition, PdfError, booklet_order};

/// One page slot on a sheet: `page` is a document page index (None = blank), `x`/`y` the slot's
/// top-left on the sheet in points.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub page: Option<usize>,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub width: f64,
    pub height: f64,
    pub slots: Vec<Slot>,
}

thread_local! {
    /// Document page index → first PDF page (sheet) index showing it, from the latest `plan` on this thread.
    /// `links.rs` reads it to point link destinations and bookmarks at the right PDF page.
    static PAGE_SHEET: std::cell::RefCell<std::collections::HashMap<usize, usize>> = Default::default();
}

/// PDF page (sheet) index on which document page `page` first appears, per the latest `plan` on this thread.
pub(crate) fn sheet_of(page: usize) -> Option<usize> {
    PAGE_SHEET.with(|m| m.borrow().get(&page).copied())
}

/// Grid of an n-up layout: (columns, rows) of `w × h` pages on a sheet with `gap` between them.
/// `(0, 0)` when not even one page fits (or the sizes are not positive numbers).
pub fn n_up_grid(sheet_w: f64, sheet_h: f64, w: f64, h: f64, gap: f64) -> (usize, usize) {
    const EPS: f64 = 1e-6;
    let fit = |avail: f64, size: f64| {
        let n = ((avail + gap) / (size + gap) + EPS).floor();
        if n.is_finite() && n >= 1.0 { n as usize } else { 0 }
    };
    if !(w > 0.0 && h > 0.0 && gap >= 0.0 && sheet_w > 0.0 && sheet_h > 0.0) {
        return (0, 0);
    }
    (fit(sheet_w, w), fit(sheet_h, h))
}

fn n_up(
    (sw, sh): (f64, f64),
    (w, h): (f64, f64),
    gap: f64,
    repeat: bool,
    indices: &[usize],
) -> Result<Vec<Sheet>, PdfError> {
    let (cols, rows) = n_up_grid(sw, sh, w, h, gap);
    if cols == 0 || rows == 0 {
        return Err(PdfError::Unsupported(format!(
            "a {w:.1} x {h:.1} pt page does not fit on a {sw:.1} x {sh:.1} pt sheet"
        )));
    }
    let per_sheet = cols * rows;
    let x0 = (sw - (cols as f64 * w + (cols - 1) as f64 * gap)) / 2.0;
    let y0 = (sh - (rows as f64 * h + (rows - 1) as f64 * gap)) / 2.0;
    let sheet = |pages: &mut dyn Iterator<Item = Option<usize>>| Sheet {
        width: sw,
        height: sh,
        slots: (0..per_sheet)
            .map(|k| Slot {
                page: pages.next().flatten(),
                x: x0 + (k % cols) as f64 * (w + gap),
                y: y0 + (k / cols) as f64 * (h + gap),
            })
            .collect(),
    };
    Ok(if repeat {
        indices.iter().map(|i| sheet(&mut std::iter::repeat(Some(*i)))).collect()
    } else {
        indices.chunks(per_sheet).map(|c| sheet(&mut c.iter().map(|i| Some(*i)))).collect()
    })
}

/// Lays out the selected pages (`indices`, document page indices) onto sheets.
pub fn plan(
    w: f64,
    h: f64,
    indices: &[usize],
    imposition: &Imposition,
    doc_sheet: Option<&newpub_core::SheetLayout>,
) -> Result<Vec<Sheet>, PdfError> {
    let sheets = plan_sheets(w, h, indices, imposition, doc_sheet)?;
    PAGE_SHEET.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (si, sheet) in sheets.iter().enumerate() {
            for p in sheet.slots.iter().filter_map(|s| s.page) {
                m.entry(p).or_insert(si);
            }
        }
    });
    Ok(sheets)
}

fn plan_sheets(
    w: f64,
    h: f64,
    indices: &[usize],
    imposition: &Imposition,
    _doc_sheet: Option<&newpub_core::SheetLayout>,
) -> Result<Vec<Sheet>, PdfError> {
    Ok(match imposition {
        Imposition::None => indices
            .iter()
            .map(|i| Sheet { width: w, height: h, slots: vec![Slot { page: Some(*i), x: 0.0, y: 0.0 }] })
            .collect(),
        Imposition::Booklet => booklet_order(indices.len())
            .into_iter()
            .map(|side| Sheet {
                width: 2.0 * w,
                height: h,
                slots: side
                    .into_iter()
                    .enumerate()
                    .map(|(k, p)| Slot { page: p.map(|p| indices[p]), x: k as f64 * w, y: 0.0 })
                    .collect(),
            })
            .collect(),
        Imposition::NUp { sheet_width, sheet_height, gap, repeat } => {
            n_up((sheet_width.0, sheet_height.0), (w, h), gap.0, *repeat, indices)?
        }
        // PRODUCTS task (PG-11).
        Imposition::DocumentSheet => {
            return Err(PdfError::Unsupported("document sheet imposition is not implemented yet".into()));
        }
    })
}
