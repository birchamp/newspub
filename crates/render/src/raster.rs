//! Raster back end (tiny-skia).

use crate::display::{GradientPaint, Item, PageDisplay, PathEl, StrokeStyle};
use newpub_core::{Affine, Color, Document, Id, ImageAdjust, LineCap, LineJoin};
use newpub_layout::{FaceId, FontStore, GlyphRun};
use std::collections::HashMap;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn ts(a: &Affine) -> Transform {
    let [sa, sb, sc, sd, se, sf] = a.0;
    Transform::from_row(sa as f32, sb as f32, sc as f32, sd as f32, se as f32, sf as f32)
}

fn paint_for(c: &Color) -> Paint<'static> {
    let [r, g, b, a] = c.to_rgba8();
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, a);
    p.anti_alias = true;
    p
}

fn build_path(els: &[PathEl]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for e in els {
        match *e {
            PathEl::Move(x, y) => pb.move_to(x as f32, y as f32),
            PathEl::Line(x, y) => pb.line_to(x as f32, y as f32),
            PathEl::Cubic(a, b, c, d, e, f) => pb.cubic_to(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32),
            PathEl::Close => pb.close(),
        }
    }
    pb.finish()
}

struct Outline(PathBuilder);

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y)
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y)
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y)
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y)
    }
    fn close(&mut self) {
        self.0.close()
    }
}

/// Caches decoded images and glyph outlines across renders.
#[derive(Default)]
pub struct Rasterizer {
    images: HashMap<(Id, String), Option<Pixmap>>,
    glyphs: HashMap<(FaceId, u16), Option<tiny_skia::Path>>,
}

