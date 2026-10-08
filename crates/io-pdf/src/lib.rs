//! newpub-io-pdf: PDF export with krilla.

use krilla::Document as KDoc;
use krilla::color::{cmyk, rgb, separation};
use krilla::geom::{PathBuilder, Point, Rect as KRect, Size, Transform};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, LineCap as KLineCap, LineJoin as KLineJoin, Stroke, StrokeDash};
use krilla::text::{Font, GlyphId, KrillaGlyph};
use newpub_core::{Affine, Color, Document, ImageAdjust};
use newpub_layout::{DocLayout, FaceId, FontStore};
use newpub_render::{GradientPaint, Item, PageDisplay, PathEl, StrokeStyle, TagMark, adjust_rgba, page_display};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod impose;
pub mod links;
pub mod separations;
pub mod standards;
mod tagging;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("pdf: {0}")]
    Krilla(String),
    #[error("no pages to export")]
    Empty,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Imposition {
    /// One page per PDF page.
    #[default]
    None,
    /// Saddle-stitch booklet: two pages side by side per sheet side, in fold order.
    Booklet,
    /// Several pages per sheet in a grid (PR-04), see `impose.rs`.
    NUp {
        sheet_width: newpub_core::Length,
        sheet_height: newpub_core::Length,
        #[serde(default)]
        gap: newpub_core::Length,
        /// Repeat each page across a whole sheet (business cards) instead of filling slots in order.
        #[serde(default)]
        repeat: bool,
    },
    /// The document's own sheet layout (`Document.sheet`, PG-11): every page repeated across one sheet,
    /// items at `left + c × (page width + col_gap)`, `top + r × (page height + row_gap)`. Error without a sheet.
    DocumentSheet,
}

/// Output standard (EX-03, AX-03), applied by `standards.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfStandard {
    PdfX4,
    PdfUa1,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfOptions {
    /// Include the document bleed around each page.
    pub bleed: bool,
    /// Draw crop marks outside the trim (and bleed).
    pub crop_marks: bool,
    pub imposition: Imposition,
    /// 0-based page indices to export (default all).
    pub pages: Option<Vec<usize>>,
    /// PDF/X-4 or PDF/UA-1 output.
    pub standard: Option<PdfStandard>,
    /// Colour separations (PR-08): one page per ink plate (see `separations.rs`).
    pub separations: bool,
}

/// Distance from the trim/bleed edge to where crop marks start, and their length.
const MARK_GAP: f64 = 6.0;
const MARK_LEN: f64 = 18.0;

fn kcolor(c: &Color) -> (krilla::paint::Paint, f32) {
    let a = c.alpha();
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let p: krilla::paint::Paint = match c {
        Color::Rgb { r, g, b, .. } => rgb::Color::new(*r, *g, *b).into(),
        // Scheme colours are resolved before export; an unresolved one prints as its default-scheme RGB.
        Color::Scheme { .. } => {
            let [r, g, b, _] = c.to_rgba8();
            rgb::Color::new(r, g, b).into()
        }
        Color::Cmyk { c, m, y, k, .. } => cmyk::Color::new(u(*c), u(*m), u(*y), u(*k)).into(),
        Color::Spot { name, c, m, y, k, tint, .. } => {
            let fallback = krilla::color::RegularColor::Cmyk(cmyk::Color::new(u(*c), u(*m), u(*y), u(*k)));
            let space =
                separation::SeparationSpace::new(separation::SeparationColorant::Custom(name.clone()), fallback);
            krilla::color::Color::Special(krilla::color::SpecialColor::Separation(separation::Color::new(
                u(*tint),
                space,
            )))
            .into()
        }
    };
    (p, a)
}

fn kt(a: &Affine) -> Transform {
    let [sa, sb, sc, sd, se, sf] = a.0;
    Transform::from_row(sa as f32, sb as f32, sc as f32, sd as f32, se as f32, sf as f32)
}

