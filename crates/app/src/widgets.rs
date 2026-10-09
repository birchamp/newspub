//! Custom widgets of the newpub look. Each reports its plain-text label to AccessKit, so screen readers and UI
//! journeys find "Text Box" even though the button shows an icon above the words.

use crate::theme::{self, Palette};
use egui::{Align2, Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType};

fn info(resp: &Response, kind: WidgetType, enabled: bool, label: &str, selected: Option<bool>) {
    let label = label.to_string();
    resp.widget_info(move || {
        let mut i = WidgetInfo::labeled(kind, enabled, &label);
        i.selected = selected;
        i
    });
}

/// Large ribbon button: icon over a label. `selected` shows it as on (tools, toggles).
pub fn ribbon_button(ui: &mut Ui, icon: &str, label: &str, selected: bool, enabled: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::medium(11.5), p.text);
    let w = (galley.size().x + 14.0).max(52.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 58.0), sense);
    let kind = if selected { WidgetType::SelectableLabel } else { WidgetType::Button };
    info(&resp, kind, enabled, label, selected.then_some(true));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let (fill, fg) = if selected {
            (p.primary_soft, p.primary)
        } else if resp.hovered() && enabled {
            (p.surface_alt, p.text)
        } else {
            (Color32::TRANSPARENT, p.text)
        };
        let fg = if enabled { fg } else { p.text_muted.gamma_multiply(0.6) };
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        if selected {
            painter.rect_stroke(
                rect,
                CornerRadius::same(8),
                Stroke::new(1.0, p.primary.gamma_multiply(0.35)),
                StrokeKind::Inside,
            );
        }
        let icon_color = if selected || !enabled { fg } else { p.primary };
        painter.text(
            rect.center_top() + Vec2::new(0.0, 19.0),
            Align2::CENTER_CENTER,
            icon,
            theme::icon_font(22.0),
            icon_color,
        );
        painter.text(
            rect.center_bottom() - Vec2::new(0.0, 12.0),
            Align2::CENTER_CENTER,
            label,
            theme::medium(11.5),
            fg,
        );
    }
    resp.on_hover_cursor(if enabled { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default })
}

/// Compact ribbon button: small icon left of a label, one row (stack two or three in a group).
pub fn small_button(ui: &mut Ui, icon: &str, label: &str, selected: bool, enabled: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::medium(12.0), p.text);
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(galley.size().x + 36.0, 26.0),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let kind = if selected { WidgetType::SelectableLabel } else { WidgetType::Button };
    info(&resp, kind, enabled, label, selected.then_some(true));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if selected {
            p.primary_soft
        } else if resp.hovered() && enabled {
            p.surface_alt
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(6), fill);
        let fg = if !enabled {
            p.text_muted.gamma_multiply(0.6)
        } else if selected {
            p.primary
        } else {
            p.text
        };
        let ic = if enabled { p.primary } else { fg };
        painter.text(
            rect.left_center() + Vec2::new(14.0, 0.0),
            Align2::CENTER_CENTER,
            icon,
            theme::icon_font(16.0),
            ic,
        );
        painter.text(rect.left_center() + Vec2::new(28.0, 0.0), Align2::LEFT_CENTER, label, theme::medium(12.0), fg);
    }
    resp
}

/// Square icon-only button with a tooltip; the label is its accessible name.
pub fn icon_button(ui: &mut Ui, icon: &str, label: &str, selected: bool, enabled: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), if enabled { Sense::click() } else { Sense::hover() });
    let kind = if selected { WidgetType::SelectableLabel } else { WidgetType::Button };
    info(&resp, kind, enabled, label, selected.then_some(true));
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.primary_soft
        } else if resp.hovered() && enabled {
            p.surface_alt
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        let fg = if !enabled {
            p.text_muted.gamma_multiply(0.5)
        } else if selected {
            p.primary
        } else {
            p.text
        };
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, theme::icon_font(17.0), fg);
    }
    resp.on_hover_text(label)
}

/// Icon button drawn in white on the gradient header.
pub fn header_button(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), if enabled { Sense::click() } else { Sense::hover() });
    info(&resp, WidgetType::Button, enabled, label, None);
    if ui.is_rect_visible(rect) {
        if resp.hovered() && enabled {
            ui.painter().rect_filled(rect, CornerRadius::same(7), Color32::from_white_alpha(38));
        }
        let fg = if enabled { Color32::WHITE } else { Color32::from_white_alpha(90) };
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, theme::icon_font(18.0), fg);
    }
    resp.on_hover_text(label)
}

