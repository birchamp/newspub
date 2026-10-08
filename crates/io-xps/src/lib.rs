//! newpub-io-xps: XPS export (EX-06).
//!
//! Writes an XPS package (an OPC zip): one `FixedPage` per publication page with `Glyphs` for text, `Path` for shapes
//! and `ImageBrush` fills for pictures. Fonts are embedded as obfuscated `.odttf` parts. Units are 1/96 in (CSS px).

use newpub_core::{
    Color, Dash, Document, Fit, Id, ImageFrame, ImageMask, Object, ObjectKind, Shape, ShapeKind, Stroke,
};
use newpub_layout::{DocLayout, FontStore, FrameLayout};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Cursor, Seek, Write};
use std::path::Path;

const NS: &str = "http://schemas.microsoft.com/xps/2005/06";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const REL_RESOURCE: &str = "http://schemas.microsoft.com/xps/2005/06/required-resource";
const REL_FIXED: &str = "http://schemas.microsoft.com/xps/2005/06/fixedrepresentation";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Points to XPS units (1/96 in).
fn u(pt: f64) -> String {
    let v = pt * 4.0 / 3.0;
    let s = format!("{:.3}", if v.is_finite() { v } else { 0.0 });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".to_string() } else { s.to_string() }
}

/// Escapes text for use inside a double-quoted XML attribute, dropping characters XML cannot carry.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{fffe}' || c == '\u{ffff}' => {}
            _ => o.push(c),
        }
    }
    o
}

