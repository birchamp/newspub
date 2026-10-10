//! Text-flow feedback on the canvas (UI-26): the chain of boxes a selected story runs through, Previous and
//! Next box buttons, and the "Link to Next Box" mode with its hint and the boxes the story may continue in.
//! Everything here is view state; the only document change is the `LinkFrames` command in `finish_link`.

use egui::{Align2, Color32, Pos2, Rect as ERect, Sense, Stroke, Vec2};
use newpub_engine::core::{Id, ObjectKind, Rect};

use crate::{NewpubApp, Tool, dashed_rect, icons, theme};

/// Radius of a Previous / Next chip on the selected box.
const CHIP: f32 = 9.0;

impl NewpubApp {
    /// The chain of text box `frame`: its index and every box of its story in reading order. None unless the
    /// story runs through two or more boxes.
    pub(crate) fn chain_of(&self, frame: Id) -> Option<(usize, Vec<Id>)> {
        let doc = self.session.doc();
        let frames = doc.story_of(frame).ok().and_then(|s| doc.story(s).ok()).map(|s| s.frames.clone())?;
        if frames.len() < 2 {
            return None;
        }
        let i = frames.iter().position(|f| *f == frame)?;
        Some((i, frames))
    }

    /// The chain of the one selected text box (None for an empty or multiple selection).
    pub(crate) fn selected_chain(&self) -> Option<(usize, Vec<Id>)> {
        if self.selection.len() != 1 {
            return None;
        }
        self.chain_of(self.selected_text_frame()?)
    }

    /// Selects the box before (`forward == false`) or after `frame` in its story, turning to its page.
    /// Nothing happens at either end of the chain.
    pub(crate) fn go_to_box(&mut self, frame: Id, forward: bool) {
        let Some((i, frames)) = self.chain_of(frame) else { return };
        let j = match (forward, i) {
            (true, i) => i + 1,
            (false, 0) => return,
            (false, i) => i - 1,
        };
        let Some(&next) = frames.get(j) else { return };
        let Some(p) = self.session.doc().page_of(next) else {
            self.status = "That text box is on a master page".into();
            return;
        };
        self.end_text_edit();
        self.page = p;
        self.selection = vec![next];
        self.status = format!("Selected box {} of {} in this story", j + 1, frames.len());
    }

    /// True when `id` is a text box that "Link to Next Box" may continue a story in: empty and not yet linked.
    pub(crate) fn is_link_target(&self, id: Id) -> bool {
        let doc = self.session.doc();
        doc.objects.get(&id).is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
            && doc
                .story_of(id)
                .ok()
                .and_then(|s| doc.story(s).ok())
                .is_some_and(|s| s.is_empty() && s.frames.len() <= 1)
    }

    /// The boxes on the current page that the story being linked may continue in.
    pub(crate) fn link_targets(&self) -> Vec<Id> {
        let Some(from) = self.view.link_from else { return Vec::new() };
        let doc = self.session.doc();
        let Some(page) = doc.pages.get(self.page) else { return Vec::new() };
        page.objects.iter().copied().filter(|id| *id != from && self.is_link_target(*id)).collect()
    }

    /// Leaves "Link to Next Box" mode without linking.
    pub(crate) fn cancel_link(&mut self) {
        if self.view.link_from.take().is_some() {
            self.status = "Linking cancelled".into();
        }
    }

    /// Screen rect of a page rect on the current page.
    fn screen_rect(&self, r: Rect) -> ERect {
        ERect::from_min_max(self.page_to_screen(r.x, r.y), self.page_to_screen(r.right(), r.bottom()))
    }