fn kpath(els: &[PathEl]) -> Option<krilla::geom::Path> {
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

fn kstroke(s: &StrokeStyle) -> Stroke {
    let (paint, a) = kcolor(&s.color);
    let line_cap = match s.cap {
        newpub_core::LineCap::Butt => KLineCap::Butt,
        newpub_core::LineCap::Round => KLineCap::Round,
        newpub_core::LineCap::Square => KLineCap::Square,
    };
    let line_join = match s.join {
        newpub_core::LineJoin::Miter => KLineJoin::Miter,
        newpub_core::LineJoin::Round => KLineJoin::Round,
        newpub_core::LineJoin::Bevel => KLineJoin::Bevel,
    };
    Stroke {
        paint,
        width: s.width as f32,
        line_cap,
        line_join,
        opacity: NormalizedF32::new(a).unwrap_or(NormalizedF32::ONE),
        dash: (!s.dash.is_empty())
            .then(|| StrokeDash { array: s.dash.iter().map(|d| *d as f32).collect(), offset: 0.0 }),
        ..Default::default()
    }
}

fn kgradient(g: &GradientPaint) -> Option<krilla::paint::Paint> {
    use krilla::paint::{LinearGradient, RadialGradient, SpreadMethod, Stop};
    let stops: Vec<Stop> = g
        .stops
        .iter()
        .map(|(at, c)| {
            let [r, gr, b, a] = c.to_rgba8();
            Stop {
                offset: NormalizedF32::new(*at as f32).unwrap_or(NormalizedF32::ZERO),
                color: rgb::Color::new(r, gr, b).into(),
                opacity: NormalizedF32::new(a as f32 / 255.0).unwrap_or(NormalizedF32::ONE),
            }
        })
        .collect();
    if stops.len() == 1 {
        let [r, gr, b, _] = g.stops.first()?.1.to_rgba8();
        return Some(rgb::Color::new(r, gr, b).into());
    }
    Some(if g.radial {
        RadialGradient {
            fx: g.from.0 as f32,
            fy: g.from.1 as f32,
            fr: 0.0,
            cx: g.from.0 as f32,
            cy: g.from.1 as f32,
            cr: g.radius as f32,
            transform: Transform::identity(),
            spread_method: SpreadMethod::Pad,
            stops,
            anti_alias: true,
        }
        .into()
    } else {
        LinearGradient {
            x1: g.from.0 as f32,
            y1: g.from.1 as f32,
            x2: g.to.0 as f32,
            y2: g.to.1 as f32,
            transform: Transform::identity(),
            spread_method: SpreadMethod::Pad,
            stops,
            anti_alias: true,
        }
        .into()
    })
}

struct Ctx<'a> {
    doc: &'a Document,
    fonts: &'a FontStore,
    kfonts: HashMap<FaceId, Option<Font>>,
    images: HashMap<(newpub_core::Id, String), Option<krilla::image::Image>>,
}

