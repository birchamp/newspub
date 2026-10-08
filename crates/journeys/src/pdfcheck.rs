//! `expect_pdf`: assertions on exported PDF files. Page numbers here are 1-based (as in PDF viewers).

use crate::runner::Ctx;
use anyhow::{Context, Result, anyhow, bail};
use lopdf::{Document as Pdf, Object};
use serde_json::Value;
use std::collections::BTreeSet;

fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// PDF text extraction may insert spaces between text runs, so matching ignores whitespace.
fn has(hay: &str, needle: &str) -> bool {
    let strip = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    hay.contains(&norm_ws(needle)) || strip(hay).contains(&strip(needle))
}

fn nums(o: &Object, pdf: &Pdf) -> Option<Vec<f64>> {
    let o = match o {
        Object::Reference(r) => pdf.get_object(*r).ok()?,
        o => o,
    };
    o.as_array()
        .ok()?
        .iter()
        .map(|x| x.as_float().ok().map(|f| f as f64).or_else(|| x.as_i64().ok().map(|i| i as f64)))
        .collect()
}

/// Page box (inherited from parents if needed) as [llx, lly, urx, ury].
fn page_box(pdf: &Pdf, page: lopdf::ObjectId, key: &[u8]) -> Option<Vec<f64>> {
    let mut cur = pdf.get_dictionary(page).ok()?;
    loop {
        if let Ok(o) = cur.get(key) {
            return nums(o, pdf);
        }
        let parent = cur.get(b"Parent").ok()?.as_reference().ok()?;
        cur = pdf.get_dictionary(parent).ok()?;
    }
}

fn approx_box(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

fn get_page(pdf: &Pdf, n: u64) -> Result<lopdf::ObjectId> {
    pdf.get_pages().get(&(n as u32)).copied().ok_or_else(|| anyhow!("PDF has no page {n}"))
}

fn as_vec(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default()
}

fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

pub fn check(ctx: &mut Ctx, spec: &Value) -> Result<()> {
    let file = spec.get("file").and_then(|f| f.as_str()).ok_or_else(|| anyhow!("expect_pdf needs file"))?;
    let path = ctx.out.join(file);
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let pdf = Pdf::load_mem(&bytes).map_err(|e| anyhow!("not a readable PDF: {e}"))?;
    let pages = pdf.get_pages();
    if let Some(n) = spec.get("pages").and_then(|v| v.as_u64())
        && pages.len() as u64 != n
    {
        bail!("expected {n} pages, PDF has {}", pages.len());
    }
    let tol = spec.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(0.5);
    // boxes: [{page, media: [llx,lly,urx,ury], trim, bleed, crop}]
    if let Some(Value::Array(list)) = spec.get("boxes") {
        for b in list {
            let n = b.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
            let pid = get_page(&pdf, n)?;
            for (k, key) in
                [("media", &b"MediaBox"[..]), ("trim", b"TrimBox"), ("bleed", b"BleedBox"), ("crop", b"CropBox")]
            {
                if let Some(want) = b.get(k) {
                    let got = page_box(&pdf, pid, key)
                        .ok_or_else(|| anyhow!("page {n} has no {}", String::from_utf8_lossy(key)))?;
                    if !approx_box(&got, &as_vec(want), tol) {
                        bail!("page {n} {k} box: expected {want}, got {got:?}");
                    }
                }
            }
        }
    }
    // page_size: {w, h} checked on every page's MediaBox (or `page` only)
    if let Some(ps) = spec.get("page_size") {
        let (w, h) =
            (ps.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0), ps.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0));
        let only = ps.get("page").and_then(|v| v.as_u64());
        for (n, pid) in &pages {
            if only.map(|o| o != *n as u64).unwrap_or(false) {
                continue;
            }
            let mb = page_box(&pdf, *pid, b"MediaBox").ok_or_else(|| anyhow!("no MediaBox"))?;
            let (gw, gh) = (mb[2] - mb[0], mb[3] - mb[1]);
            if (gw - w).abs() > tol || (gh - h).abs() > tol {
                bail!("page {n} size: expected {w}×{h}, got {gw}×{gh}");
            }
        }
    }
    let page_text = |n: u32| -> String { norm_ws(&pdf.extract_text(&[n]).unwrap_or_default()) };
    let want_all = strs(spec.get("text_contains"));
    let want_none = strs(spec.get("text_not_contains"));
    if !want_all.is_empty() || !want_none.is_empty() {
        let all: String = pages.keys().map(|n| page_text(*n)).collect::<Vec<_>>().join(" ");
        for w in want_all {
            if !has(&all, &w) {
                bail!("PDF text does not contain {w:?}; text starts {:?}", all.chars().take(200).collect::<String>());
            }
        }
        for w in want_none {
            if has(&all, &w) {
                bail!("PDF text unexpectedly contains {w:?}");
            }
        }
    }
    if let Some(Value::Array(list)) = spec.get("page_text") {
        for pt in list {
            let n = pt.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            get_page(&pdf, n as u64)?;
            let t = page_text(n);
            for w in strs(pt.get("contains")) {
                if !has(&t, &w) {
                    bail!(
                        "page {n} text does not contain {w:?}; page text starts {:?}",
                        t.chars().take(200).collect::<String>()
                    );
                }
            }
            for w in strs(pt.get("not_contains")) {
                if has(&t, &w) {
                    bail!("page {n} text unexpectedly contains {w:?}");
                }
            }
            if let Some(e) = pt.get("empty").and_then(|v| v.as_bool())
                && e != t.trim().is_empty()
            {
                bail!("page {n} text emptiness: expected {e}, text {:?}", t.chars().take(100).collect::<String>());
            }
        }
    }
    if let Some(f) = spec.get("fonts") {
        let mut names = BTreeSet::new();
        let mut unembedded = vec![];
        for obj in pdf.objects.values() {
            let Ok(d) = obj.as_dict() else { continue };
            if d.get(b"Type").ok().and_then(|t| t.as_name().ok()) != Some(b"FontDescriptor") {
                continue;
            }
            let name = d
                .get(b"FontName")
                .ok()
                .and_then(|n| n.as_name().ok())
                .map(|n| String::from_utf8_lossy(n).to_string())
                .unwrap_or_default();
            let base = name.split_once('+').map(|(_, b)| b.to_string()).unwrap_or(name.clone());
            if !(d.has(b"FontFile") || d.has(b"FontFile2") || d.has(b"FontFile3")) {
                unembedded.push(base.clone());
            }
            names.insert(base);
        }
        for want in strs(f.get("contains")) {
            if !names.iter().any(|n| n.contains(&want)) {
                bail!("PDF fonts {names:?} do not include {want:?}");
            }
        }
        if f.get("embedded").and_then(|v| v.as_bool()) == Some(true) && !unembedded.is_empty() {
            bail!("fonts not embedded: {unembedded:?}");
        }
        if let Some(n) = f.get("count").and_then(|v| v.as_u64())
            && names.len() as u64 != n
        {
            bail!("expected {n} fonts, found {names:?}");
        }
    }
    if let Some(n) = spec.get("images").and_then(|v| v.as_u64()) {
        let mut smasks = BTreeSet::new();
        let mut images = vec![];
        for (id, obj) in pdf.objects.iter() {
            let Ok(s) = obj.as_stream() else { continue };
            if s.dict.get(b"Subtype").ok().and_then(|t| t.as_name().ok()) == Some(b"Image") {
                images.push(*id);
                if let Ok(r) = s.dict.get(b"SMask").and_then(|m| m.as_reference()) {
                    smasks.insert(r);
                }
            }
        }
        let count = images.iter().filter(|i| !smasks.contains(i)).count() as u64;
        if count != n {
            bail!("expected {n} images in PDF, found {count}");
        }
    }
    if let Some(Value::Array(list)) = spec.get("ink") {
        let rendered = render(&bytes)?;
        for probe in list {
            let n = probe.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let (w, h, px) = rendered.get(n - 1).ok_or_else(|| anyhow!("no page {n} to probe"))?;
            let r = as_vec(probe.get("rect").ok_or_else(|| anyhow!("ink probe needs rect"))?);
            if r.len() != 4 {
                bail!("ink rect is [x, y, w, h]");
            }
            let frac = ink_fraction(*w, *h, px, &r, SCALE);
            let min = probe.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let max = probe.get("max").and_then(|v| v.as_f64()).unwrap_or(1.0);
            if frac < min || frac > max {
                bail!("page {n} ink in {r:?} is {frac:.4}, expected within [{min}, {max}]");
            }
        }
    }
    Ok(())
}

