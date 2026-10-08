//! `expect_png`: rendered page vs approved golden image, and ink probes on the render.

use crate::runner::Ctx;
use anyhow::{Result, anyhow, bail};
use newpub_engine::Session;
use serde_json::Value;

pub fn check(s: &mut Session, ctx: &mut Ctx, spec: &Value) -> Result<()> {
    let page = spec.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let dpi = spec.get("dpi").and_then(|v| v.as_f64()).unwrap_or(72.0);
    let pm = s.render_page(page, dpi).map_err(|e| anyhow!("{e}"))?;
    let (w, h) = (pm.width(), pm.height());
    // tiny-skia data is premultiplied RGBA on an opaque white background, so it is plain RGBA here.
    let px = pm.data().to_vec();
    if let Some(Value::Array(list)) = spec.get("ink") {
        for probe in list {
            let r: Vec<f64> = probe
                .get("rect")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                .unwrap_or_default();
            if r.len() != 4 {
                bail!("ink rect is [x, y, w, h]");
            }
            let frac = crate::pdfcheck::ink_fraction(w, h, &px, &r, (dpi / 72.0) as f32);
            let min = probe.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let max = probe.get("max").and_then(|v| v.as_f64()).unwrap_or(1.0);
            if frac < min || frac > max {
                bail!("page {page} render ink in {r:?} is {frac:.4}, expected within [{min}, {max}]");
            }
        }
    }
    // pixels: [{at: [x, y] (points), rgb: [r, g, b], tol: 40}]
    if let Some(Value::Array(list)) = spec.get("pixels") {
        let k = dpi / 72.0;
        for probe in list {
            let at: Vec<f64> = probe
                .get("at")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                .unwrap_or_default();
            let want: Vec<i32> = probe
                .get("rgb")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_i64().map(|v| v as i32)).collect())
                .unwrap_or_default();
            if at.len() != 2 || want.len() != 3 {
                bail!("pixel probe needs at: [x, y] and rgb: [r, g, b]");
            }
            let tol = probe.get("tol").and_then(|v| v.as_i64()).unwrap_or(40) as i32;
            let (x, y) = ((at[0] * k) as u32, (at[1] * k) as u32);
            if x >= w || y >= h {
                bail!("pixel probe {at:?} is outside the page");
            }
            let i = ((y * w + x) * 4) as usize;
            let got = [px[i] as i32, px[i + 1] as i32, px[i + 2] as i32];
            if got.iter().zip(&want).any(|(a, b)| (a - b).abs() > tol) {
                bail!("page {page} pixel at {at:?} is rgb{got:?}, expected rgb{want:?} ±{tol}");
            }
        }
    }
    let Some(name) = spec.get("golden").and_then(|v| v.as_str()) else { return Ok(()) };
    let tol = spec.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(0.003);
    let actual_png = pm.encode_png().map_err(|e| anyhow!("{e}"))?;
    let actual_path = ctx.out.join(format!("{name}.png"));
    std::fs::write(&actual_path, &actual_png)?;
    let rel = ctx.rel(&actual_path);
    ctx.artifacts.push(rel);
    let golden_path = ctx.root.join("goldens").join(&ctx.id).join(format!("{name}.png"));
    if !golden_path.exists() {
        let pending = ctx.out.join("pending");
        std::fs::create_dir_all(&pending)?;
        std::fs::write(pending.join(format!("{name}.png")), &actual_png)?;
        ctx.needs_approval.push(name.to_string());
        return Ok(());
    }
    let golden = image::open(&golden_path).map_err(|e| anyhow!("golden {}: {e}", golden_path.display()))?.to_rgba8();
    if golden.width() != w || golden.height() != h {
        bail!("golden {name} is {}×{}, render is {w}×{h}", golden.width(), golden.height());
    }
    let mut diff = image::RgbaImage::new(w, h);
    let mut bad = 0u64;
    for (i, (a, b)) in px.chunks_exact(4).zip(golden.as_raw().chunks_exact(4)).enumerate() {
        let d = a.iter().zip(b).take(3).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
        let (x, y) = (i as u32 % w, i as u32 / w);
        if d > 40 {
            bad += 1;
            diff.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
        } else {
            let g = b[0] / 4 + 190;
            diff.put_pixel(x, y, image::Rgba([g, g, g, 255]));
        }
    }
    let frac = bad as f64 / (w as f64 * h as f64);
    if frac > tol {
        let dp = ctx.out.join(format!("{name}-diff.png"));
        diff.save(&dp).ok();
        let rel = ctx.rel(&dp);
        ctx.artifacts.push(rel);
        bail!("render differs from golden {name}: {:.3}% of pixels (tolerance {:.3}%)", frac * 100.0, tol * 100.0);
    }
    Ok(())
}

/// `expect_image`: an exported image file — `{file, format: png|jpeg, width, height, pixels: [{at: [px, py], rgb, tol}]}`.
/// Pixel coordinates here are image pixels.
pub fn check_file(ctx: &mut Ctx, spec: &Value) -> Result<()> {
    let file = spec.get("file").and_then(|f| f.as_str()).ok_or_else(|| anyhow!("expect_image needs file"))?;
    let path = ctx.out.join(file);
    let bytes = std::fs::read(&path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let fmt = image::guess_format(&bytes).map_err(|e| anyhow!("{file}: not an image: {e}"))?;
    if let Some(want) = spec.get("format").and_then(|f| f.as_str()) {
        let got = match fmt {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpeg",
            _ => "other",
        };
        if got != want {
            bail!("{file} is {got}, expected {want}");
        }
    }
    let img = image::load_from_memory(&bytes).map_err(|e| anyhow!("{file}: {e}"))?.to_rgba8();
    for (k, v) in [("width", img.width()), ("height", img.height())] {
        if let Some(want) = spec.get(k).and_then(|x| x.as_u64()) {
            if want != v as u64 {
                bail!("{file} {k} is {v}, expected {want}");
            }
        }
    }
    if let Some(Value::Array(list)) = spec.get("pixels") {
        for probe in list {
            let at: Vec<u32> = probe
                .get("at")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_f64().map(|v| v as u32)).collect())
                .unwrap_or_default();
            let want: Vec<i32> = probe
                .get("rgb")
                .and_then(|r| r.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_i64().map(|v| v as i32)).collect())
                .unwrap_or_default();
            if at.len() != 2 || want.len() != 3 {
                bail!("pixel probe needs at: [x, y] and rgb: [r, g, b]");
            }
            if at[0] >= img.width() || at[1] >= img.height() {
                bail!("pixel probe {at:?} outside the image");
            }
            let tol = probe.get("tol").and_then(|v| v.as_i64()).unwrap_or(40) as i32;
            let p = img.get_pixel(at[0], at[1]).0;
            let got = [p[0] as i32, p[1] as i32, p[2] as i32];
            if got.iter().zip(&want).any(|(a, b)| (a - b).abs() > tol) {
                bail!("{file} pixel {at:?} is rgb{got:?}, expected rgb{want:?} ±{tol}");
            }
        }
    }
    let r = ctx.rel(&path);
    ctx.artifacts.push(r);
    Ok(())
}
