//! Built-in starter templates, built in code with core commands. All designs and copy are original.

use crate::EngineError;
use newpub_core::attrs::{Align, CharAttrs, LineSpacing, ParaAttrs};
use newpub_core::{
    Color, Command, CoreError, Document, Id, Insets, Length, ObjectPatch, PageSetup, Rect, ShapeKind, StyleRef,
    TextFramePatch,
};

/// `(id, name, pages)` of every built-in template.
pub(super) const LIST: &[(&str, &str, usize)] = &[
    ("newsletter", "Newsletter", 4),
    ("flyer", "Flyer", 1),
    ("bulletin", "Church bulletin", 4),
    ("booklet", "Folded booklet", 8),
];

pub(super) fn build(id: &str) -> Result<Document, EngineError> {
    match id {
        "newsletter" => Ok(newsletter()?),
        "flyer" => Ok(flyer()?),
        "bulletin" => Ok(bulletin()?),
        "booklet" => Ok(booklet()?),
        _ => Err(EngineError::Other(format!("no built-in template {id:?}"))),
    }
}

type R<T> = Result<T, CoreError>;

const SERIF: &str = "Liberation Serif";
const SANS: &str = "Carlito";

fn navy() -> Color {
    Color::rgb(31, 56, 100)
}

fn ink() -> Color {
    Color::rgb(34, 34, 34)
}

fn pt(v: f64) -> Option<Length> {
    Some(Length(v))
}

/// Thin wrapper that applies commands to a document under construction.
struct B {
    d: Document,
}

impl B {
    fn new(setup: PageSetup, pages: usize) -> B {
        let mut b = B { d: Document::new(setup, pages) };
        b.d.meta.lang = "en-US".into();
        b
    }

    fn cmd(&mut self, c: Command) -> R<Vec<Id>> {
        Ok(self.d.apply(&c)?.created)
    }

    #[allow(clippy::too_many_arguments)]
    fn style(
        &mut self,
        name: &str,
        font: &str,
        size: f64,
        bold: bool,
        italic: bool,
        color: Color,
        para: ParaAttrs,
    ) -> R<()> {
        let chars = CharAttrs {
            font: Some(font.into()),
            size: pt(size),
            bold: Some(bold),
            italic: Some(italic),
            color: Some(color),
            ..Default::default()
        };
        self.cmd(Command::DefineParaStyle { name: name.into(), based_on: None, next: None, para, chars })?;
        Ok(())
    }

    /// The shared set of paragraph styles.
    fn base_styles(&mut self) -> R<()> {
        let spacing = |before: f64, after: f64, ls: f64| ParaAttrs {
            space_before: pt(before),
            space_after: pt(after),
            line_spacing: Some(LineSpacing::Multiple(ls)),
            ..Default::default()
        };
        self.style("Headline", SERIF, 28.0, true, false, navy(), spacing(0.0, 0.0, 1.0))?;
        self.style("Subhead", SANS, 14.0, true, false, navy(), spacing(6.0, 3.0, 1.0))?;
        self.style("Body Text", SANS, 11.0, false, false, ink(), spacing(0.0, 6.0, 1.1))?;
        self.style("Caption", SANS, 9.0, false, true, Color::rgb(68, 68, 68), spacing(0.0, 0.0, 1.0))?;
        self.style("Contact", SANS, 12.0, false, false, ink(), spacing(0.0, 3.0, 1.1))?;
        Ok(())
    }

    /// A text frame with one paragraph per `(style, text)` entry. Returns the frame id.
    fn text(&mut self, page: usize, rect: Rect, paras: &[(&str, &str)]) -> R<Id> {
        let frame =
            self.cmd(Command::AddTextFrame { page: Some(page), master: None, rect, columns: None, gutter: None })?[0];
        self.fill(frame, paras)?;
        Ok(frame)
    }

    /// Writes paragraphs into the (empty) story of `frame` and applies each paragraph's style.
    fn fill(&mut self, frame: Id, paras: &[(&str, &str)]) -> R<()> {
        let text = paras.iter().map(|(_, t)| *t).collect::<Vec<_>>().join("\n");
        self.cmd(Command::InsertText { target: frame, at: None, text, attrs: None })?;
        let mut start = 0;
        for (style, t) in paras {
            let end = start + t.chars().count();
            self.cmd(Command::ApplyParaStyle {
                target: frame,
                start: Some(start),
                end: Some(end),
                style: Some(StyleRef::Name((*style).into())),
                clear_overrides: false,
            })?;
            start = end + 1;
        }
        Ok(())
    }

    fn link(&mut self, from: Id, to: Id) -> R<()> {
        self.cmd(Command::LinkFrames { from, to })?;
        Ok(())
    }

