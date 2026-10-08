//! Mail merge: data source, preview, recipient filter/sort, merge to PDF / publication (MM-01..MM-04).
//! Lead-owned.

use crate::{EngineError, Outcome, Query, Session, SessionAction, load_picture};
use newpub_core::merge::Record;
use newpub_core::{CoreError, Document, Id, MergeData, MergeFilter, MergeSort, ObjectKind};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Parses CSV text (RFC 4180 quoting; `,` or `;` or tab separated, chosen from the header row).
pub(crate) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let header = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| header.matches(*c).count()).unwrap_or(',');
    let mut rows = vec![];
    let mut row: Vec<String> = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => quoted = true,
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            c if c == sep => row.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    rows
}

/// Rows of a worksheet (default: the first) as text; whole numbers print without a decimal point.
fn read_xlsx(bytes: &[u8], sheet: Option<&str>) -> Result<Vec<Vec<String>>, EngineError> {
    use calamine::{Data, Reader};
    let mut wb = calamine::open_workbook_auto_from_rs(std::io::Cursor::new(bytes.to_vec()))
        .map_err(|e| EngineError::Other(format!("workbook: {e}")))?;
    let names = wb.sheet_names().to_vec();
    let name = match sheet {
        Some(s) => names
            .iter()
            .find(|n| n.eq_ignore_ascii_case(s))
            .cloned()
            .ok_or_else(|| EngineError::Other(format!("the workbook has no sheet {s:?} (sheets: {names:?})")))?,
        None => names.first().cloned().ok_or_else(|| EngineError::Other("the workbook has no sheets".into()))?,
    };
    let range = wb.worksheet_range(&name).map_err(|e| EngineError::Other(format!("sheet {name:?}: {e}")))?;
    let cell = |c: &Data| match c {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => d.to_string(),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e}"),
    };
    let mut rows: Vec<Vec<String>> = range.rows().map(|r| r.iter().map(cell).collect()).collect();
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    // A sheet with only a header still has fields; an empty sheet has none.
    Ok(rows)
}

impl Session {
    fn merge_data(&self) -> Result<&MergeData, EngineError> {
        self.doc.merge.as_ref().ok_or_else(|| EngineError::Other("no data source is attached".into()))
    }

    /// Commits a change to the merge settings.
    fn edit_merge(&mut self, f: impl FnOnce(&mut MergeData)) -> Result<Outcome, EngineError> {
        self.merge_data()?;
        let mut d = self.doc.clone();
        if let Some(m) = d.merge.as_mut() {
            f(m);
        }
        self.commit(d, None);
        Ok(Outcome::default())
    }

    /// Loads every picture named by a picture field into `d`; returns value → asset.
    /// Pictures are relative to the data source's folder; unreadable ones leave the frame empty.
    fn merge_pictures(&self, d: &mut Document) -> HashMap<String, Id> {
        let Some(m) = d.merge.clone() else { return HashMap::new() };
        let fields: BTreeSet<String> = d
            .objects
            .values()
            .filter_map(|o| match &o.kind {
                ObjectKind::Image(im) => im.merge_field.clone(),
                _ => None,
            })
            .collect();
        let dir = Path::new(&m.path).parent().map(Path::to_path_buf).unwrap_or_default();
        let values: BTreeSet<String> = m
            .records()
            .iter()
            .flat_map(|r| fields.iter().map(|f| newpub_core::merge::value(r, f).trim().to_string()))
            .filter(|v| !v.is_empty())
            .collect();
        let mut out = HashMap::new();
        for v in values {
            let p = Path::new(&v);
            let p = if p.is_absolute() { p.to_path_buf() } else { dir.join(p) };
            if let Ok(id) = load_picture(d, &p) {
                out.insert(v, id);
            }
        }
        out
    }

    /// The previewed record, if a preview is on and still valid.
    fn preview_record(&self) -> Option<Record> {
        let i = self.merge_preview?;
        self.doc.merge.as_ref()?.records().into_iter().nth(i)
    }

    /// The document with the previewed record substituted, when previewing.
    pub(crate) fn preview_doc(&self) -> Option<Document> {
        let rec = self.preview_record()?;
        let mut d = self.doc.clone();
        let pics = self.merge_pictures(&mut d);
        Some(d.merged(&rec, &pics))
    }

    /// The merged publication: one copy of the pages per record.
    fn merged_publication(&self) -> Result<Document, EngineError> {
        let recs = self.merge_data()?.records();
        let mut d = self.doc.clone();
        let pics = self.merge_pictures(&mut d);
        let catalog = d.merge.as_ref().is_some_and(|m| m.catalog.is_some());
        let mut out = if catalog { d.merge_catalog(&recs, &pics)? } else { d.merge_publication(&recs, &pics)? };
        // Drop picture assets no record used.
        let used: BTreeSet<Id> = out
            .objects
            .values()
            .filter_map(|o| match &o.kind {
                ObjectKind::Image(im) => im.asset,
                _ => None,
            })
            .collect();
        out.assets.retain(|id, _| used.contains(id) || !pics.values().any(|p| p == id));
        Ok(out)
    }

