//! Picture formats (IM-09): decoding files into assets.
//! PNG and JPEG are stored as they are; GIF, BMP and TIFF become PNG; SVG, EMF and WMF are stored as SVG.

use crate::EngineError;

/// A picture ready to store as an asset.
pub(crate) struct Picture {
    /// "image/png", "image/jpeg" or "image/svg+xml".
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    /// Pixel size (for SVG: the viewBox / width × height in CSS px, i.e. 96 per inch).
    pub px_w: u32,
    pub px_h: u32,
}

/// Recognises the file from its bytes. PNG/JPEG are kept as is; GIF, BMP and TIFF are converted to PNG;
/// SVG is kept as SVG. Anything else → error.
pub(crate) fn decode_picture(bytes: &[u8]) -> Result<Picture, EngineError> {
    if looks_like_svg(bytes) {
        return decode_svg(bytes);
    }
    // Windows metafiles become SVG drawings (IM-11).
    if newpub_io_metafile::is_emf(bytes) || newpub_io_metafile::is_wmf(bytes) {
        let svg = newpub_io_metafile::to_svg(bytes).map_err(|e| EngineError::Image(e.to_string()))?;
        return decode_svg(svg.as_bytes());
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| EngineError::Image(e.to_string()))?;
    let mime = match reader.format() {
        Some(image::ImageFormat::Png) => "image/png",
        Some(image::ImageFormat::Jpeg) => "image/jpeg",
        Some(f @ (image::ImageFormat::Gif | image::ImageFormat::Bmp | image::ImageFormat::Tiff)) => {
            let img = reader.decode().map_err(|e| EngineError::Image(e.to_string()))?;
            let (px_w, px_h) = (img.width(), img.height());
            let bytes = png_with_source(&img, &format!("{f:?}").to_uppercase())?;
            return Ok(Picture { mime: "image/png", bytes, px_w, px_h });
        }
        other => return Err(EngineError::Image(format!("unsupported picture format {other:?}"))),
    };
    let (px_w, px_h) = reader.into_dimensions().map_err(|e| EngineError::Image(e.to_string()))?;
    Ok(Picture { mime, bytes: bytes.to_vec(), px_w, px_h })
}

/// True when the first KB, after whitespace, an XML prolog, comments and a doctype, starts an `<svg` element.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).into_owned();
    let mut rest = head.trim_start_matches('\u{feff}').trim_start();
    loop {
        if rest.starts_with("<?") {
            match rest.find("?>") {
                Some(i) => rest = rest[i + 2..].trim_start(),
                None => return false,
            }
        } else if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(i) => rest = rest[i + 3..].trim_start(),
                None => return false,
            }
        } else if starts_ci(rest, "<!doctype") {
            match rest.find('>') {
                Some(i) => rest = rest[i + 1..].trim_start(),
                None => return false,
            }
        } else {
            break;
        }
    }
    starts_ci(rest, "<svg")
}

fn decode_svg(bytes: &[u8]) -> Result<Picture, EngineError> {
    let tree =
        usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(|e| EngineError::Image(e.to_string()))?;
    let size = tree.size();
    let (px_w, px_h) = ((size.width().round() as u32).max(1), (size.height().round() as u32).max(1));
    Ok(Picture { mime: "image/svg+xml", bytes: bytes.to_vec(), px_w, px_h })
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.as_bytes().get(..prefix.len()).is_some_and(|b| b.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// Re-encodes a decoded picture as PNG, noting the original format in a text chunk.
fn png_with_source(img: &image::DynamicImage, source: &str) -> Result<Vec<u8>, EngineError> {
    let err = |e: &dyn std::fmt::Display| EngineError::Image(e.to_string());
    let rgba = img.to_rgba8();
    let opaque = rgba.pixels().all(|p| p.0[3] == 255);
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, rgba.width(), rgba.height());
    enc.set_depth(png::BitDepth::Eight);
    enc.set_color(if opaque { png::ColorType::Rgb } else { png::ColorType::Rgba });
    enc.add_text_chunk("Source".to_string(), source.to_string()).map_err(|e| err(&e))?;
    let mut w = enc.write_header().map_err(|e| err(&e))?;
    if opaque {
        let rgb = img.to_rgb8();
        w.write_image_data(rgb.as_raw()).map_err(|e| err(&e))?;
    } else {
        w.write_image_data(rgba.as_raw()).map_err(|e| err(&e))?;
    }
    w.finish().map_err(|e| err(&e))?;
    Ok(out)
}