fn argb(c: &Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    let a = (c.alpha().clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

/// Writes `doc` as an XPS file at `path`.
pub fn export(doc: &Document, layout: &DocLayout, fonts: &FontStore, path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    write_xps(doc, layout, fonts, std::fs::File::create(path)?)
}

/// An embedded image part.
struct ImagePart {
    name: String,
    bytes: Vec<u8>,
    px_w: u32,
    px_h: u32,
}

/// An embedded font part.
struct FontPart {
    name: String,
    bytes: Vec<u8>,
}

struct Ctx<'a> {
    doc: &'a Document,
    layout: &'a DocLayout,
    fonts: &'a FontStore,
    images: &'a BTreeMap<Id, ImagePart>,
    font_parts: &'a BTreeMap<u32, FontPart>,
}

/// Resources one page refers to (for its relationships part).
#[derive(Default)]
struct Used {
    fonts: Vec<u32>,
    images: Vec<Id>,
}

/// Writes the XPS package to any seekable writer.
pub fn write_xps<W: Write + Seek>(doc: &Document, layout: &DocLayout, fonts: &FontStore, out: W) -> Result<(), Error> {
    let images = collect_images(doc);
    let font_parts = collect_fonts(doc, layout, fonts);
    let ctx = Ctx { doc, layout, fonts, images: &images, font_parts: &font_parts };
    let n = doc.pages.len();

    let mut z = zip::ZipWriter::new(out);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let decl = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n";

    let mut ct = format!("{decl}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    let defaults: [(&str, &str); 6] = [
        ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
        ("odttf", "application/vnd.ms-package.obfuscated-opentype"),
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("fdseq", "application/vnd.ms-package.xps-fixeddocumentsequence+xml"),
        ("fdoc", "application/vnd.ms-package.xps-fixeddocument+xml"),
    ];
    for (ext, mime) in defaults {
        let _ = write!(ct, "<Default Extension=\"{ext}\" ContentType=\"{mime}\"/>");
    }
    let _ = write!(
        ct,
        "<Default Extension=\"fpage\" ContentType=\"application/vnd.ms-package.xps-fixedpage+xml\"/></Types>"
    );
    // [Content_Types].xml first, as OPC consumers expect.
    z.start_file("[Content_Types].xml", opts)?;
    z.write_all(ct.as_bytes())?;

    z.start_file("_rels/.rels", opts)?;
    z.write_all(
        format!(
            "{decl}<Relationships xmlns=\"{REL_NS}\"><Relationship Id=\"R0\" Type=\"{REL_FIXED}\" \
             Target=\"/FixedDocumentSequence.fdseq\"/></Relationships>"
        )
        .as_bytes(),
    )?;

    z.start_file("FixedDocumentSequence.fdseq", opts)?;
    z.write_all(
        format!(
            "{decl}<FixedDocumentSequence xmlns=\"{NS}\"><DocumentReference Source=\"/Documents/1/FixedDocument.fdoc\"/>\
             </FixedDocumentSequence>"
        )
        .as_bytes(),
    )?;

    let mut fdoc = format!("{decl}<FixedDocument xmlns=\"{NS}\">");
    for i in 0..n {
        let _ = write!(fdoc, "<PageContent Source=\"Pages/{}.fpage\"/>", i + 1);
    }
    fdoc.push_str("</FixedDocument>");
    z.start_file("Documents/1/FixedDocument.fdoc", opts)?;
    z.write_all(fdoc.as_bytes())?;

    let (w, h) = (doc.setup.width.0, doc.setup.height.0);
    let lang = if doc.meta.lang.trim().is_empty() { "en-US" } else { doc.meta.lang.trim() };
    for i in 0..n {
        let mut used = Used::default();
        let mut body = String::new();
        ctx.page(&mut body, i, &mut used);
        let page = format!(
            "{decl}<FixedPage xmlns=\"{NS}\" xmlns:x=\"{NS}/resourcedictionary-key\" xml:lang=\"{}\" \
             Width=\"{}\" Height=\"{}\">\n{body}</FixedPage>\n",
            esc(lang),
            u(w),
            u(h)
        );
        z.start_file(format!("Documents/1/Pages/{}.fpage", i + 1), opts)?;
        z.write_all(page.as_bytes())?;
        if !used.fonts.is_empty() || !used.images.is_empty() {
            let mut rels = format!("{decl}<Relationships xmlns=\"{REL_NS}\">");
            let mut k = 0;
            for f in &used.fonts {
                if let Some(p) = font_parts.get(f) {
                    k += 1;
                    let _ = write!(
                        rels,
                        "<Relationship Id=\"R{k}\" Type=\"{REL_RESOURCE}\" Target=\"/Resources/Fonts/{}\"/>",
                        p.name
                    );
                }
            }
            for id in &used.images {
                if let Some(p) = images.get(id) {
                    k += 1;
                    let _ = write!(
                        rels,
                        "<Relationship Id=\"R{k}\" Type=\"{REL_RESOURCE}\" Target=\"/Resources/Images/{}\"/>",
                        p.name
                    );
                }
            }
            rels.push_str("</Relationships>");
            z.start_file(format!("Documents/1/Pages/_rels/{}.fpage.rels", i + 1), opts)?;
            z.write_all(rels.as_bytes())?;
        }
    }
    for p in images.values() {
        z.start_file(format!("Resources/Images/{}", p.name), stored)?;
        z.write_all(&p.bytes)?;
    }
    for p in font_parts.values() {
        z.start_file(format!("Resources/Fonts/{}", p.name), opts)?;
        z.write_all(&p.bytes)?;
    }
    z.finish()?;
    Ok(())
}

/// Picture parts: PNG and JPEG are embedded as they are, other decodable formats are re-encoded as PNG.
fn collect_images(doc: &Document) -> BTreeMap<Id, ImagePart> {
    let mut out = BTreeMap::new();
    for (id, a) in &doc.assets {
        if !a.mime.starts_with("image/") {
            continue;
        }
        let raw: Vec<u8> = if a.bytes.is_empty() {
            a.link.as_ref().and_then(|l| std::fs::read(l).ok()).unwrap_or_default()
        } else {
            a.bytes.to_vec()
        };
        if raw.is_empty() {
            continue;
        }
        let native = match a.mime.as_str() {
            "image/png" => Some("png"),
            "image/jpeg" | "image/jpg" => Some("jpg"),
            _ => None,
        };
        let (bytes, ext, dims) = match native {
            Some(ext) => {
                let dims = if a.px_w > 0 && a.px_h > 0 { Some((a.px_w, a.px_h)) } else { dimensions(&raw) };
                (raw, ext, dims)
            }
            None => {
                let Ok(img) = image::load_from_memory(&raw) else { continue };
                let mut buf = Vec::new();
                if img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png).is_err() {
                    continue;
                }
                (buf, "png", Some((img.width(), img.height())))
            }
        };
        let Some((px_w, px_h)) = dims.filter(|d| d.0 > 0 && d.1 > 0) else { continue };
        out.insert(*id, ImagePart { name: format!("{}.{ext}", id.0), bytes, px_w, px_h });
    }
    out
}

