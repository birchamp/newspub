//! Duplicate (Cmd/Ctrl+D): rebuilds the selected objects from existing commands, offset by 12 pt.

use super::NewpubApp;
use newpub_engine::SessionAction;
use newpub_engine::core::{Command, Id, ImagePatch, ObjectKind, ObjectPatch, Rect, ShapePatch, TextFramePatch};

const OFFSET: f64 = 12.0;

impl NewpubApp {
    /// Duplicates the selection as one undo step and selects the copies.
    pub(crate) fn duplicate_selection(&mut self) {
        let ids = self.selection.clone();
        if ids.is_empty() {
            return;
        }
        self.act(SessionAction::BeginGroup);
        let mut copies = vec![];
        for id in ids {
            match self.duplicate_one(id) {
                Some(copy) => copies.push(copy),
                None => {
                    if self.status.is_empty() {
                        self.status = "This object cannot be duplicated".into();
                    }
                }
            }
        }
        self.act(SessionAction::EndGroup);
        if !copies.is_empty() {
            self.selection = copies;
        }
    }

    fn duplicate_one(&mut self, id: Id) -> Option<Id> {
        let obj = self.session.doc().objects.get(&id)?.clone();
        let page = Some(self.session.doc().page_of(id).unwrap_or(self.page));
        let rect = Rect::new(obj.rect.x + OFFSET, obj.rect.y + OFFSET, obj.rect.w, obj.rect.h);
        let new = match &obj.kind {
            ObjectKind::Text(tf) => {
                let new = self
                    .act(Command::AddTextFrame {
                        page,
                        master: None,
                        rect,
                        columns: Some(tf.columns),
                        gutter: Some(tf.gutter),
                    })?
                    .created
                    .first()
                    .copied()?;
                let patch = TextFramePatch {
                    insets: Some(tf.insets),
                    valign: Some(tf.valign),
                    autofit: Some(tf.autofit),
                    fill: tf.fill.clone(),
                    stroke: tf.stroke.clone(),
                    continued_on: Some(tf.continued_on),
                    continued_from: Some(tf.continued_from),
                    vertical: Some(tf.vertical),
                    ..Default::default()
                };
                self.act(Command::SetTextFrame { id: new, patch })?;
                self.copy_story(tf.story, new);
                new
            }
            ObjectKind::Shape(s) => {
                let new = self
                    .act(Command::AddShape {
                        page,
                        master: None,
                        rect,
                        kind: s.kind.clone(),
                        fill: s.fill.clone(),
                        stroke: s.stroke.clone(),
                    })?
                    .created
                    .first()
                    .copied()?;
                let patch = ShapePatch {
                    arrow_start: Some(s.arrow_start),
                    arrow_end: Some(s.arrow_end),
                    gradient: s.gradient.clone(),
                    ..Default::default()
                };
                self.act(Command::SetShape { id: new, patch })?;
                new
            }
            ObjectKind::Image(im) => {
                let new = self
                    .act(Command::AddImage { page, master: None, rect, asset: im.asset })?
                    .created
                    .first()
                    .copied()?;
                let patch = ImagePatch {
                    crop: Some(im.crop),
                    fit: Some(im.fit),
                    stroke: im.stroke.clone(),
                    adjust: Some(im.adjust.clone()),
                    mask: Some(im.mask),
                    soft_edges: Some(im.soft_edges),
                    ..Default::default()
                };
                self.act(Command::SetImage { id: new, patch })?;
                new
            }
            ObjectKind::Group { .. } => {
                self.status = "Groups cannot be duplicated yet".into();
                return None;
            }
        };
        let patch = ObjectPatch {
            rotation: Some(obj.rotation),
            flip_h: Some(obj.flip_h),
            flip_v: Some(obj.flip_v),
            wrap: Some(obj.wrap),
            alt_text: obj.alt_text.clone(),
            decorative: Some(obj.decorative),
            name: Some(obj.name.clone()),
            shadow: obj.shadow.clone(),
            ..Default::default()
        };
        self.act(Command::SetObject { id: new, patch })?;
        Some(new)
    }

    /// Copies a story's text, character runs and paragraph attributes into the story of frame `to`.
    fn copy_story(&mut self, from_story: Id, to: Id) {
        let Ok(story) = self.session.doc().story(from_story).cloned() else { return };
        let chars: Vec<char> = story.text.chars().collect();
        let mut pos = 0;
        for span in &story.chars {
            let end = (pos + span.len).min(chars.len());
            let text: String = chars[pos..end].iter().collect();
            pos += span.len;
            self.act(Command::InsertText { target: to, at: None, text, attrs: Some(span.attrs.clone()) });
        }
        for (range, attrs) in story.para_ranges().into_iter().zip(story.paras.iter()) {
            if *attrs != Default::default() {
                self.act(Command::FormatParas {
                    target: to,
                    start: Some(range.start),
                    end: Some(range.end),
                    attrs: attrs.clone(),
                });
            }
        }
    }
}
