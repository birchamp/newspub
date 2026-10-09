//! The "Print publication" window and handing the job to the OS print system (PR-07).

use crate::{NewpubApp, labeled_field};
#[cfg(windows)]
#[path = "print_win.rs"]
mod print_win;
use newpub_engine::PdfOptions;
#[cfg(not(windows))]
use std::process::{Command, Stdio};
#[cfg(not(windows))]
use std::sync::mpsc;
#[cfg(not(windows))]
use std::time::Duration;

const DEFAULT_PRINTER: &str = "Default printer";
/// How long listing printers may take before we give up.
#[cfg(not(windows))]
const LIST_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, PartialEq)]
pub struct PrintState {
    printers: Vec<String>,
    printer: usize,
    copies: String,
    pages: String,
}

impl PrintState {
    pub fn new() -> PrintState {
        PrintState { printers: list_printers(), printer: 0, copies: "1".into(), pages: String::new() }
    }
}

/// Runs a command with a time limit and returns its stdout lines.
#[cfg(not(windows))]
fn run_lines(program: &'static str, args: &'static [&'static str]) -> Vec<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output();
        let _ = tx.send(out);
    });
    match rx.recv_timeout(LIST_TIMEOUT) {
        Ok(Ok(out)) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => vec![],
    }
}

#[cfg(windows)]
fn list_printers() -> Vec<String> {
    let mut names = print_win::list_printers();
    if names.is_empty() {
        names.push(DEFAULT_PRINTER.into());
    }
    names
}

#[cfg(not(windows))]
fn list_printers() -> Vec<String> {
    let mut names: Vec<String> = run_lines("lpstat", &["-e"]);
    if names.is_empty() {
        names = run_lines("lpstat", &["-a"])
            .iter()
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect();
    }
    names.dedup();
    if names.is_empty() {
        names.push(DEFAULT_PRINTER.into());
    }
    names
}

/// Parses "All"/empty or 1-based ranges like "1,3-4" into sorted, unique 0-based indices.
fn parse_pages(text: &str, count: usize) -> Result<Vec<usize>, String> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("all") {
        return Ok((0..count).collect());
    }
    let bad = || format!("Pages must look like 2-3 or 1,3-4 (publication has {count} pages)");
    let mut out = vec![];
    for part in t.split(',') {
        let part = part.trim();
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let a: usize = a.parse().map_err(|_| bad())?;
        let b: usize = b.parse().map_err(|_| bad())?;
        if a == 0 || b < a || b > count {
            return Err(bad());
        }
        out.extend((a - 1)..b);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Draws the print window; returns true when it should close.
pub fn show(app: &mut NewpubApp, ctx: &egui::Context, st: &mut PrintState) -> bool {
    let mut close = false;
    crate::files::dialog_window("Print publication").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Printer:");
            let shown = st.printers.get(st.printer).cloned().unwrap_or_default();
            let combo = egui::ComboBox::from_id_salt("printer").selected_text(shown).show_ui(ui, |ui| {
                for (i, p) in st.printers.iter().enumerate() {
                    ui.selectable_value(&mut st.printer, i, p);
                }
            });
            combo.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Printer"));
        });
        labeled_field(ui, "Copies", &mut st.copies);
        labeled_field(ui, "Pages", &mut st.pages);
        ui.weak("Pages: leave empty for all, or e.g. 2-3 or 1,3-4");
        ui.horizontal(|ui| {
            if crate::widgets::primary_button(ui, "Send to Printer").clicked() {
                close = send(app, st);
            }
            if crate::widgets::secondary_button(ui, "Cancel").clicked() {
                close = true;
            }
        });
    });
    close
}

fn send(app: &mut NewpubApp, st: &PrintState) -> bool {
    let copies = match st.copies.trim().parse::<u32>() {
        Ok(n) if (1..=999).contains(&n) => n,
        _ => {
            app.status = "Copies must be a number from 1 to 999".into();
            return false;
        }
    };
    let pages = match parse_pages(&st.pages, app.session.doc().pages.len()) {
        Ok(p) => p,
        Err(e) => {
            app.status = e;
            return false;
        }
    };
    let printer = st.printers.get(st.printer).cloned().unwrap_or_else(|| DEFAULT_PRINTER.into());
    // Windows prints natively through GDI; journeys (print_spool) still get a PDF spool file.
    #[cfg(windows)]
    if app.print_spool.is_none() {
        let title = app.session.doc().meta.title.clone();
        let session = &mut app.session;
        let mut render = |page: usize, dpi: f64| session.render_page(page, dpi).map_err(|e| e.to_string());
        let target = (printer != DEFAULT_PRINTER).then_some(printer.as_str());
        if let Err(e) = print_win::print(target, copies, &title, &pages, &mut render) {
            app.status = format!("Print failed: {e}");
            return false;
        }
        return finish(app, copies, pages, &printer, None);
    }
    let bytes = match app.session.pdf_bytes(&PdfOptions { pages: Some(pages.clone()), ..Default::default() }) {
        Ok(b) => b,
        Err(e) => {
            app.status = format!("Print failed: {e}");
            return false;
        }
    };
    let n = app.print_jobs + 1;
    let file = match &app.print_spool {
        Some(dir) => dir.join(format!("job-{n}.pdf")),
        None => std::env::temp_dir().join(format!("newpub-print-{}-{n}.pdf", std::process::id())),
    };
    if let Some(dir) = file.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        app.status = format!("Print failed: {e}");
        return false;
    }
    if let Err(e) = std::fs::write(&file, bytes) {
        app.status = format!("Print failed: {e}");
        return false;
    }
    #[cfg(not(windows))]
    if app.print_spool.is_none()
        && let Err(e) = hand_to_os(&file, &printer, copies)
    {
        app.status = format!("Print failed: {e}");
        return false;
    }
    finish(app, copies, pages, &printer, Some(file.to_string_lossy().into_owned()))
}

/// Records the finished job for the journeys and the status bar; returns true (close the window).
fn finish(app: &mut NewpubApp, copies: u32, pages: Vec<usize>, printer: &str, file: Option<String>) -> bool {
    app.print_jobs += 1;
    app.last_print_job = Some(serde_json::json!({
        "copies": copies, "pages": pages, "printer": printer, "file": file,
    }));
    app.status = format!("Sent {copies} cop{} to {printer}", if copies == 1 { "y" } else { "ies" });
    true
}

/// Hands the PDF to CUPS (`lp`).
#[cfg(not(windows))]
fn hand_to_os(file: &std::path::Path, printer: &str, copies: u32) -> Result<(), String> {
    let mut cmd = Command::new("lp");
    cmd.arg("-n").arg(copies.to_string());
    if printer != DEFAULT_PRINTER {
        cmd.arg("-d").arg(printer);
    }
    let out = cmd.arg(file).stdin(Stdio::null()).output().map_err(|e| format!("cannot run lp: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}
