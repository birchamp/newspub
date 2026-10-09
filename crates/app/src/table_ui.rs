//! Table editing: the current cell and cell selection, Tab between cells, and the Format panel's Table card.

use eframe::egui::{self, Color32, Rect as ERect};
use newpub_engine::core::{self, Command, Id, Length, ObjectKind};

use crate::{NewpubApp, icons, widgets};

impl NewpubApp {
    /// Table, row and column of the cell holding the caret, while that table is selected.
    pub(crate) fn current_cell(&self) -> Option<(Id, usize, usize)> {
        let c = self.caret?;
        let (t, r, col) = self.cell_table(c.frame)?;
        self.selection.contains(&t).then_some((t, r, col))
    }

    /// The selected block of cells as (table, row, col, rows, cols): the Shift-click extent from the caret's
    /// cell, or just that cell.
    pub(crate) fn cell_selection(&self) -> Option<(Id, usize, usize, usize, usize)> {
        let (t, r, c) = self.current_cell()?;
        match self.view.cell_extent {
            Some((tt, r2, c2)) if tt == t => {
                let (r0, r1) = (r.min(r2), r.max(r2));
                let (c0, c1) = (c.min(c2), c.max(c2));
                Some((t, r0, c0, r1 - r0 + 1, c1 - c0 + 1))
            }
            _ => Some((t, r, c, 1, 1)),
        }
    }

    /// Shift-click or drag onto another cell of the table being edited extends the cell selection.
    /// Returns true when it did.
    pub(crate) fn extend_cell_selection(&mut self, story: Id) -> bool {
        let (Some((t, r, c)), Some((t2, r2, c2))) = (self.current_cell(), self.cell_table(story)) else {
            return false;
        };
        if t != t2 || (r, c) == (r2, c2) {
            return false;
        }
        self.view.cell_extent = Some((t, r2, c2));
        true
    }

    /// Puts the caret at the end of the text of cell (`row`, `col`) of `table`.
    fn enter_cell(&mut self, table: Id, row: usize, col: usize) {
        let Some(story) = self.table_of(table).and_then(|t| t.cell(row, col).map(|c| c.story)) else { return };
        let len = self.session.doc().story(story).map(|s| s.len()).unwrap_or(0);
        self.place_caret(story, len, false);
    }

    fn table_of(&self, table: Id) -> Option<&core::table::Table> {
        match &self.session.doc().objects.get(&table)?.kind {
            ObjectKind::Table(t) => Some(t),
            _ => None,
        }
    }