fn dimensions(raw: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(raw)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// Fonts used by any laid-out text, as obfuscated `.odttf` parts named by a GUID derived from the font data.
fn collect_fonts(doc: &Document, layout: &DocLayout, fonts: &FontStore) -> BTreeMap<u32, FontPart> {
    let mut ids = std::collections::BTreeSet::new();
    for fl in layout.frames.values() {
        for line in &fl.lines {
            for run in &line.runs {
                ids.insert(run.face.0);
            }
        }
    }
    let _ = doc;
    let mut out = BTreeMap::new();
    for fid in ids {
        let face = fonts.face(newpub_layout::FaceId(fid));
        let data = face.bytes();
        // Collections (.ttc) cannot be embedded as a single font part.
        if data.len() < 4 || &data[..4] == b"ttcf" {
            continue;
        }
        let guid = guid_for(data, fid);
        out.insert(fid, FontPart { name: format!("{guid}.odttf"), bytes: obfuscate(data, &guid) });
    }
    out
}

/// A deterministic GUID string (8-4-4-4-12, upper case hex) from the font bytes.
fn guid_for(data: &[u8], salt: u32) -> String {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325 ^ u64::from(salt);
    let mut b: u64 = 0x9e37_79b9_7f4a_7c15;
    for (i, &x) in data.iter().enumerate() {
        a = (a ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
        if i % 7 == 0 {
            b = (b ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(11);
        }
    }
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    let h: String = bytes.iter().map(|x| format!("{x:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// XPS font obfuscation: XOR the first 32 bytes with the GUID's 16 bytes, taken in reverse order of the hex string.
fn obfuscate(data: &[u8], guid: &str) -> Vec<u8> {
    let hex: Vec<u8> = guid.bytes().filter(|b| *b != b'-').collect();
    let nib = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
    let mut key = [0u8; 16];
    for (i, k) in key.iter_mut().enumerate() {
        let p = 30 - 2 * i;
        *k = (nib(hex[p]) << 4) | nib(hex[p + 1]);
    }
    let mut out = data.to_vec();
    for i in 0..out.len().min(32) {
        out[i] ^= key[i % 16];
    }
    out
}

/// Rotation / flip about the object's centre as an XPS matrix, or `None` for the identity.
fn transform(ob: &Object) -> Option<String> {
    if ob.rotation == 0.0 && !ob.flip_h && !ob.flip_v {
        return None;
    }
    let (s, c) = ob.rotation.to_radians().sin_cos();
    let (fx, fy) = (if ob.flip_h { -1.0 } else { 1.0 }, if ob.flip_v { -1.0 } else { 1.0 });
    let (m11, m12, m21, m22) = (c * fx, s * fx, -s * fy, c * fy);
    let (cx, cy) = (ob.rect.x + ob.rect.w / 2.0, ob.rect.y + ob.rect.h / 2.0);
    let dx = cx - (m11 * cx + m21 * cy);
    let dy = cy - (m12 * cx + m22 * cy);
    let f = |v: f64| {
        let s = format!("{v:.5}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    Some(format!("{},{},{},{},{},{}", f(m11), f(m12), f(m21), f(m22), u(dx), u(dy)))
}

fn rect_data(x: f64, y: f64, w: f64, h: f64) -> String {
    format!("M {},{} L {},{} L {},{} L {},{} Z", u(x), u(y), u(x + w), u(y), u(x + w), u(y + h), u(x), u(y + h))
}

fn ellipse_data(x: f64, y: f64, w: f64, h: f64) -> String {
    let (rx, ry) = (u(w / 2.0), u(h / 2.0));
    format!(
        "M {},{} A {rx},{ry} 0 1 1 {},{} A {rx},{ry} 0 1 1 {},{} Z",
        u(x),
        u(y + h / 2.0),
        u(x + w),
        u(y + h / 2.0),
        u(x),
        u(y + h / 2.0)
    )
}

fn round_rect_data(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    if r <= 0.0 {
        return rect_data(x, y, w, h);
    }
    let a = format!("{},{}", u(r), u(r));
    format!(
        "M {},{} L {},{} A {a} 0 0 1 {},{} L {},{} A {a} 0 0 1 {},{} L {},{} A {a} 0 0 1 {},{} L {},{} A {a} 0 0 1 {},{} Z",
        u(x + r),
        u(y),
        u(x + w - r),
        u(y),
        u(x + w),
        u(y + r),
        u(x + w),
        u(y + h - r),
        u(x + w - r),
        u(y + h),
        u(x + r),
        u(y + h),
        u(x),
        u(y + h - r),
        u(x),
        u(y + r),
        u(x + r),
        u(y),
    )
}

fn poly_data(pts: &[(f64, f64)], closed: bool) -> String {
    let mut d = String::new();
    for (k, (x, y)) in pts.iter().enumerate() {
        let _ = write!(d, "{} {},{} ", if k == 0 { "M" } else { "L" }, u(*x), u(*y));
    }
    if closed {
        d.push('Z');
    }
    d.trim_end().to_string()
}

/// Path geometry strings for a shape drawn in the box (x, y, w, h); the bool says whether each is closed (fillable).
fn shape_geometry(kind: &ShapeKind, x: f64, y: f64, w: f64, h: f64) -> Vec<(String, bool)> {
    let at = |v: &[(f64, f64)]| -> Vec<(f64, f64)> { v.iter().map(|(a, b)| (x + a, y + b)).collect() };
    match kind {
        ShapeKind::Rect => vec![(rect_data(x, y, w, h), true)],
        ShapeKind::RoundRect { radius } => vec![(round_rect_data(x, y, w, h, radius.0), true)],
        ShapeKind::Ellipse => vec![(ellipse_data(x, y, w, h), true)],
        ShapeKind::Line => vec![(poly_data(&at(&[(0.0, 0.0), (w, h)]), false), false)],
        ShapeKind::Triangle => vec![(poly_data(&at(&[(w / 2.0, 0.0), (w, h), (0.0, h)]), true), true)],
        ShapeKind::Star { points, inner } => {
            let n = (*points).clamp(3, 64) as usize;
            let v: Vec<(f64, f64)> = (0..n * 2)
                .map(|k| {
                    let a = std::f64::consts::PI * (k as f64) / (n as f64) - std::f64::consts::FRAC_PI_2;
                    let r = if k % 2 == 0 { 1.0 } else { inner.clamp(0.05, 1.0) };
                    (w / 2.0 + a.cos() * r * w / 2.0, h / 2.0 + a.sin() * r * h / 2.0)
                })
                .collect();
            vec![(poly_data(&at(&v), true), true)]
        }
        ShapeKind::Polygon { sides } => {
            let n = (*sides).clamp(3, 64) as usize;
            let v: Vec<(f64, f64)> = (0..n)
                .map(|k| {
                    let a = 2.0 * std::f64::consts::PI * (k as f64) / (n as f64) - std::f64::consts::FRAC_PI_2;
                    (w / 2.0 + a.cos() * w / 2.0, h / 2.0 + a.sin() * h / 2.0)
                })
                .collect();
            vec![(poly_data(&at(&v), true), true)]
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
            vec![(poly_data(&at(&v), true), true)]
        }
        ShapeKind::Callout { tail } => {
            let (tx, ty) = (tail[0] * w, tail[1] * h);
            vec![(rect_data(x, y, w, h), true), (poly_data(&at(&[(w * 0.2, h), (w * 0.4, h), (tx, ty)]), true), true)]
        }
        ShapeKind::Path { points, closed } => {
            let v: Vec<(f64, f64)> = points.iter().map(|p| (p[0] * w, p[1] * h)).collect();
            if v.is_empty() { vec![] } else { vec![(poly_data(&at(&v), *closed), *closed)] }
        }
        ShapeKind::Bezier { nodes, closed } => {
            let mut d = String::new();
            for el in newpub_render::display::bezier_path(nodes, *closed, w, h) {
                match el {
                    newpub_render::PathEl::Move(a, b) => {
                        let _ = write!(d, "M {},{} ", u(x + a), u(y + b));
                    }
                    newpub_render::PathEl::Line(a, b) => {
                        let _ = write!(d, "L {},{} ", u(x + a), u(y + b));
                    }
                    newpub_render::PathEl::Cubic(a, b, c, e, f, g) => {
                        let _ = write!(
                            d,
                            "C {},{} {},{} {},{} ",
                            u(x + a),
                            u(y + b),
                            u(x + c),
                            u(y + e),
                            u(x + f),
                            u(y + g)
                        );
                    }
                    newpub_render::PathEl::Close => d.push('Z'),
                }
            }
            if d.is_empty() { vec![] } else { vec![(d.trim_end().to_string(), *closed)] }
        }
    }
}

/// Stroke attributes for a `Path`.
fn stroke_attrs(s: &Stroke) -> String {
    let t = s.width.0.max(0.25);
    let dash = match s.dash {
        Dash::Solid => "",
        Dash::Dash => " StrokeDashArray=\"3 1.5\"",
        Dash::Dot => " StrokeDashArray=\"0.5 1.5\"",
        Dash::DashDot => " StrokeDashArray=\"3 1.5 0.5 1.5\"",
        Dash::LongDash => " StrokeDashArray=\"6 2\"",
    };
    format!(" Stroke=\"{}\" StrokeThickness=\"{}\"{dash}", argb(&s.color), u(t))
}

fn gradient_brush(g: &newpub_core::Gradient) -> Option<String> {
    if g.stops.is_empty() {
        return None;
    }
    let stops: String = g
        .stops
        .iter()
        .map(|s| format!("<GradientStop Color=\"{}\" Offset=\"{:.3}\"/>", argb(&s.color), s.at.clamp(0.0, 1.0)))
        .collect();
    Some(match g.kind {
        newpub_core::GradientKind::Linear => {
            let a = g.angle.to_radians();
            let (dx, dy) = (a.cos() / 2.0, a.sin() / 2.0);
            format!(
                "<LinearGradientBrush MappingMode=\"RelativeToBoundingBox\" StartPoint=\"{:.3},{:.3}\" \
                 EndPoint=\"{:.3},{:.3}\"><LinearGradientBrush.GradientStops>{stops}\
                 </LinearGradientBrush.GradientStops></LinearGradientBrush>",
                0.5 - dx,
                0.5 - dy,
                0.5 + dx,
                0.5 + dy
            )
        }
        newpub_core::GradientKind::Radial => format!(
            "<RadialGradientBrush MappingMode=\"RelativeToBoundingBox\" Center=\"0.5,0.5\" GradientOrigin=\"0.5,0.5\" \
             RadiusX=\"0.5\" RadiusY=\"0.5\"><RadialGradientBrush.GradientStops>{stops}\
             </RadialGradientBrush.GradientStops></RadialGradientBrush>"
        ),
    })
}

impl Ctx<'_> {
    fn page(&self, o: &mut String, i: usize, used: &mut Used) {
        let doc = self.doc;
        let page = &doc.pages[i];
        let master = doc.master_for_page(i);
        let (w, h) = (doc.setup.width.0, doc.setup.height.0);
        let bg = page.background.clone().or_else(|| master.and_then(|m| m.background.clone()));
        if let Some(bg) = bg {
            let _ = writeln!(o, "<Path Data=\"{}\" Fill=\"{}\"/>", rect_data(0.0, 0.0, w, h), argb(&bg));
        }
        if let Some(m) = master {
            for id in &m.objects {
                self.object(o, *id, 0, used);
            }
        }
        for id in doc.draw_order(i) {
            self.object(o, id, 0, used);
        }
    }

    fn object(&self, o: &mut String, id: Id, depth: usize, used: &mut Used) {
        let Ok(ob) = self.doc.object(id) else { return };
        if let ObjectKind::Group { children } = &ob.kind {
            if depth < 16 {
                for c in children {
                    self.object(o, *c, depth + 1, used);
                }
            }
            return;
        }
        let tf = transform(ob);
        if let Some(m) = &tf {
            let _ = writeln!(o, "<Canvas RenderTransform=\"{m}\">");
        }
        let r = ob.rect;
        match &ob.kind {
            ObjectKind::Group { .. } | ObjectKind::WordArt(_) => {}
            ObjectKind::Text(t) => {
                if t.fill.is_some() || t.stroke.is_some() {
                    let mut a = format!("<Path Data=\"{}\"", rect_data(r.x, r.y, r.w, r.h));
                    if let Some(f) = &t.fill {
                        let _ = write!(a, " Fill=\"{}\"", argb(f));
                    }
                    if let Some(s) = &t.stroke {
                        a.push_str(&stroke_attrs(s));
                    }
                    let _ = writeln!(o, "{a}/>");
                }
                if let Some(fl) = self.layout.frames.get(&id) {
                    self.frame(o, fl, r.x, r.y, used);
                }
            }
            ObjectKind::Image(im) => self.image(o, ob, im, used),
            ObjectKind::Table(tb) => {
                for row in 0..tb.rows() {
                    for col in 0..tb.cols() {
                        let (Some(cell), Some(cr)) = (tb.cell(row, col), tb.cell_rect(row, col)) else { continue };
                        if cell.covered {
                            continue;
                        }
                        if let Some(f) = &cell.fill {
                            let _ = writeln!(
                                o,
                                "<Path Data=\"{}\" Fill=\"{}\"/>",
                                rect_data(r.x + cr.x, r.y + cr.y, cr.w, cr.h),
                                argb(f)
                            );
                        }
                    }
                }
                for cell in &tb.cells {
                    if let Some(fl) = self.layout.frames.get(&cell.story) {
                        self.frame(o, fl, r.x, r.y, used);
                    }
                }
            }
            ObjectKind::Shape(sh) => {
                self.shape(o, ob, sh);
                if sh.story.is_some()
                    && let Some(fl) = self.layout.frames.get(&id)
                {
                    self.frame(o, fl, r.x, r.y, used);
                }
            }
        }
        if tf.is_some() {
            o.push_str("</Canvas>\n");
        }
    }

    fn shape(&self, o: &mut String, ob: &Object, sh: &Shape) {
        let r = ob.rect;
        let brush = sh.gradient.as_ref().and_then(gradient_brush);
        for (data, closed) in shape_geometry(&sh.kind, r.x, r.y, r.w.max(0.0), r.h.max(0.0)) {
            let mut a = format!("<Path Data=\"{data}\"");
            let flat_fill = if closed { sh.fill.as_ref() } else { None };
            if brush.is_none()
                && let Some(f) = flat_fill
            {
                let _ = write!(a, " Fill=\"{}\"", argb(f));
            }
            if let Some(s) = &sh.stroke {
                a.push_str(&stroke_attrs(s));
            }
            match (&brush, closed) {
                (Some(b), true) => {
                    let _ = writeln!(o, "{a}><Path.Fill>{b}</Path.Fill></Path>");
                }
                _ => {
                    let _ = writeln!(o, "{a}/>");
                }
            }
        }
    }

    fn image(&self, o: &mut String, ob: &Object, im: &ImageFrame, used: &mut Used) {
        let Some(id) = im.asset else { return };
        let Some(part) = self.images.get(&id) else { return };
        if !used.images.contains(&id) {
            used.images.push(id);
        }
        let r = ob.rect;
        let c = im.crop;
        let (pw, ph) = (f64::from(part.px_w), f64::from(part.px_h));
        // Source region in image pixels after cropping.
        let mut sx = c.left.clamp(0.0, 0.99) * pw;
        let mut sy = c.top.clamp(0.0, 0.99) * ph;
        let mut sw = ((1.0 - c.left - c.right).clamp(0.01, 1.0) * pw).max(1.0);
        let mut sh = ((1.0 - c.top - c.bottom).clamp(0.01, 1.0) * ph).max(1.0);
        let (mut vx, mut vy, mut vw, mut vh) = (r.x, r.y, r.w, r.h);
        if r.w > 0.0 && r.h > 0.0 {
            let src_ar = sw / sh;
            let dst_ar = r.w / r.h;
            match im.fit {
                Fit::Stretch => {}
                Fit::Fit => {
                    if src_ar > dst_ar {
                        vh = r.w / src_ar;
                        vy = r.y + (r.h - vh) / 2.0;
                    } else {
                        vw = r.h * src_ar;
                        vx = r.x + (r.w - vw) / 2.0;
                    }
                }
                Fit::Fill => {
                    if src_ar > dst_ar {
                        let nw = sh * dst_ar;
                        sx += (sw - nw) / 2.0;
                        sw = nw;
                    } else {
                        let nh = sw / dst_ar;
                        sy += (sh - nh) / 2.0;
                        sh = nh;
                    }
                }
            }
        }
        let data = match im.mask {
            ImageMask::Rect => rect_data(r.x, r.y, r.w, r.h),
            ImageMask::Ellipse => ellipse_data(r.x, r.y, r.w, r.h),
            ImageMask::RoundRect => round_rect_data(r.x, r.y, r.w, r.h, r.w.min(r.h) * 0.12),
        };
        let mut a = format!("<Path Data=\"{data}\"");
        if let Some(s) = &im.stroke {
            a.push_str(&stroke_attrs(s));
        }
        let alt = if ob.decorative { None } else { ob.alt_text.as_deref().filter(|t| !t.is_empty()) };
        if let Some(t) = alt {
            let _ = write!(a, " AutomationProperties.HelpText=\"{}\"", esc(t));
        }
        let _ = writeln!(
            o,
            "{a}><Path.Fill><ImageBrush ImageSource=\"/Resources/Images/{}\" Viewbox=\"{},{},{},{}\" \
             ViewboxUnits=\"Absolute\" Viewport=\"{},{},{},{}\" ViewportUnits=\"Absolute\" TileMode=\"None\"/>\
             </Path.Fill></Path>",
            part.name,
            sx.round(),
            sy.round(),
            sw.round().max(1.0),
            sh.round().max(1.0),
            u(vx),
            u(vy),
            u(vw),
            u(vh),
        );
    }

    /// Text of one frame as `Glyphs` elements (one per laid-out run) plus underline and strike-through rules.
    fn frame(&self, o: &mut String, fl: &FrameLayout, ox: f64, oy: f64, used: &mut Used) {
        for line in &fl.lines {
            for run in &line.runs {
                let mut text = String::new();
                let mut last: Option<std::ops::Range<usize>> = None;
                for g in &run.glyphs {
                    if last.as_ref() != Some(&g.text_range) {
                        text.push_str(run.text.get(g.text_range.clone()).unwrap_or(""));
                        last = Some(g.text_range.clone());
                    }
                }
                let text = esc(&text);
                let (Some(first), false) = (run.glyphs.first(), text.is_empty()) else { continue };
                let Some(part) = self.font_parts.get(&run.face.0) else { continue };
                if !used.fonts.contains(&run.face.0) {
                    used.fonts.push(run.face.0);
                }
                let face = self.fonts.face(run.face);
                let sim = match (run.synthetic_bold && !face.bold, run.synthetic_italic && !face.italic) {
                    (true, true) => " StyleSimulations=\"BoldItalicSimulation\"",
                    (true, false) => " StyleSimulations=\"BoldSimulation\"",
                    (false, true) => " StyleSimulations=\"ItalicSimulation\"",
                    (false, false) => "",
                };
                let _ = writeln!(
                    o,
                    "<Glyphs OriginX=\"{}\" OriginY=\"{}\" FontRenderingEmSize=\"{}\" FontUri=\"/Resources/Fonts/{}\"{sim} \
                     Fill=\"{}\" UnicodeString=\"{text}\"/>",
                    u(ox + first.x),
                    u(oy + first.y),
                    u(run.size),
                    part.name,
                    argb(&run.color),
                );
            }
        }
        for d in &fl.decorations {
            let t = d.thickness.max(0.3);
            let y = d.y - t / 2.0;
            let _ = writeln!(
                o,
                "<Path Data=\"{}\" Fill=\"{}\"/>",
                rect_data(ox + d.x0, oy + y, (d.x1 - d.x0).max(0.0), t),
                argb(&d.color)
            );
        }
    }
}
