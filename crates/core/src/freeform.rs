//! Freeform / Bézier shapes and point editing (SH-08).

use crate::color::Color;
use crate::model::{Arrow, BezierNode, Object, ObjectKind, Shape, ShapeKind, Stroke};
use crate::units::{Length, Rect};
use crate::{Applied, Command, CoreError, Document, Id, NodeKind};
use serde::{Deserialize, Serialize};

/// A path point in page coordinates (query PathNodes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathNodeInfo {
    pub x: f64,
    pub y: f64,
    pub ctrl_in: Option<[f64; 2]>,
    pub ctrl_out: Option<[f64; 2]>,
    pub smooth: bool,
}

type P = [f64; 2];

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}

fn mid(a: P, b: P) -> P {
    scale(add(a, b), 0.5)
}

fn dist(a: P, b: P) -> f64 {
    let d = sub(a, b);
    d[0].hypot(d[1])
}

fn pos(n: &PathNodeInfo) -> P {
    [n.x, n.y]
}

/// Neighbours (prev, next) of node `i`; `None` at the ends of an open path.
fn neighbours(nodes: &[PathNodeInfo], i: usize, closed: bool) -> (Option<P>, Option<P>) {
    let n = nodes.len();
    let prev = if i > 0 {
        Some(pos(&nodes[i - 1]))
    } else if closed {
        Some(pos(&nodes[n - 1]))
    } else {
        None
    };
    let next = if i + 1 < n {
        Some(pos(&nodes[i + 1]))
    } else if closed {
        Some(pos(&nodes[0]))
    } else {
        None
    };
    (prev, next)
}

/// Sets node `i`'s handles. `catmull` uses the Catmull-Rom tangent, otherwise a third of each neighbour distance.
fn make_smooth(nodes: &mut [PathNodeInfo], i: usize, closed: bool, catmull: bool) {
    let (prev, next) = neighbours(nodes, i, closed);
    let p = pos(&nodes[i]);
    let (cin, cout) = match (prev, next) {
        (Some(a), Some(b)) => {
            let d = sub(b, a);
            let len = d[0].hypot(d[1]);
            if catmull {
                let t = scale(d, 1.0 / 6.0);
                (Some(sub(p, t)), Some(add(p, t)))
            } else if len > 1e-12 {
                let u = scale(d, 1.0 / len);
                (Some(sub(p, scale(u, dist(p, a) / 3.0))), Some(add(p, scale(u, dist(b, p) / 3.0))))
            } else {
                (None, None)
            }
        }
        (None, Some(b)) => (None, Some(add(p, scale(sub(b, p), 1.0 / 3.0)))),
        (Some(a), None) => (Some(add(p, scale(sub(a, p), 1.0 / 3.0))), None),
        (None, None) => (None, None),
    };
    nodes[i].ctrl_in = cin;
    nodes[i].ctrl_out = cout;
    nodes[i].smooth = true;
}

/// Bounding box of points and handles; a zero-size side gets 1 pt.
fn bounds(nodes: &[PathNodeInfo]) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for n in nodes {
        for p in [Some([n.x, n.y]), n.ctrl_in, n.ctrl_out].into_iter().flatten() {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
    }
    let w = if x1 - x0 > 0.0 { x1 - x0 } else { 1.0 };
    let h = if y1 - y0 > 0.0 { y1 - y0 } else { 1.0 };
    Rect { x: x0, y: y0, w, h }
}

fn normalise(nodes: &[PathNodeInfo], r: Rect) -> Vec<BezierNode> {
    let rel = |p: P| [(p[0] - r.x) / r.w, (p[1] - r.y) / r.h];
    nodes
        .iter()
        .map(|n| BezierNode {
            at: rel([n.x, n.y]),
            ctrl_in: n.ctrl_in.map(rel),
            ctrl_out: n.ctrl_out.map(rel),
            smooth: n.smooth,
        })
        .collect()
}

