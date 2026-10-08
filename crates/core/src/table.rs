//! Tables: a grid of cells, each with its own story.
//!
//! Geometry: `col_widths` are exact; `row_heights` are the *current* heights. The engine grows a
//! row (and the table's rect) after every action so the tallest cell's text fits; it never shrinks
//! a row below `min_row_heights`. See ARCHITECTURE.md §4 (engine fixups).

use crate::Id;
use crate::color::Color;
use crate::model::{Stroke, VAlign};
use crate::units::{Insets, Length};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellBorders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<Stroke>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<Stroke>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// The cell's story (its text). Covered cells keep a story so splitting restores them.
    pub story: Id,
    #[serde(default = "one")]
    pub rowspan: u32,
    #[serde(default = "one")]
    pub colspan: u32,
    /// Hidden under another cell's span.
    #[serde(default)]
    pub covered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default)]
    pub borders: CellBorders,
    #[serde(default = "cell_insets")]
    pub insets: Insets,
    #[serde(default)]
    pub valign: VAlign,
}

fn one() -> u32 {
    1
}

/// Publisher's default cell margins: 0.04 in.
pub fn cell_insets() -> Insets {
    Insets::uniform(2.88)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub col_widths: Vec<Length>,
    pub row_heights: Vec<Length>,
    pub min_row_heights: Vec<Length>,
    /// Row-major, `rows × cols` cells.
    pub cells: Vec<Cell>,
    #[serde(default)]
    pub header_rows: u32,
    /// Name of the applied table format, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

impl Table {
    pub fn rows(&self) -> usize {
        self.row_heights.len()
    }
    pub fn cols(&self) -> usize {
        self.col_widths.len()
    }
    pub fn index(&self, row: usize, col: usize) -> Option<usize> {
        (row < self.rows() && col < self.cols()).then_some(row * self.cols() + col)
    }
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.index(row, col).map(|i| &self.cells[i])
    }
    /// The visible cell covering (row, col): itself, or the spanning cell that covers it.
    pub fn owner(&self, row: usize, col: usize) -> Option<(usize, usize)> {
        for r in (0..=row).rev() {
            for c in (0..=col).rev() {
                let cell = self.cell(r, c)?;
                if !cell.covered && r + cell.rowspan as usize > row && c + cell.colspan as usize > col {
                    return Some((r, c));
                }
            }
        }
        None
    }
    /// x offsets of column edges (cols + 1 values, starting at 0).
    pub fn col_edges(&self) -> Vec<f64> {
        let mut v = vec![0.0];
        for w in &self.col_widths {
            v.push(v.last().copied().unwrap_or(0.0) + w.0);
        }
        v
    }
    /// y offsets of row edges (rows + 1 values, starting at 0).
    pub fn row_edges(&self) -> Vec<f64> {
        let mut v = vec![0.0];
        for h in &self.row_heights {
            v.push(v.last().copied().unwrap_or(0.0) + h.0);
        }
        v
    }
}

/// A default 1pt black border on every side.
pub fn default_borders() -> CellBorders {
    let s = Stroke {
        color: Color::BLACK,
        width: Length(1.0),
        dash: crate::model::Dash::Solid,
        cap: crate::model::LineCap::Butt,
        join: crate::model::LineJoin::Miter,
    };
    CellBorders { top: Some(s.clone()), bottom: Some(s.clone()), left: Some(s.clone()), right: Some(s) }
}
