//! Lengths and geometry. All internal geometry is in points (1/72 inch).

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

pub const PT_PER_IN: f64 = 72.0;
pub const PT_PER_MM: f64 = 72.0 / 25.4;
pub const PT_PER_CM: f64 = 72.0 / 2.54;
pub const PT_PER_PI: f64 = 12.0;

/// A length in points. Deserialises from a number (points) or a string with a unit
/// suffix: `"8.5in"`, `"210mm"`, `"2cm"`, `"12pt"`, `"3pi"`, `"0.5\""`.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Length(pub f64);

impl Length {
    pub fn pt(self) -> f64 {
        self.0
    }
}

impl From<f64> for Length {
    fn from(v: f64) -> Self {
        Length(v)
    }
}

/// Parses a length string with an optional unit suffix into points.
pub fn parse_length(s: &str) -> Option<f64> {
    let s = s.trim();
    let units: [(&str, f64); 7] = [
        ("in", PT_PER_IN),
        ("\"", PT_PER_IN),
        ("mm", PT_PER_MM),
        ("cm", PT_PER_CM),
        ("pt", 1.0),
        ("pi", PT_PER_PI),
        ("px", 0.75),
    ];
    for (suffix, factor) in units {
        if let Some(num) = s.strip_suffix(suffix) {
            return num.trim().parse::<f64>().ok().map(|v| v * factor);
        }
    }
    s.parse::<f64>().ok()
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self.0)
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Length;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number of points or a string like \"8.5in\"")
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Length, E> {
                Ok(Length(v))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Length, E> {
                Ok(Length(v as f64))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Length, E> {
                Ok(Length(v as f64))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Length, E> {
                parse_length(v).map(Length).ok_or_else(|| E::custom(format!("invalid length {v:?}")))
            }
        }
        d.deserialize_any(V)
    }
}

/// Axis-aligned rectangle in points: top-left corner plus size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
    pub fn inset(&self, l: f64, t: f64, r: f64, b: f64) -> Rect {
        Rect::new(self.x + l, self.y + t, (self.w - l - r).max(0.0), (self.h - t - b).max(0.0))
    }
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(x, y, self.right().max(o.right()) - x, self.bottom().max(o.bottom()) - y)
    }
}

/// Rects deserialise from `[x, y, w, h]` (each a [`Length`]) or `{x, y, w, h}`.
impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum R {
            Arr([Length; 4]),
            Obj { x: Length, y: Length, w: Length, h: Length },
        }
        Ok(match R::deserialize(d)? {
            R::Arr([x, y, w, h]) => Rect::new(x.0, y.0, w.0, h.0),
            R::Obj { x, y, w, h } => Rect::new(x.0, y.0, w.0, h.0),
        })
    }
}

/// Four-sided insets (margins, padding, crops) in points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Insets {
    #[serde(default)]
    pub top: Length,
    #[serde(default)]
    pub bottom: Length,
    #[serde(default, alias = "inside")]
    pub left: Length,
    #[serde(default, alias = "outside")]
    pub right: Length,
}

impl Insets {
    pub fn uniform(v: f64) -> Self {
        Insets { top: Length(v), bottom: Length(v), left: Length(v), right: Length(v) }
    }
}

/// 2D affine transform `[a b c d e f]` mapping (x, y) → (a·x + c·y + e, b·x + d·y + f).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    pub fn translate(x: f64, y: f64) -> Self {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }
    pub fn scale(sx: f64, sy: f64) -> Self {
        Affine([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }
    /// Rotation by `deg` degrees clockwise (y-down coordinates).
    pub fn rotate(deg: f64) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Affine([c, s, -s, c, 0.0, 0.0])
    }
    /// `self ∘ other`: apply `other` first, then `self`.
    pub fn compose(self, other: Affine) -> Affine {
        let [a, b, c, d, e, f] = other.0;
        let [a2, b2, c2, d2, e2, f2] = self.0;
        Affine([
            a2 * a + c2 * b,
            b2 * a + d2 * b,
            a2 * c + c2 * d,
            b2 * c + d2 * d,
            a2 * e + c2 * f + e2,
            b2 * e + d2 * f + f2,
        ])
    }
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
}
