//! Display lists: the shared, back-end-independent description of what a page shows.
//! Both the raster back end (tiny-skia) and the PDF back end (krilla) consume these.

use newpub_core::wordart::{Warp, WordArt};
use newpub_core::*;
use newpub_layout::{DocLayout, FontStore, GlyphRun};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathEl {
    Move(f64, f64),
    Line(f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f64,
    pub dash: Vec<f64>,
    pub cap: LineCap,
    pub join: LineJoin,
}

/// A resolved gradient: geometry in the object's local coordinates, stops sorted by position.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientPaint {
    pub radial: bool,
    /// Linear: start point. Radial: centre.
    pub from: (f64, f64),
    /// Linear: end point. Unused for radial.
    pub to: (f64, f64),
    /// Radial: radius.
    pub radius: f64,
    pub stops: Vec<(f64, Color)>,
}

/// Resolves `g` for a local box of size `w` x `h`. `None` when the gradient has no stops.
pub fn gradient_paint(g: &Gradient, w: f64, h: f64) -> Option<GradientPaint> {
    let mut stops: Vec<(f64, Color)> = g.stops.iter().map(|s| (s.at.clamp(0.0, 1.0), s.color.clone())).collect();
    if stops.is_empty() {
        return None;
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (cx, cy) = (w / 2.0, h / 2.0);
    let radial = g.kind == GradientKind::Radial;
    let a = g.angle.to_radians();
    let (dx, dy) = (a.cos(), a.sin());
    let half = (w * dx.abs() + h * dy.abs()) / 2.0;
    Some(GradientPaint {
        radial,
        from: if radial { (cx, cy) } else { (cx - dx * half, cy - dy * half) },
        to: (cx + dx * half, cy + dy * half),
        radius: (w.hypot(h) / 2.0).max(1e-6),
        stops,
    })
}

#[derive(Clone, Debug)]
pub enum Item {
    /// A path in local coordinates, mapped to the page by `transform`.
    Path { path: Vec<PathEl>, fill: Option<Color>, stroke: Option<StrokeStyle>, transform: Affine },
    /// An image. `local` maps image pixel space (0..px_w, 0..px_h) into the object's local box;
    /// drawing is clipped to `clip` (local coordinates); `transform` maps local → page.
    /// `mask` further clips the drawing (local coordinates); `opacity` scales it; `adjust` recolours the pixels.
    Image {
        asset: Id,
        local: Affine,
        clip: Rect,
        mask: Option<Vec<PathEl>>,
        opacity: f64,
        adjust: ImageAdjust,
        transform: Affine,
    },
    /// A path filled with a gradient (local coordinates). Any stroke is drawn by a separate `Path` item.
    GradPath { path: Vec<PathEl>, paint: GradientPaint, transform: Affine },
    /// Glyphs positioned in frame-local coordinates, mapped to the page by `transform`.
    Glyphs { run: GlyphRun, transform: Affine },
    /// Structure marker for tagged PDF (AX-03); renderers that do not tag ignore it.
    Tag(TagMark),
}

/// Start or end of content that belongs to a structure element. Content outside any mark is an artifact.
#[derive(Clone, Debug, PartialEq)]
pub enum TagMark {
    /// `owner` is the top-level page object the content comes from (for reading order); `path` lists the
    /// structure elements from the outermost to the one the content belongs to.
    Begin {
        owner: Id,
        path: Vec<(TagKey, Role)>,
    },
    End,
}

/// Identity of a structure element, so content drawn in pieces (lines, frames) joins one element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TagKey {
    Object(Id),
    /// Paragraph `n` of a story.
    Para(Id, usize),
    Row(Id, usize),
    Cell(Id, usize, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    /// Level 1–6 and the heading's text (PDF/UA wants a title on headings).
    Heading(u16, String),
    P,
    Figure(Option<String>),
    Table,
    Row,
    Cell,
    /// A link annotation (added by the PDF exporter).
    Link,
}

/// The top-level object containing `id` (itself if it has no group parent).
fn top_owner(doc: &Document, id: Id) -> Id {
    let mut cur = id;
    for _ in 0..64 {
        match doc.objects.get(&cur).and_then(|o| o.parent) {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

/// Heading level from a paragraph style name ("Title", "Heading 1".."Heading 6").
fn para_role(doc: &Document, story: Id, para: usize) -> Role {
    let name = doc
        .stories
        .get(&story)
        .and_then(|st| st.paras.get(para))
        .and_then(|p| p.style)
        .and_then(|sid| doc.styles.para.get(&sid))
        .map(|s| s.name.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let level = if name == "title" {
        Some(1)
    } else {
        name.strip_prefix("heading").map(str::trim).and_then(|n| n.parse::<u16>().ok()).filter(|n| (1..=6).contains(n))
    };
    match level {
        Some(n) => {
            let text = doc
                .stories
                .get(&story)
                .and_then(|st| st.para_ranges().get(para).map(|r| st.slice(r.clone()).trim().to_string()))
                .unwrap_or_default();
            Role::Heading(n, text.chars().filter(|c| !c.is_control() && *c != '\u{FFFC}').take(200).collect())
        }
        None => Role::P,
    }
}

/// Pushes a frame's glyph runs, each line marked as part of its paragraph's structure element
/// (nested inside `outer`).
fn push_lines(
    doc: &Document,
    owner: Id,
    story: Id,
    fl: &newpub_layout::FrameLayout,
    outer: &[(TagKey, Role)],
    t: Affine,
    items: &mut Vec<Item>,
) {
    for line in &fl.lines {
        let runs: Vec<&GlyphRun> = line.runs.iter().filter(|r| !r.glyphs.is_empty()).collect();
        if runs.is_empty() {
            continue;
        }
        // Lines that are not part of the story (continued notices) are artifacts.
        let tagged = line.para != usize::MAX;
        if tagged {
            let mut path = outer.to_vec();
            path.push((TagKey::Para(story, line.para), para_role(doc, story, line.para)));
            items.push(Item::Tag(TagMark::Begin { owner, path }));
        }
        for run in runs {
            items.push(Item::Glyphs { run: run.clone(), transform: t });
        }
        if tagged {
            items.push(Item::Tag(TagMark::End));
        }
    }
}

#[derive(Clone, Debug)]
pub struct PageDisplay {
    /// Trim size in points.
    pub width: f64,
    pub height: f64,
    pub items: Vec<Item>,
}

/// Local → page transform of an object (rotation about the centre, flips).
pub fn object_transform(o: &Object) -> Affine {
    let r = o.rect;
    let (cx, cy) = (r.w / 2.0, r.h / 2.0);
    Affine::translate(r.x + cx, r.y + cy)
        .compose(Affine::rotate(o.rotation))
        .compose(Affine::scale(if o.flip_h { -1.0 } else { 1.0 }, if o.flip_v { -1.0 } else { 1.0 }))
        .compose(Affine::translate(-cx, -cy))
}

pub fn rect_path(x: f64, y: f64, w: f64, h: f64) -> Vec<PathEl> {
    vec![PathEl::Move(x, y), PathEl::Line(x + w, y), PathEl::Line(x + w, y + h), PathEl::Line(x, y + h), PathEl::Close]
}

const KAPPA: f64 = 0.552_284_749_830_793_4;

pub fn ellipse_path(x: f64, y: f64, w: f64, h: f64) -> Vec<PathEl> {
    let (rx, ry) = (w / 2.0, h / 2.0);
    let (cx, cy) = (x + rx, y + ry);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    vec![
        PathEl::Move(cx + rx, cy),
        PathEl::Cubic(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        PathEl::Cubic(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        PathEl::Cubic(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        PathEl::Cubic(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        PathEl::Close,
    ]
}

fn round_rect_path(w: f64, h: f64, r: f64) -> Vec<PathEl> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = r * (1.0 - KAPPA);
    vec![
        PathEl::Move(r, 0.0),
        PathEl::Line(w - r, 0.0),
        PathEl::Cubic(w - k, 0.0, w, k, w, r),
        PathEl::Line(w, h - r),
        PathEl::Cubic(w, h - k, w - k, h, w - r, h),
        PathEl::Line(r, h),
        PathEl::Cubic(k, h, 0.0, h - k, 0.0, h - r),
        PathEl::Line(0.0, r),
        PathEl::Cubic(0.0, k, k, 0.0, r, 0.0),
        PathEl::Close,
    ]
}

fn poly(points: &[(f64, f64)], closed: bool) -> Vec<PathEl> {
    let mut v = vec![];
    for (i, &(x, y)) in points.iter().enumerate() {
        v.push(if i == 0 { PathEl::Move(x, y) } else { PathEl::Line(x, y) });
    }
    if closed {
        v.push(PathEl::Close);
    }
    v
}

/// Rectangle body with a triangular tail whose tip is at `tail` (rect-relative, scaled by `w`, `h`).
/// The tail base lies on the nearest edge, centred on the tip's projection onto it, 25% of the shorter side wide.
fn callout_path(tail: &[f64; 2], w: f64, h: f64) -> Vec<PathEl> {
    let (px, py) = (tail[0] * w, tail[1] * h);
    let half = 0.125 * w.min(h);
    // Signed distance of the tip beyond each edge: top, right, bottom, left (negative when outside).
    let d = [py, w - px, h - py, px];
    let mut edge = 0;
    for (i, v) in d.iter().enumerate() {
        if *v < d[edge] {
            edge = i;
        }
    }
    let cx = px.clamp(half.min(w / 2.0), (w - half).max(w / 2.0));
    let cy = py.clamp(half.min(h / 2.0), (h - half).max(h / 2.0));
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    if edge == 0 {
        pts.extend([(cx - half, 0.0), (px, py), (cx + half, 0.0)]);
    }
    pts.push((w, 0.0));
    if edge == 1 {
        pts.extend([(w, cy - half), (px, py), (w, cy + half)]);
    }
    pts.push((w, h));
    if edge == 2 {
        pts.extend([(cx + half, h), (px, py), (cx - half, h)]);
    }
    pts.push((0.0, h));
    if edge == 3 {
        pts.extend([(0.0, cy + half), (px, py), (0.0, cy - half)]);
    }
    poly(&pts, true)
}

/// Outline of a shape in its local box (0, 0, w, h).
pub fn shape_path(kind: &ShapeKind, w: f64, h: f64) -> Vec<PathEl> {
    match kind {
        ShapeKind::Rect => rect_path(0.0, 0.0, w, h),
        ShapeKind::RoundRect { radius } => round_rect_path(w, h, radius.0),
        ShapeKind::Ellipse => ellipse_path(0.0, 0.0, w, h),
        ShapeKind::Line => vec![PathEl::Move(0.0, 0.0), PathEl::Line(w, h)],
        ShapeKind::Triangle => poly(&[(w / 2.0, 0.0), (w, h), (0.0, h)], true),
        ShapeKind::Polygon { sides } => {
            let n = (*sides).max(3);
            let pts: Vec<(f64, f64)> = (0..n)
                .map(|i| {
                    let a = -PI / 2.0 + 2.0 * PI * i as f64 / n as f64;
                    (w / 2.0 + w / 2.0 * a.cos(), h / 2.0 + h / 2.0 * a.sin())
                })
                .collect();
            poly(&pts, true)
        }
        ShapeKind::Star { points, inner } => {
            let n = (*points).max(3) * 2;
            let pts: Vec<(f64, f64)> = (0..n)
                .map(|i| {
                    let a = -PI / 2.0 + 2.0 * PI * i as f64 / n as f64;
                    let k = if i % 2 == 0 { 1.0 } else { inner.clamp(0.05, 1.0) };
                    (w / 2.0 + w / 2.0 * k * a.cos(), h / 2.0 + h / 2.0 * k * a.sin())
                })
                .collect();
            poly(&pts, true)
        }
        ShapeKind::Arrow => {
            let (sh, hx) = (h * 0.25, w * 0.65);
            poly(&[(0.0, sh), (hx, sh), (hx, 0.0), (w, h / 2.0), (hx, h), (hx, h - sh), (0.0, h - sh)], true)
        }
        ShapeKind::Callout { tail } => callout_path(tail, w, h),
        ShapeKind::Path { points, closed } => {
            let pts: Vec<(f64, f64)> = points.iter().map(|p| (p[0] * w, p[1] * h)).collect();
            poly(&pts, *closed)
        }
        ShapeKind::Bezier { nodes, closed } => bezier_path(nodes, *closed, w, h),
    }
}

/// Cubic Bézier outline through `nodes` (rect-relative). A segment's control points are the start node's
/// `ctrl_out` and the end node's `ctrl_in`, each defaulting to its own point (straight when both absent).
pub fn bezier_path(nodes: &[BezierNode], closed: bool, w: f64, h: f64) -> Vec<PathEl> {
    let p = |q: [f64; 2]| (q[0] * w, q[1] * h);
    let Some(first) = nodes.first() else { return vec![] };
    let (x0, y0) = p(first.at);
    let mut out = vec![PathEl::Move(x0, y0)];
    let n = nodes.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    for i in 0..segs {
        let (a, b) = (&nodes[i], &nodes[(i + 1) % n]);
        if a.ctrl_out.is_none() && b.ctrl_in.is_none() {
            let (x, y) = p(b.at);
            out.push(PathEl::Line(x, y));
        } else {
            let (c1x, c1y) = p(a.ctrl_out.unwrap_or(a.at));
            let (c2x, c2y) = p(b.ctrl_in.unwrap_or(b.at));
            let (x, y) = p(b.at);
            out.push(PathEl::Cubic(c1x, c1y, c2x, c2y, x, y));
        }
    }
    if closed {
        out.push(PathEl::Close);
    }
    out
}

pub fn dash_pattern(d: Dash, width: f64) -> Vec<f64> {
    let w = width.max(0.5);
    match d {
        Dash::Solid => vec![],
        Dash::Dash => vec![4.0 * w, 3.0 * w],
        Dash::Dot => vec![w, 2.0 * w],
        Dash::DashDot => vec![4.0 * w, 2.0 * w, w, 2.0 * w],
        Dash::LongDash => vec![8.0 * w, 3.0 * w],
    }
}

fn stroke_style(s: &Stroke) -> StrokeStyle {
    StrokeStyle {
        color: s.color.clone(),
        width: s.width.0,
        dash: dash_pattern(s.dash, s.width.0),
        cap: s.cap,
        join: s.join,
    }
}

/// Arrowhead length and width (medium): both are this multiple of the line width.
const ARROW_SCALE: f64 = 3.0;

/// Maps every point of a path through `f`.
fn map_path(path: Vec<PathEl>, f: impl Fn(f64, f64) -> (f64, f64)) -> Vec<PathEl> {
    path.into_iter()
        .map(|e| match e {
            PathEl::Move(x, y) => {
                let (x, y) = f(x, y);
                PathEl::Move(x, y)
            }
            PathEl::Line(x, y) => {
                let (x, y) = f(x, y);
                PathEl::Line(x, y)
            }
            PathEl::Cubic(a, b, c, d, e, g) => {
                let (a, b) = f(a, b);
                let (c, d) = f(c, d);
                let (e, g) = f(e, g);
                PathEl::Cubic(a, b, c, d, e, g)
            }
            PathEl::Close => PathEl::Close,
        })
        .collect()
}

/// Geometry of one arrowhead: the outline (or open-V polyline) and how far the line must stop short of the tip.
struct ArrowHead {
    path: Vec<PathEl>,
    filled: bool,
    /// Distance from the tip back along the line where the drawn line should end.
    shorten: f64,
}

/// Builds an arrowhead with its tip at `tip`, opening towards unit vector `back` (from the tip into the line body).
fn arrow_head(kind: Arrow, tip: (f64, f64), back: (f64, f64), sw: f64) -> Option<ArrowHead> {
    let len = sw * ARROW_SCALE;
    let half = sw * ARROW_SCALE / 2.0;
    // Normal to the line direction.
    let n = (-back.1, back.0);
    // Local frame: `at(a, b)` is `a` along `back` and `b` along `n`, measured from the tip.
    let at = |a: f64, b: f64| (tip.0 + back.0 * a + n.0 * b, tip.1 + back.1 * a + n.1 * b);
    match kind {
        Arrow::None => None,
        Arrow::Triangle => {
            Some(ArrowHead { path: poly(&[tip, at(len, half), at(len, -half)], true), filled: true, shorten: len })
        }
        Arrow::Stealth => Some(ArrowHead {
            path: poly(&[tip, at(len, half), at(2.0 * len / 3.0, 0.0), at(len, -half)], true),
            filled: true,
            shorten: 2.0 * len / 3.0,
        }),
        Arrow::Diamond => Some(ArrowHead {
            path: poly(&[at(len / 2.0, 0.0), at(0.0, half), at(-len / 2.0, 0.0), at(0.0, -half)], true),
            filled: true,
            shorten: 0.0,
        }),
        Arrow::Oval => {
            // Ellipse centred on the tip: `len` along the line, `2 * half` across it.
            let path = map_path(ellipse_path(-len / 2.0, -half, len, 2.0 * half), |x, y| {
                (tip.0 + back.0 * x + n.0 * y, tip.1 + back.1 * x + n.1 * y)
            });
            Some(ArrowHead { path, filled: true, shorten: 0.0 })
        }
        Arrow::Open => {
            let (a, b) = (at(len, half), at(len, -half));
            Some(ArrowHead {
                path: vec![PathEl::Move(a.0, a.1), PathEl::Line(tip.0, tip.1), PathEl::Line(b.0, b.1)],
                filled: false,
                shorten: 0.0,
            })
        }
    }
}

/// Path of a line running from local (0, 0) to (w, h), shortened under any filled arrowheads,
/// and the arrowheads to draw on top of it.
fn line_with_arrows(s: &Shape, w: f64, h: f64) -> (Vec<PathEl>, Vec<ArrowHead>) {
    let len = w.hypot(h);
    let sw = s.stroke.as_ref().map_or(0.0, |st| st.width.0);
    if len <= 0.0 || sw <= 0.0 || (s.arrow_start == Arrow::None && s.arrow_end == Arrow::None) {
        return (shape_path(&s.kind, w, h), vec![]);
    }
    let u = (w / len, h / len);
    let start_head = arrow_head(s.arrow_start, (0.0, 0.0), u, sw);
    let end_head = arrow_head(s.arrow_end, (w, h), (-u.0, -u.1), sw);
    let start_d = start_head.as_ref().map_or(0.0, |a| a.shorten).min(len / 2.0);
    let end_d = end_head.as_ref().map_or(0.0, |a| a.shorten).min(len / 2.0);
    let (sx, sy) = (start_d * u.0, start_d * u.1);
    let (ex, ey) = (w - end_d * u.0, h - end_d * u.1);
    let path = vec![PathEl::Move(sx, sy), PathEl::Line(ex, ey)];
    (path, [start_head, end_head].into_iter().flatten().collect())
}

/// Outline of an image frame's mask shape, inset by `inset` on every side.
fn mask_path(mask: ImageMask, w: f64, h: f64, inset: f64) -> Vec<PathEl> {
    let (x, y, iw, ih) = (inset, inset, (w - 2.0 * inset).max(0.0), (h - 2.0 * inset).max(0.0));
    match mask {
        ImageMask::Rect => rect_path(x, y, iw, ih),
        ImageMask::Ellipse => ellipse_path(x, y, iw, ih),
        ImageMask::RoundRect => {
            let r = (0.15 * w.min(h) - inset).max(0.0);
            map_path(round_rect_path(iw, ih, r), |px, py| (px + x, py + y))
        }
    }
}

/// Drop shadow behind a silhouette: the outline offset by the shadow vector, in the shadow colour.
/// With a blur, a few expanded translucent copies approximate the falloff.
fn push_shadow(items: &mut Vec<Item>, sh: &Shadow, path: &[PathEl], filled: bool, stroke: Option<&Stroke>, t: Affine) {
    let st = Affine::translate(sh.dx.0, sh.dy.0).compose(t);
    let blur = sh.blur.0.max(0.0);
    let steps = if blur > 0.0 { 4 } else { 0 };
    let a = f64::from(sh.color.alpha()).clamp(0.0, 1.0);
    let layer_alpha = 1.0 - (1.0 - a).powf(1.0 / (steps + 1) as f64);
    let color = with_alpha(&sh.color, layer_alpha as f32);
    let base_w = stroke.map_or(0.0, |s| s.width.0);
    for i in 0..=steps {
        let grow = blur * (steps - i) as f64 / steps.max(1) as f64;
        let width = base_w + 2.0 * grow;
        let has_stroke = stroke.is_some() || grow > 0.0;
        if !filled && !has_stroke {
            continue;
        }
        items.push(Item::Path {
            path: path.to_vec(),
            fill: filled.then(|| color.clone()),
            stroke: has_stroke.then(|| StrokeStyle {
                color: color.clone(),
                width,
                dash: vec![],
                cap: stroke.map_or(LineCap::Butt, |s| s.cap),
                join: LineJoin::Round,
            }),
            transform: st,
        });
    }
}

fn with_alpha(c: &Color, alpha: f32) -> Color {
    c.clone().with_alpha(alpha)
}

/// Screen-only stand-in for an empty picture frame: a light grey box with a small picture glyph.
fn push_placeholder(items: &mut Vec<Item>, w: f64, h: f64, t: Affine) {
    let grey = |v: u8, a: f32| Color::Rgb { r: v, g: v, b: v, a };
    items.push(Item::Path { path: rect_path(0.0, 0.0, w, h), fill: Some(grey(235, 1.0)), stroke: None, transform: t });
    let s = (w.min(h) * 0.4).max(1.0);
    let (x, y) = ((w - s) / 2.0, (h - s) / 2.0);
    let ink = grey(170, 1.0);
    let frame = rect_path(x, y, s, s);
    items.push(Item::Path {
        path: frame,
        fill: None,
        stroke: Some(StrokeStyle {
            color: ink.clone(),
            width: (s * 0.06).max(0.5),
            dash: vec![],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }),
        transform: t,
    });
    items.push(Item::Path {
        path: poly(
            &[
                (x, y + s),
                (x + s * 0.35, y + s * 0.45),
                (x + s * 0.6, y + s * 0.75),
                (x + s * 0.75, y + s * 0.6),
                (x + s, y + s),
            ],
            true,
        ),
        fill: Some(ink.clone()),
        stroke: None,
        transform: t,
    });
    items.push(Item::Path {
        path: ellipse_path(x + s * 0.6, y + s * 0.12, s * 0.2, s * 0.2),
        fill: Some(ink),
        stroke: None,
        transform: t,
    });
}

fn visible(doc: &Document, o: &Object) -> bool {
    if o.hidden {
        return false;
    }
    match o.layer {
        Some(l) => doc.layers.iter().find(|x| x.id == l).map(|x| x.visible).unwrap_or(true),
        None => true,
    }
}

/// Pushes an object's items; pictures and described shapes become Figure structure elements.
#[allow(clippy::too_many_arguments)]
fn push_object(
    doc: &Document,
    layout: &DocLayout,
    fonts: &FontStore,
    page: usize,
    id: Id,
    parent: Affine,
    screen: bool,
    items: &mut Vec<Item>,
) {
    let figure = doc.objects.get(&id).filter(|o| !o.decorative).and_then(|o| match &o.kind {
        ObjectKind::Image(im) if im.asset.is_some() => Some(o.alt_text.clone()),
        ObjectKind::Shape(_) if o.alt_text.as_deref().is_some_and(|a| !a.trim().is_empty()) => Some(o.alt_text.clone()),
        _ => None,
    });
    let start = items.len();
    push_object_inner(doc, layout, fonts, page, id, parent, screen, items);
    if let Some(alt) = figure
        && items.len() > start
    {
        // A described shape's own text stays its own paragraphs: only the drawing is the figure.
        let owner = top_owner(doc, id);
        let mut out: Vec<Item> =
            vec![Item::Tag(TagMark::Begin { owner, path: vec![(TagKey::Object(id), Role::Figure(alt.clone()))] })];
        for it in items.drain(start..) {
            match it {
                Item::Tag(TagMark::Begin { owner, path }) => {
                    out.push(Item::Tag(TagMark::End));
                    out.push(Item::Tag(TagMark::Begin { owner, path }));
                }
                Item::Tag(TagMark::End) => {
                    out.push(Item::Tag(TagMark::End));
                    out.push(Item::Tag(TagMark::Begin {
                        owner,
                        path: vec![(TagKey::Object(id), Role::Figure(alt.clone()))],
                    }));
                }
                other => out.push(other),
            }
        }
        out.push(Item::Tag(TagMark::End));
        items.extend(out);
    }
}

#[allow(clippy::too_many_arguments)]
fn push_object_inner(
    doc: &Document,
    layout: &DocLayout,
    fonts: &FontStore,
    page: usize,
    id: Id,
    parent: Affine,
    screen: bool,
    items: &mut Vec<Item>,
) {
    let Some(o) = doc.objects.get(&id) else { return };
    if !visible(doc, o) {
        return;
    }
    let t = parent.compose(object_transform(o));
    let (w, h) = (o.rect.w, o.rect.h);
    match &o.kind {
        ObjectKind::Shape(s) => {
            let (path, heads) = if matches!(s.kind, ShapeKind::Line) {
                line_with_arrows(s, w, h)
            } else {
                (shape_path(&s.kind, w, h), vec![])
            };
            let grad = s.gradient.as_ref().and_then(|g| gradient_paint(g, w, h));
            if let Some(sh) = &o.shadow {
                let has_fill = grad.is_some() || s.fill.is_some();
                push_shadow(items, sh, &path, has_fill, s.stroke.as_ref(), t);
            }
            match grad {
                Some(paint) => {
                    items.push(Item::GradPath { path: path.clone(), paint, transform: t });
                    if s.stroke.is_some() {
                        items.push(Item::Path {
                            path,
                            fill: None,
                            stroke: s.stroke.as_ref().map(stroke_style),
                            transform: t,
                        });
                    }
                }
                None => items.push(Item::Path {
                    path,
                    fill: s.fill.clone(),
                    stroke: s.stroke.as_ref().map(stroke_style),
                    transform: t,
                }),
            }
            if let Some(st) = &s.stroke {
                for head in heads {
                    items.push(if head.filled {
                        Item::Path { path: head.path, fill: Some(st.color.clone()), stroke: None, transform: t }
                    } else {
                        Item::Path {
                            path: head.path,
                            fill: None,
                            stroke: Some(StrokeStyle {
                                color: st.color.clone(),
                                width: st.width.0,
                                dash: vec![],
                                cap: LineCap::Butt,
                                join: LineJoin::Bevel,
                            }),
                            transform: t,
                        }
                    });
                }
            }
            if let Some(sid) = s.story
                && let Some(fl) = layout.frames.get(&id)
            {
                push_lines(doc, top_owner(doc, id), sid, fl, &[], t, items);
            }
        }
        ObjectKind::Text(tf) => {
            if let (Some(sh), true) = (&o.shadow, tf.fill.is_some()) {
                push_shadow(items, sh, &rect_path(0.0, 0.0, w, h), true, None, t);
            }
            if tf.fill.is_some() || tf.stroke.is_some() {
                items.push(Item::Path {
                    path: rect_path(0.0, 0.0, w, h),
                    fill: tf.fill.clone(),
                    stroke: tf.stroke.as_ref().map(stroke_style),
                    transform: t,
                });
            }
            if let Some(fl) = layout.frame_on(id, Some(page)) {
                for d in &fl.decorations {
                    items.push(Item::Path {
                        path: rect_path(d.x0, d.y - d.thickness / 2.0, d.x1 - d.x0, d.thickness),
                        fill: Some(d.color.clone()),
                        stroke: None,
                        transform: t,
                    });
                }
                push_lines(doc, top_owner(doc, id), tf.story, fl, &[], t, items);
            }
        }
        ObjectKind::Image(im) => {
            let silhouette = mask_path(im.mask, w, h, 0.0);
            let soft = im.soft_edges.0.max(0.0).min(w.min(h) / 2.0);
            let asset = im.asset.and_then(|aid| doc.assets.get(&aid).map(|a| (aid, a)));
            if asset.is_none() && im.asset.is_none() {
                if screen {
                    push_placeholder(items, w, h, t);
                }
            } else if let Some(sh) = &o.shadow {
                push_shadow(items, sh, &silhouette, true, None, t);
            }
            if let Some((aid, a)) = asset {
                let (pw, ph) = (a.px_w.max(1) as f64, a.px_h.max(1) as f64);
                let c = im.crop;
                let (vx, vy) = (c.left * pw, c.top * ph);
                let (vw, vh) = ((1.0 - c.left - c.right) * pw, (1.0 - c.top - c.bottom) * ph);
                let (sx, sy) = (w / vw.max(1e-9), h / vh.max(1e-9));
                let (sx, sy, ox, oy) = match im.fit {
                    Fit::Stretch => (sx, sy, 0.0, 0.0),
                    Fit::Fit => {
                        let s = sx.min(sy);
                        (s, s, (w - vw * s) / 2.0, (h - vh * s) / 2.0)
                    }
                    Fit::Fill => {
                        let s = sx.max(sy);
                        (s, s, (w - vw * s) / 2.0, (h - vh * s) / 2.0)
                    }
                };
                let local = Affine::translate(ox - vx * sx, oy - vy * sy).compose(Affine::scale(sx, sy));
                // Clip to the frame ∩ the visible part of the image (letterboxing leaves the rest empty).
                let (cx0, cy0) = (ox.max(0.0), oy.max(0.0));
                let (cx1, cy1) = ((ox + vw * sx).min(w), (oy + vh * sy).min(h));
                let clip = Rect::new(cx0, cy0, (cx1 - cx0).max(0.0), (cy1 - cy0).max(0.0));
                let draw = |mask: Option<Vec<PathEl>>, opacity: f64| Item::Image {
                    asset: aid,
                    local,
                    clip,
                    mask,
                    opacity,
                    adjust: im.adjust.clone(),
                    transform: t,
                };
                if soft > 0.0 {
                    // Feather: stacked copies clipped to progressively inset outlines, whose opacities
                    // compose to a linear alpha ramp from the border inwards.
                    const STEPS: usize = 12;
                    let target = |j: usize| if j > STEPS { 1.0 } else { (j as f64 - 0.5) / STEPS as f64 };
                    for i in 0..=STEPS {
                        let inset = if i == STEPS { soft } else { soft * i as f64 / STEPS as f64 };
                        let (t0, t1) = (if i == 0 { 0.0 } else { target(i) }, target(i + 1));
                        let opacity = ((t1 - t0) / (1.0 - t0)).clamp(0.0, 1.0);
                        items.push(draw(Some(mask_path(im.mask, w, h, inset)), opacity));
                    }
                } else {
                    let mask = (im.mask != ImageMask::Rect).then(|| silhouette.clone());
                    items.push(draw(mask, 1.0));
                }
            }
            if let Some(s) = &im.stroke
                && soft <= 0.0
            {
                // The border sits inside the frame, so it never grows past the picture's (or its shadow's) outline.
                let path = mask_path(im.mask, w, h, s.width.0 / 2.0);
                items.push(Item::Path { path, fill: None, stroke: Some(stroke_style(s)), transform: t });
            }
        }
        ObjectKind::Table(tb) => {
            // Fills, then text, then borders on top.
            for r in 0..tb.rows() {
                for c in 0..tb.cols() {
                    let (Some(cell), Some(cr)) = (tb.cell(r, c), tb.cell_rect(r, c)) else { continue };
                    if cell.covered {
                        continue;
                    }
                    if let Some(f) = &cell.fill {
                        items.push(Item::Path {
                            path: rect_path(cr.x, cr.y, cr.w, cr.h),
                            fill: Some(f.clone()),
                            stroke: None,
                            transform: t,
                        });
                    }
                    if let Some(fl) = layout.frames.get(&cell.story) {
                        let outer = [
                            (TagKey::Object(id), Role::Table),
                            (TagKey::Row(id, r), Role::Row),
                            (TagKey::Cell(id, r, c), Role::Cell),
                        ];
                        push_lines(doc, top_owner(doc, id), cell.story, fl, &outer, t, items);
                        for d in &fl.decorations {
                            items.push(Item::Path {
                                path: rect_path(d.x0, d.y - d.thickness / 2.0, d.x1 - d.x0, d.thickness),
                                fill: Some(d.color.clone()),
                                stroke: None,
                                transform: t,
                            });
                        }
                    }
                }
            }
            for r in 0..tb.rows() {
                for c in 0..tb.cols() {
                    let (Some(cell), Some(cr)) = (tb.cell(r, c), tb.cell_rect(r, c)) else { continue };
                    if cell.covered {
                        continue;
                    }
                    let b = &cell.borders;
                    let edges = [
                        (&b.top, (cr.x, cr.y), (cr.right(), cr.y)),
                        (&b.bottom, (cr.x, cr.bottom()), (cr.right(), cr.bottom())),
                        (&b.left, (cr.x, cr.y), (cr.x, cr.bottom())),
                        (&b.right, (cr.right(), cr.y), (cr.right(), cr.bottom())),
                    ];
                    for (stroke, a, z) in edges {
                        if let Some(s) = stroke {
                            items.push(Item::Path {
                                path: vec![PathEl::Move(a.0, a.1), PathEl::Line(z.0, z.1)],
                                fill: None,
                                stroke: Some(stroke_style(s)),
                                transform: t,
                            });
                        }
                    }
                }
            }
        }
        ObjectKind::WordArt(wa) => {
            let path = wordart_path(fonts, wa, w, h);
            if path.is_empty() {
                return;
            }
            let grad = wa.gradient.as_ref().and_then(|g| gradient_paint(g, w, h));
            if let Some(sh) = &o.shadow {
                push_shadow(items, sh, &path, true, wa.outline.as_ref(), t);
            }
            let stroke = wa.outline.as_ref().map(stroke_style);
            match grad {
                Some(paint) => {
                    items.push(Item::GradPath { path: path.clone(), paint, transform: t });
                    if stroke.is_some() {
                        items.push(Item::Path { path, fill: None, stroke, transform: t });
                    }
                }
                None => items.push(Item::Path { path, fill: Some(wa.fill.clone()), stroke, transform: t }),
            }
        }
        ObjectKind::Group { children } => {
            // Children are stored in page coordinates; the group's own transform is not applied.
            for c in children {
                push_object(doc, layout, fonts, page, *c, parent, screen, items);
            }
        }
    }
}

/// Builds the print display list for page `index` (master objects first, then page objects).
/// Screen-only aids such as picture placeholders are left out.
pub fn page_display(doc: &Document, layout: &DocLayout, fonts: &FontStore, index: usize) -> PageDisplay {
    page_display_for(doc, layout, fonts, index, false)
}

/// Like [`page_display`]; with `screen` set, includes on-screen-only items (empty picture placeholders).
pub fn page_display_for(
    doc: &Document,
    layout: &DocLayout,
    fonts: &FontStore,
    index: usize,
    screen: bool,
) -> PageDisplay {
    let mut items = vec![];
    let (w, h) = (doc.setup.width.0, doc.setup.height.0);
    if let Some(page) = doc.pages.get(index) {
        let master = doc.master_for_page(index);
        let bg = page.background.clone().or_else(|| master.and_then(|m| m.background.clone()));
        if let Some(bg) = bg {
            let b = doc.setup.bleed.0;
            items.push(Item::Path {
                path: rect_path(-b, -b, w + 2.0 * b, h + 2.0 * b),
                fill: Some(bg),
                stroke: None,
                transform: Affine::IDENTITY,
            });
        }
        if let Some(m) = master {
            for id in &m.objects {
                push_object(doc, layout, fonts, index, *id, Affine::IDENTITY, screen, &mut items);
            }
        }
        for id in doc.draw_order(index) {
            push_object(doc, layout, fonts, index, id, Affine::IDENTITY, screen, &mut items);
        }
    }
    PageDisplay { width: w, height: h, items }
}

struct OutlinePath {
    els: Vec<PathEl>,
    cur: (f64, f64),
}

impl ttf_parser::OutlineBuilder for OutlinePath {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cur = (x as f64, y as f64);
        self.els.push(PathEl::Move(self.cur.0, self.cur.1));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cur = (x as f64, y as f64);
        self.els.push(PathEl::Line(self.cur.0, self.cur.1));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.cur;
        let (x1, y1, x, y) = (x1 as f64, y1 as f64, x as f64, y as f64);
        self.els.push(PathEl::Cubic(
            x0 + 2.0 / 3.0 * (x1 - x0),
            y0 + 2.0 / 3.0 * (y1 - y0),
            x + 2.0 / 3.0 * (x1 - x),
            y + 2.0 / 3.0 * (y1 - y),
            x,
            y,
        ));
        self.cur = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.els.push(PathEl::Cubic(x1 as f64, y1 as f64, x2 as f64, y2 as f64, x as f64, y as f64));
        self.cur = (x as f64, y as f64);
    }
    fn close(&mut self) {
        self.els.push(PathEl::Close);
    }
}

/// A glyph's outline in font units (y up). Empty for blank glyphs.
pub fn glyph_outline(face: &ttf_parser::Face, gid: u16) -> Vec<PathEl> {
    let mut b = OutlinePath { els: vec![], cur: (0.0, 0.0) };
    face.outline_glyph(ttf_parser::GlyphId(gid), &mut b);
    b.els
}

fn cubic_at(p: [(f64, f64); 4], t: f64) -> (f64, f64) {
    let m = 1.0 - t;
    let (a, b, c, d) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    (a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0, a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1)
}

/// Applies `f` to every point of `els`.
pub(crate) fn map_els(els: &[PathEl], f: impl Fn((f64, f64)) -> (f64, f64)) -> Vec<PathEl> {
    els.iter()
        .map(|e| match *e {
            PathEl::Move(x, y) => {
                let (x, y) = f((x, y));
                PathEl::Move(x, y)
            }
            PathEl::Line(x, y) => {
                let (x, y) = f((x, y));
                PathEl::Line(x, y)
            }
            PathEl::Cubic(a, b, c, d, e, g) => {
                let (a, b) = f((a, b));
                let (c, d) = f((c, d));
                let (e, g) = f((e, g));
                PathEl::Cubic(a, b, c, d, e, g)
            }
            PathEl::Close => PathEl::Close,
        })
        .collect()
}

/// Lays `wa.text` out as one line of glyph outlines (em units, y up, pen starting at 0).
fn wordart_outlines(fonts: &FontStore, wa: &WordArt) -> Vec<PathEl> {
    let main = fonts.face(fonts.resolve(&wa.font, wa.bold, wa.italic));
    let skew = if wa.italic && !main.italic { 0.21 } else { 0.0 };
    let mut out = vec![];
    let mut pen = 0.0;
    for ch in wa.text.chars() {
        let ch = if ch.is_control() { ' ' } else { ch };
        let face = if main.has_glyph(ch) {
            main.clone()
        } else {
            fonts.fallback_for(ch, wa.bold, wa.italic).map_or_else(|| main.clone(), |id| fonts.face(id))
        };
        let Some(ttf) = face.ttf() else { continue };
        let gid = ttf.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
        let k = 1.0 / face.units_per_em;
        out.extend(map_els(&glyph_outline(&ttf, gid.0), |(x, y)| (pen + x * k + skew * y * k, y * k)));
        pen += ttf.glyph_hor_advance(gid).map_or(0.0, f64::from) * k;
    }
    out
}

/// Top and bottom of the glyph band at horizontal fraction `u`, as fractions of the rect height.
fn warp_band(warp: Warp, u: f64) -> (f64, f64) {
    let k = 1.0 - (2.0 * u - 1.0).powi(2);
    match warp {
        Warp::None => (0.0, 1.0),
        Warp::ArchUp => {
            let bottom = 1.0 - 0.4 * k;
            (bottom - 0.6, bottom)
        }
        Warp::ArchDown => {
            let top = 0.4 * k;
            (top, top + 0.6)
        }
        Warp::Wave => {
            let bottom = 0.85 - 0.15 * (2.0 * PI * u).sin();
            (bottom - 0.7, bottom)
        }
        Warp::SlantUp => {
            let bottom = 1.0 - 0.3 * u;
            (bottom - 0.7, bottom)
        }
        Warp::SlantDown => {
            let bottom = 0.7 + 0.3 * u;
            (bottom - 0.7, bottom)
        }
        Warp::Inflate => {
            let gh = 0.5 + 0.5 * (PI * u).sin();
            ((1.0 - gh) / 2.0, (1.0 + gh) / 2.0)
        }
    }
}

/// The WordArt text as filled outlines in the object's local box (`w` x `h`): the line is stretched to fill
/// the box, then bent by the warp (see [`Warp`]). Empty when there is nothing to draw.
pub fn wordart_path(fonts: &FontStore, wa: &WordArt, w: f64, h: f64) -> Vec<PathEl> {
    let src = wordart_outlines(fonts, wa);
    // Ink bounds, sampling curves.
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let mut note = |p: (f64, f64)| {
        x0 = x0.min(p.0);
        x1 = x1.max(p.0);
        y0 = y0.min(p.1);
        y1 = y1.max(p.1);
    };
    let mut cur = (0.0, 0.0);
    for e in &src {
        match *e {
            PathEl::Move(x, y) | PathEl::Line(x, y) => {
                cur = (x, y);
                note(cur);
            }
            PathEl::Cubic(a, b, c, d, e, f) => {
                for i in 1..=8 {
                    note(cubic_at([cur, (a, b), (c, d), (e, f)], i as f64 / 8.0));
                }
                cur = (e, f);
            }
            PathEl::Close => {}
        }
    }
    if x0 >= x1 || y0 >= y1 || w <= 0.0 || h <= 0.0 {
        return vec![];
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let map = |p: (f64, f64)| -> (f64, f64) {
        let u = (p.0 - x0) / dx;
        let s = (y1 - p.1) / dy;
        let (top, bot) = warp_band(wa.warp, u);
        (u * w, (top + s * (bot - top)) * h)
    };
    if wa.warp == Warp::None {
        return map_els(&src, map);
    }
    // Warped: flatten, and split long runs so the bent baseline is followed.
    let mut out = vec![];
    let line_to = |out: &mut Vec<PathEl>, from: (f64, f64), to: (f64, f64)| {
        let n = (((to.0 - from.0).abs() / dx) * 96.0).ceil().clamp(1.0, 256.0) as usize;
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let (x, y) = map((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t));
            out.push(PathEl::Line(x, y));
        }
    };
    let mut cur = (0.0, 0.0);
    for e in &src {
        match *e {
            PathEl::Move(x, y) => {
                cur = (x, y);
                let (mx, my) = map(cur);
                out.push(PathEl::Move(mx, my));
            }
            PathEl::Line(x, y) => {
                line_to(&mut out, cur, (x, y));
                cur = (x, y);
            }
            PathEl::Cubic(a, b, c, d, e, f) => {
                let mut prev = cur;
                for i in 1..=12 {
                    let p = cubic_at([cur, (a, b), (c, d), (e, f)], i as f64 / 12.0);
                    line_to(&mut out, prev, p);
                    prev = p;
                }
                cur = (e, f);
            }
            PathEl::Close => out.push(PathEl::Close),
        }
    }
    out
}
