// Release builds on Windows are GUI apps: no console window opens with them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use newpub_app::NewpubApp;
use newpub_engine::Session;
use newpub_engine::layout::FontStore;
use std::sync::Arc;

fn main() -> eframe::Result {
    let mut session = Session::new(Arc::new(FontStore::with_system()));
    session.base_dir = std::env::current_dir().unwrap_or_default();
    // A file passed on the command line (double-click on a .npub file) opens instead of the template picker.
    let opened = std::env::args().nth(1).is_some_and(|path| {
        session.run(&newpub_engine::Action::Session(newpub_engine::SessionAction::Open { path })).is_ok()
    });
    let mut viewport = egui::ViewportBuilder::default().with_title("newpub").with_inner_size([1280.0, 860.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/newpub-256.png")) {
        viewport = viewport.with_icon(icon);
    }
    // OpenGL first; computers without OpenGL 2 (some virtual machines, old drivers) fall back to wgpu, which uses
    // Direct3D 12 (with its software renderer) on Windows, Metal on macOS and Vulkan on Linux.
    let session = std::cell::RefCell::new(Some(session));
    let mut errors = vec![];
    for renderer in [eframe::Renderer::Glow, eframe::Renderer::Wgpu] {
        let options = eframe::NativeOptions { viewport: viewport.clone(), renderer, ..Default::default() };
        let result = eframe::run_native(
            "newpub",
            options,
            Box::new(|_cc| {
                let session = session.borrow_mut().take().ok_or("the app was already started")?;
                let mut app = NewpubApp::new(session).with_persistent_recent();
                app.startup_picker = !opened;
                Ok(Box::new(app))
            }),
        );
        match result {
            Ok(()) => return Ok(()),
            // The app itself started and then failed: no second attempt.
            Err(e) if session.borrow().is_none() => return Err(e),
            Err(e) => {
                eprintln!("newpub: the {renderer} renderer could not start ({e})");
                errors.push(format!("{renderer}: {e}"));
            }
        }
    }
    let message = format!("newpub could not open its window.\n\n{}", errors.join("\n"));
    show_error(&message);
    Err(eframe::Error::AppCreation(message.into()))
}

/// Tells the user why the app cannot start (release builds on Windows have no console).
fn show_error(message: &str) {
    eprintln!("{message}");
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (text, title) = (wide(message), wide("newpub cannot start"));
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR) };
    }
}