fn bezier_of(o: &Object) -> Result<(&[BezierNode], bool), CoreError> {
    if let ObjectKind::Shape(s) = &o.kind
        && let ShapeKind::Bezier { nodes, closed } = &s.kind
    {
        return Ok((nodes, *closed));
    }
    Err(CoreError::WrongKind(o.id, "freeform path"))
}

fn to_page(nodes: &[BezierNode], r: Rect) -> Vec<PathNodeInfo> {
    let abs = |p: P| [r.x + p[0] * r.w, r.y + p[1] * r.h];
    nodes
        .iter()
        .map(|n| {
            let a = abs(n.at);
            PathNodeInfo {
                x: a[0],
                y: a[1],
                ctrl_in: n.ctrl_in.map(abs),
                ctrl_out: n.ctrl_out.map(abs),
                smooth: n.smooth,
            }
        })
        .collect()
}

fn finite(v: f64) -> Result<f64, CoreError> {
    if v.is_finite() { Ok(v) } else { Err(CoreError::Invalid("coordinate is not finite".into())) }
}

fn range_err(i: usize) -> CoreError {
    CoreError::Invalid(format!("path point index {i} out of range"))
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::AddFreeform { page, master, points, closed, smooth, fill, stroke } => {
            let min = if *closed { 3 } else { 2 };
            if points.len() < min {
                return Err(CoreError::Invalid(format!("a freeform needs at least {min} points")));
            }
            let mut nodes = Vec::with_capacity(points.len());
            for p in points {
                nodes.push(PathNodeInfo {
                    x: finite(p[0].pt())?,
                    y: finite(p[1].pt())?,
                    ctrl_in: None,
                    ctrl_out: None,
                    smooth: false,
                });
            }
            if *smooth {
                for i in 0..nodes.len() {
                    make_smooth(&mut nodes, i, *closed, true);
                }
            }
            let stroke = match (fill, stroke) {
                (None, None) => Some(Stroke {
                    color: Color::BLACK,
                    width: Length(1.0),
                    dash: Default::default(),
                    cap: Default::default(),
                    join: Default::default(),
                }),
                (_, s) => s.clone(),
            };
            let rect = bounds(&nodes);
            let shape = Shape {
                kind: ShapeKind::Bezier { nodes: normalise(&nodes, rect), closed: *closed },
                fill: fill.clone(),
                stroke,
                arrow_start: Arrow::None,
                arrow_end: Arrow::None,
                gradient: None,
                story: None,
            };
            let obj = doc.new_object(rect, ObjectKind::Shape(shape));
            let id = doc.place(*page, *master, obj)?;
            Ok(Applied { created: vec![id] })
        }
        Command::MovePathNode { id, index, x, y } => edit(doc, *id, |nodes, _| {
            let n = nodes.get_mut(*index).ok_or_else(|| range_err(*index))?;
            let (nx, ny) = (finite(x.pt())?, finite(y.pt())?);
            let (dx, dy) = (nx - n.x, ny - n.y);
            n.x = nx;
            n.y = ny;
            for h in [&mut n.ctrl_in, &mut n.ctrl_out].into_iter().flatten() {
                h[0] += dx;
                h[1] += dy;
            }
            Ok(())
        }),
        Command::InsertPathNode { id, after } => edit(doc, *id, |nodes, closed| {
            let n = nodes.len();
            if *after >= n || (!closed && *after + 1 >= n) {
                return Err(range_err(*after));
            }
            let j = (*after + 1) % n;
            let (a, b) = (pos(&nodes[*after]), pos(&nodes[j]));
            let curved = nodes[*after].ctrl_out.is_some() || nodes[j].ctrl_in.is_some();
            let new = if curved {
                let p1 = nodes[*after].ctrl_out.unwrap_or(a);
                let p2 = nodes[j].ctrl_in.unwrap_or(b);
                let (p01, p12, p23) = (mid(a, p1), mid(p1, p2), mid(p2, b));
                let (p012, p123) = (mid(p01, p12), mid(p12, p23));
                if nodes[*after].ctrl_out.is_some() {
                    nodes[*after].ctrl_out = Some(p01);
                }
                if nodes[j].ctrl_in.is_some() {
                    nodes[j].ctrl_in = Some(p23);
                }
                let m = mid(p012, p123);
                PathNodeInfo { x: m[0], y: m[1], ctrl_in: Some(p012), ctrl_out: Some(p123), smooth: true }
            } else {
                let m = mid(a, b);
                PathNodeInfo { x: m[0], y: m[1], ctrl_in: None, ctrl_out: None, smooth: false }
            };
            nodes.insert(*after + 1, new);
            Ok(())
        }),
        Command::DeletePathNode { id, index } => edit(doc, *id, |nodes, closed| {
            if *index >= nodes.len() {
                return Err(range_err(*index));
            }
            let min = if closed { 3 } else { 2 };
            if nodes.len() <= min {
                return Err(CoreError::Invalid(format!("a path keeps at least {min} points")));
            }
            nodes.remove(*index);
            Ok(())
        }),
        Command::SetPathNodeKind { id, index, kind } => edit(doc, *id, |nodes, closed| {
            if *index >= nodes.len() {
                return Err(range_err(*index));
            }
            match kind {
                NodeKind::Smooth => make_smooth(nodes, *index, closed, false),
                NodeKind::Corner => {
                    let n = &mut nodes[*index];
                    n.ctrl_in = None;
                    n.ctrl_out = None;
                    n.smooth = false;
                }
            }
            Ok(())
        }),
        _ => Err(CoreError::Unsupported(format!("{cmd:?} is not a freeform command"))),
    }
}

