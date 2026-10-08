//! Colour separations (PR-08): one gray page per ink plate, with knockouts and overprint.

use crate::{Ctx, PdfError, PdfOptions};
use krilla::Document as KDoc;
use krilla::geom::{Point, Rect as KRect};
use krilla::page::PageSettings;
use krilla::paint::Fill;
use krilla::text::{GlyphId, KrillaGlyph};
use newpub_core::{Color, Document, Id, ImageAdjust};
use newpub_layout::{DocLayout, FontStore};
use newpub_render::{GradientPaint, Item, PageDisplay, adjust_rgba, page_display};
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};

thread_local! {
    static GRAY_MODE: Cell<bool> = const { Cell::new(false) };
}

/// While plates are being drawn, `Color::Rgb` values are emitted as DeviceGray (their channels are equal);
/// consulted by `kcolor` and `kgradient` in lib.rs.
pub(crate) fn gray_mode() -> bool {
    GRAY_MODE.with(Cell::get)
}

struct GrayGuard;
impl GrayGuard {
    fn new() -> GrayGuard {
        GRAY_MODE.with(|g| g.set(true));
        GrayGuard
    }
}
impl Drop for GrayGuard {
    fn drop(&mut self) {
        GRAY_MODE.with(|g| g.set(false));
    }
}

const PROCESS: [&str; 4] = ["Cyan", "Magenta", "Yellow", "Black"];

fn collect_spots(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => {
            if m.get("space").and_then(|s| s.as_str()) == Some("spot")
                && let Some(n) = m.get("name").and_then(|n| n.as_str())
            {
                out.insert(n.to_string());
            }
            m.values().for_each(|x| collect_spots(x, out));
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_spots(x, out)),
        _ => {}
    }
}

/// Plate names in output order: "Cyan", "Magenta", "Yellow", "Black", then the spot colours used (by name).
pub fn plates(doc: &Document) -> Vec<String> {
    let mut spots = BTreeSet::new();
    if let Ok(v) = serde_json::to_value(doc) {
        collect_spots(&v, &mut spots);
    }
    spots.retain(|n| !PROCESS.contains(&n.as_str()));
    PROCESS.iter().map(|s| s.to_string()).chain(spots).collect()
}

/// Tint (0..=1) of colour `c` on the named plate.
fn tint(c: &Color, plate: &str) -> f32 {
    let naive = |[r, g, b, _]: [u8; 4]| {
        let (c, m, y) = (1.0 - r as f32 / 255.0, 1.0 - g as f32 / 255.0, 1.0 - b as f32 / 255.0);
        let k = c.min(m).min(y);
        if k >= 1.0 { (0.0, 0.0, 0.0, 1.0) } else { ((c - k) / (1.0 - k), (m - k) / (1.0 - k), (y - k) / (1.0 - k), k) }
    };
    let pick = |(c, m, y, k): (f32, f32, f32, f32)| match plate {
        "Cyan" => c,
        "Magenta" => m,
        "Yellow" => y,
        "Black" => k,
        _ => 0.0,
    };
    let t = match c {
        Color::Cmyk { c, m, y, k, .. } => pick((*c, *m, *y, *k)),
        // A spot colour goes to its own plate only.
        Color::Spot { name, tint, .. } => {
            if name == plate && !PROCESS.contains(&plate) {
                *tint
            } else {
                0.0
            }
        }
        other => pick(naive(other.to_rgba8())),
    };
    t.clamp(0.0, 1.0)
}

