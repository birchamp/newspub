//! newpub-io-html: HTML export (EX-04).
//!
//! Writes one HTML5 file per publication page (`index.html`, `page-2.html`, …) plus an `assets/` folder with the
//! picture files. Objects are absolutely positioned inside a fixed-size page box (px = pt × 4/3).

use newpub_core::{
    Color, Dash, Document, Fit, Id, ImageFrame, ImageMask, Object, ObjectKind, Shape, ShapeKind, Stroke,
};
use newpub_layout::{DecorationKind, DocLayout, FontStore, FrameLayout};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Points to CSS pixels.
fn px(pt: f64) -> String {
    let v = pt * 4.0 / 3.0;
    let s = format!("{:.2}", if v.is_finite() { v } else { 0.0 });
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn css_color(c: &Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    let a = c.alpha().clamp(0.0, 1.0);
    if a >= 0.999 { format!("rgb({r},{g},{b})") } else { format!("rgba({r},{g},{b},{a:.3})") }
}

fn page_file(i: usize) -> String {
    if i == 0 { "index.html".to_string() } else { format!("page-{}.html", i + 1) }
}

/// Asset file names (relative to `assets/`), unique per asset id.
fn asset_names(doc: &Document) -> BTreeMap<Id, String> {
    let mut out = BTreeMap::new();
    for (id, a) in &doc.assets {
        if !a.mime.starts_with("image/") {
            continue;
        }
        let stem: String = a
            .name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let ext = match a.mime.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/svg+xml" => "svg",
            "image/webp" => "webp",
            _ => "bin",
        };
        let stem = stem.trim_start_matches('.');
        let has_ext = stem
            .rsplit_once('.')
            .is_some_and(|(_, e)| e.eq_ignore_ascii_case(ext) || (ext == "jpg" && e.eq_ignore_ascii_case("jpeg")));
        let name = if has_ext { stem.to_string() } else { format!("{stem}.{ext}") };
        out.insert(*id, format!("{}-{}", id.0, name));
    }
    out
}

/// Exports `doc` as a set of HTML pages plus an `assets/` folder into `dir` (created if needed).
pub fn export(doc: &Document, layout: &DocLayout, fonts: &FontStore, dir: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(dir)?;
    let names = asset_names(doc);
    if !names.is_empty() {
        let adir = dir.join("assets");
        std::fs::create_dir_all(&adir)?;
        for (id, name) in &names {
            let a = &doc.assets[id];
            let bytes: Vec<u8> = if a.bytes.is_empty() {
                a.link.as_ref().and_then(|l| std::fs::read(l).ok()).unwrap_or_default()
            } else {
                a.bytes.to_vec()
            };
            std::fs::write(adir.join(name), bytes)?;
        }
    }
    let n = doc.pages.len();
    let ctx = Ctx { doc, layout, fonts, names: &names };
    for i in 0..n {
        std::fs::write(dir.join(page_file(i)), ctx.page_html(i, n))?;
    }
    Ok(())
}

struct Ctx<'a> {
    doc: &'a Document,
    layout: &'a DocLayout,
    fonts: &'a FontStore,
    names: &'a BTreeMap<Id, String>,
}

