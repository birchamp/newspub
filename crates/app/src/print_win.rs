//! Native Windows printing through GDI (PR-07): printer enumeration and a print job that renders
//! each page with the session's raster renderer and stretches it onto the printer device context.
//!
//! Test hook: when the environment variable `NEWPUB_PRINT_OUTPUT` is set, its value is passed as
//! `DOCINFOW.lpszOutput`, which makes "Microsoft Print to PDF" write to that file without showing
//! a save dialog. CI uses this to check a real print job end to end.

use newpub_engine::render::Pixmap;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DIB_RGB_COLORS, DM_COPIES, DM_IN_BUFFER, DM_OUT_BUFFER,
    DeleteDC, GetDeviceCaps, HDC, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY,
    PHYSICALWIDTH, SRCCOPY, STRETCH_HALFTONE, SetStretchBltMode, StretchDIBits,
};
use windows_sys::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS,
    PRINTER_ENUM_LOCAL, PRINTER_HANDLE, PRINTER_INFO_4W,
};
use windows_sys::Win32::Storage::Xps::{DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};

/// Highest resolution pages are rendered at, to bound memory (a letter page at 600 dpi is ~135 MB).
const MAX_DPI: f64 = 600.0;
/// `GetDeviceCaps` index COPIES: the number of copies the device can make itself.
const COPIES_CAP: i32 = 18;

/// Renders one 0-based page index at the given dpi.
pub type RenderFn<'a> = dyn FnMut(usize, f64) -> Result<Pixmap, String> + 'a;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: `p` points at a NUL-terminated UTF-16 string owned by the caller's buffer.
    unsafe {
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }
}

fn last_error(what: &str) -> String {
    format!("{what} failed ({})", std::io::Error::last_os_error())
}