impl Ctx<'_> {
    fn font(&mut self, id: FaceId) -> Option<Font> {
        let fonts = self.fonts;
        self.kfonts
            .entry(id)
            .or_insert_with(|| {
                let f = fonts.face(id);
                Font::new(krilla::Data::from(f.data.clone()), f.index)
            })
            .clone()
    }

    fn image(&mut self, id: newpub_core::Id, adjust: &ImageAdjust) -> Option<krilla::image::Image> {
        let doc = self.doc;
        self.images
            .entry((id, format!("{adjust:?}")))
            .or_insert_with(|| {
                let a = doc.assets.get(&id)?;
                if !adjust.is_identity() {
                    // Bake the adjustments into the embedded pixels.
                    let (w, h, rgba) = adjust_rgba(&a.bytes, adjust)?;
                    return Some(krilla::image::Image::from_rgba8(rgba, w, h));
                }
                let data: Vec<u8> = a.bytes.to_vec();
                let img = if a.mime == "image/jpeg" {
                    krilla::image::Image::from_jpeg(data.into(), true)
                } else {
                    krilla::image::Image::from_png(data.into(), true)
                };
                img.ok()
            })
            .clone()
    }

    /// Draws the display list. With a tagger, structure markers open marked content and everything else is
    /// drawn as an artifact (PDF/UA).
    fn draw(&mut self, s: &mut krilla::surface::Surface, page: &PageDisplay, mut tags: Option<&mut tagging::Tagger>) {
        for item in &page.items {
            match (item, tags.as_deref_mut()) {
                (Item::Tag(TagMark::Begin { owner, path }), Some(t)) => t.begin(s, *owner, path),
                (Item::Tag(TagMark::End), Some(t)) => t.end(s),
                (Item::Tag(_), None) => {}
                (_, Some(t)) if !t.is_open() => {
                    tagging::Tagger::start_artifact(s);
                    self.draw_item(s, item);
                    s.end_tagged();
                }
                _ => self.draw_item(s, item),
            }
        }
        if let Some(t) = tags {
            t.end(s);
        }
    }

    fn draw_item(&mut self, s: &mut krilla::surface::Surface, item: &Item) {
        match item {
            Item::Path { path, fill, stroke, transform } => {
                let Some(p) = kpath(path) else { return };
                s.push_transform(&kt(transform));
                match fill {
                    Some(f) => {
                        let (paint, a) = kcolor(f);
                        s.set_fill(Some(Fill {
                            paint,
                            opacity: NormalizedF32::new(a).unwrap_or(NormalizedF32::ONE),
                            rule: FillRule::NonZero,
                        }));
                    }
                    None => s.set_fill(None),
                }
                s.set_stroke(stroke.as_ref().map(kstroke));
                s.draw_path(&p);
                s.pop();
            }
            Item::GradPath { path, paint, transform } => {
                let (Some(p), Some(kp)) = (kpath(path), kgradient(paint)) else { return };
                s.push_transform(&kt(transform));
                s.set_fill(Some(Fill { paint: kp, opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
                s.set_stroke(None);
                s.draw_path(&p);
                s.pop();
            }
            Item::Image { asset, local, clip, mask, opacity, adjust, transform } => {
                let Some(img) = self.image(*asset, adjust) else { return };
                let (pw, ph) = img.size();
                let Some(size) = Size::from_wh(pw as f32, ph as f32) else { return };
                let Some(cr) = KRect::from_xywh(clip.x as f32, clip.y as f32, clip.w as f32, clip.h as f32) else {
                    return;
                };
                let mut pb = PathBuilder::new();
                pb.push_rect(cr);
                let Some(cp) = pb.finish() else { return };
                let mp = match mask {
                    Some(m) => match kpath(m) {
                        Some(mp) => Some(mp),
                        None => return,
                    },
                    None => None,
                };
                let faded = *opacity < 0.999;
                s.push_transform(&kt(transform));
                s.push_clip_path(&cp, &FillRule::NonZero);
                if let Some(mp) = &mp {
                    s.push_clip_path(mp, &FillRule::NonZero);
                }
                if faded {
                    s.push_opacity(NormalizedF32::new(*opacity as f32).unwrap_or(NormalizedF32::ONE));
                }
                s.push_transform(&kt(local));
                s.draw_image(img, size);
                s.pop();
                if faded {
                    s.pop();
                }
                if mp.is_some() {
                    s.pop();
                }
                s.pop();
                s.pop();
            }
            Item::Glyphs { run, transform } => {
                let (back, front) = if run.effects.is_empty() {
                    (vec![], vec![])
                } else {
                    newpub_render::raster::glyph_effects(self.fonts, run, *transform)
                };
                for it in &back {
                    self.draw_item(s, it);
                }
                self.draw_glyph_run(s, run, transform);
                for it in &front {
                    self.draw_item(s, it);
                }
            }
            Item::Tag(_) => {}
        }
    }
}

impl Ctx<'_> {
    /// The run's real text (extractable), drawn with the font.
    fn draw_glyph_run(&mut self, s: &mut krilla::surface::Surface, run: &newpub_layout::GlyphRun, transform: &Affine) {
        let Some(font) = self.font(run.face) else { return };
        let Some(first) = run.glyphs.first() else { return };
        let (x0, y0) = (first.x, first.y);
        let size = run.size as f32;
        let xs = run.x_scale.max(0.01);
        let glyphs: Vec<KrillaGlyph> = run
            .glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let next_x = run.glyphs.get(i + 1).map(|n| n.x).unwrap_or(g.x + g.advance);
                let adv = ((next_x - g.x) / xs / run.size) as f32;
                KrillaGlyph::new(
                    GlyphId::new(g.id as u32),
                    adv,
                    0.0,
                    ((g.y - y0) / run.size) as f32,
                    0.0,
                    g.text_range.clone(),
                    None,
                )
            })
            .collect();
        let skew = if run.synthetic_italic { -0.21 } else { 0.0 };
        let local = Affine::translate(x0, y0).compose(Affine([xs, 0.0, skew, 1.0, 0.0, 0.0]));
        s.push_transform(&kt(&transform.compose(local)));
        let (paint, a) = kcolor(&run.color);
        let opacity = NormalizedF32::new(a).unwrap_or(NormalizedF32::ONE);
        s.set_fill(Some(Fill { paint: paint.clone(), opacity, rule: FillRule::NonZero }));
        s.set_stroke(run.synthetic_bold.then(|| Stroke {
            paint,
            width: (run.size * 0.03) as f32,
            opacity,
            ..Default::default()
        }));
        s.draw_glyphs(Point::from_xy(0.0, 0.0), &glyphs, font, &run.text, size, false);
        s.set_stroke(None);
        s.pop();
    }
}