impl Ctx<'_> {
    fn page_html(&self, i: usize, n: usize) -> String {
        let doc = self.doc;
        let title = if doc.meta.title.trim().is_empty() { format!("Page {}", i + 1) } else { doc.meta.title.clone() };
        let lang = if doc.meta.lang.trim().is_empty() { "en" } else { doc.meta.lang.trim() };
        let (w, h) = (doc.setup.width.0, doc.setup.height.0);
        let page = &doc.pages[i];
        let master = doc.master_for_page(i);
        let bg = page
            .background
            .clone()
            .or_else(|| master.and_then(|m| m.background.clone()))
            .map(|c| css_color(&c))
            .unwrap_or_else(|| "#fff".to_string());
        let mut o = String::new();
        let _ = write!(
            o,
            "<!DOCTYPE html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n",
            escape(lang),
            escape(&title)
        );
        if !doc.meta.author.is_empty() {
            let _ = writeln!(o, "<meta name=\"author\" content=\"{}\">", escape(&doc.meta.author));
        }
        o.push_str(
            "<style>\nbody{margin:0;background:#e6e6e6;font-family:sans-serif}\n\
             nav{text-align:center;padding:8px}\nnav a{margin:0 12px}\n\
             .page{position:relative;margin:8px auto;overflow:hidden;box-shadow:0 0 6px rgba(0,0,0,.4)}\n\
             .obj{position:absolute;box-sizing:border-box}\n\
             .ln{position:absolute;white-space:pre;margin:0}\n\
             .obj img{display:block}\n</style>\n</head>\n<body>\n",
        );
        nav(&mut o, i, n);
        let _ = writeln!(o, "<div class=\"page\" style=\"width:{}px;height:{}px;background:{}\">", px(w), px(h), bg);
        if let Some(m) = master {
            for id in &m.objects {
                self.object_html(&mut o, *id, 0);
            }
        }
        for id in doc.draw_order(i) {
            self.object_html(&mut o, id, 0);
        }
        o.push_str("</div>\n");
        nav(&mut o, i, n);
        o.push_str("</body>\n</html>\n");
        o
    }

    fn object_html(&self, o: &mut String, id: Id, depth: usize) {
        let Ok(ob) = self.doc.object(id) else { return };
        let label = if ob.name.is_empty() { String::new() } else { format!(" data-name=\"{}\"", escape(&ob.name)) };
        match &ob.kind {
            ObjectKind::Group { children } => {
                if depth < 16 {
                    for c in children {
                        self.object_html(o, *c, depth + 1);
                    }
                }
            }
            ObjectKind::Text(tf) => {
                let mut st = object_style(ob);
                if let Some(f) = &tf.fill {
                    let _ = write!(st, ";background:{}", css_color(f));
                }
                if let Some(s) = &tf.stroke {
                    st.push_str(&border_css(s));
                }
                let _ = writeln!(o, "<div class=\"obj text\"{label} style=\"{st}\">");
                if let Some(fl) = self.layout.frames.get(&id) {
                    self.frame_html(o, fl);
                }
                o.push_str("</div>\n");
            }
            ObjectKind::Image(im) => self.image_html(o, ob, im, &label),
            ObjectKind::WordArt(wa) => {
                let st = object_style(ob);
                let _ = writeln!(o, "<div class=\"obj wordart\"{label} style=\"{st}\">");
                wordart_svg(o, ob, wa, self.fonts);
                o.push_str("</div>\n");
            }
            ObjectKind::Table(tb) => {
                // Cell text is laid out relative to the table's top-left corner.
                let st = object_style(ob);
                let _ = writeln!(o, "<div class=\"obj table\"{label} style=\"{st}\">");
                for r in 0..tb.rows() {
                    for c in 0..tb.cols() {
                        let (Some(cell), Some(cr)) = (tb.cell(r, c), tb.cell_rect(r, c)) else { continue };
                        if cell.covered {
                            continue;
                        }
                        let mut cs = format!(
                            "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px",
                            px(cr.x),
                            px(cr.y),
                            px(cr.w),
                            px(cr.h)
                        );
                        if let Some(f) = &cell.fill {
                            let _ = write!(cs, ";background:{}", css_color(f));
                        }
                        let _ = writeln!(o, "<div class=\"cell\" style=\"{cs}\"></div>");
                    }
                }
                for cell in &tb.cells {
                    if let Some(fl) = self.layout.frames.get(&cell.story) {
                        self.frame_html(o, fl);
                    }
                }
                o.push_str("</div>\n");
            }
            ObjectKind::Shape(sh) => {
                let st = object_style(ob);
                let _ = writeln!(o, "<div class=\"obj shape\"{label} style=\"{st}\">");
                shape_svg(o, ob, sh);
                if sh.story.is_some()
                    && let Some(fl) = self.layout.frames.get(&id)
                {
                    self.frame_html(o, fl);
                }
                o.push_str("</div>\n");
            }
        }
    }

    fn frame_html(&self, o: &mut String, fl: &FrameLayout) {
        for line in &fl.lines {
            let _ = write!(
                o,
                "<p class=\"ln\" style=\"left:{}px;top:{}px;height:{}px;line-height:{}px\">",
                px(line.x),
                px(line.top),
                px(line.height),
                px(line.height)
            );
            for run in &line.runs {
                let mut text = String::new();
                let mut last: Option<std::ops::Range<usize>> = None;
                for g in &run.glyphs {
                    if last.as_ref() != Some(&g.text_range) {
                        text.push_str(run.text.get(g.text_range.clone()).unwrap_or(""));
                        last = Some(g.text_range.clone());
                    }
                }
                if text.is_empty() {
                    continue;
                }
                let face = self.fonts.face(run.face);
                let mut st = format!(
                    "font-family:'{}',sans-serif;font-size:{}px;color:{}",
                    face.family.replace(['\'', '"', '\\', '<', '>', '&'], ""),
                    px(run.size),
                    css_color(&run.color)
                );
                if face.bold || run.synthetic_bold {
                    st.push_str(";font-weight:bold");
                }
                if face.italic || run.synthetic_italic {
                    st.push_str(";font-style:italic");
                }
                let (x0, x1) = match (run.glyphs.first(), run.glyphs.last()) {
                    (Some(a), Some(b)) => (a.x, b.x + b.advance),
                    _ => (0.0, 0.0),
                };
                let (mut under, mut strike) = (false, false);
                for d in &fl.decorations {
                    if d.y >= line.top - 1.0 && d.y <= line.top + line.height + 1.0 && d.x0 < x1 && d.x1 > x0 {
                        match d.kind {
                            DecorationKind::Underline => under = true,
                            DecorationKind::Strike => strike = true,
                        }
                    }
                }
                if under || strike {
                    let _ = write!(
                        st,
                        ";text-decoration:{}{}",
                        if under { "underline" } else { "" },
                        if strike { " line-through" } else { "" }
                    );
                }
                let _ = write!(o, "<span style=\"{}\">{}</span>", st, escape(&text));
            }
            o.push_str("</p>\n");
        }
    }

    fn image_html(&self, o: &mut String, ob: &Object, im: &ImageFrame, label: &str) {
        let Some(name) = im.asset.and_then(|a| self.names.get(&a)) else { return };
        let alt = if ob.decorative { String::new() } else { ob.alt_text.clone().unwrap_or_default() };
        let mut st = object_style(ob);
        st.push_str(";overflow:hidden");
        match im.mask {
            ImageMask::Rect => {}
            ImageMask::Ellipse => st.push_str(";border-radius:50%"),
            ImageMask::RoundRect => st.push_str(";border-radius:12%"),
        }
        if let Some(s) = &im.stroke {
            st.push_str(&border_css(s));
        }
        let c = im.crop;
        let kept_w = (1.0 - c.left - c.right).clamp(0.01, 1.0);
        let kept_h = (1.0 - c.top - c.bottom).clamp(0.01, 1.0);
        let fit = match im.fit {
            Fit::Stretch => "fill",
            Fit::Fit => "contain",
            Fit::Fill => "cover",
        };
        let _ = writeln!(
            o,
            "<div class=\"obj pic\"{label} style=\"{st}\"><img src=\"assets/{}\" alt=\"{}\" style=\"position:absolute;\
             width:{:.3}%;height:{:.3}%;left:{:.3}%;top:{:.3}%;object-fit:{fit}\"></div>",
            escape(name),
            escape(&alt),
            100.0 / kept_w,
            100.0 / kept_h,
            -c.left / kept_w * 100.0,
            -c.top / kept_h * 100.0,
        );
    }
}

