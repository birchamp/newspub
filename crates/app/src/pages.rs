//! The page navigator: rendered thumbnails of every page, the current one highlighted.

use crate::{NewpubApp, icons as ic, theme, widgets};
use egui::{Align2, Color32, CornerRadius, Sense, Stroke, StrokeKind, Vec2};
use newpub_engine::core::Command;

/// Thumbnail width in screen points.
const THUMB_W: f32 = 84.0;

impl NewpubApp {
    pub(crate) fn page_navigator(&mut self, ui: &mut egui::Ui) {
        let p = widgets::pal(ui);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Pages").font(theme::semibold(13.0)).color(p.text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, ic::PLUS, "Insert Page After", false, true).clicked() {
                    let at = self.page + 1;
                    if self.act(Command::InsertPages { at: Some(at), count: 1, master: None }).is_some() {
                        self.page = at;
                    }
                }
            });
        });
        ui.add_space(4.0);
        let n = self.session.doc().pages.len();
        let (pw, ph) = {
            let s = &self.session.doc().setup;
            (s.width.0 as f32, s.height.0 as f32)
        };
        let th = THUMB_W * ph / pw.max(1.0);
        let rev = self.session.revision();
        let ppp = ui.ctx().pixels_per_point();
        // At most one stale thumbnail is re-rendered per frame, so editing a long publication stays smooth.
        let mut budget = 1;
        self.thumbs.retain(|k, _| *k < n);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for i in 0..n {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), th + 26.0), Sense::click());
                let label = format!("Page {}", i + 1);
                let on = self.page == i;
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, &label));
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let stale = self.thumbs.get(&i).is_none_or(|(_, r)| *r != rev);
                if stale && budget > 0 {
                    budget -= 1;
                    let dpi = 72.0 * (THUMB_W * ppp / pw.max(1.0)) as f64 * 1.5;
                    if let Ok(pm) = self.session.render_page(i, dpi) {
                        let img = egui::ColorImage::from_rgba_premultiplied(
                            [pm.width() as usize, pm.height() as usize],
                            pm.data(),
                        );
                        let tex = ui.ctx().load_texture(format!("thumb-{i}"), img, egui::TextureOptions::LINEAR);
                        self.thumbs.insert(i, (tex, rev));
                    }
                } else if stale {
                    ui.ctx().request_repaint();
                }
                let img_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 4.0 + th / 2.0),
                    Vec2::new(THUMB_W, th),
                );
                let painter = ui.painter();
                if on || resp.hovered() {
                    painter.rect_filled(
                        rect.shrink(1.0),
                        CornerRadius::same(8),
                        if on { p.primary_soft } else { p.surface_alt },
                    );
                }
                let shadow =
                    egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(36) };
                painter.add(shadow.as_shape(img_rect, 2.0));
                match self.thumbs.get(&i) {
                    Some((t, _)) => {
                        painter.image(
                            t.id(),
                            img_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => {
                        painter.rect_filled(img_rect, 0.0, Color32::WHITE);
                    }
                }
                let border = if on { Stroke::new(2.0, p.primary) } else { Stroke::new(1.0, p.border) };
                painter.rect_stroke(img_rect, CornerRadius::same(1), border, StrokeKind::Outside);
                painter.text(
                    egui::pos2(rect.center().x, rect.bottom() - 11.0),
                    Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    theme::medium(11.5),
                    if on { p.primary } else { p.text_muted },
                );
                if resp.clicked() {
                    self.page = i;
                    self.selection.clear();
                    self.end_text_edit();
                }
            }
        });
    }
}