/// Loads the path nodes in page space, runs `f`, then recomputes the rect and re-normalises the nodes.
fn edit(
    doc: &mut Document,
    id: Id,
    f: impl FnOnce(&mut Vec<PathNodeInfo>, bool) -> Result<(), CoreError>,
) -> Result<Applied, CoreError> {
    if doc.object_locked(id) {
        return Err(CoreError::Locked(id));
    }
    // An older polyline `Path` becomes the equivalent Bézier path (corner points, no handles) on its first edit.
    if let ObjectKind::Shape(Shape { kind: k @ ShapeKind::Path { .. }, .. }) = &mut doc.object_mut(id)?.kind
        && let ShapeKind::Path { points, closed } = k.clone()
    {
        let nodes =
            points.iter().map(|p| BezierNode { at: *p, ctrl_in: None, ctrl_out: None, smooth: false }).collect();
        *k = ShapeKind::Bezier { nodes, closed };
    }
    let o = doc.object(id)?;
    let (nodes, closed) = bezier_of(o)?;
    let mut page = to_page(nodes, o.rect);
    f(&mut page, closed)?;
    let rect = bounds(&page);
    let new = normalise(&page, rect);
    let o = doc.object_mut(id)?;
    o.rect = rect;
    if let ObjectKind::Shape(Shape { kind: ShapeKind::Bezier { nodes, .. }, .. }) = &mut o.kind {
        *nodes = new;
    }
    Ok(Applied::default())
}

impl Document {
    /// Points of a Bézier (or polyline `Path`) shape in page coordinates.
    pub fn path_nodes(&self, id: Id) -> Result<Vec<PathNodeInfo>, CoreError> {
        let o = self.object(id)?;
        if let ObjectKind::Shape(s) = &o.kind {
            match &s.kind {
                ShapeKind::Bezier { nodes, .. } => return Ok(to_page(nodes, o.rect)),
                ShapeKind::Path { points, .. } => {
                    let r = o.rect;
                    return Ok(points
                        .iter()
                        .map(|p| PathNodeInfo {
                            x: r.x + p[0] * r.w,
                            y: r.y + p[1] * r.h,
                            ctrl_in: None,
                            ctrl_out: None,
                            smooth: false,
                        })
                        .collect());
                }
                _ => {}
            }
        }
        Err(CoreError::WrongKind(id, "freeform path"))
    }
}