fn nav(o: &mut String, i: usize, n: usize) {
    if n <= 1 {
        return;
    }
    o.push_str("<nav>");
    if i > 0 {
        let _ = write!(o, "<a href=\"{}\" rel=\"prev\">Previous</a>", page_file(i - 1));
    }
    if i + 1 < n {
        let _ = write!(o, "<a href=\"{}\" rel=\"next\">Next</a>", page_file(i + 1));
    }
    o.push_str("</nav>\n");
}

fn object_style(ob: &Object) -> String {
    let r = ob.rect;
    let mut s = format!("left:{}px;top:{}px;width:{}px;height:{}px", px(r.x), px(r.y), px(r.w), px(r.h));
    let mut t = String::new();
    if ob.rotation != 0.0 {
        let _ = write!(t, "rotate({}deg)", ob.rotation);
    }
    if ob.flip_h || ob.flip_v {
        let _ = write!(t, " scale({},{})", if ob.flip_h { -1 } else { 1 }, if ob.flip_v { -1 } else { 1 });
    }
    if !t.is_empty() {
        let _ = write!(s, ";transform:{}", t.trim());
    }
    if let Some(sh) = &ob.shadow {
        let _ = write!(
            s,
            ";filter:drop-shadow({}px {}px {}px {})",
            px(sh.dx.0),
            px(sh.dy.0),
            px(sh.blur.0),
            css_color(&sh.color)
        );
    }
    s
}

