//! The newpub look: a Venice Blue palette (#085078 → #85D8CE) on calm slate neutrals, the Inter UI typeface and
//! Phosphor icons. Light and dark palettes share the same roles; widgets read colours from [`Palette`].

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2};
use std::sync::Arc;

/// Font family of the regular Phosphor icons.
pub const ICONS: &str = "phosphor";
/// Font family of the filled Phosphor icons.
pub const ICONS_FILL: &str = "phosphor-fill";
/// Medium-weight Inter (labels, tabs).
pub const MEDIUM: &str = "inter-medium";
/// Semi-bold Inter (headings).
pub const SEMIBOLD: &str = "inter-semibold";

/// Venice Blue, the brand colour.
pub const VENICE: Color32 = Color32::from_rgb(0x08, 0x50, 0x78);
/// The light end of the brand gradient.
pub const SEAFOAM: Color32 = Color32::from_rgb(0x85, 0xD8, 0xCE);

/// Colour roles. Every custom widget draws with these, so light and dark stay consistent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    /// Window background behind panels.
    pub app_bg: Color32,
    /// Panels, cards, popups.
    pub surface: Color32,
    /// Slightly raised areas inside surfaces (inputs, hovered rows).
    pub surface_alt: Color32,
    /// Area around the pages.
    pub pasteboard: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    /// Primary action / selection colour.
    pub primary: Color32,
    /// Lighter primary for hover.
    pub primary_hover: Color32,
    /// Very light primary tint for selected rows and active tabs.
    pub primary_soft: Color32,
    /// Text on primary fills.
    pub on_primary: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    /// Gradient stops of the header and primary buttons.
    pub grad_a: Color32,
    pub grad_b: Color32,
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        dark: false,
        app_bg: Color32::from_rgb(0xF3, 0xF6, 0xF8),
        surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        surface_alt: Color32::from_rgb(0xF1, 0xF5, 0xF8),
        pasteboard: Color32::from_rgb(0xE3, 0xEA, 0xEF),
        border: Color32::from_rgb(0xDB, 0xE3, 0xEA),
        text: Color32::from_rgb(0x0F, 0x1E, 0x2B),
        text_muted: Color32::from_rgb(0x5B, 0x6B, 0x7A),
        primary: VENICE,
        primary_hover: Color32::from_rgb(0x0B, 0x67, 0x98),
        primary_soft: Color32::from_rgb(0xE2, 0xF1, 0xF6),
        on_primary: Color32::WHITE,
        accent: Color32::from_rgb(0x2F, 0xA8, 0x9A),
        danger: Color32::from_rgb(0xD1, 0x43, 0x43),
        success: Color32::from_rgb(0x1F, 0x9D, 0x6B),
        grad_a: VENICE,
        grad_b: Color32::from_rgb(0x3E, 0x9F, 0xA8),
    };

    pub const DARK: Palette = Palette {
        dark: true,
        app_bg: Color32::from_rgb(0x0B, 0x14, 0x1D),
        surface: Color32::from_rgb(0x12, 0x1D, 0x29),
        surface_alt: Color32::from_rgb(0x19, 0x26, 0x34),
        pasteboard: Color32::from_rgb(0x0E, 0x18, 0x23),
        border: Color32::from_rgb(0x24, 0x33, 0x43),
        text: Color32::from_rgb(0xE6, 0xEE, 0xF4),
        text_muted: Color32::from_rgb(0x8A, 0x9B, 0xAB),
        primary: Color32::from_rgb(0x3A, 0xA6, 0xD4),
        primary_hover: Color32::from_rgb(0x5C, 0xBA, 0xE0),
        primary_soft: Color32::from_rgb(0x15, 0x34, 0x47),
        on_primary: Color32::from_rgb(0x06, 0x1A, 0x26),
        accent: SEAFOAM,
        danger: Color32::from_rgb(0xF0, 0x6A, 0x6A),
        success: Color32::from_rgb(0x4C, 0xC9, 0x95),
        grad_a: Color32::from_rgb(0x08, 0x50, 0x78),
        grad_b: Color32::from_rgb(0x2F, 0x8C, 0x93),
    };
}

/// The palette in effect for `ctx`.
pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.global_style().visuals.dark_mode { Palette::DARK } else { Palette::LIGHT }
}

/// Registers Inter and the Phosphor icon fonts. Icons also fall back into the proportional family so a glyph
/// in a plain label still renders.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add(&mut fonts, "inter", include_bytes!("../../../assets/ui/Inter-Regular.ttf"));
    add(&mut fonts, MEDIUM, include_bytes!("../../../assets/ui/Inter-Medium.ttf"));
    add(&mut fonts, SEMIBOLD, include_bytes!("../../../assets/ui/Inter-SemiBold.ttf"));
    add(&mut fonts, ICONS, include_bytes!("../../../assets/ui/Phosphor.ttf"));
    add(&mut fonts, ICONS_FILL, include_bytes!("../../../assets/ui/Phosphor-Fill.ttf"));
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".into());
    prop.push(ICONS.into());
    for (family, first) in [(MEDIUM, MEDIUM), (SEMIBOLD, SEMIBOLD), (ICONS, ICONS), (ICONS_FILL, ICONS_FILL)] {
        let mut chain = vec![first.to_string()];
        if family == MEDIUM || family == SEMIBOLD {
            chain.extend(["inter".to_string(), ICONS.to_string()]);
        }
        let rest: Vec<String> =
            fonts.families[&FontFamily::Proportional].iter().filter(|f| !chain.contains(f)).cloned().collect();
        chain.extend(rest);
        fonts.families.insert(FontFamily::Name(family.into()), chain);
    }
    ctx.set_fonts(fonts);
}

