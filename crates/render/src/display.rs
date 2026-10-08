//! Display lists: the shared, back-end-independent description of what a page shows.
//! Both the raster back end (tiny-skia) and the PDF back end (krilla) consume these.

use newpub_core::*;
use newpub_layout::{DocLayout, GlyphRun};
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
}

#[derive(Clone, Debug)]
pub enum Item {
    /// A path in local coordinates, mapped to the page by `transform`.
    Path { path: Vec<PathEl>, fill: Option<Color>, stroke: Option<StrokeStyle>, transform: Affine },
    /// An image. `local` maps image pixel space (0..px_w, 0..px_h) into the object's local box;
    /// drawing is clipped to `clip` (local coordinates); `transform` maps local → page.
    Image { asset: Id, local: Affine, clip: Rect, transform: Affine },
    /// Glyphs positioned in frame-local coordinates, mapped to the page by `transform`.
    Glyphs { run: GlyphRun, transform: Affine },
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
        ShapeKind::Path { points, closed } => {
            let pts: Vec<(f64, f64)> = points.iter().map(|p| (p[0] * w, p[1] * h)).collect();
            poly(&pts, *closed)
        }
    }
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
    StrokeStyle { color: s.color.clone(), width: s.width.0, dash: dash_pattern(s.dash, s.width.0) }
}

fn visible(doc: &Document, o: &Object) -> bool {
    match o.layer {
        Some(l) => doc.layers.iter().find(|x| x.id == l).map(|x| x.visible).unwrap_or(true),
        None => true,
    }
}

fn push_object(doc: &Document, layout: &DocLayout, id: Id, parent: Affine, items: &mut Vec<Item>) {
    let Some(o) = doc.objects.get(&id) else { return };
    if !visible(doc, o) {
        return;
    }
    let t = parent.compose(object_transform(o));
    let (w, h) = (o.rect.w, o.rect.h);
    match &o.kind {
        ObjectKind::Shape(s) => {
            items.push(Item::Path {
                path: shape_path(&s.kind, w, h),
                fill: s.fill.clone(),
                stroke: s.stroke.as_ref().map(stroke_style),
                transform: t,
            });
        }
        ObjectKind::Text(tf) => {
            if tf.fill.is_some() || tf.stroke.is_some() {
                items.push(Item::Path {
                    path: rect_path(0.0, 0.0, w, h),
                    fill: tf.fill.clone(),
                    stroke: tf.stroke.as_ref().map(stroke_style),
                    transform: t,
                });
            }
            if let Some(fl) = layout.frames.get(&id) {
                for d in &fl.decorations {
                    items.push(Item::Path {
                        path: rect_path(d.x0, d.y - d.thickness / 2.0, d.x1 - d.x0, d.thickness),
                        fill: Some(d.color.clone()),
                        stroke: None,
                        transform: t,
                    });
                }
                for line in &fl.lines {
                    for run in &line.runs {
                        if !run.glyphs.is_empty() {
                            items.push(Item::Glyphs { run: run.clone(), transform: t });
                        }
                    }
                }
            }
        }
        ObjectKind::Image(im) => {
            if let Some(aid) = im.asset
                && let Some(a) = doc.assets.get(&aid) {
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
                    items.push(Item::Image { asset: aid, local, clip: Rect::new(0.0, 0.0, w, h), transform: t });
                }
            if let Some(s) = &im.stroke {
                items.push(Item::Path {
                    path: rect_path(0.0, 0.0, w, h),
                    fill: None,
                    stroke: Some(stroke_style(s)),
                    transform: t,
                });
            }
        }
        ObjectKind::Group { children } => {
            // Children are stored in page coordinates; the group's own transform is not applied.
            for c in children {
                push_object(doc, layout, *c, parent, items);
            }
        }
    }
}

/// Builds the display list for page `index` (master objects first, then page objects).
pub fn page_display(doc: &Document, layout: &DocLayout, index: usize) -> PageDisplay {
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
                push_object(doc, layout, *id, Affine::IDENTITY, &mut items);
            }
        }
        for id in &page.objects {
            push_object(doc, layout, *id, Affine::IDENTITY, &mut items);
        }
    }
    PageDisplay { width: w, height: h, items }
}
