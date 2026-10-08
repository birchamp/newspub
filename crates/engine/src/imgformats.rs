//! Picture formats (IM-09): decoding files into assets.
//! Owner: Batch 4 task IMAGES. PNG and JPEG are stored as they are; other formats are placeholders.

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
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| EngineError::Image(e.to_string()))?;
    let mime = match reader.format() {
        Some(image::ImageFormat::Png) => "image/png",
        Some(image::ImageFormat::Jpeg) => "image/jpeg",
        other => return Err(EngineError::Image(format!("unsupported picture format {other:?}"))),
    };
    let (px_w, px_h) = reader.into_dimensions().map_err(|e| EngineError::Image(e.to_string()))?;
    Ok(Picture { mime, bytes: bytes.to_vec(), px_w, px_h })
}