fn border_css(s: &Stroke) -> String {
    let style = match s.dash {
        Dash::Solid => "solid",
        Dash::Dot => "dotted",
        _ => "dashed",
    };
    format!(";border:{}px {} {}", px(s.width.0.max(0.5)), style, css_color(&s.color))
}

fn svg_paint(sh: &Shape, id_hint: u64, defs: &mut String) -> String {
    if let Some(g) = &sh.gradient
        && !g.stops.is_empty()
    {
        let gid = format!("g{id_hint}");
        let stops: String = g
            .stops
            .iter()
            .map(|s| format!("<stop offset=\"{:.3}\" stop-color=\"{}\"/>", s.at.clamp(0.0, 1.0), css_color(&s.color)))
            .collect();
        match g.kind {
            newpub_core::GradientKind::Linear => {
                let a = g.angle.to_radians();
                let (dx, dy) = (a.cos() / 2.0, a.sin() / 2.0);
                let _ = write!(
                    defs,
                    "<linearGradient id=\"{gid}\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\">{stops}</linearGradient>",
                    0.5 - dx,
                    0.5 - dy,
                    0.5 + dx,
                    0.5 + dy
                );
            }
            newpub_core::GradientKind::Radial => {
                let _ = write!(defs, "<radialGradient id=\"{gid}\">{stops}</radialGradient>");
            }
        }
        return format!("url(#{gid})");
    }
    sh.fill.as_ref().map(css_color).unwrap_or_else(|| "none".to_string())
}