fn crop_marks(s: &mut krilla::surface::Surface, x: f64, y: f64, w: f64, h: f64, off: f64) {
    let mut pb = PathBuilder::new();
    let (x1, y1) = (x + w, y + h);
    for (cx, cy, dx, dy) in [(x, y, -1.0, -1.0), (x1, y, 1.0, -1.0), (x, y1, -1.0, 1.0), (x1, y1, 1.0, 1.0)] {
        // horizontal mark
        pb.move_to((cx + dx * off) as f32, cy as f32);
        pb.line_to((cx + dx * (off + MARK_LEN)) as f32, cy as f32);
        // vertical mark
        pb.move_to(cx as f32, (cy + dy * off) as f32);
        pb.line_to(cx as f32, (cy + dy * (off + MARK_LEN)) as f32);
    }
    if let Some(p) = pb.finish() {
        s.set_fill(None);
        // Registration colour: prints on every separation.
        let reg = separation::Color::new(
            255,
            separation::SeparationSpace::new(
                separation::SeparationColorant::AllColorants,
                krilla::color::RegularColor::Cmyk(cmyk::Color::new(255, 255, 255, 255)),
            ),
        );
        s.set_stroke(Some(Stroke {
            paint: krilla::color::Color::Special(krilla::color::SpecialColor::Separation(reg)).into(),
            width: 0.25,
            ..Default::default()
        }));
        s.draw_path(&p);
        s.set_stroke(None);
    }
}

