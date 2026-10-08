//! newpub-render: display lists and raster output.

pub mod display;
pub mod raster;

pub use display::{Item, PageDisplay, PathEl, StrokeStyle, page_display};
pub use raster::{Rasterizer, decode_image, render_page};
pub use tiny_skia::Pixmap;