/// WordArt (TY-19) as an inline SVG with the same outlines the other back ends draw.
fn wordart_svg(o: &mut String, ob: &Object, wa: &newpub_core::wordart::WordArt, fonts: &FontStore) {
    use newpub_render::PathEl;
    let (w, h) = (ob.rect.w.max(0.0), ob.rect.h.max(0.0));
    let mut d = String::new();
    for el in newpub_render::display::wordart_path(fonts, wa, w, h) {
        match el {
            PathEl::Move(x, y) => d.push_str(&format!("M{x:.2} {y:.2} ")),
            PathEl::Line(x, y) => d.push_str(&format!("L{x:.2} {y:.2} ")),
            PathEl::Cubic(a, b, c, e, x, y) => d.push_str(&format!("C{a:.2} {b:.2} {c:.2} {e:.2} {x:.2} {y:.2} ")),
            PathEl::Close => d.push_str("Z "),
        }
    }
    let mut defs = String::new();
    let mut fill = css_color(&wa.fill);
    if let Some(g) = wa.gradient.as_ref().filter(|g| !g.stops.is_empty()) {
        let gid = format!("wa{}", ob.id.0);
        let stops: String = g
            .stops
            .iter()
            .map(|s| format!("<stop offset=\"{:.3}\" stop-color=\"{}\"/>", s.at.clamp(0.0, 1.0), css_color(&s.color)))
            .collect();
        match g.kind {
            newpub_core::GradientKind::Linear => {
                let a = g.angle.to_radians();
                let (dx, dy) = (a.cos() / 2.0, a.sin() / 2.0);
                let _ = write!(
                    defs,
                    "<linearGradient id=\"{gid}\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\">{stops}</linearGradient>",
                    0.5 - dx,
                    0.5 - dy,
                    0.5 + dx,
                    0.5 + dy
                );
            }
            newpub_core::GradientKind::Radial => {
                let _ = write!(defs, "<radialGradient id=\"{gid}\">{stops}</radialGradient>");
            }
        }
        fill = format!("url(#{gid})");
    }
    let stroke = match &wa.outline {
        Some(s) => {
            format!(" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", css_color(&s.color), s.width.0)
        }
        None => " stroke=\"none\"".to_string(),
    };
    let defs = if defs.is_empty() { String::new() } else { format!("<defs>{defs}</defs>") };
    let alt = if ob.decorative { String::new() } else { ob.alt_text.clone().unwrap_or_else(|| wa.text.clone()) };
    let aria = if alt.is_empty() {
        " aria-hidden=\"true\"".to_string()
    } else {
        format!(" role=\"img\" aria-label=\"{}\"", escape(&alt))
    };
    let _ = writeln!(
        o,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {w} {h}\" \
         style=\"position:absolute;left:0;top:0;overflow:visible\"{aria}>{defs}<path d=\"{}\" fill=\"{fill}\"{stroke}/></svg>",
        px(w),
        px(h),
        d.trim_end()
    );
}