/// Decodes image bytes and applies `adjust` (greyscale, brightness, contrast, recolour, in that order).
/// Returns width, height and straight (non-premultiplied) RGBA bytes.
pub fn adjust_rgba(bytes: &[u8], adjust: &ImageAdjust) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut data = img.into_raw();
    if adjust.is_identity() {
        return Some((w, h, data));
    }
    let tint = adjust.recolor.as_ref().map(|c| c.to_rgba8());
    let b = adjust.brightness.clamp(-1.0, 1.0);
    let k = adjust.contrast.clamp(-1.0, 1.0);
    let lum = |c: [f64; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    for px in data.chunks_exact_mut(4) {
        let mut c = [px[0] as f64, px[1] as f64, px[2] as f64];
        if adjust.greyscale {
            c = [lum(c); 3];
        }
        for v in &mut c {
            if b > 0.0 {
                *v += (255.0 - *v) * b;
            } else if b < 0.0 {
                *v *= 1.0 + b;
            }
            *v = (128.0 + (*v - 128.0) * (1.0 + k)).clamp(0.0, 255.0);
        }
        if let Some(t) = tint {
            let l = lum(c) / 255.0;
            c = [t[0] as f64 * l, t[1] as f64 * l, t[2] as f64 * l];
        }
        for (d, v) in px[..3].iter_mut().zip(c) {
            *d = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    Some((w, h, data))
}

/// Decodes image bytes into a premultiplied pixmap.
pub fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    decode_adjusted(bytes, &ImageAdjust::default())
}

fn decode_adjusted(bytes: &[u8], adjust: &ImageAdjust) -> Option<Pixmap> {
    let (w, h, mut data) = adjust_rgba(bytes, adjust)?;
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u16;
        for c in &mut px[..3] {
            *c = ((*c as u16 * a + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?)
}

impl Rasterizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets cached images (call when assets change).
    pub fn clear_images(&mut self) {
        self.images.clear();
    }

    fn glyph_path(&mut self, fonts: &FontStore, face: FaceId, gid: u16) -> Option<tiny_skia::Path> {
        self.glyphs
            .entry((face, gid))
            .or_insert_with(|| {
                let f = fonts.face(face);
                let t = f.ttf()?;
                let mut o = Outline(PathBuilder::new());
                t.outline_glyph(ttf_parser::GlyphId(gid), &mut o)?;
                o.0.finish()
            })
            .clone()
    }

    fn draw_run(&mut self, pm: &mut Pixmap, fonts: &FontStore, run: &GlyphRun, t: Transform) {
        let face = fonts.face(run.face);
        let upem = face.units_per_em;
        let paint = paint_for(&run.color);
        let sx = run.size * run.x_scale / upem;
        let sy = run.size / upem;
        let skew = if run.synthetic_italic { 0.21 } else { 0.0 };
        for g in &run.glyphs {
            let Some(path) = self.glyph_path(fonts, run.face, g.id) else { continue };
            // font units (y up) → frame-local
            let gt = Transform::from_row(sx as f32, 0.0, (-skew * sy) as f32, -sy as f32, g.x as f32, g.y as f32);
            let full = t.pre_concat(gt);
            pm.fill_path(&path, &paint, FillRule::Winding, full, None);
            if run.synthetic_bold {
                let st = tiny_skia::Stroke { width: (upem * 0.03) as f32, ..Default::default() };
                pm.stroke_path(&path, &paint, &st, full, None);
            }
        }
    }

    fn stroke(pm: &mut Pixmap, path: &tiny_skia::Path, s: &StrokeStyle, t: Transform) {
        if s.width <= 0.0 {
            return;
        }
        let line_cap = match s.cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        };
        let line_join = match s.join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        };
        let mut st = tiny_skia::Stroke { width: s.width as f32, line_cap, line_join, ..Default::default() };
        if !s.dash.is_empty() {
            st.dash = tiny_skia::StrokeDash::new(s.dash.iter().map(|d| *d as f32).collect(), 0.0);
        }
        pm.stroke_path(path, &paint_for(&s.color), &st, t, None);
    }

    fn gradient_shader(g: &GradientPaint) -> Option<tiny_skia::Shader<'static>> {
        let stops: Vec<tiny_skia::GradientStop> = g
            .stops
            .iter()
            .map(|(at, c)| {
                let [r, gr, b, a] = c.to_rgba8();
                tiny_skia::GradientStop::new(*at as f32, tiny_skia::Color::from_rgba8(r, gr, b, a))
            })
            .collect();
        let pt = |p: (f64, f64)| tiny_skia::Point::from_xy(p.0 as f32, p.1 as f32);
        if g.radial {
            tiny_skia::RadialGradient::new(
                pt(g.from),
                0.0,
                pt(g.from),
                g.radius as f32,
                stops,
                tiny_skia::SpreadMode::Pad,
                Transform::identity(),
            )
        } else {
            tiny_skia::LinearGradient::new(
                pt(g.from),
                pt(g.to),
                stops,
                tiny_skia::SpreadMode::Pad,
                Transform::identity(),
            )
        }
    }

    /// Paints a display list. `base` maps page points to pixels.
    pub fn paint(&mut self, pm: &mut Pixmap, doc: &Document, fonts: &FontStore, page: &PageDisplay, base: Affine) {
        for item in &page.items {
            self.paint_item(pm, doc, fonts, item, base);
        }
    }

    fn paint_item(&mut self, pm: &mut Pixmap, doc: &Document, fonts: &FontStore, item: &Item, base: Affine) {
        {
            match item {
                Item::Path { path, fill, stroke, transform } => {
                    let Some(p) = build_path(path) else { return };
                    let t = ts(&base.compose(*transform));
                    if let Some(f) = fill {
                        pm.fill_path(&p, &paint_for(f), FillRule::Winding, t, None);
                    }
                    if let Some(s) = stroke {
                        Self::stroke(pm, &p, s, t);
                    }
                }
                Item::Tag(_) => {}
                Item::Glyphs { run, transform } => {
                    let (back, front) =
                        if run.effects.is_empty() { (vec![], vec![]) } else { glyph_effects(fonts, run, *transform) };
                    for it in &back {
                        self.paint_item(pm, doc, fonts, it, base);
                    }
                    let t = ts(&base.compose(*transform));
                    self.draw_run(pm, fonts, run, t);
                    for it in &front {
                        self.paint_item(pm, doc, fonts, it, base);
                    }
                }
                Item::GradPath { path, paint, transform } => {
                    let (Some(p), Some(shader)) = (build_path(path), Self::gradient_shader(paint)) else { return };
                    let paint = Paint { anti_alias: true, shader, ..Default::default() };
                    pm.fill_path(&p, &paint, FillRule::Winding, ts(&base.compose(*transform)), None);
                }
                Item::Image { asset, local, clip, mask, opacity, adjust, transform } => {
                    let img = self
                        .images
                        .entry((*asset, format!("{adjust:?}")))
                        .or_insert_with(|| doc.assets.get(asset).and_then(|a| decode_adjusted(&a.bytes, adjust)));
                    let Some(img) = img.as_ref() else { return };
                    let Some(rect) =
                        tiny_skia::Rect::from_xywh(clip.x as f32, clip.y as f32, clip.w as f32, clip.h as f32)
                    else {
                        return;
                    };
                    let p = PathBuilder::from_rect(rect);
                    let paint = Paint {
                        anti_alias: true,
                        shader: tiny_skia::Pattern::new(
                            img.as_ref(),
                            tiny_skia::SpreadMode::Pad,
                            tiny_skia::FilterQuality::Bicubic,
                            *opacity as f32,
                            ts(local),
                        ),
                        ..Default::default()
                    };
                    let t = ts(&base.compose(*transform));
                    let clip_mask = mask.as_ref().and_then(|m| {
                        let mp = build_path(m)?;
                        let mut cm = tiny_skia::Mask::new(pm.width(), pm.height())?;
                        cm.fill_path(&mp, FillRule::Winding, true, t);
                        Some(cm)
                    });
                    if mask.is_some() && clip_mask.is_none() {
                        return;
                    }
                    pm.fill_path(&p, &paint, FillRule::Winding, t, clip_mask.as_ref());
                }
            }
        }
    }
}

