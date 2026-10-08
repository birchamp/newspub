//! Shared drawing state and SVG output for both metafile flavours.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn from_colorref(v: u32) -> Rgb {
        Rgb((v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8)
    }
    fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug)]
pub enum Obj {
    Pen { null: bool, width: f64, color: Rgb },
    Brush { null: bool, color: Rgb },
    Font { height: f64, face: String, bold: bool, italic: bool },
    Other,
}

/// Affine [a b c d e f]: x' = a·x + c·y + e, y' = b·x + d·y + f.
pub type Xf = [f64; 6];
pub const IDENT: Xf = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub fn compose(m: &Xf, n: &Xf) -> Xf {
    // m ∘ n: apply n, then m
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

#[derive(Clone, Debug)]
pub struct Dc {
    pub pen: Obj,
    pub brush: Obj,
    pub font: Obj,
    pub text_color: Rgb,
    pub win_org: (f64, f64),
    pub win_ext: (f64, f64),
    pub vp_org: (f64, f64),
    pub vp_ext: (f64, f64),
    /// Window/viewport mapping applies (anisotropic/isotropic modes, and always in WMF).
    pub mapped: bool,
    pub world: Xf,
    pub winding: bool,
    pub pos: (f64, f64),
    /// Text alignment flags (TA_*): 0 top-left; 8 bottom; 24 baseline; 2 right; 6 centre.
    pub align: u32,
}

impl Default for Dc {
    fn default() -> Self {
        Dc {
            pen: Obj::Pen { null: false, width: 1.0, color: Rgb(0, 0, 0) },
            brush: Obj::Brush { null: false, color: Rgb(255, 255, 255) },
            font: Obj::Font { height: 12.0, face: "Liberation Sans".into(), bold: false, italic: false },
            text_color: Rgb(0, 0, 0),
            win_org: (0.0, 0.0),
            win_ext: (1.0, 1.0),
            vp_org: (0.0, 0.0),
            vp_ext: (1.0, 1.0),
            mapped: false,
            world: IDENT,
            winding: false,
            pos: (0.0, 0.0),
            align: 0,
        }
    }
}

impl Dc {
    /// Logical → output coordinates.
    pub fn map(&self, x: f64, y: f64) -> (f64, f64) {
        let w = &self.world;
        let (x, y) = (w[0] * x + w[2] * y + w[4], w[1] * x + w[3] * y + w[5]);
        if !self.mapped {
            return (x, y);
        }
        let sx = if self.win_ext.0 != 0.0 { self.vp_ext.0 / self.win_ext.0 } else { 1.0 };
        let sy = if self.win_ext.1 != 0.0 { self.vp_ext.1 / self.win_ext.1 } else { 1.0 };
        ((x - self.win_org.0) * sx + self.vp_org.0, (y - self.win_org.1) * sy + self.vp_org.1)
    }

    /// Scale of a logical length (for pen widths and font sizes).
    pub fn scale(&self) -> f64 {
        let (x0, y0) = self.map(0.0, 0.0);
        let (x1, y1) = self.map(1.0, 0.0);
        let (x2, y2) = self.map(0.0, 1.0);
        let a = ((x1 - x0).hypot(y1 - y0) + (x2 - x0).hypot(y2 - y0)) / 2.0;
        if a.is_finite() && a > 0.0 { a } else { 1.0 }
    }
}

/// Collects SVG elements.
pub struct Out {
    pub body: String,
}

impl Out {
    pub fn new() -> Out {
        Out { body: String::new() }
    }

    fn style(&self, dc: &Dc, fill: bool, stroke: bool) -> String {
        let mut s = String::new();
        match (&dc.brush, fill) {
            (Obj::Brush { null: false, color }, true) => {
                let _ = write!(s, " fill=\"{}\"", color.css());
                if dc.winding {
                    s.push_str(" fill-rule=\"nonzero\"");
                } else {
                    s.push_str(" fill-rule=\"evenodd\"");
                }
            }
            _ => s.push_str(" fill=\"none\""),
        }
        match (&dc.pen, stroke) {
            (Obj::Pen { null: false, width, color }, true) => {
                let w = (width * dc.scale()).max(1.0);
                let _ = write!(s, " stroke=\"{}\" stroke-width=\"{:.3}\"", color.css(), w);
            }
            _ => s.push_str(" stroke=\"none\""),
        }
        s
    }

    /// Path data from figures of output-space points (`closed` per figure).
    pub fn path(&mut self, dc: &Dc, d: &str, fill: bool, stroke: bool) {
        if d.is_empty() {
            return;
        }
        let st = self.style(dc, fill, stroke);
        let _ = writeln!(self.body, "<path d=\"{d}\"{st}/>");
    }

    pub fn text(&mut self, dc: &Dc, x: f64, y: f64, text: &str) {
        if text.is_empty() {
            return;
        }
        let (px, py) = dc.map(x, y);
        let (size, face, bold, italic) = match &dc.font {
            Obj::Font { height, face, bold, italic } => {
                ((height.abs() * dc.scale()).max(1.0), face.clone(), *bold, *italic)
            }
            _ => (12.0, "Liberation Sans".to_string(), false, false),
        };
        let esc = text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let baseline = match dc.align & 24 {
            24 => "alphabetic",
            8 => "text-after-edge",
            _ => "text-before-edge",
        };
        let anchor = match dc.align & 6 {
            6 => "middle",
            2 => "end",
            _ => "start",
        };
        let face = face.replace(['"', '\'', '<', '>', '&'], "");
        let _ = writeln!(
            self.body,
            "<text x=\"{px:.3}\" y=\"{py:.3}\" font-family=\"{face}, Liberation Sans, sans-serif\" font-size=\"{size:.3}\"{}{} fill=\"{}\" dominant-baseline=\"{baseline}\" text-anchor=\"{anchor}\">{esc}</text>",
            if bold { " font-weight=\"bold\"" } else { "" },
            if italic { " font-style=\"italic\"" } else { "" },
            dc.text_color.css()
        );
    }

    pub fn finish(self, view: (f64, f64, f64, f64), width: &str, height: &str) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"{:.3} {:.3} {:.3} {:.3}\">\n{}</svg>\n",
            view.0, view.1, view.2, view.3, self.body
        )
    }
}