    fn preview_changed(&mut self) {
        self.layout = None;
        self.view = None;
        self.revision += 1;
    }

    pub(crate) fn merge_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        use SessionAction::*;
        match a {
            AttachDataSource { path, sheet } => {
                let p = self.resolve(path);
                let bytes = std::fs::read(&p)?;
                let rows = if bytes.starts_with(b"PK\x03\x04") {
                    read_xlsx(&bytes, sheet.as_deref())?
                } else {
                    if sheet.is_some() {
                        return Err(EngineError::Other(format!("{path}: only workbooks have sheets")));
                    }
                    parse_csv(&String::from_utf8_lossy(&bytes))
                };
                let mut rows = rows.into_iter();
                let fields: Vec<String> = rows
                    .next()
                    .ok_or_else(|| EngineError::Other(format!("{path}: the data source is empty")))?
                    .into_iter()
                    .map(|f| f.trim().to_string())
                    .collect();
                let mut d = self.doc.clone();
                d.merge = Some(MergeData {
                    path: p.to_string_lossy().to_string(),
                    fields,
                    rows: rows.collect(),
                    filter: None,
                    sort: None,
                    skip_blank_lines: false,
                    catalog: None,
                });
                self.merge_preview = None;
                self.commit(d, None);
                Ok(Outcome::default())
            }
            SetMergePreview { record } => {
                if let Some(i) = record {
                    let n = self.merge_data()?.records().len();
                    if *i >= n {
                        return Err(EngineError::Other(format!("record {i} out of range (0..{n})")));
                    }
                }
                self.merge_preview = *record;
                self.preview_changed();
                Ok(Outcome::default())
            }
            SetMergeFilter { field, op, value } => {
                let filter = match field {
                    None => None,
                    Some(f) => {
                        let m = self.merge_data()?;
                        if !m.fields.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                            return Err(CoreError::Invalid(format!("no field {f:?} in the data source")).into());
                        }
                        Some(MergeFilter {
                            field: f.clone(),
                            op: op.unwrap_or(newpub_core::FilterOp::Equals),
                            value: value.clone().unwrap_or_default(),
                        })
                    }
                };
                self.edit_merge(|m| m.filter = filter)
            }
            SetMergeSort { field, descending } => {
                let sort = field.as_ref().map(|f| MergeSort { field: f.clone(), descending: *descending });
                self.edit_merge(|m| m.sort = sort)
            }
            SetMergeOptions { skip_blank_lines } => {
                let skip = *skip_blank_lines;
                self.edit_merge(|m| {
                    if let Some(v) = skip {
                        m.skip_blank_lines = v;
                    }
                })
            }
            MergeToPdf { path, options } => {
                let mut d = self.merged_publication()?;
                d.resolve_object_colors();
                let layout = newpub_layout::layout_document(&d, &self.fonts);
                let bytes = newpub_io_pdf::export_pdf(&d, &layout, &self.fonts, &options.clone().unwrap_or_default())?;
                std::fs::write(self.resolve(path), bytes)?;
                Ok(Outcome::default())
            }
            MergeToPublication {} => {
                let d = self.merged_publication()?;
                self.merge_preview = None;
                self.raster.clear_images();
                self.commit(d, None);
                Ok(Outcome::default())
            }
            SetCatalogArea { page, rect, across, down, gap } => {
                let pid = self.doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.id;
                if *across == 0 || *down == 0 || rect.w <= 0.0 || rect.h <= 0.0 || gap.0 < 0.0 {
                    return Err(CoreError::Invalid("the catalog area needs a size and at least one cell".into()).into());
                }
                let area = newpub_core::CatalogArea { page: pid, rect: *rect, across: *across, down: *down, gap: *gap };
                self.edit_merge(|m| m.catalog = Some(area))
            }
            ClearCatalogArea {} => self.edit_merge(|m| m.catalog = None),
            _ => Err(EngineError::Other(format!("{a:?} is not a mail-merge action"))),
        }
    }

    pub(crate) fn merge_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::DataSource => Ok(match &self.doc.merge {
                None => Value::Null,
                Some(m) => json!({"fields": m.fields, "records": m.records().len(), "path": m.path,
                                  "preview": self.merge_preview,
                                  "catalog": m.catalog.as_ref().map(|c| json!({
                                      "per_page": c.across * c.down, "across": c.across, "down": c.down})),
                }),
            }),
            _ => Err(EngineError::Other(format!("query {q:?} is not a mail-merge query"))),
        }
    }
}
