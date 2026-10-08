//! Paste tab-separated text into a table (TB-05).
//! Owner: Batch 4 task TABLEPASTE. Placeholder until that task lands.

use crate::attrs::CharAttrs;
use crate::model::ObjectKind;
use crate::table::{self, Table};
use crate::{Applied, Command, CoreError, Document, Id};

/// Publisher's limits for one table (the same as `AddTable`).
const MAX_ROWS: usize = 500;
const MAX_COLS: usize = 100;

/// Splits pasted text into rows of cells: CRLF and CR become LF, one trailing LF is dropped,
/// rows split on LF and cells on TAB. Always returns at least one row with one cell.
fn parse(text: &str) -> Vec<Vec<String>> {
    let normal = text.replace("\r\n", "\n").replace('\r', "\n");
    let body = normal.strip_suffix('\n').unwrap_or(&normal);
    body.split('\n').map(|line| line.split('\t').map(str::to_owned).collect()).collect()
}

fn table_of(doc: &Document, id: Id) -> Result<&Table, CoreError> {
    match &doc.object(id)?.kind {
        ObjectKind::Table(t) => Ok(t),
        _ => Err(CoreError::WrongKind(id, "table")),
    }
}

/// Replaces the whole text of a cell story, keeping the formatting of its first char.
fn set_story_text(doc: &mut Document, story: Id, text: &str) -> Result<(), CoreError> {
    let s = doc.story_mut(story)?;
    let attrs = if s.is_empty() {
        None
    } else {
        let mut a: CharAttrs = s.span_attrs_at(0);
        a.field = None;
        Some(a)
    };
    let len = s.len();
    s.delete(0..len)?;
    s.insert(0, text, attrs)
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    let Command::PasteTableText { table, row, col, text } = cmd else {
        return Err(CoreError::Unsupported(format!("{cmd:?} is not a table paste")));
    };
    let grid = parse(text);
    let (rows, cols) = {
        let t = table_of(doc, *table)?;
        (t.rows(), t.cols())
    };
    if *row >= rows || *col >= cols {
        return Err(CoreError::Invalid(format!(
            "cannot paste at row {row}, column {col} (table has {rows} rows and {cols} columns)"
        )));
    }
    let need_rows = row + grid.len();
    let need_cols = col + grid.iter().map(Vec::len).max().unwrap_or(0);
    if need_rows > MAX_ROWS || need_cols > MAX_COLS {
        return Err(CoreError::Invalid("the pasted text would make the table too large".into()));
    }
    // Grow with the table's own insert logic: new columns copy the last width, new rows the last height.
    if need_cols > cols {
        table::apply(doc, &Command::InsertTableCols { table: *table, at: cols, count: need_cols - cols })?;
    }
    if need_rows > rows {
        table::apply(doc, &Command::InsertTableRows { table: *table, at: rows, count: need_rows - rows })?;
    }
    for (dr, cells) in grid.iter().enumerate() {
        for (dc, cell_text) in cells.iter().enumerate() {
            // A covered cell writes into the cell that owns its span.
            let story = {
                let t = table_of(doc, *table)?;
                let (r, c) = t
                    .owner(row + dr, col + dc)
                    .ok_or_else(|| CoreError::Invalid("no cell at the paste position".into()))?;
                t.cell(r, c)
                    .map(|cell| cell.story)
                    .ok_or_else(|| CoreError::Invalid("no cell at the paste position".into()))?
            };
            set_story_text(doc, story, cell_text)?;
        }
    }
    Ok(Applied::default())
}
