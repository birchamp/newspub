use newpub_app::NewpubApp;
use newpub_engine::Session;
use newpub_engine::layout::FontStore;
use std::sync::Arc;

fn main() -> eframe::Result {
    let mut session = Session::new(Arc::new(FontStore::with_system()));
    session.base_dir = std::env::current_dir().unwrap_or_default();
    if let Some(path) = std::env::args().nth(1) {
        let _ = session.run(&newpub_engine::Action::Session(newpub_engine::SessionAction::Open { path }));
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("newpub").with_inner_size([1280.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "newpub",
        options,
        Box::new(|_cc| Ok(Box::new(NewpubApp::new(session).with_persistent_recent()))),
    )
}
