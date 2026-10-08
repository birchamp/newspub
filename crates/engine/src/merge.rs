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
        let mut out = d.merge_publication(&recs, &pics)?;
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
            AttachDataSource { path } => {
                let p = self.resolve(path);
                let bytes = std::fs::read(&p)?;
                let mut rows = parse_csv(&String::from_utf8_lossy(&bytes)).into_iter();
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
                let d = self.merged_publication()?;
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
            _ => Err(EngineError::Other(format!("{a:?} is not a mail-merge action"))),
        }
    }

    pub(crate) fn merge_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::DataSource => Ok(match &self.doc.merge {
                None => Value::Null,
                Some(m) => json!({"fields": m.fields, "records": m.records().len(), "path": m.path,
                                  "preview": self.merge_preview}),
            }),
            _ => Err(EngineError::Other(format!("query {q:?} is not a mail-merge query"))),
        }
    }
}