/// Header pill button (white outline on the gradient), icon plus label.
pub fn header_pill(ui: &mut Ui, icon: &str, label: &str, solid: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::medium(12.5), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 40.0, 30.0), Sense::click());
    info(&resp, WidgetType::Button, true, label, None);
    if ui.is_rect_visible(rect) {
        let r = CornerRadius::same(15);
        let (fill, fg) = if solid {
            (if resp.hovered() { Color32::from_rgb(0xF0, 0xFA, 0xF8) } else { Color32::WHITE }, theme::VENICE)
        } else {
            (Color32::from_white_alpha(if resp.hovered() { 46 } else { 26 }), Color32::WHITE)
        };
        ui.painter().rect_filled(rect, r, fill);
        if !solid {
            ui.painter().rect_stroke(rect, r, Stroke::new(1.0, Color32::from_white_alpha(80)), StrokeKind::Inside);
        }
        ui.painter().text(
            rect.left_center() + Vec2::new(16.0, 0.0),
            Align2::CENTER_CENTER,
            icon,
            theme::icon_font(15.0),
            fg,
        );
        ui.painter().text(
            rect.left_center() + Vec2::new(29.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            theme::medium(12.5),
            fg,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Gradient call-to-action button (Create, Export, Send to Printer).
pub fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::semibold(13.0), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 32.0, 32.0), Sense::click());
    info(&resp, WidgetType::Button, true, label, None);
    if ui.is_rect_visible(rect) {
        let (a, b) = if resp.hovered() { (p.primary_hover, p.grad_b) } else { (p.grad_a, p.grad_b) };
        theme::gradient_rect(ui.painter(), rect, a, b, 8.0);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::semibold(13.0), Color32::WHITE);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Quiet outlined button (Cancel and secondary actions).
pub fn secondary_button(ui: &mut Ui, label: &str) -> Response {
    let p = theme::palette(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::medium(13.0), p.text);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0, 32.0), Sense::click());
    info(&resp, WidgetType::Button, true, label, None);
    if ui.is_rect_visible(rect) {
        let r = CornerRadius::same(8);
        ui.painter().rect_filled(rect, r, if resp.hovered() { p.surface_alt } else { p.surface });
        ui.painter().rect_stroke(rect, r, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::medium(13.0), p.text);
    }
    resp
}

/// A ribbon group: its buttons in a row with a muted caption underneath, then a divider.
pub fn group<R>(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    let r = ui
        .vertical(|ui| {
            let r = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                add(ui)
            });
            ui.add_space(1.0);
            let w = ui.min_rect().width();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 13.0), Sense::hover());
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                caption,
                egui::FontId::proportional(10.5),
                p.text_muted,
            );
            r.inner
        })
        .inner;
    divider(ui, 72.0);
    r
}

/// A thin vertical divider of `height`.
pub fn divider(ui: &mut Ui, height: f32) {
    let p = theme::palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(13.0, height), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range().shrink(6.0), Stroke::new(1.0, p.border));
}

/// A card section of the inspector with a small caps title.
pub fn section<R>(ui: &mut Ui, icon: &str, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = theme::palette(ui.ctx());
    egui::Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).font(theme::icon_font(14.0)).color(p.primary));
                ui.label(egui::RichText::new(title).font(theme::semibold(12.5)).color(p.text));
            });
            ui.add_space(2.0);
            add(ui)
        })
        .inner
}

/// Muted helper text.
pub fn hint(ui: &mut Ui, text: &str) {
    let p = theme::palette(ui.ctx());
    ui.label(egui::RichText::new(text).size(12.0).color(p.text_muted));
}

/// The palette for `ui` (shorthand).
pub fn pal(ui: &Ui) -> Palette {
    theme::palette(ui.ctx())
}

/// A small colour chip button; `label` is its accessible name.
pub fn color_chip(ui: &mut Ui, color: Color32, label: &str, selected: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let kind = if selected { WidgetType::SelectableLabel } else { WidgetType::Button };
    info(&resp, kind, true, label, selected.then_some(true));
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, CornerRadius::same(5), color);
        let stroke = if selected { Stroke::new(2.0, p.primary) } else { Stroke::new(1.0, p.border) };
        ui.painter().rect_stroke(rect, CornerRadius::same(5), stroke, StrokeKind::Inside);
    }
    resp.on_hover_text(label).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A labelled hex colour field (`#rrggbb`); returns the colour when the text was edited and parses.
pub fn color_field(ui: &mut Ui, label: &str, value: &mut String) -> Option<newpub_engine::core::Color> {
    use newpub_engine::core::Color;
    let changed = ui
        .horizontal(|ui| {
            let l = ui.label(label);
            let r = ui.add(egui::TextEdit::singleline(value).desired_width(90.0)).labelled_by(l.id);
            if let Some(c) = Color::parse(value) {
                let [r8, g8, b8, _] = c.to_rgba8();
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(r8, g8, b8));
            }
            r.changed()
        })
        .inner;
    if changed { Color::parse(value) } else { None }
}

/// A colour swatch `size` points square. `rgba` None draws the "no colour" slash. With `click` it is a button
/// named `label`; otherwise a passive preview.
pub fn swatch(ui: &mut Ui, rgba: Option<[u8; 4]>, size: f32, label: &str, selected: bool, click: bool) -> Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), if click { Sense::click() } else { Sense::hover() });
    if click {
        info(&resp, WidgetType::Button, true, label, selected.then_some(true));
    }
    if ui.is_rect_visible(rect) {
        let r = CornerRadius::same(5);
        match rgba {
            Some([r8, g8, b8, a8]) => {
                ui.painter().rect_filled(rect, r, Color32::from_rgba_unmultiplied(r8, g8, b8, a8));
            }
            None => {
                ui.painter().rect_filled(rect, r, p.surface);
                ui.painter().line_segment(
                    [rect.left_bottom() + Vec2::new(3.0, -3.0), rect.right_top() + Vec2::new(-3.0, 3.0)],
                    Stroke::new(1.5, p.danger),
                );
            }
        }
        let (w, col) = if selected {
            (2.0, p.primary)
        } else if resp.hovered() && click {
            (1.5, p.text_muted)
        } else {
            (1.0, p.border)
        };
        ui.painter().rect_stroke(rect, r, Stroke::new(w, col), StrokeKind::Inside);
    }
    if click { resp.on_hover_text(label).on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}
