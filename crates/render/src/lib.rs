//! newpub-render: display lists and raster output.

pub mod display;
pub mod raster;

pub use display::{GradientPaint, Item, PageDisplay, PathEl, StrokeStyle, page_display, page_display_for};
pub use raster::{Rasterizer, adjust_rgba, decode_image, render_page};
pub use tiny_skia::Pixmap;