    /// An empty frame meant to be linked into a chain.
    fn empty_frame(&mut self, page: usize, rect: Rect) -> R<Id> {
        Ok(self.cmd(Command::AddTextFrame { page: Some(page), master: None, rect, columns: None, gutter: None })?[0])
    }

    fn decorative(&mut self, id: Id) -> R<()> {
        let patch = ObjectPatch { decorative: Some(true), ..Default::default() };
        self.cmd(Command::SetObject { id, patch })?;
        Ok(())
    }

    /// A decorative filled rectangle on a page (`master` None) or on a master.
    fn bar(&mut self, page: Option<usize>, master: Option<Id>, rect: Rect, fill: Color) -> R<()> {
        let id =
            self.cmd(Command::AddShape { page, master, rect, kind: ShapeKind::Rect, fill: Some(fill), stroke: None })?
                [0];
        self.decorative(id)
    }

    /// An empty picture frame marked decorative, so it passes the accessibility check until a picture is placed.
    fn picture(&mut self, page: usize, rect: Rect) -> R<()> {
        let id = self.cmd(Command::AddImage { page: Some(page), master: None, rect, asset: None })?[0];
        self.decorative(id)
    }

    fn panel(&mut self, frame: Id, fill: Color, insets: f64) -> R<()> {
        let patch = TextFramePatch { fill: Some(fill), insets: Some(Insets::uniform(insets)), ..Default::default() };
        self.cmd(Command::SetTextFrame { id: frame, patch })?;
        Ok(())
    }

    /// A master with a footer rule and a small running line; applied to every page.
    fn footer_master(&mut self, name: &str, rule: Rect, line: Rect, running: &str) -> R<()> {
        let m = self.cmd(Command::AddMaster { name: name.into() })?[0];
        self.bar(None, Some(m), rule, navy())?;
        let frame =
            self.cmd(Command::AddTextFrame { page: None, master: Some(m), rect: line, columns: None, gutter: None })?
                [0];
        self.fill(frame, &[("Caption", running)])?;
        let patch = TextFramePatch { insets: Some(Insets::uniform(0.0)), ..Default::default() };
        self.cmd(Command::SetTextFrame { id: frame, patch })?;
        self.cmd(Command::ApplyMaster { pages: None, master: Some(m) })?;
        Ok(())
    }
}

fn setup(width: f64, height: f64, margins: Insets, facing: bool) -> PageSetup {
    PageSetup { width: Length(width), height: Length(height), margins, facing, bleed: Length(0.0) }
}

fn centered(attrs: &mut ParaAttrs) {
    attrs.align = Some(Align::Center);
}

fn newsletter() -> R<Document> {
    let mut b = B::new(setup(612.0, 792.0, Insets::uniform(36.0), false), 4);
    b.base_styles()?;
    b.footer_master(
        "Newsletter Master",
        Rect::new(36.0, 758.0, 540.0, 1.0),
        Rect::new(36.0, 764.0, 540.0, 14.0),
        "Your organization name  |  Newsletter",
    )?;

    // Page 1: headline and the first two body columns.
    b.text(0, Rect::new(36.0, 36.0, 540.0, 60.0), &[("Headline", "Your Newsletter Title")])?;
    let lead = b.text(
        0,
        Rect::new(36.0, 108.0, 264.0, 648.0),
        &[
            ("Subhead", "Lead story headline"),
            (
                "Body Text",
                "Type your lead story here. Replace this sample copy with your own news, then add pictures and \
                 headings to suit. The text in these columns continues from one column to the next, and on to the \
                 following pages.",
            ),
            (
                "Body Text",
                "Keep paragraphs short so readers can scan the page. Use the Subhead style for section titles and \
                 the Body Text style for ordinary copy.",
            ),
            ("Subhead", "Second story"),
            (
                "Body Text",
                "A second story can start right below the first. Click in this column and begin typing to replace \
                 the sample copy with your own words.",
            ),
        ],
    )?;
    let mut prev = lead;
    let mut chain = vec![(0usize, 312.0, 648.0)];
    chain.extend([(1, 36.0, 648.0), (1, 312.0, 648.0), (2, 36.0, 432.0), (2, 312.0, 432.0)]);
    for (page, x, h) in chain {
        let y = 108.0;
        let f = b.empty_frame(page, Rect::new(x, y, 264.0, h))?;
        b.link(prev, f)?;
        prev = f;
    }

    // Pages 2 and 3 get a section heading above the columns.
    b.text(1, Rect::new(36.0, 56.0, 540.0, 36.0), &[("Subhead", "Inside this issue")])?;
    b.text(2, Rect::new(36.0, 56.0, 540.0, 36.0), &[("Subhead", "More from our community")])?;

    // Page 3: image band below the columns.
    b.picture(2, Rect::new(36.0, 552.0, 540.0, 204.0))?;

    // Page 4: events and contact information.
    b.text(3, Rect::new(36.0, 36.0, 540.0, 52.0), &[("Headline", "Upcoming events")])?;
    b.text(
        3,
        Rect::new(36.0, 96.0, 540.0, 380.0),
        &[
            ("Subhead", "Event title, date and time"),
            ("Body Text", "Describe the event in a sentence or two and say where it will be held."),
            ("Subhead", "Another event"),
            ("Body Text", "Add the details people need to join in, such as the cost and who to ask."),
            ("Subhead", "Volunteer with us"),
            ("Body Text", "Tell readers how they can help and how to sign up."),
        ],
    )?;
    let contact = b.text(
        3,
        Rect::new(36.0, 540.0, 540.0, 190.0),
        &[
            ("Subhead", "Contact us"),
            ("Contact", "Your organization name"),
            ("Contact", "123 Example Street, Anytown, ST 00000"),
            ("Contact", "Phone: (555) 010-0100"),
            ("Contact", "Email: newsletter@example.org"),
        ],
    )?;
    b.panel(contact, Color::rgb(232, 238, 247), 12.0)?;
    Ok(b.d)
}

