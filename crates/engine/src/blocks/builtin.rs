//! Built-in building blocks: original designs assembled from core commands on a scratch document.

use super::Block;
use crate::EngineError;
use newpub_core::{
    Align, CharAttrs, Color, Command, Dash, Document, Id, Insets, Length, PageSetup, ParaAttrs, Rect, ShapeKind,
    Stroke, TextFramePatch,
};

const SERIF: &str = "Liberation Serif";
const SANS: &str = "Carlito";

struct Scratch {
    d: Document,
    roots: Vec<Id>,
}

/// Text appearance for [`Scratch::text`].
struct Look {
    font: &'static str,
    size: f64,
    bold: bool,
    italic: bool,
    color: Color,
    align: Align,
}

impl Look {
    fn new(font: &'static str, size: f64, color: Color) -> Look {
        Look { font, size, bold: false, italic: false, color, align: Align::Left }
    }
    fn bold(mut self) -> Look {
        self.bold = true;
        self
    }
    fn italic(mut self) -> Look {
        self.italic = true;
        self
    }
    fn align(mut self, a: Align) -> Look {
        self.align = a;
        self
    }
}

fn line(color: Color, width: f64, dash: Dash) -> Stroke {
    Stroke { color, width: Length(width), dash, cap: Default::default(), join: Default::default() }
}

impl Scratch {
    fn new() -> Scratch {
        Scratch { d: Document::new(PageSetup::default(), 1), roots: vec![] }
    }

    fn apply(&mut self, c: Command) -> Result<Vec<Id>, EngineError> {
        Ok(self.d.apply(&c)?.created)
    }

    fn first(created: Vec<Id>) -> Result<Id, EngineError> {
        created.first().copied().ok_or_else(|| EngineError::Other("building block: nothing created".into()))
    }

    fn shape(
        &mut self,
        r: [f64; 4],
        kind: ShapeKind,
        fill: Option<Color>,
        stroke: Option<Stroke>,
    ) -> Result<Id, EngineError> {
        let rect = Rect::new(r[0], r[1], r[2], r[3]);
        let id =
            Self::first(self.apply(Command::AddShape { page: Some(0), master: None, rect, kind, fill, stroke })?)?;
        self.roots.push(id);
        Ok(id)
    }

    fn text(
        &mut self,
        r: [f64; 4],
        text: &str,
        look: Look,
        fill: Option<Color>,
        stroke: Option<Stroke>,
        inset: f64,
    ) -> Result<Id, EngineError> {
        let rect = Rect::new(r[0], r[1], r[2], r[3]);
        let id = Self::first(self.apply(Command::AddTextFrame {
            page: Some(0),
            master: None,
            rect,
            columns: None,
            gutter: None,
        })?)?;
        self.apply(Command::SetTextFrame {
            id,
            patch: TextFramePatch { insets: Some(Insets::uniform(inset)), fill, stroke, ..Default::default() },
        })?;
        self.apply(Command::InsertText { target: id, at: None, text: text.to_string(), attrs: None })?;
        self.apply(Command::FormatChars {
            target: id,
            start: None,
            end: None,
            attrs: CharAttrs {
                font: Some(look.font.to_string()),
                size: Some(Length(look.size)),
                bold: Some(look.bold),
                italic: Some(look.italic),
                color: Some(look.color),
                ..Default::default()
            },
        })?;
        self.apply(Command::FormatParas {
            target: id,
            start: None,
            end: None,
            attrs: ParaAttrs { align: Some(look.align), ..Default::default() },
        })?;
        self.roots.push(id);
        Ok(id)
    }

    fn finish(self, name: &str, category: &str) -> Result<Block, EngineError> {
        let fragment = self.d.extract_fragment(&self.roots)?;
        Ok(Block { name: name.into(), category: category.into(), fragment, files: vec![] })
    }
}

