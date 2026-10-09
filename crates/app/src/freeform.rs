//! Freeform tool and on-canvas point editing (SH-08).

use crate::{NewpubApp, Tool};
use egui::{Color32, Pos2, Rect as ERect, Sense, Stroke, Vec2};
use newpub_engine::core::{self as core, Command, Id, Length, NodeKind, ObjectKind, ShapeKind};

/// A click this close (screen points) to the first point closes the shape.
const CLOSE_RADIUS: f32 = 6.0;
/// Side of a node handle, in screen points.
const HANDLE: f32 = 10.0;
const ACCENT: Color32 = Color32::from_rgb(40, 120, 220);

/// Freeform drawing and point-editing state.
#[derive(Default)]
pub(crate) struct FreeformState {
    /// Points placed so far with the Freeform tool, in page coordinates.
    points: Vec<[f64; 2]>,
    /// The path whose points are being edited.
    editing: Option<Id>,
    /// The selected point of the edited path.
    selected: Option<usize>,
    /// Node handle being dragged: point index, press position, pointer position.
    dragging: Option<(usize, Pos2, Pos2)>,
}

impl NewpubApp {
    /// The selected object when it is the only one and a path or Bézier shape.
    fn editable_path(&self) -> Option<Id> {
        let [id] = self.selection[..] else { return None };
        match &self.session.doc().objects.get(&id)?.kind {
            ObjectKind::Shape(s) if matches!(s.kind, ShapeKind::Bezier { .. } | ShapeKind::Path { .. }) => Some(id),
            _ => None,
        }
    }

    /// The path being point-edited, while it is still the single selection.
    fn editing_path(&self) -> Option<Id> {
        self.freeform.editing.filter(|id| self.editable_path() == Some(*id))
    }

    /// View-state fields for UI journeys.
    pub(crate) fn freeform_view(&self, v: &mut serde_json::Value) {
        v["point_editing"] = self.editing_path().is_some().into();
        v["selected_point"] = self.freeform.selected.into();
    }

    /// Canvas click with the Freeform tool: adds a point, or closes the shape on the first point.
    pub(crate) fn freeform_input(&mut self, resp: &egui::Response) {
        if !resp.clicked() {
            return;
        }
        let Some(p) = resp.interact_pointer_pos() else { return };
        let closes = self.freeform.points.len() >= 3
            && self
                .freeform
                .points
                .first()
                .is_some_and(|f| self.page_to_screen(f[0], f[1]).distance(p) <= CLOSE_RADIUS);
        if closes {
            self.freeform_finish(true);
        } else {
            let (x, y) = self.screen_to_page(p);
            self.freeform.points.push([x, y]);
        }
    }

    /// Enter: finishes an open shape.
    pub(crate) fn freeform_enter(&mut self) {
        if self.tool == Tool::Freeform {
            self.freeform_finish(false);
        }
    }

    /// Escape: cancels drawing, else leaves point editing. Returns whether it was used.
    pub(crate) fn freeform_escape(&mut self) -> bool {
        if !self.freeform.points.is_empty() {
            self.freeform.points.clear();
            true
        } else if self.freeform.editing.take().is_some() {
            self.freeform.selected = None;
            self.freeform.dragging = None;
            true
        } else {
            false
        }
    }

    fn freeform_finish(&mut self, closed: bool) {
        if self.freeform.points.len() < 2 {
            return;
        }
        let points = self.freeform.points.iter().map(|p| [Length(p[0]), Length(p[1])]).collect();
        let stroke = Some(core::Stroke {
            color: core::Color::BLACK,
            width: Length(1.0),
            dash: core::Dash::Solid,
            cap: core::LineCap::Butt,
            join: core::LineJoin::Miter,
        });
        let fill = closed.then(|| core::Color::rgb(0xc8, 0xd8, 0xf0));
        let page = Some(self.page);
        let out = self.act(Command::AddFreeform { page, master: None, points, closed, smooth: false, fill, stroke });
        if let Some(id) = out.and_then(|o| o.created.first().copied()) {
            self.freeform.points.clear();
            self.selection = vec![id];
            self.tool = Tool::Select;
        }
    }