fn flyer() -> R<Document> {
    let mut b = B::new(setup(612.0, 792.0, Insets::uniform(36.0), false), 1);
    b.base_styles()?;
    let mut centre = ParaAttrs::default();
    centered(&mut centre);
    b.style("Flyer Title", SERIF, 54.0, true, false, navy(), centre.clone())?;
    b.style("Flyer Subtitle", SANS, 22.0, true, false, ink(), centre.clone())?;
    b.style("Flyer Body", SANS, 16.0, false, false, ink(), centre)?;
    let mut centre = ParaAttrs::default();
    centered(&mut centre);
    b.style("Flyer Contact", SANS, 14.0, true, false, navy(), centre)?;

    let m = b.cmd(Command::AddMaster { name: "Flyer Master".into() })?[0];
    b.bar(None, Some(m), Rect::new(36.0, 36.0, 540.0, 12.0), navy())?;
    b.bar(None, Some(m), Rect::new(36.0, 744.0, 540.0, 12.0), navy())?;
    b.cmd(Command::ApplyMaster { pages: None, master: Some(m) })?;

    b.text(0, Rect::new(36.0, 64.0, 540.0, 140.0), &[("Flyer Title", "Your Event Title")])?;
    b.text(0, Rect::new(36.0, 210.0, 540.0, 40.0), &[("Flyer Subtitle", "Saturday, Month 1  |  10 a.m. to 2 p.m.")])?;
    b.picture(0, Rect::new(36.0, 268.0, 540.0, 250.0))?;
    b.text(
        0,
        Rect::new(36.0, 534.0, 540.0, 120.0),
        &[
            ("Flyer Body", "Tell people what is happening and why they should come."),
            ("Flyer Body", "Everyone is welcome. Admission is free."),
        ],
    )?;
    b.text(
        0,
        Rect::new(36.0, 664.0, 540.0, 66.0),
        &[
            ("Flyer Contact", "Community Hall, 123 Example Street"),
            ("Flyer Contact", "Call (555) 010-0100 or write to hello@example.org"),
        ],
    )?;
    Ok(b.d)
}