const SCALE: f32 = 2.0;

/// Renders all pages with hayro at 144 dpi; returns (w, h, rgba).
fn render(bytes: &[u8]) -> Result<Vec<(u32, u32, Vec<u8>)>> {
    let pdf = hayro::hayro_syntax::Pdf::new(bytes.to_vec()).map_err(|e| anyhow!("hayro: {e:?}"))?;
    let cache = hayro::RenderCache::new();
    let settings = hayro::hayro_interpret::InterpreterSettings::default();
    let mut out = vec![];
    for page in pdf.pages().iter() {
        let pm = hayro::render(
            page,
            &cache,
            &settings,
            &hayro::RenderSettings::default(),
            &hayro::PixmapSettings {
                x_scale: SCALE,
                y_scale: SCALE,
                bg_color: hayro::vello_cpu::color::palette::css::WHITE,
            },
        );
        out.push((pm.width() as u32, pm.height() as u32, pm.data_as_u8_slice().to_vec()));
    }
    Ok(out)
}

/// Fraction of pixels darker than near-white inside `r` = [x, y, w, h] (points, top-left origin).
pub fn ink_fraction(w: u32, h: u32, px: &[u8], r: &[f64], scale: f32) -> f64 {
    let s = scale as f64;
    let (x0, y0) = ((r[0] * s).floor().max(0.0) as u32, (r[1] * s).floor().max(0.0) as u32);
    let (x1, y1) = (((r[0] + r[2]) * s).ceil().min(w as f64) as u32, ((r[1] + r[3]) * s).ceil().min(h as f64) as u32);
    let (mut ink, mut total) = (0u64, 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            let (rr, g, b) = (px[i] as u32, px[i + 1] as u32, px[i + 2] as u32);
            total += 1;
            if rr + g + b < 3 * 225 {
                ink += 1;
            }
        }
    }
    if total == 0 { 0.0 } else { ink as f64 / total as f64 }
}
