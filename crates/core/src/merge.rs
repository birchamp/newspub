//! Mail merge: substituting records into a publication (MM-01..MM-04, lead-owned).

use crate::field::{FIELD_CHAR, Field};
use crate::model::ObjectKind;
use crate::story::{PARA_SEP, Story};
use crate::{CoreError, Document, Id};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One data-source record: field name → value.
pub type Record = BTreeMap<String, String>;

/// Value of `field` in `rec` (field names match case-insensitively; missing fields are blank).
pub fn value<'a>(rec: &'a Record, field: &str) -> &'a str {
    rec.iter().find(|(k, _)| k.eq_ignore_ascii_case(field)).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// Replaces the merge fields in `st` with `rec`'s values. With `skip_blank`, paragraphs whose
/// merge fields are all blank are removed.
pub fn substitute_story(st: &mut Story, rec: &Record, skip_blank: bool) {
    // Merge-field positions, each with its field name.
    let fields: Vec<(usize, String)> = st
        .text
        .chars()
        .enumerate()
        .filter(|&(_, c)| c == FIELD_CHAR)
        .filter_map(|(i, _)| match st.span_attrs_at(i).field {
            Some(Field::Merge(name)) => Some((i, name)),
            _ => None,
        })
        .collect();
    if fields.is_empty() {
        return;
    }
    let mut dead: Vec<std::ops::Range<usize>> = vec![];
    if skip_blank {
        let ranges = st.para_ranges();
        for (pi, r) in ranges.iter().enumerate() {
            let mut inside = fields.iter().filter(|(i, _)| r.contains(i)).peekable();
            if inside.peek().is_some() && inside.all(|(_, f)| value(rec, f).trim().is_empty()) {
                // Remove the paragraph with one separator (the one before it, or after it for the first).
                let cut = if pi > 0 {
                    r.start - 1..r.end
                } else if ranges.len() > 1 {
                    r.start..r.end + 1
                } else {
                    r.clone()
                };
                dead.push(cut);
            }
        }
    }
    // Work from the end so earlier indices stay valid.
    let mut edits: Vec<(usize, Option<String>)> = fields
        .iter()
        .filter(|(i, _)| !dead.iter().any(|d| d.contains(i)))
        .map(|(i, f)| (*i, Some(value(rec, f).to_string())))
        .collect();
    edits.extend(dead.iter().map(|d| (d.start, None)));
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut dead_iter = dead.iter().rev();
    for (at, v) in edits {
        match v {
            Some(text) => {
                let mut attrs = st.span_attrs_at(at);
                attrs.field = None;
                let _ = st.delete(at..at + 1);
                let _ = st.insert(at, &text.replace("\r\n", "\n").replace(PARA_SEP, " "), Some(attrs));
            }
            None => {
                if let Some(d) = dead_iter.next() {
                    let _ = st.delete(d.clone());
                }
            }
        }
    }
}

impl Document {
    /// Stories and picture frames reachable from objects (group children, tables, shapes included).
    fn reachable(&self, objects: &[Id], stories: &mut BTreeSet<Id>, images: &mut Vec<Id>) {
        for id in objects {
            let Ok(o) = self.object(*id) else { continue };
            match &o.kind {
                ObjectKind::Text(t) => {
                    stories.insert(t.story);
                }
                ObjectKind::Image(_) => images.push(*id),
                ObjectKind::Group { children } => self.reachable(children, stories, images),
                ObjectKind::Table(t) => stories.extend(t.cells.iter().map(|c| c.story)),
                ObjectKind::Shape(s) => stories.extend(s.story),
                #[allow(unreachable_patterns)]
                _ => {}
            }
        }
    }

    /// Applies `rec` to the given stories and picture frames. `pics` maps a picture value
    /// (as written in the data source) to an asset already in the document.
    fn apply_record(
        &mut self,
        stories: &BTreeSet<Id>,
        images: &[Id],
        rec: &Record,
        pics: &HashMap<String, Id>,
        skip_blank: bool,
    ) {
        for sid in stories {
            if let Some(st) = self.stories.get_mut(sid) {
                substitute_story(st, rec, skip_blank);
            }
        }
        for id in images {
            if let Some(o) = self.objects.get_mut(id)
                && let ObjectKind::Image(im) = &mut o.kind
                && let Some(f) = im.merge_field.take()
            {
                im.asset = pics.get(value(rec, &f).trim()).copied();
            }
        }
    }

    /// The publication as it looks for one record (used for preview).
    pub fn merged(&self, rec: &Record, pics: &HashMap<String, Id>) -> Document {
        let mut d = self.clone();
        let skip = d.merge.as_ref().is_some_and(|m| m.skip_blank_lines);
        let stories: BTreeSet<Id> = d.stories.keys().copied().collect();
        let images: Vec<Id> = d.objects.keys().copied().collect();
        d.apply_record(&stories, &images, rec, pics, skip);
        d
    }

    /// A new publication with one copy of every page per record, fields replaced by text.
    /// Master pages are shared and show the first record. The data source is detached.
    pub fn merge_publication(&self, recs: &[Record], pics: &HashMap<String, Id>) -> Result<Document, CoreError> {
        if recs.is_empty() {
            return Err(CoreError::Invalid("the recipient list is empty".into()));
        }
        let mut d = self.clone();
        let skip = d.merge.as_ref().is_some_and(|m| m.skip_blank_lines);
        let n = d.pages.len();
        // Copies 1.. first, while every story still holds its fields.
        let mut copies: Vec<(BTreeSet<Id>, Vec<Id>)> = vec![];
        for _ in 1..recs.len() {
            let mut story_map = HashMap::new();
            let mut tops = vec![];
            for p in 0..n {
                let src = d.pages[p].clone();
                let at = d.pages.len();
                let id = d.alloc();
                d.pages.push(crate::model::Page { id, objects: vec![], ..src.clone() });
                for o in &src.objects {
                    tops.push(d.duplicate_object(*o, at, &mut story_map)?);
                }
            }
            let (mut s, mut i) = (BTreeSet::new(), vec![]);
            d.reachable(&tops, &mut s, &mut i);
            copies.push((s, i));
        }
        // Copy 0: the original pages, plus master-page content.
        let (mut s0, mut i0) = (BTreeSet::new(), vec![]);
        let originals: Vec<Id> = d.pages[..n].iter().flat_map(|p| p.objects.clone()).collect();
        let masters: Vec<Id> = d.masters.iter().flat_map(|m| m.objects.clone()).collect();
        d.reachable(&originals, &mut s0, &mut i0);
        d.reachable(&masters, &mut s0, &mut i0);
        d.apply_record(&s0, &i0, &recs[0], pics, skip);
        for (k, (s, i)) in copies.iter().enumerate() {
            d.apply_record(s, i, &recs[k + 1], pics, skip);
        }
        d.merge = None;
        Ok(d)
    }
}