fn shape_svg(o: &mut String, ob: &Object, sh: &Shape) {
    let (w, h) = (ob.rect.w.max(0.0), ob.rect.h.max(0.0));
    let mut defs = String::new();
    let fill = svg_paint(sh, ob.id.0, &mut defs);
    let mut attrs = format!("fill=\"{fill}\"");
    match &sh.stroke {
        Some(s) => {
            let _ = write!(attrs, " stroke=\"{}\" stroke-width=\"{}\"", css_color(&s.color), s.width.0);
            match s.dash {
                Dash::Solid => {}
                Dash::Dash => attrs.push_str(" stroke-dasharray=\"6 3\""),
                Dash::Dot => attrs.push_str(" stroke-dasharray=\"1 3\""),
                Dash::DashDot => attrs.push_str(" stroke-dasharray=\"6 3 1 3\""),
                Dash::LongDash => attrs.push_str(" stroke-dasharray=\"12 4\""),
            }
        }
        None => attrs.push_str(" stroke=\"none\""),
    }
    let pts = |v: &[(f64, f64)]| v.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ");
    let body = match &sh.kind {
        ShapeKind::Rect => format!("<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" {attrs}/>"),
        ShapeKind::RoundRect { radius } => {
            let r = radius.0.clamp(0.0, w.min(h) / 2.0);
            format!("<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" rx=\"{r}\" ry=\"{r}\" {attrs}/>")
        }
        ShapeKind::Ellipse => {
            format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {attrs}/>", w / 2.0, h / 2.0, w / 2.0, h / 2.0)
        }
        ShapeKind::Line => format!("<line x1=\"0\" y1=\"0\" x2=\"{w}\" y2=\"{h}\" {attrs}/>"),
        ShapeKind::Triangle => format!("<polygon points=\"{}\" {attrs}/>", pts(&[(w / 2.0, 0.0), (w, h), (0.0, h)])),
        ShapeKind::Star { points, inner } => {
            let n = (*points).clamp(3, 64) as usize;
            let v: Vec<(f64, f64)> = (0..n * 2)
                .map(|k| {
                    let a = std::f64::consts::PI * (k as f64) / (n as f64) - std::f64::consts::FRAC_PI_2;
                    let r = if k % 2 == 0 { 1.0 } else { inner.clamp(0.05, 1.0) };
                    (w / 2.0 + a.cos() * r * w / 2.0, h / 2.0 + a.sin() * r * h / 2.0)
                })
                .collect();
            format!("<polygon points=\"{}\" {attrs}/>", pts(&v))
        }
        ShapeKind::Polygon { sides } => {
            let n = (*sides).clamp(3, 64) as usize;
            let v: Vec<(f64, f64)> = (0..n)
                .map(|k| {
                    let a = 2.0 * std::f64::consts::PI * (k as f64) / (n as f64) - std::f64::consts::FRAC_PI_2;
                    (w / 2.0 + a.cos() * w / 2.0, h / 2.0 + a.sin() * h / 2.0)
                })
                .collect();
            format!("<polygon points=\"{}\" {attrs}/>", pts(&v))
        }
        ShapeKind::Arrow => {
            let v = [
                (0.0, h * 0.3),
                (w * 0.6, h * 0.3),
                (w * 0.6, 0.0),
                (w, h / 2.0),
                (w * 0.6, h),
                (w * 0.6, h * 0.7),
                (0.0, h * 0.7),
            ];
            format!("<polygon points=\"{}\" {attrs}/>", pts(&v))
        }
        ShapeKind::Callout { tail } => {
            let (tx, ty) = (tail[0] * w, tail[1] * h);
            format!(
                "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" {attrs}/><polygon points=\"{}\" {attrs}/>",
                pts(&[(w * 0.2, h), (w * 0.4, h), (tx, ty)])
            )
        }
        ShapeKind::Path { points, closed } => {
            let v: Vec<(f64, f64)> = points.iter().map(|p| (p[0] * w, p[1] * h)).collect();
            if *closed {
                format!("<polygon points=\"{}\" {attrs}/>", pts(&v))
            } else {
                let a = attrs.replacen(&format!("fill=\"{fill}\""), "fill=\"none\"", 1);
                format!("<polyline points=\"{}\" {a}/>", pts(&v))
            }
        }
        ShapeKind::Bezier { nodes, closed } => {
            let mut d = String::new();
            for el in newpub_render::display::bezier_path(nodes, *closed, w, h) {
                match el {
                    newpub_render::PathEl::Move(x, y) => d.push_str(&format!("M{x:.2} {y:.2} ")),
                    newpub_render::PathEl::Line(x, y) => d.push_str(&format!("L{x:.2} {y:.2} ")),
                    newpub_render::PathEl::Cubic(a, b, c, e, x, y) => {
                        d.push_str(&format!("C{a:.2} {b:.2} {c:.2} {e:.2} {x:.2} {y:.2} "))
                    }
                    newpub_render::PathEl::Close => d.push('Z'),
                }
            }
            let a =
                if *closed { attrs.clone() } else { attrs.replacen(&format!("fill=\"{fill}\""), "fill=\"none\"", 1) };
            format!("<path d=\"{}\" {a}/>", d.trim_end())
        }
    };
    let alt = if ob.decorative { String::new() } else { ob.alt_text.clone().unwrap_or_default() };
    let aria = if alt.is_empty() {
        " aria-hidden=\"true\"".to_string()
    } else {
        format!(" role=\"img\" aria-label=\"{}\"", escape(&alt))
    };
    let defs = if defs.is_empty() { String::new() } else { format!("<defs>{defs}</defs>") };
    let _ = writeln!(
        o,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {w} {h}\" \
         style=\"position:absolute;left:0;top:0;overflow:visible\"{aria}>{defs}{body}</svg>",
        px(w),
        px(h)
    );
}
