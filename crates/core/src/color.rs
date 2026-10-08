//! Colours. The model keeps the colour space the user chose (RGB, CMYK, or spot) so print
//! output can honour it; screen rendering converts to sRGB.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "space", rename_all = "snake_case")]
pub enum Color {
    /// sRGB components 0–255 and alpha 0–1.
    Rgb { r: u8, g: u8, b: u8, a: f32 },
    /// Process CMYK, components 0–1.
    Cmyk { c: f32, m: f32, y: f32, k: f32, a: f32 },
    /// Named spot colour with a CMYK alternate and a tint 0–1.
    Spot { name: String, c: f32, m: f32, y: f32, k: f32, tint: f32, a: f32 },
}

impl Color {
    pub const BLACK: Color = Color::Rgb { r: 0, g: 0, b: 0, a: 1.0 };
    pub const WHITE: Color = Color::Rgb { r: 255, g: 255, b: 255, a: 1.0 };

    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::Rgb { r, g, b, a: 1.0 }
    }

    pub fn alpha(&self) -> f32 {
        match self {
            Color::Rgb { a, .. } | Color::Cmyk { a, .. } | Color::Spot { a, .. } => *a,
        }
    }

    /// Naive device conversion to sRGB for screen display.
    pub fn to_rgba8(&self) -> [u8; 4] {
        let cmyk = |c: f32, m: f32, y: f32, k: f32| {
            let f = |v: f32| ((1.0 - v.clamp(0.0, 1.0)) * (1.0 - k.clamp(0.0, 1.0)) * 255.0).round() as u8;
            [f(c), f(m), f(y)]
        };
        let a8 = (self.alpha().clamp(0.0, 1.0) * 255.0).round() as u8;
        match self {
            Color::Rgb { r, g, b, .. } => [*r, *g, *b, a8],
            Color::Cmyk { c, m, y, k, .. } => {
                let [r, g, b] = cmyk(*c, *m, *y, *k);
                [r, g, b, a8]
            }
            Color::Spot { c, m, y, k, tint, .. } => {
                let [r, g, b] = cmyk(c * tint, m * tint, y * tint, k * tint);
                [r, g, b, a8]
            }
        }
    }

    /// Parses `#rrggbb`, `#rrggbbaa`, `cmyk(c,m,y,k)` with 0–100 components, or a few CSS names.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
            return match hex.len() {
                6 => Some(Color::Rgb { r: p(0)?, g: p(2)?, b: p(4)?, a: 1.0 }),
                8 => Some(Color::Rgb { r: p(0)?, g: p(2)?, b: p(4)?, a: p(6)? as f32 / 255.0 }),
                _ => None,
            };
        }
        if let Some(inner) = s.strip_prefix("cmyk(").and_then(|r| r.strip_suffix(')')) {
            let v: Vec<f32> = inner.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 4 {
                return Some(Color::Cmyk {
                    c: v[0] / 100.0,
                    m: v[1] / 100.0,
                    y: v[2] / 100.0,
                    k: v[3] / 100.0,
                    a: 1.0,
                });
            }
            return None;
        }
        match s {
            "black" => Some(Color::BLACK),
            "white" => Some(Color::WHITE),
            "red" => Some(Color::rgb(255, 0, 0)),
            "green" => Some(Color::rgb(0, 128, 0)),
            "blue" => Some(Color::rgb(0, 0, 255)),
            "none" | "transparent" => Some(Color::Rgb { r: 0, g: 0, b: 0, a: 0.0 }),
            _ => None,
        }
    }
}

/// Colours deserialise from a string (see [`Color::parse`]) or the tagged object form.
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "space", rename_all = "snake_case")]
        enum Tagged {
            Rgb {
                r: u8,
                g: u8,
                b: u8,
                #[serde(default = "one")]
                a: f32,
            },
            Cmyk {
                c: f32,
                m: f32,
                y: f32,
                k: f32,
                #[serde(default = "one")]
                a: f32,
            },
            Spot {
                name: String,
                c: f32,
                m: f32,
                y: f32,
                k: f32,
                #[serde(default = "one")]
                tint: f32,
                #[serde(default = "one")]
                a: f32,
            },
        }
        fn one() -> f32 {
            1.0
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Any {
            Str(String),
            Tagged(Tagged),
        }
        match Any::deserialize(d)? {
            Any::Str(s) => Color::parse(&s).ok_or_else(|| de::Error::custom(format!("invalid colour {s:?}"))),
            Any::Tagged(Tagged::Rgb { r, g, b, a }) => Ok(Color::Rgb { r, g, b, a }),
            Any::Tagged(Tagged::Cmyk { c, m, y, k, a }) => Ok(Color::Cmyk { c, m, y, k, a }),
            Any::Tagged(Tagged::Spot { name, c, m, y, k, tint, a }) => Ok(Color::Spot { name, c, m, y, k, tint, a }),
        }
    }
}