    /// Tab: the next cell (Shift: the previous one), skipping cells hidden by a merge. Tab in the last cell adds
    /// a row.
    pub(crate) fn tab_cell(&mut self, forward: bool) {
        let Some((table, r, c)) = self.current_cell() else { return };
        let Some(t) = self.table_of(table) else { return };
        let (rows, cols) = (t.rows(), t.cols());
        let open: Vec<(usize, usize)> = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (r, c)))
            .filter(|&(r, c)| t.cell(r, c).is_some_and(|cell| !cell.covered))
            .collect();
        let Some(i) = open.iter().position(|&p| p == (r, c)) else { return };
        if forward {
            if let Some(&(nr, nc)) = open.get(i + 1) {
                self.enter_cell(table, nr, nc);
            } else if self.act(Command::InsertTableRows { table, at: rows, count: 1 }).is_some() {
                self.enter_cell(table, rows, 0);
            }
        } else if i > 0 {
            let (pr, pc) = open[i - 1];
            self.enter_cell(table, pr, pc);
        }
    }

    /// Tints the selected cells while more than one is selected.
    pub(crate) fn draw_cell_selection(&self, painter: &egui::Painter) {
        let Some((table, r, c, rows, cols)) = self.cell_selection() else { return };
        if rows * cols < 2 {
            return;
        }
        let Some(o) = self.session.doc().objects.get(&table) else { return };
        let ObjectKind::Table(t) = &o.kind else { return };
        for rr in r..r + rows {
            for cc in c..c + cols {
                let Some(cr) = t.cell_rect(rr, cc) else { continue };
                let a = self.page_to_screen(o.rect.x + cr.x, o.rect.y + cr.y);
                let b = self.page_to_screen(o.rect.x + cr.right(), o.rect.y + cr.bottom());
                painter.rect_filled(ERect::from_min_max(a, b), 0.0, Color32::from_rgba_unmultiplied(8, 80, 120, 50));
            }
        }
    }

    /// Format panel: the table's rows and columns at the current cell, merging, sizes, style and cell fill.
    pub(crate) fn table_section(&mut self, ui: &mut egui::Ui, table: Id) {
        let Some(t) = self.table_of(table).cloned() else { return };
        let sel = self.cell_selection().filter(|s| s.0 == table);
        let cell = sel.map(|s| (s.1, s.2));
        let on = cell.is_some();
        if !on {
            widgets::hint(ui, "Click in a cell to edit its row and column.");
        }
        let mut cmd = None;
        let mut deleted = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            let (r, c) = cell.unwrap_or((0, 0));
            let rows_left = t.rows() > 1;
            let cols_left = t.cols() > 1;
            for (icon, label, enabled, command) in [
                (icons::ROWS_PLUS_TOP, "Insert Row Above", on, Command::InsertTableRows { table, at: r, count: 1 }),
                (
                    icons::ROWS_PLUS_BOTTOM,
                    "Insert Row Below",
                    on,
                    Command::InsertTableRows { table, at: r + 1, count: 1 },
                ),
                (
                    icons::COLUMNS_PLUS_LEFT,
                    "Insert Column Left",
                    on,
                    Command::InsertTableCols { table, at: c, count: 1 },
                ),
                (
                    icons::COLUMNS_PLUS_RIGHT,
                    "Insert Column Right",
                    on,
                    Command::InsertTableCols { table, at: c + 1, count: 1 },
                ),
                (icons::TRASH, "Delete Row", on && rows_left, Command::DeleteTableRows { table, at: r, count: 1 }),
                (icons::TRASH, "Delete Column", on && cols_left, Command::DeleteTableCols { table, at: c, count: 1 }),
            ] {
                if widgets::small_button(ui, icon, label, false, enabled).clicked() {
                    if matches!(command, Command::DeleteTableRows { .. } | Command::DeleteTableCols { .. }) {
                        deleted = Some((r, c));
                    }
                    cmd = Some(command);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            let block = sel.filter(|s| s.3 * s.4 > 1);
            if widgets::small_button(ui, icons::SQUARES_FOUR, "Merge Cells", false, block.is_some())
                .on_disabled_hover_text("Shift-click another cell to select a block first")
                .clicked()
                && let Some((_, r, c, rows, cols)) = block
            {
                cmd = Some(Command::MergeTableCells { table, row: r, col: c, rows, cols });
            }
            let merged = cell.and_then(|(r, c)| t.cell(r, c)).is_some_and(|x| x.rowspan > 1 || x.colspan > 1);
            if widgets::small_button(ui, icons::SQUARE_SPLIT_HORIZONTAL, "Split Cell", false, merged).clicked()
                && let Some((r, c)) = cell
            {
                cmd = Some(Command::SplitTableCell { table, row: r, col: c });
            }
        });
        if let Some(command) = cmd {
            let merge = matches!(command, Command::MergeTableCells { .. });
            if self.act(command).is_some() {
                self.view.cell_extent = None;
                if merge && let Some((_, r, c, _, _)) = sel {
                    self.enter_cell(table, r, c);
                }
                if let Some((r, c)) = deleted
                    && let Some(t) = self.table_of(table)
                {
                    // The caret's cell is gone: continue in the cell that took its place.
                    let (r, c) = (r.min(t.rows().saturating_sub(1)), c.min(t.cols().saturating_sub(1)));
                    self.enter_cell(table, r, c);
                }
            }
            return;
        }
        // Sizes of the current cell's column and row.
        if let Some((r, c)) = cell {
            let cw = t.col_widths.get(c).map(|l| crate::fmt_len(l.0)).unwrap_or_default();
            let rh = t.min_row_heights.get(r).or(t.row_heights.get(r)).map(|l| crate::fmt_len(l.0)).unwrap_or_default();
            if let Some(v) =
                crate::inspector::length_field(self, ui, "col_width", "Column width", &cw).filter(|v| *v >= 1.0)
            {
                self.act_run(format!("colw:{table}"), Command::SetTableColWidth { table, col: c, width: Length(v) });
            }
            if let Some(v) =
                crate::inspector::length_field(self, ui, "row_height", "Row height", &rh).filter(|v| *v >= 1.0)
            {
                self.act_run(format!("rowh:{table}"), Command::SetTableRowHeight { table, row: r, height: Length(v) });
            }
        }
        let names: Vec<&str> = core::table::FORMATS.iter().map(|f| f.0).collect();
        let current = t.format.clone().unwrap_or_else(|| "None".into());
        if let Some(i) = crate::inspector::choice_row(ui, "Table style", &current, &names) {
            self.act(Command::ApplyTableFormat { table, format: names[i].to_string() });
        }
        if let Some((_, r, c, rows, cols)) = sel {
            let now = t.cell(r, c).and_then(|x| x.fill.clone());
            if let Some(fill) = crate::inspector::color_row(self, ui, "cell_fill", "Cell fill", now.as_ref()) {
                self.act_run(
                    format!("cellfill:{table}"),
                    Command::SetTableCells { table, row: r, col: c, rows, cols, fill: Some(fill), borders: None },
                );
            }
        }
    }
}