    /// Canvas overlay drawn after the page: the link-mode hint and targets, then the chain of the selected box.
    pub(crate) fn flow_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        self.link_overlay(ui, canvas);
        self.chain_overlay(ui, canvas);
    }

    /// While linking: the source box, every box the story may continue in, and a banner with a Cancel button.
    fn link_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        let Some(from) = self.view.link_from else { return };
        let p = theme::palette(ui.ctx());
        let painter = ui.painter().with_clip_rect(canvas);
        let hover = ui
            .ctx()
            .pointer_hover_pos()
            .filter(|q| canvas.contains(*q))
            .map(|q| self.screen_to_page(q))
            .and_then(|(x, y)| self.hit(x, y));
        let doc = self.session.doc();
        if let Some(o) = doc.objects.get(&from).filter(|_| doc.page_of(from) == Some(self.page)) {
            dashed_rect(&painter, self.screen_rect(o.rect), Stroke::new(2.0, p.primary), 6.0);
        }
        let icon = theme::icon_font(16.0 * self.zoom.clamp(0.6, 1.6));
        for id in self.link_targets() {
            let Some(o) = doc.objects.get(&id) else { continue };
            let rr = self.screen_rect(o.rect);
            if hover == Some(id) {
                painter.rect_filled(rr, 0.0, p.success.gamma_multiply(0.1));
                painter.rect_stroke(rr, 0.0, Stroke::new(2.0, p.success), egui::StrokeKind::Outside);
            } else {
                dashed_rect(&painter, rr, Stroke::new(1.5, p.success), 5.0);
            }
            painter.text(
                rr.center() - Vec2::new(0.0, 18.0),
                Align2::CENTER_CENTER,
                icons::LINK,
                icon.clone(),
                p.success,
            );
        }

        // Banner: what to do next, and a way out.
        let text = "Click an empty text box to continue the story there";
        let galley = painter.layout_no_wrap(text.to_string(), theme::medium(12.0), p.on_primary);
        let size = Vec2::new(galley.size().x + 70.0, 30.0);
        let r = ERect::from_center_size(Pos2::new(canvas.center().x, canvas.top() + 12.0 + size.y / 2.0), size);
        // Click-sensing (not hover), so a click on the banner never reaches the canvas underneath.
        let banner = ui.interact(r, ui.id().with("link-banner"), Sense::click());
        banner.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Linking text boxes"));
        let cr = ERect::from_center_size(r.right_center() - Vec2::new(17.0, 0.0), Vec2::splat(24.0));
        let cancel = ui
            .interact(cr, ui.id().with("link-cancel"), Sense::click())
            .on_hover_text("Stop linking (Esc)")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        cancel.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Cancel linking"));
        painter.rect_filled(r, 15.0, p.primary);
        painter.text(
            r.left_center() + Vec2::new(16.0, 0.0),
            Align2::CENTER_CENTER,
            icons::LINK,
            theme::icon_font(16.0),
            p.on_primary,
        );
        painter.galley(r.left_center() + Vec2::new(30.0, -galley.size().y / 2.0), galley, p.on_primary);
        if cancel.hovered() {
            painter.circle_filled(cr.center(), 11.0, Color32::from_white_alpha(60));
        }
        painter.text(cr.center(), Align2::CENTER_CENTER, icons::X, theme::icon_font(14.0), p.on_primary);
        if cancel.clicked() {
            self.cancel_link();
        }
    }

    /// With one box of a linked story selected: a numbered tag on every box of the story on this page, a
    /// dashed outline on the other boxes, dashed connectors between consecutive boxes, and Previous / Next
    /// chips on the selected box.
    fn chain_overlay(&mut self, ui: &mut egui::Ui, canvas: ERect) {
        if self.tool != Tool::Select || self.view.link_from.is_some() {
            return;
        }
        let Some((i, frames)) = self.selected_chain() else { return };
        let p = theme::palette(ui.ctx());
        let painter = ui.painter().with_clip_rect(canvas);
        let n = frames.len();
        let rects: Vec<Option<ERect>> = {
            let doc = self.session.doc();
            frames
                .iter()
                .map(|f| {
                    doc.objects.get(f).filter(|_| doc.page_of(*f) == Some(self.page)).map(|o| self.screen_rect(o.rect))
                })
                .collect()
        };
        let faint = Stroke::new(1.0, p.primary.gamma_multiply(0.7));
        for (k, rr) in rects.iter().enumerate() {
            let Some(rr) = rr else { continue };
            if k != i {
                dashed_rect(&painter, *rr, faint, 4.0);
            }
            // Numbered tag inside the top-left corner.
            let label = format!("{} of {}", k + 1, n);
            let galley = painter.layout_no_wrap(label, theme::medium(10.0), p.on_primary);
            let tag = ERect::from_min_size(rr.left_top() + Vec2::splat(7.0), galley.size() + Vec2::new(8.0, 3.0));
            painter.rect_filled(tag, 3.0, p.primary);
            painter.galley(tag.left_top() + Vec2::new(4.0, 1.5), galley, p.on_primary);
            // Connector to the next box when both are on this page.
            if let Some(Some(next)) = rects.get(k + 1) {
                let (a, b) = (chips(*rr).1, chips(*next).0);
                painter.extend(egui::Shape::dashed_line(&[a, b], faint, 4.0, 4.0));
            }
        }
        let Some(sel) = rects[i] else { return };
        let frame = frames[i];
        let (prev_c, next_c) = chips(sel);
        let mut go = None;
        for (c, forward, exists, icon, label, hint) in [
            (
                prev_c,
                false,
                i > 0,
                icons::CARET_LEFT,
                "Go to previous text box",
                "The box before this one in the story (Shift+Tab)",
            ),
            (
                next_c,
                true,
                i + 1 < n,
                icons::CARET_RIGHT,
                "Go to next text box",
                "The box the story continues in (Tab)",
            ),
        ] {
            if !exists {
                continue;
            }
            let resp = ui
                .interact(
                    ERect::from_center_size(c, Vec2::splat(2.0 * CHIP)),
                    ui.id().with(("flow-chip", label)),
                    Sense::click(),
                )
                .on_hover_text(hint)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
            painter.circle_filled(c, CHIP, if resp.hovered() { p.primary_hover } else { p.primary });
            painter.circle_stroke(c, CHIP, Stroke::new(1.5, p.on_primary));
            painter.text(c, Align2::CENTER_CENTER, icon, theme::icon_font(12.0), p.on_primary);
            if resp.clicked() {
                go = Some(forward);
            }
        }
        if let Some(forward) = go {
            self.go_to_box(frame, forward);
        }
    }

    /// Extra `view` query fields for UI journeys: link mode, its targets, the status message and the chain.
    pub(crate) fn flow_view(&self, v: &mut serde_json::Value) {
        v["link_mode"] = serde_json::json!(self.view.link_from);
        v["link_targets"] = serde_json::json!(self.link_targets());
        v["status"] = self.status.clone().into();
        v["story_nav"] = match self.selected_chain() {
            Some((i, frames)) => serde_json::json!({ "index": i, "count": frames.len(), "frames": frames }),
            None => serde_json::Value::Null,
        };
    }
}

/// Centres of the Previous (above the top-left corner) and Next (below the bottom-right corner) chips of a box,
/// clear of its resize handles and of the overflow badge.
fn chips(rr: ERect) -> (Pos2, Pos2) {
    (Pos2::new(rr.left() + 18.0, rr.top() - 12.0), Pos2::new(rr.right() - 18.0, rr.bottom() + 12.0))
}
