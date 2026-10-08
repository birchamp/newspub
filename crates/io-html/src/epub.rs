//! EPUB 3 fixed-layout export (EX-06).
//!
//! One XHTML file per page (`OEBPS/page-<n>.xhtml`) built from the same absolutely positioned page box the HTML export
//! uses, plus images, the fonts the text uses, a navigation document and the package document.

use super::{Ctx, Error, asset_bytes, asset_names, css_family, escape, px};
use newpub_core::Document;
use newpub_layout::{DocLayout, FaceId, FontStore};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Seek, Write};
use std::path::Path;

const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// Writes `doc` as a fixed-layout EPUB 3 file at `path`.
pub fn export_epub(doc: &Document, layout: &DocLayout, fonts: &FontStore, path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    write_epub(doc, layout, fonts, file)
}

/// Writes the EPUB to any seekable writer.
pub fn write_epub<W: Write + Seek>(doc: &Document, layout: &DocLayout, fonts: &FontStore, out: W) -> Result<(), Error> {
    let names = asset_names(doc);
    let ctx = Ctx::new(doc, layout, fonts, &names, "images/");
    let n = doc.pages.len();
    let (w, h) = (doc.setup.width.0, doc.setup.height.0);
    let title =
        if doc.meta.title.trim().is_empty() { "Untitled".to_string() } else { doc.meta.title.trim().to_string() };
    let lang = if doc.meta.lang.trim().is_empty() { "en".to_string() } else { doc.meta.lang.trim().to_string() };

    // Page bodies first, so we know which faces and images are used.
    let mut divs = Vec::with_capacity(n);
    let mut page_faces: Vec<Vec<u32>> = Vec::with_capacity(n);
    let mut all_faces: Vec<u32> = Vec::new();
    for i in 0..n {
        ctx.used_faces.borrow_mut().clear();
        divs.push(ctx.page_div(i));
        let used: Vec<u32> = ctx.used_faces.borrow().iter().copied().collect();
        for f in &used {
            if !all_faces.contains(f) {
                all_faces.push(*f);
            }
        }
        page_faces.push(used);
    }

    // Fonts: only plain single-font files can be embedded (not .ttc collections).
    let mut font_files: BTreeMap<u32, (String, &'static str, Vec<u8>)> = BTreeMap::new();
    for fid in &all_faces {
        let face = fonts.face(FaceId(*fid));
        let bytes = face.bytes();
        if bytes.len() < 4 || &bytes[..4] == b"ttcf" {
            continue;
        }
        let (ext, mime) = if &bytes[..4] == b"OTTO" { ("otf", "font/otf") } else { ("ttf", "font/ttf") };
        font_files.insert(*fid, (format!("font-{fid}.{ext}"), mime, bytes.to_vec()));
    }

    // Images actually referenced by a page.
    let mut images: Vec<(String, &str, Vec<u8>)> = Vec::new();
    for (id, name) in &names {
        let quoted = format!("src=\"images/{}\"", escape(name));
        if divs.iter().any(|d| d.contains(&quoted)) {
            let a = &doc.assets[id];
            images.push((name.clone(), a.mime.as_str(), asset_bytes(a)));
        }
    }

    let page_names: Vec<String> = (0..n).map(|i| format!("page-{}.xhtml", i + 1)).collect();
    let mut xhtml_pages = Vec::with_capacity(n);
    for i in 0..n {
        let mut o = String::new();
        let _ = write!(
            o,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"{XHTML_NS}\" \
             xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{l}\" xml:lang=\"{l}\">\n<head>\n\
             <meta charset=\"utf-8\"/>\n<meta name=\"viewport\" content=\"width={vw}, height={vh}\"/>\n\
             <title>{t} - page {p}</title>\n<style type=\"text/css\">\n",
            l = escape(&lang),
            vw = px(w),
            vh = px(h),
            t = escape(&title),
            p = i + 1,
        );
        for fid in &page_faces[i] {
            if let Some((file, _, _)) = font_files.get(fid) {
                let face = fonts.face(FaceId(*fid));
                let _ = writeln!(
                    o,
                    "@font-face{{font-family:'{}';font-weight:{};font-style:{};src:url(fonts/{file})}}",
                    css_family(&face.family),
                    if face.bold { "bold" } else { "normal" },
                    if face.italic { "italic" } else { "normal" },
                );
            }
        }
        let _ = write!(
            o,
            "html,body{{margin:0;padding:0;width:{vw}px;height:{vh}px}}\n\
             .page{{position:relative;overflow:hidden}}\n\
             .obj{{position:absolute;box-sizing:border-box}}\n\
             .ln{{position:absolute;white-space:pre;margin:0}}\n\
             .obj img{{display:block}}\n</style>\n</head>\n<body>\n{div}</body>\n</html>\n",
            vw = px(w),
            vh = px(h),
            div = divs[i],
        );
        xhtml_pages.push(o);
    }

    let uuid = content_uuid(&xhtml_pages, &title, &lang);

    // Navigation document.
    let mut nav = String::new();
    let _ = write!(
        nav,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"{XHTML_NS}\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{l}\" xml:lang=\"{l}\">\n<head>\n<meta charset=\"utf-8\"/>\n\
         <title>{t}</title>\n</head>\n<body>\n<nav epub:type=\"toc\" id=\"toc\">\n<h1>{t}</h1>\n<ol>\n",
        l = escape(&lang),
        t = escape(&title),
    );
    for (i, pn) in page_names.iter().enumerate() {
        let _ = writeln!(nav, "<li><a href=\"{pn}\">Page {}</a></li>", i + 1);
    }
    nav.push_str("</ol>\n</nav>\n</body>\n</html>\n");

    // Package document.
    let mut opf = String::new();
    let _ = write!(
        opf,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"book-id\" xml:lang=\"{}\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n\
         <dc:identifier id=\"book-id\">urn:uuid:{uuid}</dc:identifier>\n<dc:title>{}</dc:title>\n\
         <dc:language>{}</dc:language>\n",
        escape(&lang),
        escape(&title),
        escape(&lang),
    );
    if !doc.meta.author.trim().is_empty() {
        let _ = writeln!(opf, "<dc:creator>{}</dc:creator>", escape(doc.meta.author.trim()));
    }
    let _ = write!(
        opf,
        "<meta property=\"dcterms:modified\">{}</meta>\n<meta property=\"rendition:layout\">pre-paginated</meta>\n\
         <meta property=\"rendition:orientation\">auto</meta>\n<meta property=\"rendition:spread\">none</meta>\n\
         </metadata>\n<manifest>\n\
         <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n",
        utc_now()
    );
    for (i, pn) in page_names.iter().enumerate() {
        let props = if divs[i].contains("<svg") { " properties=\"svg\"" } else { "" };
        let _ =
            writeln!(opf, "<item id=\"page-{}\" href=\"{pn}\" media-type=\"application/xhtml+xml\"{props}/>", i + 1);
    }
    for (k, (name, mime, _)) in images.iter().enumerate() {
        let _ =
            writeln!(opf, "<item id=\"img-{k}\" href=\"images/{}\" media-type=\"{}\"/>", escape(name), escape(mime));
    }
    for (fid, (file, mime, _)) in &font_files {
        let _ = writeln!(opf, "<item id=\"font-{fid}\" href=\"fonts/{file}\" media-type=\"{mime}\"/>");
    }
    opf.push_str("</manifest>\n<spine>\n");
    for i in 0..n {
        let _ = writeln!(opf, "<itemref idref=\"page-{}\"/>", i + 1);
    }
    opf.push_str("</spine>\n</package>\n");

    let container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles>\n\
        <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
        </rootfiles>\n</container>\n";

    let mut z = zip::ZipWriter::new(out);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", stored)?;
    z.write_all(b"application/epub+zip")?;
    z.start_file("META-INF/container.xml", deflated)?;
    z.write_all(container.as_bytes())?;
    z.start_file("OEBPS/content.opf", deflated)?;
    z.write_all(opf.as_bytes())?;
    z.start_file("OEBPS/nav.xhtml", deflated)?;
    z.write_all(nav.as_bytes())?;
    for (pn, page) in page_names.iter().zip(&xhtml_pages) {
        z.start_file(format!("OEBPS/{pn}"), deflated)?;
        z.write_all(page.as_bytes())?;
    }
    for (name, _, bytes) in &images {
        z.start_file(format!("OEBPS/images/{name}"), stored)?;
        z.write_all(bytes)?;
    }
    for (file, _, bytes) in font_files.values() {
        z.start_file(format!("OEBPS/fonts/{file}"), deflated)?;
        z.write_all(bytes)?;
    }
    z.finish()?;
    Ok(())
}

/// A version-4-shaped UUID derived from the content (FNV-1a, two lanes), so equal documents get equal identifiers.
fn content_uuid(pages: &[String], title: &str, lang: &str) -> String {
    let mut a: u64 = 0xcbf2_9ce4_8422_2325;
    let mut b: u64 = 0x8422_2325_cbf2_9ce4;
    let mut feed = |bytes: &[u8]| {
        for &x in bytes {
            a = (a ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
            b = (b ^ u64::from(x).rotate_left(3)).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(5);
        }
    };
    feed(title.as_bytes());
    feed(lang.as_bytes());
    for p in pages {
        feed(p.as_bytes());
        feed(&[0]);
    }
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Current UTC time as `CCYY-MM-DDThh:mm:ssZ`.
fn utc_now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
