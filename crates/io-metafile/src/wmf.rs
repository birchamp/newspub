//! WMF (MS-WMF) records → SVG.

use crate::MetafileError;
use crate::svg::{Dc, Obj, Out, Rgb, ellipse_d, poly_d, round_rect_d};

struct R<'a> {
    b: &'a [u8],
}

impl R<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        self.b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&self, at: usize) -> Option<i16> {
        self.u16(at).map(|v| v as i16)
    }
    fn u32(&self, at: usize) -> Option<u32> {
        self.b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
}

/// Object table: a created object takes the lowest free index.
fn add(objs: &mut Vec<Option<Obj>>, o: Obj) {
    match objs.iter().position(Option::is_none) {
        Some(i) => objs[i] = Some(o),
        None => objs.push(Some(o)),
    }
}

/// ANSI text (Windows-1252 subset as Latin-1).
fn ansi(b: &[u8]) -> String {
    b.iter().take_while(|c| **c != 0).map(|&c| c as char).collect()
}

pub fn convert(bytes: &[u8]) -> Result<String, MetafileError> {
    let r = R { b: bytes };
    let bad = |m: &str| MetafileError::Damaged(m.to_string());
    // Placeable header: key, hmf, bbox (l, t, r, b), units per inch.
    let (mut off, frame) = if r.u32(0) == Some(0x9AC6_CDD7) {
        let g = |at| r.i16(at).ok_or_else(|| bad("short header"));
        let bbox = (g(6)? as f64, g(8)? as f64, g(10)? as f64, g(12)? as f64);
        let inch = r.u16(14).unwrap_or(1440).max(1) as f64;
        (22usize, Some((bbox, inch)))
    } else {
        (0usize, None)
    };
    let hwords = r.u16(off + 2).ok_or_else(|| bad("short header"))? as usize;
    off += hwords * 2;

    let mut out = Out::new();
    let mut dc = Dc { mapped: true, ..Dc::default() };
    let mut saved: Vec<Dc> = vec![];
    let mut objs: Vec<Option<Obj>> = vec![];
    // Without a placeable header the window defines the picture.
    let mut win_seen = false;
    while off + 6 <= bytes.len() {
        let words = r.u32(off).unwrap_or(0) as usize;
        let func = r.u16(off + 4).unwrap_or(0);
        if words < 3 || off + words * 2 > bytes.len() {
            return Err(bad("record size"));
        }
        let p = off + 6;
        let end = off + words * 2;
        let w = |i: usize| r.i16(p + i * 2).unwrap_or(0) as f64;
        let color = |i: usize| Rgb::from_colorref(r.u32(p + i * 2).unwrap_or(0));
        match func {
            0x0000 => break,
            0x020B => dc.win_org = (w(1), w(0)),
            0x020C => {
                dc.win_ext = (w(1), w(0));
                win_seen = true;
            }
            0x0106 => dc.winding = w(0) as i32 == 2,
            0x012E => dc.align = r.u16(p).unwrap_or(0) as u32,
            0x0209 => dc.text_color = color(0),
            0x001E => saved.push(dc.clone()),
            0x0127 => {
                let n = w(0) as i32;
                let k = if n < 0 { (-n) as usize } else { 1 };
                for _ in 0..k {
                    if let Some(s) = saved.pop() {
                        dc = s;
                    }
                }
            }
            0x02FA => {
                let style = r.u16(p).unwrap_or(0);
                add(&mut objs, Obj::Pen { null: style & 0xF == 5, width: w(1), color: color(3) });
            }
            0x02FC => {
                let style = r.u16(p).unwrap_or(0);
                add(&mut objs, Obj::Brush { null: style == 1, color: color(1) });
            }
            0x02FB => {
                let weight = w(4) as i32;
                let italic = bytes.get(p + 10).copied().unwrap_or(0) != 0;
                let face = ansi(bytes.get(p + 18..end).unwrap_or(&[]));
                add(&mut objs, Obj::Font { height: w(0), face, bold: weight >= 600, italic });
            }
            // Palettes, pattern brushes, regions: occupy a slot.
            0x00F7 | 0x01F9 | 0x0142 | 0x06FF => add(&mut objs, Obj::Other),
            0x012D => {
                let i = r.u16(p).unwrap_or(u16::MAX) as usize;
                if let Some(Some(o)) = objs.get(i).cloned() {
                    match o {
                        Obj::Pen { .. } => dc.pen = o,
                        Obj::Brush { .. } => dc.brush = o,
                        Obj::Font { .. } => dc.font = o,
                        Obj::Other => {}
                    }
                }
            }
            0x01F0 => {
                let i = r.u16(p).unwrap_or(u16::MAX) as usize;
                if let Some(slot) = objs.get_mut(i) {
                    *slot = None;
                }
            }
            0x041B | 0x0418 | 0x061C => {
                let o = if func == 0x061C { 2 } else { 0 };
                let (b, rr, t, l) = (w(o), w(o + 1), w(o + 2), w(o + 3));
                let (x0, y0) = dc.map(l, t);
                let (x1, y1) = dc.map(rr, b);
                let d = match func {
                    0x0418 => ellipse_d(x0, y0, x1, y1),
                    0x041B => poly_d(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true),
                    _ => {
                        let s = dc.scale();
                        round_rect_d(x0, y0, x1, y1, w(1) * s, w(0) * s)
                    }
                };
                out.path(&dc, &d, true, true);
            }
            0x0324 | 0x0325 => {
                let n = w(0).max(0.0) as usize;
                if p + 2 + n * 4 > end {
                    return Err(bad("point count"));
                }
                let pts: Vec<(f64, f64)> = (0..n).map(|i| dc.map(w(1 + 2 * i), w(2 + 2 * i))).collect();
                let closed = func == 0x0324;
                out.path(&dc, &poly_d(&pts, closed), closed, true);
            }
            0x0538 => {
                let polys = w(0).max(0.0) as usize;
                let mut k = 1 + polys;
                let mut d = String::new();
                for i in 0..polys {
                    let c = w(1 + i).max(0.0) as usize;
                    if p + (k + 2 * c) * 2 > end {
                        return Err(bad("point count"));
                    }
                    let pts: Vec<(f64, f64)> = (0..c).map(|j| dc.map(w(k + 2 * j), w(k + 2 * j + 1))).collect();
                    d.push_str(&poly_d(&pts, true));
                    k += 2 * c;
                }
                out.path(&dc, &d, true, true);
            }
            0x0214 => dc.pos = (w(1), w(0)),
            0x0213 => {
                let to = (w(1), w(0));
                let a = dc.map(dc.pos.0, dc.pos.1);
                let b = dc.map(to.0, to.1);
                out.path(&dc, &poly_d(&[a, b], false), false, true);
                dc.pos = to;
            }
            0x0521 => {
                let len = w(0).max(0.0) as usize;
                let text = ansi(bytes.get(p + 2..(p + 2 + len).min(end)).unwrap_or(&[]));
                let at = p + 2 + len.div_ceil(2) * 2;
                let (y, x) = (r.i16(at).unwrap_or(0) as f64, r.i16(at + 2).unwrap_or(0) as f64);
                out.text(&dc, x, y, &text);
            }
            0x0A32 => {
                let (y, x) = (w(0), w(1));
                let len = w(2).max(0.0) as usize;
                let opts = r.u16(p + 6).unwrap_or(0);
                let s = p + 8 + if opts & 0x0006 != 0 { 8 } else { 0 };
                let text = ansi(bytes.get(s..(s + len).min(end)).unwrap_or(&[]));
                out.text(&dc, x, y, &text);
            }
            _ => {}
        }
        off = end;
        // The placeable bbox is the output space: map the window onto it once both are known.
        if let Some(((l, t, rr, b), _)) = frame {
            if win_seen {
                dc.vp_org = (l, t);
                dc.vp_ext = (rr - l, b - t);
            } else {
                dc.vp_org = dc.win_org;
                dc.vp_ext = dc.win_ext;
            }
        } else {
            // No placeable header: the window becomes a positive output space, so a negative extent (a flipped
            // axis, common in pictures embedded in other documents) maps the right way up.
            dc.vp_org = (0.0, 0.0);
            dc.vp_ext = (dc.win_ext.0.abs(), dc.win_ext.1.abs());
        }
    }
    let (view, wdim, hdim) = match frame {
        Some(((l, t, rr, b), inch)) => {
            ((l, t, rr - l, b - t), format!("{:.4}in", (rr - l).abs() / inch), format!("{:.4}in", (b - t).abs() / inch))
        }
        None => {
            let (ew, eh) = (dc.win_ext.0.abs(), dc.win_ext.1.abs());
            ((0.0, 0.0, ew, eh), format!("{ew:.3}px"), format!("{eh:.3}px"))
        }
    };
    if !(view.2.abs() > 0.0 && view.3.abs() > 0.0) {
        return Err(bad("empty picture frame"));
    }
    Ok(out.finish(view, &wdim, &hdim))
}
