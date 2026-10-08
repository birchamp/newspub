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
    /// Table-local rect of the (possibly spanning) cell anchored at (row, col).
    pub fn cell_rect(&self, row: usize, col: usize) -> Option<crate::units::Rect> {
        let cell = self.cell(row, col)?;
        let (xs, ys) = (self.col_edges(), self.row_edges());
        let c1 = (col + cell.colspan as usize).min(self.cols());
        let r1 = (row + cell.rowspan as usize).min(self.rows());
        Some(crate::units::Rect::new(xs[col], ys[row], xs[c1] - xs[col], ys[r1] - ys[row]))
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

/// Border assignment for a block of cells: `outer` edges, `inner` edges between the cells, or `all`.
/// A side given as `"none"` removes the border.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BorderSpec {
    pub all: Option<BorderValue>,
    pub outer: Option<BorderValue>,
    pub inner: Option<BorderValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BorderValue {
    /// "none"
    Keyword(String),
    Stroke(Stroke),
}

impl BorderValue {
    fn stroke(&self) -> Option<Stroke> {
        match self {
            BorderValue::Stroke(s) => Some(s.clone()),
            BorderValue::Keyword(_) => None,
        }
    }
}

/// Preset table formats (original designs): (name, header fill, header text colour, band fill, border colour).
pub const FORMATS: &[(&str, &str, &str, &str, &str)] = &[
    ("Banded rows", "#1f3864", "#ffffff", "#dde4f0", "#1f3864"),
    ("Simple grid", "#ffffff", "#000000", "#ffffff", "#000000"),
    ("Harvest", "#7a4a12", "#ffffff", "#f6ead8", "#7a4a12"),
    ("Slate", "#3a4750", "#ffffff", "#eceff1", "#3a4750"),
    ("Plain header", "#e8e8e8", "#000000", "#ffffff", "#9e9e9e"),
];

use crate::model::{Object, ObjectKind};
use crate::story::Story;
use crate::{Applied, Command, CoreError, Document};

fn new_cell(doc: &mut Document) -> Cell {
    let sid = doc.alloc();
    doc.stories.insert(sid, Story::new(sid));
    Cell {
        story: sid,
        rowspan: 1,
        colspan: 1,
        covered: false,
        fill: None,
        borders: default_borders(),
        insets: cell_insets(),
        valign: VAlign::Top,
    }
}

fn table_mut(doc: &mut Document, id: Id) -> Result<&mut Table, CoreError> {
    match &mut doc.object_mut(id)?.kind {
        ObjectKind::Table(t) => Ok(t),
        _ => Err(CoreError::WrongKind(id, "table")),
    }
}

/// Recomputes the object's rect size from the table's columns and rows.
pub(crate) fn sync_rect(obj: &mut Object) {
    if let ObjectKind::Table(t) = &obj.kind {
        obj.rect.w = t.col_widths.iter().map(|w| w.0).sum();
        obj.rect.h = t.row_heights.iter().map(|h| h.0).sum();
    }
}

fn no_spans_cross(t: &Table, rows: std::ops::Range<usize>, cols: std::ops::Range<usize>) -> bool {
    // A block is safe to delete/split around if no merged cell straddles its boundary.
    for r in 0..t.rows() {
        for c in 0..t.cols() {
            let cell = &t.cells[r * t.cols() + c];
            if cell.covered || (cell.rowspan == 1 && cell.colspan == 1) {
                continue;
            }
            let (r1, c1) = (r + cell.rowspan as usize, c + cell.colspan as usize);
            let rin = r < rows.end && r1 > rows.start;
            let cin = c < cols.end && c1 > cols.start;
            let rfull = r >= rows.start && r1 <= rows.end;
            let cfull = c >= cols.start && c1 <= cols.end;
            if rin && cin && !(rfull && cfull) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    use Command::*;
    match cmd {
        AddTable { page, master, rect, rows, cols } => {
            if *rows == 0 || *cols == 0 || *rows > 500 || *cols > 100 {
                return Err(CoreError::Invalid("a table needs 1–500 rows and 1–100 columns".into()));
            }
            let mut cells = vec![];
            let mut created = vec![];
            for _ in 0..rows * cols {
                let c = new_cell(doc);
                created.push(c.story);
                cells.push(c);
            }
            let cw = Length(rect.w / *cols as f64);
            let rh = Length(rect.h / *rows as f64);
            let table = Table {
                col_widths: vec![cw; *cols],
                row_heights: vec![rh; *rows],
                min_row_heights: vec![rh; *rows],
                cells,
                header_rows: 0,
                format: None,
            };
            let obj = doc.new_object(*rect, ObjectKind::Table(table));
            let id = doc.place(*page, *master, obj)?;
            created.insert(0, id);
            Ok(Applied { created })
        }
        InsertTableRows { table, at, count } => {
            let (rows, cols) = {
                let t = table_mut(doc, *table)?;
                (t.rows(), t.cols())
            };
            if *at > rows || *count == 0 {
                return Err(CoreError::Invalid(format!("cannot insert rows at {at} (table has {rows})")));
            }
            let mut new_cells = vec![];
            for _ in 0..count * cols {
                new_cells.push(new_cell(doc));
            }
            let t = table_mut(doc, *table)?;
            if !no_spans_cross(t, *at..*at, 0..cols) && *at > 0 && *at < rows {
                return Err(CoreError::Invalid("cannot insert rows inside merged cells".into()));
            }
            let h = t.row_heights.get(at.saturating_sub(1).min(rows - 1)).copied().unwrap_or(Length(18.0));
            let pos = at * cols;
            t.cells.splice(pos..pos, new_cells);
            for k in 0..*count {
                t.row_heights.insert(at + k, h);
                t.min_row_heights.insert(at + k, h);
            }
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        DeleteTableRows { table, at, count } => {
            let t = table_mut(doc, *table)?;
            let (rows, cols) = (t.rows(), t.cols());
            if *count == 0 || at + count > rows || *count >= rows {
                return Err(CoreError::Invalid("cannot delete those rows (a table keeps at least one row)".into()));
            }
            if !no_spans_cross(t, *at..at + count, 0..cols) {
                return Err(CoreError::Invalid("cannot delete rows that split merged cells".into()));
            }
            let removed: Vec<Id> = t.cells.drain(at * cols..(at + count) * cols).map(|c| c.story).collect();
            t.row_heights.drain(*at..at + count);
            t.min_row_heights.drain(*at..at + count);
            for s in removed {
                doc.stories.remove(&s);
            }
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        InsertTableCols { table, at, count } => {
            let (rows, cols) = {
                let t = table_mut(doc, *table)?;
                (t.rows(), t.cols())
            };
            if *at > cols || *count == 0 {
                return Err(CoreError::Invalid(format!("cannot insert columns at {at} (table has {cols})")));
            }
            let mut new_cells = vec![];
            for _ in 0..count * rows {
                new_cells.push(new_cell(doc));
            }
            let t = table_mut(doc, *table)?;
            if !no_spans_cross(t, 0..rows, *at..*at) && *at > 0 && *at < cols {
                return Err(CoreError::Invalid("cannot insert columns inside merged cells".into()));
            }
            let w = t.col_widths[at.saturating_sub(1).min(cols - 1)];
            let mut it = new_cells.into_iter();
            for r in (0..rows).rev() {
                let pos = r * cols + at;
                let chunk: Vec<Cell> = (0..*count).filter_map(|_| it.next()).collect();
                t.cells.splice(pos..pos, chunk);
            }
            for k in 0..*count {
                t.col_widths.insert(at + k, w);
            }
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        DeleteTableCols { table, at, count } => {
            let t = table_mut(doc, *table)?;
            let (rows, cols) = (t.rows(), t.cols());
            if *count == 0 || at + count > cols || *count >= cols {
                return Err(CoreError::Invalid(
                    "cannot delete those columns (a table keeps at least one column)".into(),
                ));
            }
            if !no_spans_cross(t, 0..rows, *at..at + count) {
                return Err(CoreError::Invalid("cannot delete columns that split merged cells".into()));
            }
            let mut removed = vec![];
            for r in (0..rows).rev() {
                let pos = r * cols + at;
                removed.extend(t.cells.drain(pos..pos + count).map(|c| c.story));
            }
            t.col_widths.drain(*at..at + count);
            for s in removed {
                doc.stories.remove(&s);
            }
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        SetTableColWidth { table, col, width } => {
            let t = table_mut(doc, *table)?;
            if *col >= t.cols() || width.0 <= 0.0 {
                return Err(CoreError::Invalid("bad column or width".into()));
            }
            t.col_widths[*col] = *width;
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        SetTableRowHeight { table, row, height } => {
            let t = table_mut(doc, *table)?;
            if *row >= t.rows() || height.0 <= 0.0 {
                return Err(CoreError::Invalid("bad row or height".into()));
            }
            t.min_row_heights[*row] = *height;
            t.row_heights[*row] = *height;
            sync_rect(doc.object_mut(*table)?);
            Ok(Applied::default())
        }
        MergeTableCells { table, row, col, rows, cols } => {
            let t = table_mut(doc, *table)?;
            if *rows == 0 || *cols == 0 || row + rows > t.rows() || col + cols > t.cols() || rows * cols < 2 {
                return Err(CoreError::Invalid("merge needs a block of at least two cells inside the table".into()));
            }
            if !no_spans_cross(t, *row..row + rows, *col..col + cols) {
                return Err(CoreError::Invalid("the block overlaps another merged cell".into()));
            }
            let w = t.cols();
            let anchor = t.cells[row * w + col].story;
            let mut moved: Vec<Id> = vec![];
            for r in *row..row + rows {
                for c in *col..col + cols {
                    let cell = &mut t.cells[r * w + c];
                    if r == *row && c == *col {
                        cell.rowspan = *rows as u32;
                        cell.colspan = *cols as u32;
                    } else {
                        cell.covered = true;
                        cell.rowspan = 1;
                        cell.colspan = 1;
                        moved.push(cell.story);
                    }
                }
            }
            // Text of the covered cells joins the anchor cell as new paragraphs.
            for s in moved {
                let text = doc.story(s)?.text.clone();
                if text.is_empty() {
                    continue;
                }
                let len = doc.story(anchor)?.len();
                let joined = if len > 0 { format!("\n{text}") } else { text };
                doc.story_mut(anchor)?.insert(len, &joined, None)?;
                let st = doc.story_mut(s)?;
                let n = st.len();
                st.delete(0..n)?;
            }
            Ok(Applied::default())
        }
        SplitTableCell { table, row, col } => {
            let t = table_mut(doc, *table)?;
            let w = t.cols();
            let idx = t.index(*row, *col).ok_or_else(|| CoreError::Invalid("no such cell".into()))?;
            let (rs, cs) = (t.cells[idx].rowspan as usize, t.cells[idx].colspan as usize);
            if t.cells[idx].covered || (rs == 1 && cs == 1) {
                return Err(CoreError::Invalid("the cell is not merged".into()));
            }
            for r in *row..row + rs {
                for c in *col..col + cs {
                    let cell = &mut t.cells[r * w + c];
                    cell.covered = false;
                    cell.rowspan = 1;
                    cell.colspan = 1;
                }
            }
            Ok(Applied::default())
        }
        SetTableCells { table, row, col, rows, cols, fill, borders } => {
            let t = table_mut(doc, *table)?;
            if *rows == 0 || *cols == 0 || row + rows > t.rows() || col + cols > t.cols() {
                return Err(CoreError::Invalid("cell block outside the table".into()));
            }
            let w = t.cols();
            let (r1, c1) = (row + rows, col + cols);
            for r in *row..r1 {
                for c in *col..c1 {
                    let cell = &mut t.cells[r * w + c];
                    if let Some(f) = fill {
                        cell.fill = Some(f.clone());
                    }
                    if let Some(b) = borders {
                        let pick = |outer_edge: bool| -> Option<Option<Option<Stroke>>> {
                            let v = if outer_edge { b.outer.as_ref() } else { b.inner.as_ref() };
                            v.or(b.all.as_ref()).map(|v| Some(v.stroke()))
                        };
                        if let Some(Some(s)) = pick(r == *row) {
                            cell.borders.top = s;
                        }
                        if let Some(Some(s)) = pick(r + 1 == r1) {
                            cell.borders.bottom = s;
                        }
                        if let Some(Some(s)) = pick(c == *col) {
                            cell.borders.left = s;
                        }
                        if let Some(Some(s)) = pick(c + 1 == c1) {
                            cell.borders.right = s;
                        }
                    }
                }
            }
            Ok(Applied::default())
        }
        ApplyTableFormat { table, format } => {
            let (name, head, head_text, band, line) = *FORMATS
                .iter()
                .find(|f| f.0.eq_ignore_ascii_case(format))
                .ok_or_else(|| CoreError::Invalid(format!("no table format {format:?}")))?;
            let parse = |s: &str| Color::parse(s).unwrap_or(Color::BLACK);
            let stroke = Stroke {
                color: parse(line),
                width: Length(1.0),
                dash: crate::model::Dash::Solid,
                cap: crate::model::LineCap::Butt,
                join: crate::model::LineJoin::Miter,
            };
            let t = table_mut(doc, *table)?;
            let w = t.cols();
            let mut header_stories = vec![];
            for (i, cell) in t.cells.iter_mut().enumerate() {
                let r = i / w;
                cell.fill = Some(if r == 0 {
                    parse(head)
                } else if r % 2 == 1 {
                    parse(band)
                } else {
                    Color::WHITE
                });
                cell.borders = CellBorders {
                    top: Some(stroke.clone()),
                    bottom: Some(stroke.clone()),
                    left: Some(stroke.clone()),
                    right: Some(stroke.clone()),
                };
                if r == 0 {
                    header_stories.push(cell.story);
                }
            }
            t.header_rows = 1;
            t.format = Some(name.to_string());
            // Header text: bold in the header colour (applied as the paragraph style's char attrs on the cell stories).
            for s in header_stories {
                let st = doc.story_mut(s)?;
                let attrs =
                    crate::attrs::CharAttrs { bold: Some(true), color: Some(parse(head_text)), ..Default::default() };
                for p in &mut st.paras {
                    p.style = None;
                }
                let n = st.len();
                st.format_chars(0..n, &attrs)?;
                st.typing_attrs = Some(attrs);
            }
            Ok(Applied::default())
        }
        _ => Err(CoreError::Unsupported(format!("{cmd:?} is not a table command"))),
    }
}
