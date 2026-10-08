//! EMF (MS-EMF) records → SVG.

use crate::MetafileError;
use crate::svg::{Dc, IDENT, Obj, Out, Rgb, Xf, bezier_d, compose, ellipse_d, poly_d, round_rect_d};
use std::collections::HashMap;

struct R<'a> {
    b: &'a [u8],
}

impl R<'_> {
    fn u32(&self, at: usize) -> Option<u32> {
        self.b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn i32(&self, at: usize) -> Option<i32> {
        self.u32(at).map(|v| v as i32)
    }
    fn i16(&self, at: usize) -> Option<i16> {
        self.b.get(at..at + 2).map(|s| i16::from_le_bytes([s[0], s[1]]))
    }
    fn f32(&self, at: usize) -> Option<f32> {
        self.u32(at).map(f32::from_bits)
    }
}

fn stock(ih: u32) -> Option<Obj> {
    let gray = |v: u8| Obj::Brush { null: false, color: Rgb(v, v, v) };
    Some(match ih {
        0x8000_0000 => gray(255),
        0x8000_0001 => gray(192),
        0x8000_0002 => gray(128),
        0x8000_0003 => gray(64),
        0x8000_0004 => gray(0),
        0x8000_0005 => Obj::Brush { null: true, color: Rgb(0, 0, 0) },
        0x8000_0006 => Obj::Pen { null: false, width: 1.0, color: Rgb(255, 255, 255) },
        0x8000_0007 => Obj::Pen { null: false, width: 1.0, color: Rgb(0, 0, 0) },
        0x8000_0008 => Obj::Pen { null: true, width: 0.0, color: Rgb(0, 0, 0) },
        0x8000_000A..=0x8000_0011 => {
            Obj::Font { height: 12.0, face: "Liberation Sans".into(), bold: false, italic: false }
        }
        _ => return None,
    })
}

/// Device units per millimetre for the fixed map modes (MM_LOMETRIC .. MM_TWIPS): logical units per mm.
fn units_per_mm(mode: u32) -> Option<f64> {
    Some(match mode {
        2 => 10.0,          // MM_LOMETRIC
        3 => 100.0,         // MM_HIMETRIC
        4 => 100.0 / 25.4,  // MM_LOENGLISH
        5 => 1000.0 / 25.4, // MM_HIENGLISH
        6 => 1440.0 / 25.4, // MM_TWIPS
        _ => return None,
    })
}

