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

/// Grid of an n-up layout: (columns, rows) of `w × h` pages on a sheet with `gap` between them.
/// Placeholder until PR-04 lands (Batch 2 task PDF).
pub fn n_up_grid(_sheet_w: f64, _sheet_h: f64, _w: f64, _h: f64, _gap: f64) -> (usize, usize) {
    (0, 0)
}

/// Lays out the selected pages (`indices`, document page indices) onto sheets.
pub fn plan(w: f64, h: f64, indices: &[usize], imposition: &Imposition) -> Result<Vec<Sheet>, PdfError> {
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
        Imposition::NUp { .. } => return Err(PdfError::Unsupported("n-up imposition is not implemented yet".into())),
    })
}