/// Exports the document to PDF bytes.
pub fn export_pdf(
    doc: &Document,
    layout: &DocLayout,
    fonts: &FontStore,
    opts: &PdfOptions,
) -> Result<Vec<u8>, PdfError> {
    let indices: Vec<usize> = opts.pages.clone().unwrap_or_else(|| (0..doc.pages.len()).collect());
    let indices: Vec<usize> = indices.into_iter().filter(|i| *i < doc.pages.len()).collect();
    if indices.is_empty() {
        return Err(PdfError::Empty);
    }
    let (w, h) = (doc.setup.width.0, doc.setup.height.0);
    let bleed = if opts.bleed { doc.setup.bleed.0 } else { 0.0 };
    let marks = if opts.crop_marks { MARK_GAP + MARK_LEN + 2.0 } else { 0.0 };
    let pad = bleed + marks;
    let ua = opts.standard == Some(PdfStandard::PdfUa1);
    let mut kd = if ua {
        let configuration = krilla::configure::ConfigurationBuilder::new()
            .with_accessibility_validator(krilla::configure::Accessibility::UA1)
            .finish()
            .map_err(|e| PdfError::Krilla(format!("{e:?}")))?;
        KDoc::new_with(krilla::SerializeSettings { configuration, enable_tagging: true, ..Default::default() })
    } else {
        KDoc::new()
    };
    let mut tagger: Option<tagging::Tagger> = ua.then(Default::default);
    let mut ctx = Ctx { doc, fonts, kfonts: HashMap::new(), images: HashMap::new() };
    let mut meta = krilla::metadata::Metadata::new().creator("newpub".to_string());
    if !doc.meta.title.is_empty() {
        meta = meta.title(doc.meta.title.clone());
    }
    if !doc.meta.author.is_empty() {
        meta = meta.authors(vec![doc.meta.author.clone()]);
    }
    if !doc.meta.lang.is_empty() {
        meta = meta.language(doc.meta.lang.clone());
    }
    kd.set_metadata(meta);

    if opts.separations {
        return separations::export_separations(doc, layout, fonts, opts, &indices);
    }
    let sheets = impose::plan(w, h, &indices, &opts.imposition, doc.sheet.as_ref())?;
    for sheet in &sheets {
        let (sw, sh) = (sheet.width, sheet.height);
        let (mw, mh) = (sw + 2.0 * pad, sh + 2.0 * pad);
        let r = |x: f64, y: f64, w: f64, h: f64| KRect::from_xywh(x as f32, y as f32, w as f32, h as f32);
        let settings = PageSettings::from_wh(mw as f32, mh as f32)
            .ok_or(PdfError::Empty)?
            .with_trim_box(r(pad, pad, sw, sh))
            .with_bleed_box(r(pad - bleed, pad - bleed, sw + 2.0 * bleed, sh + 2.0 * bleed));
        let mut page = kd.start_page_with(settings);
        let mut s = page.surface();
        for slot in &sheet.slots {
            let Some(pi) = slot.page else { continue };
            let disp = page_display(doc, layout, pi);
            let (ox, oy) = (pad + slot.x, pad + slot.y);
            // Clip to the page's trim, plus bleed on the sheet's outer edges.
            let eps = 0.01;
            let cl = if slot.x < eps { bleed } else { 0.0 };
            let ct = if slot.y < eps { bleed } else { 0.0 };
            let cr = if slot.x + w > sw - eps { bleed } else { 0.0 };
            let cb = if slot.y + h > sh - eps { bleed } else { 0.0 };
            let Some(clip) = r(ox - cl, oy - ct, w + cl + cr, h + ct + cb) else { continue };
            let mut pb = PathBuilder::new();
            pb.push_rect(clip);
            if let Some(cp) = pb.finish() {
                s.push_clip_path(&cp, &FillRule::NonZero);
                s.push_transform(&Transform::from_translate(ox as f32, oy as f32));
                if let Some(t) = tagger.as_mut() {
                    t.set_page(doc, pi);
                }
                ctx.draw(&mut s, &disp, tagger.as_mut());
                s.pop();
                s.pop();
            }
        }
        if opts.crop_marks {
            if ua {
                tagging::Tagger::start_artifact(&mut s);
            }
            crop_marks(&mut s, pad, pad, sw, sh, bleed + MARK_GAP);
            if ua {
                s.end_tagged();
            }
        }
        s.finish();
        for slot in &sheet.slots {
            if let Some(pi) = slot.page {
                if let Some(t) = tagger.as_mut() {
                    t.set_page(doc, pi);
                }
                links::annotate_page(&mut page, doc, layout, pi, (pad + slot.x, pad + slot.y), tagger.as_mut());
            }
        }
        page.finish();
    }
    links::outline(&mut kd, doc, &sheets);
    if let Some(t) = &tagger
        && doc.bookmarks.is_empty()
    {
        links::heading_outline(&mut kd, &t.headings, doc.pages.len());
    }
    if let Some(t) = tagger {
        let lang = (!doc.meta.lang.is_empty()).then(|| doc.meta.lang.clone());
        kd.set_tag_tree(t.tree(doc, lang));
    }
    let bytes = kd.finish().map_err(|e| PdfError::Krilla(format!("{e:?}")))?;
    standards::finish(bytes, doc, opts)
}

/// Saddle-stitch page order. Returns sheet sides, each `[left, right]` as indices into the
/// page list (or `None` for blank padding pages).
pub fn booklet_order(n: usize) -> Vec<Vec<Option<usize>>> {
    let total = n.div_ceil(4).max(1) * 4;
    let at = |k: usize| (k < n).then_some(k);
    let mut sides = vec![];
    for s in 0..total / 4 {
        sides.push(vec![at(total - 1 - 2 * s), at(2 * s)]);
        sides.push(vec![at(2 * s + 1), at(total - 2 - 2 * s)]);
    }
    sides
}