/// Renders one page to a pixmap at `dpi`, white background, trim size.
pub fn render_page(
    r: &mut Rasterizer,
    doc: &Document,
    fonts: &FontStore,
    page: &PageDisplay,
    dpi: f64,
) -> Option<Pixmap> {
    let k = dpi / 72.0;
    let (w, h) = ((page.width * k).round().max(1.0) as u32, (page.height * k).round().max(1.0) as u32);
    let mut pm = Pixmap::new(w, h)?;
    pm.fill(tiny_skia::Color::WHITE);
    r.paint(&mut pm, doc, fonts, page, Affine::scale(k, k));
    Some(pm)
}

/// Vector layers for a glyph run's text effects (TY-18), both in page space via `transform`.
/// Returns (behind the glyphs, in front of them). Order behind to front: glow, shadow, emboss/engrave,
/// reflection, glyphs, outline. Both back ends draw these as ordinary path items.
pub fn glyph_effects(fonts: &FontStore, run: &GlyphRun, transform: Affine) -> (Vec<Item>, Vec<Item>) {
    let (mut back, mut front) = (vec![], vec![]);
    let fx = &run.effects;
    if fx.is_empty() {
        return (back, front);
    }
    let face = fonts.face(run.face);
    let Some(ttf) = face.ttf() else { return (back, front) };
    let upem = face.units_per_em;
    let sx = run.size * run.x_scale / upem;
    let sy = run.size / upem;
    let skew = if run.synthetic_italic { 0.21 } else { 0.0 };
    // Frame-local outlines (y down); `mirror` reflects each glyph about its own baseline plus the gap.
    let outline = |mirror: Option<f64>| -> Vec<PathEl> {
        let mut out = vec![];
        for g in &run.glyphs {
            let map = |x: f64, y: f64| {
                let (px, py) = (g.x + sx * x - skew * sy * y, g.y - sy * y);
                match mirror {
                    Some(gap) => (px, 2.0 * g.y + gap - py),
                    None => (px, py),
                }
            };
            for e in crate::display::glyph_outline(&ttf, g.id) {
                out.push(match e {
                    PathEl::Move(x, y) => {
                        let (x, y) = map(x, y);
                        PathEl::Move(x, y)
                    }
                    PathEl::Line(x, y) => {
                        let (x, y) = map(x, y);
                        PathEl::Line(x, y)
                    }
                    PathEl::Cubic(a, b, c, d, e, f) => {
                        let (a, b) = map(a, b);
                        let (c, d) = map(c, d);
                        let (e, f) = map(e, f);
                        PathEl::Cubic(a, b, c, d, e, f)
                    }
                    PathEl::Close => PathEl::Close,
                });
            }
        }
        out
    };
    let shifted = |dx: f64, dy: f64| transform.compose(Affine::translate(dx, dy));
    let stroke = |color: Color, width: f64| StrokeStyle {
        color,
        width,
        dash: vec![],
        cap: LineCap::Round,
        join: LineJoin::Round,
    };
    let plain = outline(None);
    if plain.is_empty() {
        return (back, front);
    }

    if let Some(glow) = &fx.glow
        && glow.radius > 0.0
    {
        const N: usize = 8;
        // Layer k (1..=N) strokes out to radius k/N of the glow; the composite opacity in ring j is
        // 0.8 * (1 - (j - 0.5) / N), so it falls from 0.8 at the glyph edge to 0 at the radius.
        let target = |j: usize| 0.8 * (1.0 - (j as f64 - 0.5) / N as f64);
        let base_a = f64::from(glow.color.alpha()).clamp(0.0, 1.0);
        for k in (1..=N).rev() {
            let t_next = if k == N { 0.0 } else { target(k + 1) };
            let a = (1.0 - (1.0 - target(k)) / (1.0 - t_next)) * base_a;
            back.push(Item::Path {
                path: plain.clone(),
                fill: None,
                stroke: Some(stroke(glow.color.clone().with_alpha(a as f32), 2.0 * glow.radius * k as f64 / N as f64)),
                transform,
            });
        }
    }

    if let Some(sh) = &fx.shadow {
        let blur = sh.blur.max(0.0);
        let steps = if blur > 0.0 { 4 } else { 0 };
        let a = f64::from(sh.color.alpha()).clamp(0.0, 1.0);
        let layer_a = 1.0 - (1.0 - a).powf(1.0 / (steps + 1) as f64);
        let color = sh.color.clone().with_alpha(layer_a as f32);
        for i in 0..=steps {
            let grow = blur * (steps - i) as f64 / steps.max(1) as f64;
            back.push(Item::Path {
                path: plain.clone(),
                fill: Some(color.clone()),
                stroke: (grow > 0.0).then(|| stroke(color.clone(), 2.0 * grow)),
                transform: shifted(sh.dx, sh.dy),
            });
        }
    }

    if fx.emboss || fx.engrave {
        let dark = Color::Rgb { r: 0, g: 0, b: 0, a: 0.6 };
        let light = Color::Rgb { r: 255, g: 255, b: 255, a: 0.85 };
        // Emboss: dark down-right, light up-left. Engrave swaps them.
        let (down_right, up_left) = if fx.engrave && !fx.emboss { (light, dark) } else { (dark, light) };
        for (c, d) in [(down_right, 1.0), (up_left, -1.0)] {
            back.push(Item::Path { path: plain.clone(), fill: Some(c), stroke: None, transform: shifted(d, d) });
        }
    }

    if let Some(refl) = fx.reflection {
        let o = refl.clamp(0.0, 1.0);
        if o > 0.0
            && let Some(first) = run.glyphs.first()
        {
            let gap = 0.0;
            let h = if face.cap_height > 0.0 { face.cap_height * run.size } else { 0.7 * run.size };
            let (x0, y0) = (first.x, first.y + gap);
            let c = run.color.clone();
            let base_a = f64::from(c.alpha());
            back.push(Item::GradPath {
                path: outline(Some(gap)),
                paint: GradientPaint {
                    radial: false,
                    from: (x0, y0),
                    to: (x0, y0 + h),
                    radius: 0.0,
                    stops: vec![(0.0, c.clone().with_alpha((base_a * o) as f32)), (1.0, c.with_alpha(0.0))],
                },
                transform,
            });
        }
    }

    if let Some(ol) = &fx.outline
        && ol.width > 0.0
    {
        front.push(Item::Path {
            path: plain,
            fill: None,
            stroke: Some(StrokeStyle {
                color: ol.color.clone(),
                width: ol.width,
                dash: vec![],
                cap: LineCap::Butt,
                join: LineJoin::Round,
            }),
            transform,
        });
    }
    (back, front)
}
