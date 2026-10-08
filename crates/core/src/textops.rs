//! Fields, sections, baseline grid, special characters, shape text (lead-owned).

use crate::attrs::CharAttrs;
use crate::field::{FIELD_CHAR, Field, Section, format_number};
use crate::model::ObjectKind;
use crate::story::Story;
use crate::{Applied, Command, CoreError, Document, Id, NumberFormat};

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    use Command::*;
    match cmd {
        InsertField { target, at, field } => {
            let sid = doc.story_of(*target)?;
            let st = doc.story_mut(sid)?;
            let at = at.unwrap_or(st.len());
            // The field char takes the formatting of the text before it, plus the field marker.
            let mut attrs = st.span_attrs_at(at.saturating_sub(1));
            attrs.field = Some(field.clone());
            st.insert(at, &FIELD_CHAR.to_string(), Some(attrs))?;
            Ok(Applied::default())
        }
        InsertMergeField { target, at, field } => {
            apply(doc, &InsertField { target: *target, at: *at, field: Field::Merge(field.clone()) })
        }
        SetPictureField { id, field } => {
            match &mut doc.object_mut(*id)?.kind {
                ObjectKind::Image(im) => im.merge_field = field.clone(),
                _ => return Err(CoreError::WrongKind(*id, "picture")),
            }
            Ok(Applied::default())
        }
        SetSection { page, start_at, format } => {
            let pid = doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.id;
            doc.sections.retain(|s| s.page != pid);
            doc.sections.push(Section { page: pid, start_at: *start_at, format: *format });
            Ok(Applied::default())
        }
        RemoveSection { page } => {
            let pid = doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?.id;
            let before = doc.sections.len();
            doc.sections.retain(|s| s.page != pid);
            if doc.sections.len() == before {
                return Err(CoreError::Invalid(format!("page {page} does not start a section")));
            }
            Ok(Applied::default())
        }
        SetBaselineGrid { spacing, offset } => {
            if spacing.0 <= 0.0 {
                return Err(CoreError::Invalid("baseline spacing must be positive".into()));
            }
            doc.baseline_grid = Some(crate::model::BaselineGrid { spacing: *spacing, offset: *offset });
            Ok(Applied::default())
        }
        ClearBaselineGrid => {
            doc.baseline_grid = None;
            Ok(Applied::default())
        }
        InsertSpecialChar { target, at, char } => {
            let sid = doc.story_of(*target)?;
            let st = doc.story_mut(sid)?;
            let at = at.unwrap_or(st.len());
            st.insert(at, &char.char().to_string(), None)?;
            Ok(Applied::default())
        }
        AddShapeText { id } => {
            if let ObjectKind::Shape(s) = &doc.object(*id)?.kind {
                if let Some(existing) = s.story {
                    return Ok(Applied { created: vec![existing] });
                }
            } else {
                return Err(CoreError::WrongKind(*id, "shape"));
            }
            let sid = doc.alloc();
            let mut story = Story::new(sid);
            story.frames.push(*id);
            doc.stories.insert(sid, story);
            if let ObjectKind::Shape(s) = &mut doc.object_mut(*id)?.kind {
                s.story = Some(sid);
            }
            Ok(Applied { created: vec![sid] })
        }
        _ => Err(CoreError::Unsupported(format!("{cmd:?} is not a text operation"))),
    }
}

impl Document {
    /// Section governing page index `i`: (first page index of the section, start number, format).
    fn section_for(&self, i: usize) -> (usize, u32, NumberFormat) {
        let mut best: Option<(usize, u32, NumberFormat)> = None;
        for s in &self.sections {
            if let Some(pi) = self.page_index(s.page)
                && pi <= i
                && best.map(|b| pi >= b.0).unwrap_or(true)
            {
                best = Some((pi, s.start_at, s.format));
            }
        }
        best.unwrap_or((0, 1, NumberFormat::Decimal))
    }

    /// Page label of page index `i`, honouring sections ("1", "ii", …).
    pub fn page_label(&self, i: usize) -> String {
        let (first, start, format) = self.section_for(i);
        format_number(start + (i - first) as u32, format)
    }

    /// Text a field shows on page `page` (None when the frame is not on a page: shows placeholders).
    pub fn field_text(&self, field: &Field, page: Option<usize>) -> String {
        match field {
            Field::PageNumber => page.map(|p| self.page_label(p)).unwrap_or_else(|| "#".into()),
            Field::PageCount => self.pages.len().to_string(),
            Field::SectionPageCount => {
                let Some(p) = page else { return self.pages.len().to_string() };
                let (first, _, _) = self.section_for(p);
                let next = self
                    .sections
                    .iter()
                    .filter_map(|s| self.page_index(s.page))
                    .filter(|&pi| pi > first)
                    .min()
                    .unwrap_or(self.pages.len());
                (next - first).to_string()
            }
            Field::Merge(name) => format!("\u{AB}{name}\u{BB}"),
        }
    }

    /// Does the story contain any field characters?
    pub fn story_has_fields(&self, story: Id) -> bool {
        self.stories.get(&story).map(|s| s.text.contains(FIELD_CHAR)).unwrap_or(false)
    }
}

/// Is `attrs` a field run?
pub fn field_of(attrs: &CharAttrs) -> Option<&Field> {
    attrs.field.as_ref()
}