/// Font of the regular icons at `size`.
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICONS.into()))
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Configures egui's light and dark styles from the two palettes (the theme preference picks one).
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.style_mut_of(egui::Theme::Light, |s| configure(s, &Palette::LIGHT));
    ctx.style_mut_of(egui::Theme::Dark, |s| configure(s, &Palette::DARK));
}

/// Applies a palette to egui's built-in widgets.
fn configure(style: &mut egui::Style, p: &Palette) {
    {
        let r6 = CornerRadius::same(6);
        let v = &mut style.visuals;
        *v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = p.surface;
        v.window_fill = p.surface;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.window_shadow = Shadow {
            offset: [0, 10],
            blur: 32,
            spread: 0,
            color: Color32::from_black_alpha(if p.dark { 110 } else { 38 }),
        };
        v.popup_shadow = Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(if p.dark { 100 } else { 30 }),
        };
        v.faint_bg_color = p.surface_alt;
        v.extreme_bg_color = if p.dark { Color32::from_rgb(0x0D, 0x17, 0x22) } else { Color32::WHITE };
        v.text_edit_bg_color = Some(if p.dark { Color32::from_rgb(0x0D, 0x17, 0x22) } else { Color32::WHITE });
        v.hyperlink_color = p.primary;
        v.selection.bg_fill = if p.dark { p.primary_soft } else { Color32::from_rgb(0xC9, 0xE5, 0xEF) };
        v.selection.stroke = Stroke::new(1.0, p.primary);
        v.override_text_color = None;
        v.error_fg_color = p.danger;
        v.warn_fg_color = Color32::from_rgb(0xD9, 0x8A, 0x1C);
        v.striped = false;
        v.slider_trailing_fill = true;
        v.handle_shape = egui::style::HandleShape::Circle;
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = p.surface;
        w.noninteractive.weak_bg_fill = p.surface;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        w.noninteractive.corner_radius = r6;
        w.inactive.bg_fill = p.surface_alt;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
        w.inactive.bg_stroke = Stroke::new(1.0, p.border);
        w.inactive.fg_stroke = Stroke::new(1.0, p.text);
        w.inactive.corner_radius = r6;
        w.hovered.bg_fill = p.primary_soft;
        w.hovered.weak_bg_fill = p.primary_soft;
        w.hovered.bg_stroke = Stroke::new(1.0, p.primary.gamma_multiply(0.45));
        w.hovered.fg_stroke = Stroke::new(1.5, p.text);
        w.hovered.corner_radius = r6;
        w.hovered.expansion = 0.0;
        w.active.bg_fill = p.primary_soft;
        w.active.weak_bg_fill = p.primary_soft;
        w.active.bg_stroke = Stroke::new(1.0, p.primary);
        w.active.fg_stroke = Stroke::new(1.5, p.primary);
        w.active.corner_radius = r6;
        w.active.expansion = 0.0;
        w.open = w.active;
        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(8.0, 6.0);
        s.button_padding = Vec2::new(10.0, 5.0);
        s.interact_size = Vec2::new(32.0, 26.0);
        s.window_margin = Margin::same(16);
        s.menu_margin = Margin::same(6);
        s.combo_width = 140.0;
        s.text_edit_width = 120.0;
        s.slider_width = 120.0;
        s.indent = 14.0;
        use FontFamily::Proportional;
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, Proportional)),
            (TextStyle::Body, FontId::new(13.0, Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Name(MEDIUM.into()))),
            (TextStyle::Heading, FontId::new(17.0, FontFamily::Name(SEMIBOLD.into()))),
            (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
        ]
        .into();
    }
}

/// A horizontal gradient rectangle (header, primary buttons).
pub fn gradient_rect(painter: &egui::Painter, rect: egui::Rect, a: Color32, b: Color32, radius: f32) {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    if radius <= 0.5 {
        let mut mesh = Mesh::default();
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        for (i, c) in corners.into_iter().enumerate() {
            mesh.vertices.push(Vertex { pos: c, uv: WHITE_UV, color: if i == 1 || i == 2 { b } else { a } });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(mesh);
        return;
    }
    // Rounded: vertical strips clipped to the rounded outline.
    let steps = (rect.width() / 2.0).ceil().max(1.0) as usize;
    let lerp = |t: f32| -> Color32 {
        let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
        Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
    };
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = rect.left() + rect.width() * t;
        // Inset of the rounded corner at this x.
        let dx = if x < rect.left() + radius {
            radius - (x - rect.left())
        } else if x > rect.right() - radius {
            x - (rect.right() - radius)
        } else {
            0.0
        };
        let dy = if dx > 0.0 { radius - (radius * radius - dx * dx).max(0.0).sqrt() } else { 0.0 };
        let c = lerp(t);
        mesh.vertices.push(Vertex { pos: egui::pos2(x, rect.top() + dy), uv: WHITE_UV, color: c });
        mesh.vertices.push(Vertex { pos: egui::pos2(x, rect.bottom() - dy), uv: WHITE_UV, color: c });
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    painter.add(mesh);
}
