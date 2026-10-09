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
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "newpub",
        options,
        Box::new(|_cc| {
            Ok(Box::new({
                let mut app = NewpubApp::new(session).with_persistent_recent();
                app.startup_picker = !opened;
                app
            }))
        }),
    )
}
