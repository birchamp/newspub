//! Hyperlink annotations and bookmarks (EX-05).

use crate::impose::{Sheet, sheet_of};
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::destination::{Destination, XyzDestination};
use krilla::geom::{Point, Quadrilateral};
use krilla::outline::{Outline, OutlineNode};
use newpub_core::{Affine, Document, Id, Link, ObjectKind};
use newpub_layout::DocLayout;
use newpub_render::display::object_transform;
use std::ops::Range;

/// Where a link goes, resolved against the document.
enum Dest<'a> {
    Url(&'a str),
    /// Document page index.
    Page(usize),
}

fn target(dest: &Dest<'_>) -> Option<Target> {
    Some(match dest {
        Dest::Url(u) => Target::Action(krilla::action::Action::Link(krilla::action::LinkAction::new((*u).to_string()))),
        Dest::Page(p) => {
            Target::Destination(Destination::Xyz(XyzDestination::new(sheet_of(*p)?, Point::from_xy(0.0, 0.0))))
        }
    })
}

/// Link runs of a story: `(char range, link)`, adjacent equal links merged.
fn link_runs(doc: &Document, story: Id) -> Vec<(Range<usize>, &Link)> {
    let mut out: Vec<(Range<usize>, &Link)> = vec![];
    let Some(st) = doc.stories.get(&story) else { return out };
    for (r, a) in st.runs() {
        let Some(l) = &a.link else { continue };
        match out.last_mut() {
            Some((prev, pl)) if prev.end == r.start && *pl == l => prev.end = r.end,
            _ => out.push((r, l)),
        }
    }
    out
}

fn collect_text_frames(doc: &Document, id: Id, parent: Affine, out: &mut Vec<(Id, Affine)>) {
    let Some(o) = doc.objects.get(&id) else { return };
    let t = parent.compose(object_transform(o));
    match &o.kind {
        ObjectKind::Text(_) => out.push((id, t)),
        ObjectKind::Group { children } => {
            // Children are stored in page coordinates; the group's own transform is not applied.
            for c in children {
                collect_text_frames(doc, *c, parent, out);
            }
        }
        _ => {}
    }
}

/// Adds link annotations for document page `page` drawn at `origin` (points, top-left) on this PDF page.
pub fn annotate_page(
    pdf_page: &mut krilla::page::Page,
    doc: &Document,
    layout: &DocLayout,
    page: usize,
    origin: (f64, f64),
) {
    if page >= doc.pages.len() {
        return;
    }
    let mut frames = vec![];
    if let Some(m) = doc.master_for_page(page) {
        for id in &m.objects {
            collect_text_frames(doc, *id, Affine::IDENTITY, &mut frames);
        }
    }
    for id in doc.draw_order(page) {
        collect_text_frames(doc, id, Affine::IDENTITY, &mut frames);
    }
    let to_page = Affine::translate(origin.0, origin.1);
    for (fid, t) in frames {
        let Some(ObjectKind::Text(tf)) = doc.objects.get(&fid).map(|o| &o.kind) else { continue };
        let Some(fl) = layout.frames.get(&fid) else { continue };
        let runs = link_runs(doc, tf.story);
        if runs.is_empty() {
            continue;
        }
        let t = to_page.compose(t);
        for (range, link) in runs {
            let dest = match link {
                Link::Url(u) => Dest::Url(u),
                Link::Page(id) => match doc.pages.iter().position(|pg| pg.id == *id) {
                    Some(i) => Dest::Page(i),
                    None => continue,
                },
            };
            // One quad per line piece, covering the linked glyphs.
            let mut quads = vec![];
            for line in &fl.lines {
                let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
                for run in &line.runs {
                    for g in run.glyphs.iter().filter(|g| !g.generated && range.contains(&g.char_index)) {
                        x0 = x0.min(g.x);
                        x1 = x1.max(g.x + g.advance);
                    }
                }
                if x1 > x0 {
                    let (y0, y1) = (line.top, line.top + line.height);
                    let pt = |x: f64, y: f64| {
                        let (px, py) = t.apply(x, y);
                        Point::from_xy(px as f32, py as f32)
                    };
                    quads.push(Quadrilateral([pt(x0, y1), pt(x1, y1), pt(x1, y0), pt(x0, y0)]));
                }
            }
            if quads.is_empty() {
                continue;
            }
            let Some(tg) = target(&dest) else { continue };
            pdf_page.add_annotation(Annotation::new_link(LinkAnnotation::new_with_quad_points(quads, tg), None));
        }
    }
}

/// Adds the document outline (bookmarks).
pub fn outline(kd: &mut krilla::Document, doc: &Document, _sheets: &[Sheet]) {
    let mut outline = Outline::new();
    let mut any = false;
    for b in &doc.bookmarks {
        let Some(pi) = doc.pages.iter().position(|p| p.id == b.page) else { continue };
        // Pages that were not exported have no destination.
        let Some(sheet) = sheet_of(pi) else { continue };
        outline.push_child(OutlineNode::new(b.title.clone(), XyzDestination::new(sheet, Point::from_xy(0.0, 0.0))));
        any = true;
    }
    if any {
        kd.set_outline(outline);
    }
}