pub fn convert(bytes: &[u8]) -> Result<String, MetafileError> {
    let r = R { b: bytes };
    let bad = |m: &str| MetafileError::Damaged(m.to_string());
    let hsize = r.u32(4).ok_or_else(|| bad("short header"))? as usize;
    if hsize < 88 || hsize > bytes.len() {
        return Err(bad("header size"));
    }
    let g = |at| r.i32(at).ok_or_else(|| bad("short header"));
    let (fl, ft, fr, fb) = (g(24)? as f64, g(28)? as f64, g(32)? as f64, g(36)? as f64);
    let (dev_w, dev_h, mm_w, mm_h) = (g(72)? as f64, g(76)? as f64, g(80)? as f64, g(84)? as f64);
    let dpmm_x = if mm_w > 0.0 && dev_w > 0.0 { dev_w / mm_w } else { 96.0 / 25.4 };
    let dpmm_y = if mm_h > 0.0 && dev_h > 0.0 { dev_h / mm_h } else { 96.0 / 25.4 };
    // The picture frame (0.01 mm) in device units is the SVG's view box.
    let view = (fl / 100.0 * dpmm_x, ft / 100.0 * dpmm_y, (fr - fl) / 100.0 * dpmm_x, (fb - ft) / 100.0 * dpmm_y);
    if !(view.2 > 0.0 && view.3 > 0.0) {
        return Err(bad("empty frame"));
    }

    let mut out = Out::new();
    let mut dc = Dc::default();
    let mut saved: Vec<Dc> = vec![];
    let mut objs: HashMap<u32, Obj> = HashMap::new();
    let mut path: Option<String> = None;
    let mut off = 0usize;
    while off + 8 <= bytes.len() {
        let ty = r.u32(off).unwrap_or(0);
        let size = r.u32(off + 4).unwrap_or(0) as usize;
        if size < 8 || off + size > bytes.len() {
            return Err(bad("record size"));
        }
        let p = off + 8;
        let pt32 = |i: usize| -> (f64, f64) { (r.i32(i).unwrap_or(0) as f64, r.i32(i + 4).unwrap_or(0) as f64) };
        let pt16 = |i: usize| -> (f64, f64) { (r.i16(i).unwrap_or(0) as f64, r.i16(i + 2).unwrap_or(0) as f64) };
        let rect = |i: usize| -> (f64, f64, f64, f64) {
            let (a, b) = pt32(i);
            let (c, d) = pt32(i + 8);
            (a, b, c, d)
        };
        // Emits or accumulates a figure.
        let figure = |d: String, fill: bool, stroke: bool, dc: &Dc, out: &mut Out, path: &mut Option<String>| match path
        {
            Some(acc) => {
                acc.push_str(&d);
                acc.push(' ');
            }
            None => out.path(dc, &d, fill, stroke),
        };
        match ty {
            14 => break, // EOF
            9 => dc.win_ext = pt32(p),
            10 => dc.win_org = pt32(p),
            11 => dc.vp_ext = pt32(p),
            12 => dc.vp_org = pt32(p),
            17 => {
                let mode = r.u32(p).unwrap_or(1);
                match mode {
                    7 | 8 => dc.mapped = true,
                    1 => dc.mapped = false,
                    m => {
                        if let Some(u) = units_per_mm(m) {
                            dc.mapped = true;
                            dc.win_ext = (1.0, 1.0);
                            dc.vp_ext = (dpmm_x / u, -dpmm_y / u);
                        }
                    }
                }
            }
            19 => dc.winding = r.u32(p) == Some(2),
            22 => dc.align = r.u32(p).unwrap_or(0),
            24 => dc.text_color = Rgb::from_colorref(r.u32(p).unwrap_or(0)),
            27 => {
                dc.pos = pt32(p);
                if let Some(acc) = path.as_mut() {
                    let (x, y) = dc.map(dc.pos.0, dc.pos.1);
                    acc.push_str(&format!("M{x:.3} {y:.3} "));
                }
            }
            54 => {
                let to = pt32(p);
                let a = dc.map(dc.pos.0, dc.pos.1);
                let b = dc.map(to.0, to.1);
                match path.as_mut() {
                    Some(acc) => acc.push_str(&format!("L{:.3} {:.3} ", b.0, b.1)),
                    None => out.path(&dc, &poly_d(&[a, b], false), false, true),
                }
                dc.pos = to;
            }
            33 => saved.push(dc.clone()),
            34 => {
                let n = r.i32(p).unwrap_or(-1);
                let k = if n < 0 { (-n) as usize } else { saved.len().saturating_sub(n as usize) };
                for _ in 0..k.max(1) {
                    if let Some(s) = saved.pop() {
                        dc = s;
                    }
                }
            }
            35 | 36 => {
                let xf: Xf = [
                    r.f32(p).unwrap_or(1.0) as f64,
                    r.f32(p + 4).unwrap_or(0.0) as f64,
                    r.f32(p + 8).unwrap_or(0.0) as f64,
                    r.f32(p + 12).unwrap_or(1.0) as f64,
                    r.f32(p + 16).unwrap_or(0.0) as f64,
                    r.f32(p + 20).unwrap_or(0.0) as f64,
                ];
                if ty == 35 {
                    dc.world = xf;
                } else {
                    match r.u32(p + 24).unwrap_or(1) {
                        1 => dc.world = IDENT,
                        2 => dc.world = compose(&dc.world, &xf),
                        3 => dc.world = compose(&xf, &dc.world),
                        4 => dc.world = xf,
                        _ => {}
                    }
                }
            }
            37 => {
                let ih = r.u32(p).unwrap_or(0);
                if let Some(o) = objs.get(&ih).cloned().or_else(|| stock(ih)) {
                    match o {
                        Obj::Pen { .. } => dc.pen = o,
                        Obj::Brush { .. } => dc.brush = o,
                        Obj::Font { .. } => dc.font = o,
                        Obj::Other => {}
                    }
                }
            }
            38 => {
                let ih = r.u32(p).unwrap_or(0);
                let style = r.u32(p + 4).unwrap_or(0);
                let width = r.i32(p + 8).unwrap_or(1) as f64;
                let color = Rgb::from_colorref(r.u32(p + 16).unwrap_or(0));
                objs.insert(ih, Obj::Pen { null: style & 0xF == 5, width, color });
            }
            95 => {
                let ih = r.u32(p).unwrap_or(0);
                let lp = p + 20;
                let style = r.u32(lp).unwrap_or(0);
                let width = r.u32(lp + 4).unwrap_or(1) as f64;
                let brush_style = r.u32(lp + 8).unwrap_or(0);
                let color = Rgb::from_colorref(r.u32(lp + 12).unwrap_or(0));
                objs.insert(ih, Obj::Pen { null: style & 0xF == 5 || brush_style == 1, width, color });
            }
            39 => {
                let ih = r.u32(p).unwrap_or(0);
                let style = r.u32(p + 4).unwrap_or(0);
                let color = Rgb::from_colorref(r.u32(p + 8).unwrap_or(0));
                objs.insert(ih, Obj::Brush { null: style == 1, color });
            }
            82 => {
                let ih = r.u32(p).unwrap_or(0);
                let lf = p + 4;
                let height = r.i32(lf).unwrap_or(12) as f64;
                let weight = r.i32(lf + 16).unwrap_or(400);
                let italic = bytes.get(lf + 20).copied().unwrap_or(0) != 0;
                let face_bytes = bytes.get(lf + 28..(lf + 28 + 64).min(off + size)).unwrap_or(&[]);
                let units: Vec<u16> = face_bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .take_while(|u| *u != 0)
                    .collect();
                let face = String::from_utf16_lossy(&units);
                objs.insert(ih, Obj::Font { height, face, bold: weight >= 600, italic });
            }
            40 => {
                objs.remove(&r.u32(p).unwrap_or(0));
            }
            // Brushes from bitmaps, palettes, colour spaces: keep the handle so selection does nothing.
            49 | 93 | 94 | 99 | 122 => {
                objs.insert(r.u32(p).unwrap_or(0), Obj::Other);
            }
            42 | 43 | 44 => {
                let (l, t, rr, b) = rect(p);
                let (x0, y0) = dc.map(l, t);
                let (x1, y1) = dc.map(rr, b);
                let d = match ty {
                    42 => ellipse_d(x0, y0, x1, y1),
                    43 => poly_d(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true),
                    _ => {
                        let (cw, ch) = pt32(p + 16);
                        let s = dc.scale();
                        round_rect_d(x0, y0, x1, y1, cw * s, ch * s)
                    }
                };
                figure(d, true, true, &dc, &mut out, &mut path);
            }
            2..=6 | 85..=89 => {
                let wide = ty <= 6;
                let n = r.u32(p + 16).unwrap_or(0) as usize;
                let start = p + 20;
                let step = if wide { 8 } else { 4 };
                if start + n * step > off + size {
                    return Err(bad("point count"));
                }
                let mut pts: Vec<(f64, f64)> = (0..n)
                    .map(|i| {
                        let (x, y) = if wide { pt32(start + i * 8) } else { pt16(start + i * 4) };
                        dc.map(x, y)
                    })
                    .collect();
                let kind = if wide { ty } else { ty - 83 };
                match kind {
                    2 => figure(bezier_d(&pts, false), false, true, &dc, &mut out, &mut path),
                    3 => figure(poly_d(&pts, true), true, true, &dc, &mut out, &mut path),
                    4 => figure(poly_d(&pts, false), false, true, &dc, &mut out, &mut path),
                    5 | 6 => {
                        // *TO variants continue from the current position.
                        let cur = dc.map(dc.pos.0, dc.pos.1);
                        let d = if kind == 5 {
                            let mut v = vec![cur];
                            v.append(&mut pts.clone());
                            bezier_d(&v, false)
                        } else {
                            pts.insert(0, cur);
                            poly_d(&pts, false)
                        };
                        let d = if path.is_some() { d.replacen('M', "L", 1) } else { d };
                        figure(d, false, true, &dc, &mut out, &mut path);
                        if let Some(last) = (0..n).last() {
                            dc.pos = if wide { pt32(start + last * 8) } else { pt16(start + last * 4) };
                        }
                    }
                    _ => {}
                }
            }
            7 | 8 | 90 | 91 => {
                let wide = ty <= 8;
                let polys = r.u32(p + 16).unwrap_or(0) as usize;
                let total = r.u32(p + 20).unwrap_or(0) as usize;
                let counts_at = p + 24;
                let pts_at = counts_at + polys * 4;
                let step = if wide { 8 } else { 4 };
                if pts_at + total * step > off + size {
                    return Err(bad("point count"));
                }
                let mut d = String::new();
                let mut k = 0;
                for i in 0..polys {
                    let c = r.u32(counts_at + i * 4).unwrap_or(0) as usize;
                    let pts: Vec<(f64, f64)> = (k..(k + c).min(total))
                        .map(|j| {
                            let (x, y) = if wide { pt32(pts_at + j * 8) } else { pt16(pts_at + j * 4) };
                            dc.map(x, y)
                        })
                        .collect();
                    d.push_str(&poly_d(&pts, ty == 8 || ty == 91));
                    k += c;
                }
                let filled = ty == 8 || ty == 91;
                figure(d, filled, true, &dc, &mut out, &mut path);
            }
            59 => path = Some(String::new()),
            60 => {}
            61 => {
                if let Some(acc) = path.as_mut() {
                    acc.push_str("Z ");
                }
            }
            62..=64 => {
                if let Some(d) = path.take() {
                    out.path(&dc, d.trim(), ty != 64, ty != 62);
                }
            }
            68 => path = None,
            84 => {
                // EMR_EXTTEXTOUTW: bounds, graphics mode, scales, then EMRTEXT.
                let t = p + 28;
                let (x, y) = pt32(t);
                let chars = r.u32(t + 8).unwrap_or(0) as usize;
                let off_str = r.u32(t + 12).unwrap_or(0) as usize;
                let s0 = off + off_str;
                if let Some(raw) = bytes.get(s0..s0 + chars * 2) {
                    let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                    out.text(&dc, x, y, &String::from_utf16_lossy(&units));
                }
            }
            _ => {}
        }
        off += size;
    }
    let w = format!("{:.3}mm", (fr - fl) / 100.0);
    let h = format!("{:.3}mm", (fb - ft) / 100.0);
    Ok(out.finish(view, &w, &h))
}