/// The system default printer's name, if one is set.
pub fn default_printer() -> Option<String> {
    let mut len = 0u32;
    // SAFETY: a null buffer with length 0 only asks for the required size.
    unsafe { GetDefaultPrinterW(null_mut(), &mut len) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` holds `len` UTF-16 units as announced.
    if unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) } == 0 {
        return None;
    }
    Some(from_wide(buf.as_ptr()))
}

/// Local and connected printers, default printer first. Empty when none could be listed.
pub fn list_printers() -> Vec<String> {
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut needed, mut count) = (0u32, 0u32);
    // SAFETY: the first call with no buffer only reports the size needed.
    unsafe { EnumPrintersW(flags, null(), 4, null_mut(), 0, &mut needed, &mut count) };
    if needed == 0 {
        return vec![];
    }
    // u64 elements keep the buffer aligned for the PRINTER_INFO_4W structs.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    let size = (buf.len() * 8) as u32;
    // SAFETY: `buf` is at least `size` bytes.
    let ok = unsafe { EnumPrintersW(flags, null(), 4, buf.as_mut_ptr().cast(), size, &mut needed, &mut count) };
    if ok == 0 {
        return vec![];
    }
    // SAFETY: on success the buffer starts with `count` PRINTER_INFO_4W records.
    let infos = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<PRINTER_INFO_4W>(), count as usize) };
    let mut names: Vec<String> = infos.iter().map(|i| from_wide(i.pPrinterName)).filter(|n| !n.is_empty()).collect();
    if let Some(def) = default_printer()
        && let Some(i) = names.iter().position(|n| *n == def)
    {
        let d = names.remove(i);
        names.insert(0, d);
    }
    names
}

/// An open printer handle, closed on drop.
struct Printer(PRINTER_HANDLE);

impl Drop for Printer {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful OpenPrinterW.
        unsafe { ClosePrinter(self.0) };
    }
}

/// A printer device context, deleted on drop.
struct Dc(HDC);

impl Drop for Dc {
    fn drop(&mut self) {
        // SAFETY: the DC came from a successful CreateDCW.
        unsafe { DeleteDC(self.0) };
    }
}

/// The printer's default DEVMODE with `dmCopies` set to `copies`. Drivers that ignore this are
/// detected by the caller through the COPIES device capability.
fn devmode(name: &[u16], copies: i16) -> Result<Vec<u64>, String> {
    let mut h = PRINTER_HANDLE::default();
    // SAFETY: `name` is NUL-terminated and `h` is a valid out pointer.
    if unsafe { OpenPrinterW(name.as_ptr(), &mut h, null()) } == 0 {
        return Err(last_error("OpenPrinter"));
    }
    let printer = Printer(h);
    // SAFETY: a null output buffer asks for the DEVMODE size.
    let size = unsafe { DocumentPropertiesW(null_mut(), printer.0, name.as_ptr(), null_mut(), null(), 0) };
    if size <= 0 {
        return Err(last_error("DocumentProperties"));
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    let dm = buf.as_mut_ptr().cast::<DEVMODEW>();
    // SAFETY: `buf` has at least `size` bytes, as DocumentPropertiesW requested.
    let r = unsafe { DocumentPropertiesW(null_mut(), printer.0, name.as_ptr(), dm, null(), DM_OUT_BUFFER) };
    if r < 0 {
        return Err(last_error("DocumentProperties"));
    }
    // SAFETY: `dm` points at the initialised DEVMODEW the driver just wrote.
    unsafe {
        (*dm).dmFields |= DM_COPIES;
        (*dm).Anonymous1.Anonymous1.dmCopies = copies;
    }
    // Let the driver validate and merge the edited copy.
    let input = buf.clone();
    // SAFETY: both buffers are `size` bytes and hold the DEVMODEW from above.
    let r = unsafe {
        DocumentPropertiesW(
            null_mut(),
            printer.0,
            name.as_ptr(),
            dm,
            input.as_ptr().cast(),
            DM_IN_BUFFER | DM_OUT_BUFFER,
        )
    };
    if r < 0 {
        return Err(last_error("DocumentProperties"));
    }
    Ok(buf)
}

fn open_dc(name: &[u16], copies: i16) -> Result<Dc, String> {
    let dm = devmode(name, copies)?;
    let driver = wide("WINSPOOL");
    // SAFETY: all strings are NUL-terminated and `dm` holds a DEVMODEW from DocumentPropertiesW.
    let dc = unsafe { CreateDCW(driver.as_ptr(), name.as_ptr(), null(), dm.as_ptr().cast()) };
    if dc.is_null() { Err(last_error("CreateDC")) } else { Ok(Dc(dc)) }
}

/// Converts a premultiplied RGBA pixmap to opaque top-down BGRA, composed over white paper.
fn to_bgra(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for px in pm.data().chunks_exact(4) {
        let hole = 255 - px[3];
        out.extend_from_slice(&[
            px[2].saturating_add(hole),
            px[1].saturating_add(hole),
            px[0].saturating_add(hole),
            255,
        ]);
    }
    out
}

/// Prints the 0-based `pages` (each `copies` times, collated) to `printer` (`None` = system default).
pub fn print(
    printer: Option<&str>,
    copies: u32,
    title: &str,
    pages: &[usize],
    render: &mut RenderFn,
) -> Result<(), String> {
    let name = match printer {
        Some(p) => p.to_string(),
        None => default_printer().ok_or("no default printer is set")?,
    };
    let name = wide(&name);
    let want = copies.clamp(1, i16::MAX as u32) as i16;
    let mut dc = open_dc(&name, want)?;
    let mut repeat = copies;
    // SAFETY: the DC is valid.
    if i32::from(want) <= unsafe { GetDeviceCaps(dc.0, COPIES_CAP) } {
        repeat = 1;
    } else {
        // The driver cannot make the copies itself: print one copy and repeat the pages.
        dc = open_dc(&name, 1)?;
    }
    let hdc = dc.0;
    // SAFETY: `hdc` is a valid printer DC for as long as `dc` lives.
    let caps = |i: u32| unsafe { GetDeviceCaps(hdc, i as i32) };
    let (dpi_x, dpi_y) = (caps(LOGPIXELSX).max(72), caps(LOGPIXELSY).max(72));
    let dpi = f64::from(dpi_x.max(dpi_y)).min(MAX_DPI);
    let (paper_w, paper_h) = (caps(PHYSICALWIDTH), caps(PHYSICALHEIGHT));
    let (off_x, off_y) = (caps(PHYSICALOFFSETX), caps(PHYSICALOFFSETY));
    if paper_w <= 0 || paper_h <= 0 {
        return Err("the printer reported no paper size".into());
    }

    let doc_name = wide(if title.trim().is_empty() { "newpub publication" } else { title });
    let output = std::env::var("NEWPUB_PRINT_OUTPUT").ok().filter(|s| !s.is_empty()).map(|s| wide(&s));
    let info = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: doc_name.as_ptr(),
        lpszOutput: output.as_ref().map_or(null(), |o| o.as_ptr()),
        lpszDatatype: null(),
        fwType: 0,
    };
    // SAFETY: `info` and the strings it points at outlive the call.
    if unsafe { StartDocW(hdc, &info) } <= 0 {
        return Err(last_error("StartDoc"));
    }
    let result = (|| {
        // SAFETY: valid DC.
        unsafe { SetStretchBltMode(hdc, STRETCH_HALFTONE) };
        for _ in 0..repeat {
            for &page in pages {
                let pm = render(page, dpi)?;
                let (w, h) = (pm.width() as i32, pm.height() as i32);
                let bits = to_bgra(&pm);
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h, // negative: top-down rows
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        ..Default::default()
                    },
                    bmiColors: Default::default(),
                };
                // Scale proportionally to the paper and centre it there; the DC origin is the
                // printable corner, so subtract the physical offset.
                let scale = (f64::from(paper_w) / f64::from(w)).min(f64::from(paper_h) / f64::from(h));
                let (dw, dh) = ((f64::from(w) * scale).round() as i32, (f64::from(h) * scale).round() as i32);
                let (dx, dy) = ((paper_w - dw) / 2 - off_x, (paper_h - dh) / 2 - off_y);
                // SAFETY: valid DC; `bits` holds w*h 32-bit pixels matching `bmi`.
                unsafe {
                    if StartPage(hdc) <= 0 {
                        return Err(last_error("StartPage"));
                    }
                    let n = StretchDIBits(
                        hdc,
                        dx,
                        dy,
                        dw,
                        dh,
                        0,
                        0,
                        w,
                        h,
                        bits.as_ptr().cast(),
                        &bmi,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                    if n == 0 || n == -1 {
                        return Err(last_error("StretchDIBits"));
                    }
                    if EndPage(hdc) <= 0 {
                        return Err(last_error("EndPage"));
                    }
                }
            }
        }
        Ok(())
    })();
    // SAFETY: the document was started above; end it even after an error.
    let end = unsafe { EndDoc(hdc) };
    result?;
    if end <= 0 {
        return Err(last_error("EndDoc"));
    }
    Ok(())
}
