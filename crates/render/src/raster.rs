//! Raster back end (tiny-skia).

use crate::display::{Item, PageDisplay, PathEl, StrokeStyle};
use newpub_core::{Affine, Color, Document, Id, LineCap, LineJoin};
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
    images: HashMap<Id, Option<Pixmap>>,
    glyphs: HashMap<(FaceId, u16), Option<tiny_skia::Path>>,
}

/// Decodes image bytes into a premultiplied pixmap.
pub fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut data = img.into_raw();
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

    /// Paints a display list. `base` maps page points to pixels.
    pub fn paint(&mut self, pm: &mut Pixmap, doc: &Document, fonts: &FontStore, page: &PageDisplay, base: Affine) {
        for item in &page.items {
            match item {
                Item::Path { path, fill, stroke, transform } => {
                    let Some(p) = build_path(path) else { continue };
                    let t = ts(&base.compose(*transform));
                    if let Some(f) = fill {
                        pm.fill_path(&p, &paint_for(f), FillRule::Winding, t, None);
                    }
                    if let Some(s) = stroke {
                        Self::stroke(pm, &p, s, t);
                    }
                }
                Item::Glyphs { run, transform } => {
                    let t = ts(&base.compose(*transform));
                    self.draw_run(pm, fonts, run, t);
                }
                Item::Image { asset, local, clip, transform } => {
                    let img = self
                        .images
                        .entry(*asset)
                        .or_insert_with(|| doc.assets.get(asset).and_then(|a| decode_image(&a.bytes)));
                    let Some(img) = img.as_ref() else { continue };
                    let Some(rect) =
                        tiny_skia::Rect::from_xywh(clip.x as f32, clip.y as f32, clip.w as f32, clip.h as f32)
                    else {
                        continue;
                    };
                    let p = PathBuilder::from_rect(rect);
                    let paint = Paint {
                        anti_alias: true,
                        shader: tiny_skia::Pattern::new(
                            img.as_ref(),
                            tiny_skia::SpreadMode::Pad,
                            tiny_skia::FilterQuality::Bicubic,
                            1.0,
                            ts(local),
                        ),
                        ..Default::default()
                    };
                    pm.fill_path(&p, &paint, FillRule::Winding, ts(&base.compose(*transform)), None);
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
