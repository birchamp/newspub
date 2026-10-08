//! Picture captions (IM-10).

use crate::attrs::{CharAttrs, ParaAttrs};
use crate::color::Color;
use crate::command::{CaptionPosition, StyleRef};
use crate::model::{ObjectKind, Owner};
use crate::units::{Length, Rect};
use crate::{Applied, Command, CoreError, Document};

const STYLE_NAME: &str = "Caption";
const SIZE: f64 = 9.0;
const GAP: f64 = 4.0;
const LINE_FACTOR: f64 = 1.4;
const INSET: f64 = 5.76;
/// Conservative average glyph advance as a fraction of the font size, used to estimate wrapping.
const AVG_ADVANCE: f64 = 0.55;

/// Estimated number of wrapped lines for `text` in `width` points.
fn estimate_lines(text: &str, width: f64) -> usize {
    let per_line = ((width / (SIZE * AVG_ADVANCE)).floor() as usize).max(1);
    text.split(['\n', '\r']).map(|l| l.chars().count().div_ceil(per_line).max(1)).sum::<usize>().max(1)
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    let Command::AddCaption { picture, text, position } = cmd else {
        return Err(CoreError::Unsupported(format!("{cmd:?} is not handled by captions")));
    };
    let pic = doc.object(*picture)?;
    if !matches!(pic.kind, ObjectKind::Image(_)) {
        return Err(CoreError::WrongKind(*picture, "picture"));
    }
    let Some(Owner::Page(page)) = doc.owner_of(*picture) else {
        return Err(CoreError::Invalid("a caption needs a picture placed directly on a page".into()));
    };
    let pr = pic.rect;
    let text = text.clone().unwrap_or_else(|| "Caption".to_string());

    let lines = estimate_lines(&text, (pr.w - 2.0 * INSET).max(1.0));
    let h = lines as f64 * SIZE * LINE_FACTOR + 2.0 * INSET;
    let y = match position {
        CaptionPosition::Below => pr.bottom() + GAP,
        CaptionPosition::Above => pr.y - GAP - h,
        CaptionPosition::Overlay => (pr.bottom() - GAP - h).max(pr.y),
    };
    let rect = Rect::new(pr.x, y, pr.w, h);

    // Work on a copy so a failure part-way leaves the document untouched.
    let mut work = doc.clone();
    if work.find_para_style(STYLE_NAME).is_none() {
        let chars = CharAttrs { size: Some(Length(SIZE)), italic: Some(true), ..CharAttrs::default() };
        work.apply(&Command::DefineParaStyle {
            name: STYLE_NAME.into(),
            based_on: None,
            next: None,
            para: ParaAttrs::default(),
            chars,
        })?;
    }
    let created =
        work.apply(&Command::AddTextFrame { page: Some(page), master: None, rect, columns: None, gutter: None })?;
    let frame = *created.created.first().ok_or_else(|| CoreError::Invalid("no frame created".into()))?;
    work.apply(&Command::InsertText { target: frame, at: None, text, attrs: None })?;
    work.apply(&Command::ApplyParaStyle {
        target: frame,
        start: None,
        end: None,
        style: Some(StyleRef::Name(STYLE_NAME.into())),
        clear_overrides: false,
    })?;
    if *position == CaptionPosition::Overlay
        && let ObjectKind::Text(t) = &mut work.object_mut(frame)?.kind
    {
        t.fill = Some(Color::Rgb { r: 255, g: 255, b: 255, a: 0.7 });
    }
    let group = work.apply(&Command::Group { ids: vec![*picture, frame] })?;
    let gid = *group.created.first().ok_or_else(|| CoreError::Invalid("no group created".into()))?;
    *doc = work;
    Ok(Applied { created: vec![gid, frame] })
}