pub(super) fn builtin_blocks() -> Result<Vec<Block>, EngineError> {
    let ink = Color::rgb(0x22, 0x2b, 0x36);
    let navy = Color::rgb(0x1f, 0x3a, 0x5f);
    let accent = Color::rgb(0xb5, 0x4a, 0x2a);
    let paper = Color::rgb(0xee, 0xf1, 0xf5);
    let white = Color::rgb(0xff, 0xff, 0xff);
    let mut out = vec![];

    // Pull quote: large italic serif inside a thin frame, with an accent bar on the left.
    let mut s = Scratch::new();
    s.text(
        [0.0, 0.0, 252.0, 112.0],
        "\u{201c}A good page is quiet enough to let the words be heard.\u{201d}",
        Look::new(SERIF, 20.0, navy.clone()).italic(),
        None,
        Some(line(navy.clone(), 0.75, Dash::Solid)),
        12.0,
    )?;
    s.shape([0.0, 0.0, 5.0, 112.0], ShapeKind::Rect, Some(accent.clone()), None)?;
    out.push(s.finish("Pull quote", "Page parts")?);

    // Sidebar: tinted panel with a heading and short body text.
    let mut s = Scratch::new();
    s.shape([0.0, 0.0, 180.0, 220.0], ShapeKind::Rect, Some(paper.clone()), None)?;
    s.shape([0.0, 0.0, 180.0, 4.0], ShapeKind::Rect, Some(navy.clone()), None)?;
    s.text([8.0, 12.0, 164.0, 32.0], "Sidebar heading", Look::new(SANS, 18.0, navy.clone()).bold(), None, None, 4.0)?;
    s.text(
        [8.0, 48.0, 164.0, 164.0],
        "Use this panel for a short related story, a list of dates or a note that stands apart from the main text.",
        Look::new(SANS, 11.0, ink.clone()),
        None,
        None,
        4.0,
    )?;
    out.push(s.finish("Sidebar", "Page parts")?);

    // Contact block.
    let mut s = Scratch::new();
    s.text(
        [0.0, 0.0, 200.0, 110.0],
        "Contact us\nYour Organization\n123 Main Street, Your Town\nPhone 555-0100\nname@example.org",
        Look::new(SANS, 11.0, ink.clone()),
        None,
        Some(line(navy.clone(), 1.0, Dash::Solid)),
        10.0,
    )?;
    out.push(s.finish("Contact block", "Page parts")?);

    // Heading bar: dark band with white heading text.
    let mut s = Scratch::new();
    s.shape([0.0, 0.0, 468.0, 40.0], ShapeKind::Rect, Some(navy.clone()), None)?;
    s.text([8.0, 2.0, 452.0, 36.0], "Section heading", Look::new(SANS, 20.0, white.clone()).bold(), None, None, 3.0)?;
    s.shape([0.0, 40.0, 468.0, 3.0], ShapeKind::Rect, Some(accent.clone()), None)?;
    out.push(s.finish("Heading bar", "Borders & accents")?);

    // Divider: a rule with a star in the middle.
    let mut s = Scratch::new();
    s.shape([0.0, 9.0, 200.0, 0.0], ShapeKind::Line, None, Some(line(navy.clone(), 1.0, Dash::Solid)))?;
    s.shape([194.0, 0.0, 18.0, 18.0], ShapeKind::Star { points: 5, inner: 0.45 }, Some(accent.clone()), None)?;
    s.shape([206.0, 9.0, 200.0, 0.0], ShapeKind::Line, None, Some(line(navy.clone(), 1.0, Dash::Solid)))?;
    out.push(s.finish("Divider", "Borders & accents")?);

    // Coupon: dashed outline with an offer.
    let mut s = Scratch::new();
    s.text(
        [0.0, 0.0, 216.0, 108.0],
        "SPECIAL OFFER\nSave 20% on your next visit\nPresent this coupon. Expires soon.",
        Look::new(SANS, 13.0, ink.clone()).align(Align::Center),
        Some(white.clone()),
        Some(line(accent.clone(), 1.5, Dash::Dash)),
        10.0,
    )?;
    out.push(s.finish("Coupon", "Advertisements")?);

    // Event box: date badge beside the event details.
    let mut s = Scratch::new();
    s.shape([0.0, 0.0, 64.0, 72.0], ShapeKind::RoundRect { radius: Length(6.0) }, Some(accent.clone()), None)?;
    s.text(
        [0.0, 4.0, 64.0, 64.0],
        "Month\n15",
        Look::new(SANS, 18.0, white.clone()).bold().align(Align::Center),
        None,
        None,
        2.0,
    )?;
    s.text(
        [72.0, 0.0, 190.0, 72.0],
        "Event title\nTime and place go here.",
        Look::new(SANS, 12.0, ink),
        Some(paper),
        None,
        8.0,
    )?;
    out.push(s.finish("Event box", "Calendars")?);

    Ok(out)
}