/// Path data helpers (points already in output space).
pub fn poly_d(pts: &[(f64, f64)], close: bool) -> String {
    let mut d = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let _ = write!(d, "{}{x:.3} {y:.3} ", if i == 0 { 'M' } else { 'L' });
    }
    if close && !pts.is_empty() {
        d.push('Z');
    }
    d
}

/// Bézier points: start, then (c1, c2, end) triples.
pub fn bezier_d(pts: &[(f64, f64)], continue_from: bool) -> String {
    let mut d = String::new();
    let mut it = pts.iter();
    if !continue_from {
        if let Some((x, y)) = it.next() {
            let _ = write!(d, "M{x:.3} {y:.3} ");
        }
    }
    let rest: Vec<_> = it.collect();
    for c in rest.chunks(3) {
        if let [a, b, e] = c {
            let _ = write!(d, "C{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} ", a.0, a.1, b.0, b.1, e.0, e.1);
        }
    }
    d
}

/// Ellipse inscribed in the (output-space, axis-aligned after mapping) box.
pub fn ellipse_d(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0);
    format!(
        "M{:.3} {cy:.3} A{rx:.3} {ry:.3} 0 1 0 {:.3} {cy:.3} A{rx:.3} {ry:.3} 0 1 0 {:.3} {cy:.3} Z",
        cx - rx,
        cx + rx,
        cx - rx
    )
}

pub fn round_rect_d(x0: f64, y0: f64, x1: f64, y1: f64, rw: f64, rh: f64) -> String {
    let (l, t, r, b) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
    let rx = (rw / 2.0).min((r - l) / 2.0).max(0.0);
    let ry = (rh / 2.0).min((b - t) / 2.0).max(0.0);
    format!(
        "M{:.3} {t:.3} H{:.3} A{rx:.3} {ry:.3} 0 0 1 {r:.3} {:.3} V{:.3} A{rx:.3} {ry:.3} 0 0 1 {:.3} {b:.3} H{:.3} A{rx:.3} {ry:.3} 0 0 1 {l:.3} {:.3} V{:.3} A{rx:.3} {ry:.3} 0 0 1 {:.3} {t:.3} Z",
        l + rx,
        r - rx,
        t + ry,
        b - ry,
        r - rx,
        l + rx,
        b - ry,
        t + ry,
        l + rx
    )
}