    /// Ribbon controls: Edit Points, and the point commands while editing.
    pub(crate) fn freeform_controls(&mut self, ui: &mut egui::Ui) {
        use crate::{icons as ic, widgets::small_button};
        if small_button(
            ui,
            ic::BEZIER_CURVE,
            "Edit Points",
            self.editing_path().is_some(),
            self.editable_path().is_some(),
        )
        .clicked()
        {
            self.freeform.editing = self.editable_path();
            self.freeform.selected = None;
        }
        let Some(id) = self.editing_path() else { return };
        let sel = self.freeform.selected;
        let has = sel.is_some();
        let count = self.session.doc().path_nodes(id).map(|n| n.len()).unwrap_or(0);
        let col = |ui: &mut egui::Ui, a: (&str, &str), b: (&str, &str)| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let x = small_button(ui, a.0, a.1, false, has).clicked();
                let y = small_button(ui, b.0, b.1, false, has).clicked();
                (x, y)
            })
            .inner
        };
        let (smooth, corner) = col(ui, (ic::WAVE_SINE, "Smooth Point"), (ic::CORNERS_OUT, "Corner Point"));
        let (add, delete) = col(ui, (ic::PLUS_CIRCLE, "Add Point"), (ic::MINUS_CIRCLE, "Delete Point"));
        let Some(index) = sel.filter(|i| *i < count) else { return };
        if smooth {
            self.act(Command::SetPathNodeKind { id, index, kind: NodeKind::Smooth });
        } else if corner {
            self.act(Command::SetPathNodeKind { id, index, kind: NodeKind::Corner });
        } else if delete {
            if self.act(Command::DeletePathNode { id, index }).is_some() {
                self.freeform.selected = None;
            }
        } else if add && self.act(Command::InsertPathNode { id, after: index }).is_some() {
            self.freeform.selected = Some(index + 1);
        }
    }

    /// Paints the in-progress polyline and the point handles of the edited path, and makes each handle a
    /// named, clickable and draggable widget ("Point 1" ...).
    pub(crate) fn freeform_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        let painter = ui.painter().with_clip_rect(canvas);
        if self.tool != Tool::Freeform {
            self.freeform.points.clear();
        }
        let pts: Vec<Pos2> = self.freeform.points.iter().map(|p| self.page_to_screen(p[0], p[1])).collect();
        if pts.len() >= 2 {
            painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, ACCENT)));
        }
        for p in &pts {
            painter.circle_filled(*p, 3.0, ACCENT);
        }

        let Some(id) = self.editing_path() else {
            self.freeform.editing = None;
            self.freeform.dragging = None;
            return;
        };
        let Ok(nodes) = self.session.doc().path_nodes(id) else { return };
        let zoom = self.zoom;
        let grey = Stroke::new(1.0, Color32::from_gray(120));
        let mut moved = None;
        for (i, n) in nodes.iter().enumerate() {
            let c = self.page_to_screen(n.x, n.y);
            let shown = match self.freeform.dragging {
                Some((d, a, b)) if d == i => c + (b - a),
                _ => c,
            };
            if n.smooth {
                for h in [n.ctrl_in, n.ctrl_out].into_iter().flatten() {
                    let hp = self.page_to_screen(h[0], h[1]) + (shown - c);
                    painter.line_segment([shown, hp], grey);
                    painter.circle(hp, 3.0, Color32::WHITE, Stroke::new(1.0, ACCENT));
                }
            }
            let selected = self.freeform.selected == Some(i);
            let sq = ERect::from_center_size(shown, Vec2::splat(HANDLE - 2.0));
            painter.rect_filled(sq, 0.0, if selected { ACCENT } else { Color32::WHITE });
            painter.rect_stroke(sq, 0.0, Stroke::new(1.0, ACCENT), egui::StrokeKind::Middle);

            let label = format!("Point {}", i + 1);
            let resp = ui.interact(
                ERect::from_center_size(c, Vec2::splat(HANDLE + 2.0)),
                ui.id().with(("path-point", i)),
                Sense::click_and_drag(),
            );
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &label));
            if resp.drag_started() {
                let origin = ui.ctx().input(|inp| inp.pointer.press_origin()).unwrap_or(c);
                self.freeform.selected = Some(i);
                self.freeform.dragging = Some((i, origin, origin));
            }
            if resp.dragged()
                && let (Some((d, a, _)), Some(p)) = (self.freeform.dragging, resp.interact_pointer_pos())
                && d == i
            {
                self.freeform.dragging = Some((d, a, p));
            }
            if resp.drag_stopped()
                && let Some((d, a, b)) = self.freeform.dragging.take()
            {
                let b = ui.ctx().input(|inp| inp.pointer.latest_pos()).unwrap_or(b);
                let delta = (b - a) / zoom;
                if delta.length() > 0.5 {
                    moved = Some((d, n.x + delta.x as f64, n.y + delta.y as f64));
                }
            } else if resp.clicked() {
                self.freeform.selected = Some(i);
            }
        }
        if let Some((index, x, y)) = moved {
            self.act(Command::MovePathNode { id, index, x: Length(x), y: Length(y) });
        }
    }
}
