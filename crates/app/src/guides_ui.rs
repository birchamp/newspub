//! Ruler guides on the canvas: drag one out of a ruler, drag it to move it, drag it back onto a ruler to remove it.

use eframe::egui::{self, Color32, Pos2, Rect as ERect, Stroke};
use newpub_engine::core::{Command, Guide, Id, Length, Orientation};

use crate::NewpubApp;

/// How close (screen points) the pointer must be to grab a guide.
const GRAB: f32 = 4.0;

/// A guide being dragged: an existing one (`id`) or a new one coming out of a ruler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuideDrag {
    pub id: Option<Id>,
    pub orientation: Orientation,
}

fn guide_color() -> Color32 {
    Color32::from_rgb(0, 160, 200)
}

impl NewpubApp {
    /// Ruler guides shown on the current page: its own and its master's.
    pub(crate) fn visible_guides(&self) -> Vec<Guide> {
        let doc = self.session.doc();
        let Some(page) = doc.pages.get(self.page) else { return vec![] };
        let master = (!page.ignore_master).then_some(page.master).flatten();
        doc.guides
            .ruler
            .iter()
            .filter(|g| g.page == Some(page.id) || (g.master.is_some() && g.master == master))
            .cloned()
            .collect()
    }

    /// The guide under screen point `p`.
    pub(crate) fn guide_at(&self, p: Pos2) -> Option<Guide> {
        let (x, y) = self.screen_to_page(p);
        let tol = (GRAB / self.zoom) as f64;
        self.visible_guides().into_iter().find(|g| match g.orientation {
            Orientation::Horizontal => (g.pos.0 - y).abs() <= tol,
            Orientation::Vertical => (g.pos.0 - x).abs() <= tol,
        })
    }

    /// Guides across the canvas, and the one being dragged at the pointer.
    pub(crate) fn draw_ruler_guides(&self, painter: &egui::Painter, canvas: ERect, pointer: Option<Pos2>) {
        let stroke = Stroke::new(1.0, guide_color());
        let line = |o: Orientation, at: f32| match o {
            Orientation::Horizontal => [Pos2::new(canvas.left(), at), Pos2::new(canvas.right(), at)],
            Orientation::Vertical => [Pos2::new(at, canvas.top()), Pos2::new(at, canvas.bottom())],
        };
        let dragged = self.view.guide_drag.and_then(|d| d.id);
        for g in self.visible_guides().iter().filter(|g| Some(g.id) != dragged) {
            let at = match g.orientation {
                Orientation::Horizontal => self.page_to_screen(0.0, g.pos.0).y,
                Orientation::Vertical => self.page_to_screen(g.pos.0, 0.0).x,
            };
            painter.line_segment(line(g.orientation, at), stroke);
        }
        if let (Some(d), Some(p)) = (self.view.guide_drag, pointer) {
            let at = if d.orientation == Orientation::Horizontal { p.y } else { p.x };
            painter.line_segment(line(d.orientation, at), Stroke::new(1.5, guide_color()));
        }
    }

    /// Ends a guide drag at screen point `p`: dropped on the page it adds or moves the guide; dropped outside the
    /// canvas (onto a ruler) it removes it.
    pub(crate) fn drop_guide(&mut self, p: Option<Pos2>, canvas: ERect) {
        let Some(d) = self.view.guide_drag.take() else { return };
        let Some(p) = p else { return };
        let (x, y) = self.screen_to_page(p);
        let pos = Length(if d.orientation == Orientation::Horizontal { y } else { x });
        let on_canvas = canvas.contains(p);
        match (d.id, on_canvas) {
            (None, true) => {
                let page = Some(self.page);
                self.act(Command::AddGuide { page, master: None, orientation: d.orientation, pos });
            }
            (Some(guide), true) => {
                self.act(Command::MoveGuide { guide, pos });
            }
            (Some(guide), false) => {
                self.act(Command::DeleteGuide { guide });
            }
            (None, false) => {}
        }
    }

    /// Rulers: a drag out of one makes a new guide.
    pub(crate) fn ruler_drag(&mut self, resp: &egui::Response, orientation: Orientation, canvas: ERect) {
        if resp.hovered() || resp.dragged() {
            resp.ctx.set_cursor_icon(if orientation == Orientation::Horizontal {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
        }
        if resp.drag_started() {
            self.view.guide_drag = Some(GuideDrag { id: None, orientation });
        }
        if resp.drag_stopped() {
            let p = resp.interact_pointer_pos().or(resp.ctx.pointer_latest_pos());
            self.drop_guide(p, canvas);
        }
    }
}