fn gray(t: f32, alpha: f32) -> Color {
    let v = ((1.0 - t.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Color::Rgb { r: v, g: v, b: v, a: alpha }
}

/// The colour to paint with on this plate, or `None` when the colour leaves the plate untouched.
fn map_color(c: &Color, plate: &str, overprint: bool) -> Option<Color> {
    let t = tint(c, plate);
    let a = c.alpha();
    if t <= 0.0 && (overprint || a < 0.999) {
        return None;
    }
    Some(gray(t, a))
}

struct Mapper {
    plate: String,
    plate_ix: usize,
    images: HashMap<(Id, String, usize, bool), Id>,
    next_fake: u64,
}

impl Mapper {
    fn item(&mut self, ctx: &mut Ctx, item: &Item, overprint: bool) -> Option<Item> {
        let plate = self.plate.as_str();
        Some(match item {
            Item::Path { path, fill, stroke, transform } => {
                let fill = fill.as_ref().and_then(|f| map_color(f, plate, overprint));
                let stroke = stroke.as_ref().and_then(|s| {
                    let mut s = s.clone();
                    s.color = map_color(&s.color, plate, overprint)?;
                    Some(s)
                });
                if fill.is_none() && stroke.is_none() {
                    return None;
                }
                Item::Path { path: path.clone(), fill, stroke, transform: *transform }
            }
            Item::GradPath { path, paint, transform } => {
                if overprint && paint.stops.iter().all(|(_, c)| tint(c, plate) <= 0.0) {
                    return None;
                }
                let stops = paint.stops.iter().map(|(at, c)| (*at, gray(tint(c, plate), c.alpha()))).collect();
                let paint = GradientPaint { stops, ..paint.clone() };
                Item::GradPath { path: path.clone(), paint, transform: *transform }
            }
            Item::Glyphs { run, transform } => {
                let mut run = run.clone();
                run.color = map_color(&run.color, plate, overprint)?;
                Item::Glyphs { run, transform: *transform }
            }
            Item::Image { asset, local, clip, mask, opacity, adjust, transform } => {
                let key = (*asset, format!("{adjust:?}"), self.plate_ix, overprint);
                let fake = match self.images.get(&key) {
                    Some(id) => *id,
                    None => {
                        let a = ctx.doc.assets.get(asset)?;
                        let (w, h, mut rgba) = adjust_rgba(&a.bytes, adjust)?;
                        for px in rgba.chunks_exact_mut(4) {
                            let t = tint(&Color::rgb(px[0], px[1], px[2]), plate);
                            let v = ((1.0 - t) * 255.0).round() as u8;
                            if t <= 0.0 && overprint {
                                px[3] = 0;
                            }
                            px[0] = v;
                            px[1] = v;
                            px[2] = v;
                        }
                        self.next_fake += 1;
                        let id = Id(u64::MAX - self.next_fake);
                        let img = krilla::image::Image::from_rgba8(rgba, w, h);
                        ctx.images.insert((id, format!("{:?}", ImageAdjust::default())), Some(img));
                        self.images.insert(key, id);
                        id
                    }
                };
                Item::Image {
                    asset: fake,
                    local: *local,
                    clip: *clip,
                    mask: mask.clone(),
                    opacity: *opacity,
                    adjust: ImageAdjust::default(),
                    transform: *transform,
                }
            }
            Item::Tag(_) => return None,
        })
    }
}

/// The page's display items, each flagged with whether its top-level object overprints.
fn flagged_items(doc: &Document, layout: &DocLayout, pi: usize) -> Vec<(Item, bool)> {
    let full = page_display(doc, layout, pi);
    if !doc.objects.values().any(|o| o.overprint) {
        return full.items.into_iter().map(|i| (i, false)).collect();
    }
    let mut top: Vec<Id> = doc.master_for_page(pi).map(|m| m.objects.clone()).unwrap_or_default();
    top.extend(doc.draw_order(pi));
    let top_of = |mut id: Id| {
        while let Some(p) = doc.objects.get(&id).and_then(|o| o.parent) {
            id = p;
        }
        id
    };
    // Render each top-level object alone (everything else hidden) to learn which items it produced.
    let mut hid = doc.clone();
    for o in hid.objects.values_mut() {
        o.hidden = true;
    }
    let base = page_display(&hid, layout, pi).items.len();
    let mut out: Vec<(Item, bool)> = full.items.iter().take(base).map(|i| (i.clone(), false)).collect();
    for id in top {
        let Some(top_obj) = doc.objects.get(&id) else { continue };
        let family: Vec<Id> = doc.objects.keys().copied().filter(|k| top_of(*k) == id).collect();
        for k in &family {
            if let (Some(h), Some(orig)) = (hid.objects.get_mut(k), doc.objects.get(k)) {
                h.hidden = orig.hidden;
            }
        }
        let items = page_display(&hid, layout, pi).items;
        out.extend(items.into_iter().skip(base).map(|i| (i, top_obj.overprint)));
        for k in &family {
            if let Some(h) = hid.objects.get_mut(k) {
                h.hidden = true;
            }
        }
    }
    out
}

fn draw_label(s: &mut krilla::surface::Surface, ctx: &mut Ctx, fonts: &FontStore, text: &str, page_h: f64) {
    let face_id = fonts.resolve("Arial", false, false);
    let face = fonts.face(face_id);
    let Some(font) = ctx.font(face_id) else { return };
    let ttf = face.ttf();
    let upem = face.units_per_em.max(1.0);
    let mut glyphs = vec![];
    let mut at = 0;
    for ch in text.chars() {
        let len = ch.len_utf8();
        let g = ttf.as_ref().and_then(|t| t.glyph_index(ch));
        let adv = ttf.as_ref().zip(g).and_then(|(t, g)| t.glyph_hor_advance(g)).map(|a| a as f64 / upem);
        let adv = adv.unwrap_or(0.5) as f32;
        let gid = g.map(|g| g.0 as u32).unwrap_or(0);
        glyphs.push(KrillaGlyph::new(GlyphId::new(gid), adv, 0.0, 0.0, 0.0, at..at + len, None));
        at += len;
    }
    s.push_transform(&krilla::geom::Transform::from_translate(4.0, (page_h - 2.0) as f32));
    s.set_fill(Some(Fill { paint: krilla::color::luma::Color::black().into(), ..Default::default() }));
    s.set_stroke(None);
    s.draw_glyphs(Point::from_xy(0.0, 0.0), &glyphs, font, text, 5.0, false);
    s.pop();
}

/// One PDF page per plate per exported page (pages outer, plates inner).
pub fn export_separations(
    doc: &Document,
    layout: &DocLayout,
    fonts: &FontStore,
    _opts: &PdfOptions,
    indices: &[usize],
) -> Result<Vec<u8>, PdfError> {
    if indices.is_empty() {
        return Err(PdfError::Empty);
    }
    let _gray = GrayGuard::new();
    let names = plates(doc);
    let (w, h) = (doc.setup.width.0, doc.setup.height.0);
    let mut kd = KDoc::new();
    let mut ctx = Ctx { doc, fonts, kfonts: HashMap::new(), images: HashMap::new() };
    let mut mapper = Mapper { plate: String::new(), plate_ix: 0, images: HashMap::new(), next_fake: 0 };
    for &pi in indices {
        let items = flagged_items(doc, layout, pi);
        for (ix, name) in names.iter().enumerate() {
            let settings = PageSettings::from_wh(w as f32, h as f32).ok_or(PdfError::Empty)?;
            let mut page = kd.start_page_with(settings);
            let mut s = page.surface();
            // The paper is white on every plate.
            let mut pb = krilla::geom::PathBuilder::new();
            if let Some(r) = KRect::from_xywh(0.0, 0.0, w as f32, h as f32) {
                pb.push_rect(r);
            }
            if let Some(p) = pb.finish() {
                s.set_fill(Some(Fill { paint: krilla::color::luma::Color::white().into(), ..Default::default() }));
                s.set_stroke(None);
                s.draw_path(&p);
            }
            mapper.plate = name.clone();
            mapper.plate_ix = ix;
            let mapped: Vec<Item> = items.iter().filter_map(|(it, op)| mapper.item(&mut ctx, it, *op)).collect();
            let disp = PageDisplay { width: w, height: h, items: mapped };
            ctx.draw(&mut s, &disp, None);
            draw_label(&mut s, &mut ctx, fonts, name, h);
            s.finish();
            page.finish();
        }
    }
    kd.finish().map_err(|e| PdfError::Krilla(format!("{e:?}")))
}
