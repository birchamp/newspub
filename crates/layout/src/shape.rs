//! Shaping with rustybuzz.

use crate::fonts::Face;
use rustybuzz::ttf_parser::Tag;
use rustybuzz::{Direction, Feature, UnicodeBuffer};

#[derive(Clone, Debug)]
pub struct ShapedGlyph {
    pub glyph: u16,
    /// Byte offset of the cluster in the shaped text.
    pub cluster: usize,
    /// Advance and offsets in em units.
    pub advance: f64,
    pub dx: f64,
    pub dy: f64,
}

pub struct ShapeOpts<'a> {
    pub kerning: bool,
    pub ligatures: bool,
    pub dlig: bool,
    pub small_caps: bool,
    pub extra: &'a [String],
    pub rtl: bool,
}

fn tag(s: &str) -> Option<Tag> {
    let b = s.as_bytes();
    (b.len() == 4).then(|| Tag::from_bytes(&[b[0], b[1], b[2], b[3]]))
}

/// Shaping results are cached: layout re-runs after every edit, but most paragraphs are unchanged.
/// Key: the face's data address and index (a FontStore's faces never change), the text and the options.
type ShapeKey = (usize, usize, u32, String, String, u8, Vec<String>);

static SHAPE_CACHE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<ShapeKey, Vec<ShapedGlyph>>>> =
    std::sync::LazyLock::new(Default::default);
const SHAPE_CACHE_MAX: usize = 50_000;

/// Shapes `text` with `face`. Returns glyphs in visual order with em-unit metrics.
pub fn shape(face: &Face, text: &str, o: &ShapeOpts) -> Vec<ShapedGlyph> {
    let flags = o.kerning as u8
        | (o.ligatures as u8) << 1
        | (o.dlig as u8) << 2
        | (o.small_caps as u8) << 3
        | (o.rtl as u8) << 4;
    let key: ShapeKey = (
        face.bytes().as_ptr() as usize,
        face.bytes().len(),
        face.index,
        face.postscript_name.clone(),
        text.to_string(),
        flags,
        o.extra.to_vec(),
    );
    if let Ok(cache) = SHAPE_CACHE.lock()
        && let Some(hit) = cache.get(&key)
    {
        return hit.clone();
    }
    let out = shape_uncached(face, text, o);
    if let Ok(mut cache) = SHAPE_CACHE.lock() {
        if cache.len() >= SHAPE_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, out.clone());
    }
    out
}

fn shape_uncached(face: &Face, text: &str, o: &ShapeOpts) -> Vec<ShapedGlyph> {
    let Some(rb) = rustybuzz::Face::from_slice(face.bytes(), face.index) else {
        return vec![];
    };
    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    buf.guess_segment_properties();
    buf.set_direction(if o.rtl { Direction::RightToLeft } else { Direction::LeftToRight });
    let mut feats = vec![];
    let toggle = |name: &str, on: bool, feats: &mut Vec<Feature>| {
        if let Some(t) = tag(name) {
            feats.push(Feature::new(t, on as u32, ..));
        }
    };
    toggle("kern", o.kerning, &mut feats);
    toggle("liga", o.ligatures, &mut feats);
    toggle("clig", o.ligatures, &mut feats);
    if o.dlig {
        toggle("dlig", true, &mut feats);
    }
    if o.small_caps {
        toggle("smcp", true, &mut feats);
    }
    for f in o.extra {
        if let Some(name) = f.strip_prefix('-') {
            toggle(name, false, &mut feats);
        } else {
            toggle(f, true, &mut feats);
        }
    }
    let out = rustybuzz::shape(&rb, &feats, buf);
    let upem = face.units_per_em;
    out.glyph_infos()
        .iter()
        .zip(out.glyph_positions())
        .map(|(i, p)| ShapedGlyph {
            glyph: i.glyph_id as u16,
            cluster: i.cluster as usize,
            advance: p.x_advance as f64 / upem,
            dx: p.x_offset as f64 / upem,
            dy: p.y_offset as f64 / upem,
        })
        .collect()
}

/// Does the face support an OpenType GSUB feature?
pub fn has_feature(face: &Face, name: &str) -> bool {
    let (Some(t), Some(f)) = (tag(name), face.ttf()) else { return false };
    f.tables().gsub.map(|g| g.features.into_iter().any(|x| x.tag == t)).unwrap_or(false)
}