/// Half-letter pages (5.5 x 8.5 in) for the folded bulletin.
fn bulletin() -> R<Document> {
    let mut b = B::new(setup(396.0, 612.0, Insets::uniform(36.0), false), 4);
    b.base_styles()?;
    b.footer_master(
        "Bulletin Master",
        Rect::new(36.0, 584.0, 324.0, 1.0),
        Rect::new(36.0, 590.0, 324.0, 14.0),
        "Your congregation name  |  Weekly bulletin",
    )?;

    b.text(0, Rect::new(36.0, 36.0, 324.0, 50.0), &[("Headline", "Your Church Name")])?;
    b.text(0, Rect::new(36.0, 90.0, 324.0, 30.0), &[("Subhead", "Sunday worship, 10 a.m.")])?;
    b.picture(0, Rect::new(36.0, 132.0, 324.0, 200.0))?;
    b.text(
        0,
        Rect::new(36.0, 346.0, 324.0, 230.0),
        &[
            ("Subhead", "Welcome"),
            (
                "Body Text",
                "We are glad you are here. Replace this greeting with a short welcome for guests and regulars.",
            ),
            (
                "Body Text",
                "Nursery care is available during the service. Ask an usher if you need help finding a seat.",
            ),
        ],
    )?;

    b.text(1, Rect::new(36.0, 36.0, 324.0, 32.0), &[("Subhead", "Order of service")])?;
    b.text(
        1,
        Rect::new(36.0, 72.0, 324.0, 504.0),
        &[
            ("Body Text", "Welcome and call to worship"),
            ("Body Text", "Opening hymn"),
            ("Body Text", "Prayer of confession"),
            ("Body Text", "Reading from scripture"),
            ("Body Text", "Message"),
            ("Body Text", "Offering"),
            ("Body Text", "Closing hymn and blessing"),
        ],
    )?;

    b.text(2, Rect::new(36.0, 36.0, 324.0, 32.0), &[("Subhead", "Announcements")])?;
    b.text(
        2,
        Rect::new(36.0, 72.0, 324.0, 230.0),
        &[
            ("Body Text", "Add a notice about a meeting, a meal, or a service project, and say when and where."),
            ("Body Text", "Add another notice here. Keep each one to a sentence or two."),
        ],
    )?;
    b.text(2, Rect::new(36.0, 316.0, 324.0, 32.0), &[("Subhead", "Prayer list")])?;
    b.text(
        2,
        Rect::new(36.0, 352.0, 324.0, 224.0),
        &[("Body Text", "List the names or needs your community is holding in prayer this week.")],
    )?;

    b.text(3, Rect::new(36.0, 36.0, 324.0, 32.0), &[("Subhead", "Connect with us")])?;
    let contact = b.text(
        3,
        Rect::new(36.0, 80.0, 324.0, 150.0),
        &[
            ("Contact", "Your Church Name"),
            ("Contact", "123 Example Street, Anytown, ST 00000"),
            ("Contact", "Phone: (555) 010-0100"),
            ("Contact", "Email: office@example.org"),
            ("Contact", "Office hours: weekdays, 9 a.m. to noon"),
        ],
    )?;
    b.panel(contact, Color::rgb(232, 238, 247), 12.0)?;
    Ok(b.d)
}

/// Eight half-letter facing pages for a folded booklet.
fn booklet() -> R<Document> {
    let margins = Insets { top: Length(36.0), bottom: Length(36.0), left: Length(48.0), right: Length(36.0) };
    let mut b = B::new(setup(396.0, 612.0, margins, true), 8);
    b.base_styles()?;
    let content = |d: &Document, page: usize| {
        let (t, bo, l, r) = d.page_margins(page);
        (l, t, d.setup.width.0 - l - r, d.setup.height.0 - t - bo)
    };

    // Cover.
    let (x, _, w, _) = content(&b.d, 0);
    b.text(0, Rect::new(x, 120.0, w, 110.0), &[("Headline", "Your Booklet Title")])?;
    b.text(0, Rect::new(x, 236.0, w, 30.0), &[("Subhead", "A short subtitle or author name")])?;
    b.picture(0, Rect::new(x, 290.0, w, 250.0))?;

    // Inside pages: one story threaded through pages 2 to 7.
    let (x, y, w, h) = content(&b.d, 1);
    let first = b.text(
        1,
        Rect::new(x, y, w, h),
        &[
            ("Subhead", "Welcome"),
            ("Body Text", "Use this booklet to share a program, a guide, or a collection of short pieces. Replace this sample text with your own."),
            ("Subhead", "Part one"),
            ("Body Text", "Each page of this booklet is linked to the next, so text you type here flows forward when a page is full."),
            ("Subhead", "Part two"),
            ("Body Text", "Add headings with the Subhead style and keep ordinary paragraphs in the Body Text style."),
        ],
    )?;
    let mut prev = first;
    for page in 2..7 {
        let (x, y, w, h) = content(&b.d, page);
        let f = b.empty_frame(page, Rect::new(x, y, w, h))?;
        b.link(prev, f)?;
        prev = f;
    }

    // Back cover.
    let (x, _, w, _) = content(&b.d, 7);
    b.text(7, Rect::new(x, 330.0, w, 32.0), &[("Subhead", "Contact")])?;
    let contact = b.text(
        7,
        Rect::new(x, 366.0, w, 130.0),
        &[
            ("Contact", "Your organization name"),
            ("Contact", "123 Example Street, Anytown, ST 00000"),
            ("Contact", "Phone: (555) 010-0100"),
            ("Contact", "Email: info@example.org"),
        ],
    )?;
    b.panel(contact, Color::rgb(232, 238, 247), 12.0)?;

    // Running footer rule on the inside pages' master.
    let m = b.cmd(Command::AddMaster { name: "Booklet Master".into() })?[0];
    b.bar(None, Some(m), Rect::new(48.0, 584.0, 312.0, 1.0), navy())?;
    b.cmd(Command::ApplyMaster { pages: Some((1..7).collect()), master: Some(m) })?;
    Ok(b.d)
}
