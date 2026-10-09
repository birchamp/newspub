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
    launch(viewport, session, opened)
}

#[cfg(target_os = "macos")]
fn launch(viewport: egui::ViewportBuilder, session: Session, opened: bool) -> eframe::Result {
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    macos::run(options, move || {
        let mut app = NewpubApp::new(session).with_persistent_recent();
        app.startup_picker = !opened;
        app
    })
}

#[cfg(not(target_os = "macos"))]
fn launch(viewport: egui::ViewportBuilder, session: Session, opened: bool) -> eframe::Result {
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

/// macOS sends double-clicked documents as "open" events to the app delegate, not as arguments. newpub builds
/// winit's event loop itself, adds `application:openURLs:` to winit's delegate, and hands the paths to the app.
#[cfg(target_os = "macos")]
mod macos {
    use newpub_app::NewpubApp;
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{class, msg_send, sel};
    use std::ffi::{CStr, c_char};

    pub fn run(options: eframe::NativeOptions, make_app: impl FnOnce() -> NewpubApp) -> eframe::Result {
        let fail = |e: &dyn std::fmt::Display| {
            let message = format!("newpub could not open its window.\n\n{e}");
            super::show_error(&message);
            eframe::Error::AppCreation(message.into())
        };
        let event_loop =
            winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event().build().map_err(|e| fail(&e))?;
        install_open_handler();
        let mut make_app = Some(make_app);
        let mut app = eframe::create_native(
            "newpub",
            options,
            Box::new(move |_cc| {
                let make_app = make_app.take().ok_or("the app was already started")?;
                Ok(Box::new(make_app()))
            }),
            &event_loop,
        );
        event_loop.run_app(&mut app).map_err(|e| fail(&e))
    }

    /// `-[WinitApplicationDelegate application:openURLs:]`: queues every file URL for the app.
    unsafe extern "C" fn open_urls(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, urls: *mut AnyObject) {
        // SAFETY: AppKit passes an NSArray of NSURL; every message below exists on those classes.
        unsafe {
            let count: usize = msg_send![urls, count];
            for i in 0..count {
                let url: *mut AnyObject = msg_send![urls, objectAtIndex: i];
                let is_file: bool = msg_send![url, isFileURL];
                if !is_file {
                    continue;
                }
                let path: *mut AnyObject = msg_send![url, path];
                if path.is_null() {
                    continue;
                }
                let utf8: *const c_char = msg_send![path, UTF8String];
                if !utf8.is_null() {
                    newpub_app::request_open(CStr::from_ptr(utf8).to_string_lossy().into_owned().into());
                }
            }
        }
    }

    /// Adds `application:openURLs:` to winit's application delegate (created with the event loop), then sets the
    /// delegate again so AppKit notices the new method.
    fn install_open_handler() {
        let Some(cls) = AnyClass::get("WinitApplicationDelegate") else {
            eprintln!("newpub: winit's application delegate was not found; double-clicked files will not open");
            return;
        };
        let imp: unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject) = open_urls;
        // SAFETY: the type string "v@:@@" matches `open_urls` (void; self, _cmd, two objects), the class exists,
        // and NSApplication's delegate is winit's delegate object, which winit keeps alive.
        unsafe {
            objc2::ffi::class_addMethod(
                cls as *const AnyClass as *mut objc2::ffi::objc_class,
                sel!(application:openURLs:).as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
                    unsafe extern "C" fn(),
                >(imp)),
                c"v@:@@".as_ptr(),
            );
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let delegate: *mut AnyObject = msg_send![app, delegate];
            let none: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![app, setDelegate: none];
            let _: () = msg_send![app, setDelegate: delegate];
        }
    }
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
