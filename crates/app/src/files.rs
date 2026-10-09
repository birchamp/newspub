//! File dialogs: open, save, export, insert picture.

use crate::icons as ic;
use crate::{Dialog, NewpubApp, picker, print, theme, widgets};
use egui::{Align2, CornerRadius, Sense, Stroke, StrokeKind, Vec2, WidgetInfo, WidgetType};
use newpub_engine::action::ImageFormat;
use newpub_engine::{PdfOptions, SessionAction};
use newpub_io_pdf::PdfStandard;

/// What a "Browse…" button asks the operating system for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Browse {
    /// Open a publication.
    OpenPub,
    /// Choose where to save a publication.
    SavePub,
    /// Open a picture.
    Picture,
    /// Open a text file to import.
    Text,
    /// Choose where to save a file (filter name, extension).
    Save(&'static str, &'static str),
    /// Choose a folder.
    Folder,
}

/// Show the native file dialog and return the chosen path (None when cancelled).
fn browse(kind: Browse, current: &str) -> Option<String> {
    let mut dlg = rfd::FileDialog::new();
    if let Some(dir) = std::path::Path::new(current).parent().filter(|d| !d.as_os_str().is_empty() && d.is_dir()) {
        dlg = dlg.set_directory(dir);
    }
    let picked = match kind {
        Browse::OpenPub => dlg
            .add_filter("Publications", &["npub", "newspub", "pub"])
            .add_filter("Microsoft Publisher", &["pub"])
            .add_filter("All files", &["*"])
            .pick_file(),
        Browse::SavePub => dlg.add_filter("Publication", &["npub"]).set_file_name("publication.npub").save_file(),
        Browse::Picture => dlg
            .add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "tif", "tiff"])
            .add_filter("All files", &["*"])
            .pick_file(),
        Browse::Text => {
            dlg.add_filter("Text files", &["txt", "text", "docx"]).add_filter("All files", &["*"]).pick_file()
        }
        Browse::Save(name, ext) => dlg.add_filter(name, &[ext]).set_file_name(format!("publication.{ext}")).save_file(),
        Browse::Folder => dlg.pick_folder(),
    };
    picked.map(|p| p.to_string_lossy().to_string())
}

/// A labelled path field with a Browse… button beside it.
fn path_field(ui: &mut egui::Ui, label: &str, value: &mut String, kind: Browse) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(240.0)).labelled_by(l.id);
        if widgets::small_button(ui, ic::FOLDER_OPEN, "Browse…", false, true).clicked()
            && let Some(p) = browse(kind, value)
        {
            *value = p;
        }
    });
}

/// The export formats of the Export As dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    PdfX4,
    PdfUa,
    Booklet,
    Png,
    Jpeg,
    Html,
    Epub,
    Xps,
    PackAndGo,
}

impl ExportFormat {
    const ALL: [ExportFormat; 10] = [
        Self::Pdf,
        Self::PdfX4,
        Self::PdfUa,
        Self::Booklet,
        Self::Png,
        Self::Jpeg,
        Self::Html,
        Self::Epub,
        Self::Xps,
        Self::PackAndGo,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::PdfX4 => "PDF/X-4 for print shops",
            Self::PdfUa => "PDF/UA accessible PDF",
            Self::Booklet => "Booklet PDF",
            Self::Png => "PNG image",
            Self::Jpeg => "JPEG image",
            Self::Html => "Web page (HTML)",
            Self::Epub => "EPUB e-book",
            Self::Xps => "XPS document",
            Self::PackAndGo => "Pack and Go",
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Self::Pdf => "Fonts embedded, ready to share",
            Self::PdfX4 => "Press-ready, with crop marks and bleed",
            Self::PdfUa => "Tagged for screen readers",
            Self::Booklet => "Pages imposed for folding and stapling",
            Self::Png => "One page as a lossless picture",
            Self::Jpeg => "One page as a small picture",
            Self::Html => "A folder with a web page",
            Self::Epub => "Fixed-layout e-book",
            Self::Xps => "Fixed-layout XPS document",
            Self::PackAndGo => "Publication, fonts and pictures in one folder",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Pdf | Self::PdfX4 | Self::PdfUa => ic::FILE_PDF,
            Self::Booklet => ic::BOOK_OPEN,
            Self::Png => ic::FILE_PNG,
            Self::Jpeg => ic::FILE_JPG,
            Self::Html => ic::FILE_HTML,
            Self::Epub => ic::BOOK,
            Self::Xps => ic::FILE_DOC,
            Self::PackAndGo => ic::PACKAGE,
        }
    }

    fn default_file(self) -> &'static str {
        match self {
            Self::Pdf | Self::PdfX4 | Self::PdfUa => "publication.pdf",
            Self::Booklet => "booklet.pdf",
            Self::Png => "page.png",
            Self::Jpeg => "page.jpg",
            Self::Html => "site",
            Self::Epub => "publication.epub",
            Self::Xps => "publication.xps",
            Self::PackAndGo => "packed",
        }
    }

    fn is_pdf(self) -> bool {
        matches!(self, Self::Pdf | Self::PdfX4 | Self::PdfUa | Self::Booklet)
    }

    fn is_folder(self) -> bool {
        matches!(self, Self::Html | Self::PackAndGo)
    }

    fn browse_kind(self) -> Browse {
        match self {
            Self::Html | Self::PackAndGo => Browse::Folder,
            Self::Png => Browse::Save("PNG image", "png"),
            Self::Jpeg => Browse::Save("JPEG image", "jpg"),
            Self::Epub => Browse::Save("EPUB e-book", "epub"),
            Self::Xps => Browse::Save("XPS document", "xps"),
            _ => Browse::Save("PDF", "pdf"),
        }
    }
}

/// State of the export dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportState {
    pub format: ExportFormat,
    pub path: String,
    /// Page range such as "1-3, 5"; empty means all pages.
    pub pages: String,
    pub crop_marks: bool,
    pub dpi: f64,
    pub quality: u8,
    pub include_pdf: bool,
    pub include_fonts: bool,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            format: ExportFormat::Pdf,
            path: ExportFormat::Pdf.default_file().into(),
            pages: String::new(),
            crop_marks: false,
            dpi: 150.0,
            quality: 90,
            include_pdf: false,
            include_fonts: true,
        }
    }
}

/// Parse "1-3, 5" into 0-based page indices (None for empty or invalid input).
fn parse_pages(text: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?),
            None => {
                let n = part.parse::<usize>().ok()?;
                (n, n)
            }
        };
        if a == 0 || b < a || b > 100_000 {
            return None;
        }
        out.extend((a..=b).map(|n| n - 1));
    }
    (!out.is_empty()).then_some(out)
}

/// One selectable row of the format list: icon, name, one-line description.
fn format_row(ui: &mut egui::Ui, f: ExportFormat, selected: bool) -> egui::Response {
    let p = theme::palette(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(250.0, 44.0), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, f.name()));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if selected {
            p.primary_soft
        } else if resp.hovered() {
            p.surface_alt
        } else {
            egui::Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        if selected {
            painter.rect_stroke(
                rect,
                CornerRadius::same(8),
                Stroke::new(1.0, p.primary.gamma_multiply(0.35)),
                StrokeKind::Inside,
            );
        }
        painter.text(
            rect.left_center() + Vec2::new(20.0, 0.0),
            Align2::CENTER_CENTER,
            f.icon(),
            theme::icon_font(20.0),
            p.primary,
        );
        let fg = if selected { p.primary } else { p.text };
        painter.text(rect.left_top() + Vec2::new(40.0, 14.0), Align2::LEFT_CENTER, f.name(), theme::medium(12.5), fg);
        painter.text(
            rect.left_top() + Vec2::new(40.0, 31.0),
            Align2::LEFT_CENTER,
            f.describe(),
            theme::medium(10.5),
            p.text_muted,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Save / Cancel button row shared by the simple dialogs; returns (confirmed, cancelled).
fn confirm_row(ui: &mut egui::Ui, confirm: &str) -> (bool, bool) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let ok = widgets::primary_button(ui, confirm).clicked();
        let cancel = widgets::secondary_button(ui, "Cancel").clicked();
        (ok, cancel)
    })
    .inner
}

impl NewpubApp {
    /// Run the export described by the dialog state; true on success.
    fn run_export(&mut self, st: &ExportState) -> bool {
        use ExportFormat as F;
        let path = st.path.clone();
        let action = match st.format {
            F::Pdf | F::PdfX4 | F::PdfUa | F::Booklet => {
                let print_ready = st.crop_marks || st.format == F::PdfX4;
                let options = PdfOptions {
                    crop_marks: print_ready,
                    bleed: print_ready,
                    imposition: if st.format == F::Booklet {
                        newpub_engine::Imposition::Booklet
                    } else {
                        newpub_engine::Imposition::None
                    },
                    pages: parse_pages(&st.pages),
                    standard: match st.format {
                        F::PdfX4 => Some(PdfStandard::PdfX4),
                        F::PdfUa => Some(PdfStandard::PdfUa1),
                        _ => None,
                    },
                    separations: false,
                };
                SessionAction::ExportPdf { path, options }
            }
            F::Png | F::Jpeg => SessionAction::ExportImage {
                path,
                page: self.page,
                dpi: st.dpi,
                format: if st.format == F::Png { ImageFormat::Png } else { ImageFormat::Jpeg },
                quality: st.quality,
            },
            F::Html => SessionAction::ExportHtml { path },
            F::Epub => SessionAction::ExportEpub { path },
            F::Xps => SessionAction::ExportXps { path },
            F::PackAndGo => SessionAction::PackAndGo { dir: path, fonts: st.include_fonts, pdf: st.include_pdf },
        };
        if self.act(action).is_some() {
            self.status = format!("Exported {} as {}", st.path, st.format.name());
            true
        } else {
            false
        }
    }

    fn export_dialog(&mut self, ctx: &egui::Context, st: &mut ExportState) -> bool {
        let mut close = false;
        dialog_window("Export As").show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(250.0);
                    for f in ExportFormat::ALL {
                        if format_row(ui, f, st.format == f).clicked() && st.format != f {
                            if st.path == st.format.default_file() {
                                st.path = f.default_file().into();
                            }
                            st.format = f;
                        }
                    }
                });
                ui.vertical(|ui| {
                    ui.set_width(360.0);
                    let f = st.format;
                    widgets::section(ui, f.icon(), f.name(), |ui| {
                        widgets::hint(ui, f.describe());
                        ui.add_space(4.0);
                        if f.is_pdf() {
                            crate::labeled_field(ui, "Pages", &mut st.pages);
                            widgets::hint(ui, "For example 1-3, 5. Leave empty for all pages.");
                            if f == ExportFormat::PdfX4 {
                                widgets::hint(ui, "Crop marks and bleed are always included.");
                            } else {
                                ui.checkbox(&mut st.crop_marks, "Crop marks");
                            }
                        }
                        if matches!(f, ExportFormat::Png | ExportFormat::Jpeg) {
                            widgets::hint(ui, "Exports the page you are viewing.");
                            ui.horizontal(|ui| {
                                ui.label("Resolution (dpi)");
                                ui.add(egui::DragValue::new(&mut st.dpi).range(36.0..=1200.0));
                            });
                            if f == ExportFormat::Jpeg {
                                ui.horizontal(|ui| {
                                    ui.label("Quality");
                                    ui.add(egui::Slider::new(&mut st.quality, 1..=100));
                                });
                            }
                        }
                        if f == ExportFormat::PackAndGo {
                            ui.checkbox(&mut st.include_fonts, "Include fonts");
                            ui.checkbox(&mut st.include_pdf, "Include PDF");
                        }
                    });
                    ui.add_space(6.0);
                    path_field(ui, "Export file", &mut st.path, f.browse_kind());
                    if f.is_folder() {
                        widgets::hint(ui, "A folder is created for this format.");
                    }
                    let (ok, cancel) = confirm_row(ui, "Export File");
                    if ok {
                        let snapshot = st.clone();
                        if self.run_export(&snapshot) {
                            close = true;
                        }
                    }
                    close |= cancel;
                });
            });
        });
        close
    }

    pub(crate) fn dialogs(&mut self, ctx: &egui::Context) {
        let mut dialog = std::mem::replace(&mut self.dialog, Dialog::None);
        // Escape cancels any dialog.
        let mut close = !matches!(dialog, Dialog::None) && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if close && matches!(dialog, Dialog::Save { .. }) {
            self.save_finished(false);
        }
        match &mut dialog {
            Dialog::None => {}
            Dialog::Picker(st) => close = picker::show(self, ctx, st),
            Dialog::Print(st) => close = print::show(self, ctx, st),
            Dialog::ExportPdf { path, crop_marks, booklet } => {
                dialog_window("Export PDF").show(ctx, |ui| {
                    widgets::section(ui, ic::FILE_PDF, "Where to save", |ui| {
                        path_field(ui, "PDF file", path, Browse::Save("PDF", "pdf"));
                    });
                    ui.add_space(6.0);
                    widgets::section(ui, ic::BOOK_OPEN, "Print options", |ui| {
                        ui.checkbox(crop_marks, "Crop marks");
                        ui.checkbox(booklet, "Booklet");
                        widgets::hint(ui, "Booklet places pages in folding order.");
                    });
                    let (ok, cancel) = confirm_row(ui, "Export");
                    if ok {
                        let options = PdfOptions {
                            crop_marks: *crop_marks,
                            bleed: *crop_marks,
                            imposition: if *booklet {
                                newpub_engine::Imposition::Booklet
                            } else {
                                newpub_engine::Imposition::None
                            },
                            pages: None,
                            standard: None,
                            separations: false,
                        };
                        if self.act(SessionAction::ExportPdf { path: path.clone(), options }).is_some() {
                            self.status = format!("Exported {path}");
                            close = true;
                        }
                    }
                    close |= cancel;
                });
            }
            Dialog::Save { path } => {
                dialog_window("Save Publication").show(ctx, |ui| {
                    widgets::section(ui, ic::FOLDER_OPEN, "Save as", |ui| {
                        path_field(ui, "File name", path, Browse::SavePub);
                        widgets::hint(ui, "Publications are saved as .npub files.");
                    });
                    let mut as_template = false;
                    if self.user_templates.is_some() {
                        ui.add_space(4.0);
                        as_template = widgets::small_button(ui, ic::LAYOUT, "Save as Template", false, true)
                            .on_hover_text("Keep this publication in My templates, to start new ones from it")
                            .clicked();
                    }
                    let (ok, cancel) = confirm_row(ui, "Save File");
                    if as_template {
                        // The template dialog takes over from this one.
                        let title = self.session.doc().meta.title.clone();
                        self.dialog = Dialog::SaveTemplate { name: title, keep_text: true, keep_images: true };
                        close = true;
                    } else if ok && self.act(SessionAction::Save { path: path.clone() }).is_some() {
                        self.remember_recent();
                        self.status = format!("Saved {path}");
                        close = true;
                        self.save_finished(true);
                    } else if cancel {
                        close = true;
                        self.save_finished(false);
                    }
                });
            }
            Dialog::Open { path } => {
                dialog_window("Open Publication").show(ctx, |ui| {
                    widgets::section(ui, ic::FOLDER_OPEN, "Open a file", |ui| {
                        path_field(ui, "File name", path, Browse::OpenPub);
                        widgets::hint(
                            ui,
                            "newpub publications (.npub), or Microsoft Publisher files (.pub) to import.",
                        );
                    });
                    let recent: Vec<_> = self.recent.files().to_vec();
                    if !recent.is_empty() {
                        ui.add_space(6.0);
                        widgets::section(ui, ic::BOOK, "Recent files", |ui| {
                            for file in recent {
                                let name =
                                    file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                                if widgets::secondary_button(ui, &format!("Recent: {name}")).clicked()
                                    && self.open_file(&file)
                                {
                                    close = true;
                                }
                            }
                        });
                    }
                    let (ok, cancel) = confirm_row(ui, "Open File");
                    if ok && self.open_file(std::path::Path::new(path.as_str())) {
                        close = true;
                    }
                    close |= cancel;
                });
            }
            Dialog::Export(st) => close = self.export_dialog(ctx, st),
            Dialog::InsertPicture { path } => {
                dialog_window("Insert Picture").show(ctx, |ui| {
                    widgets::section(ui, ic::IMAGE, "Picture", |ui| {
                        path_field(ui, "Picture file", path, Browse::Picture);
                        widgets::hint(ui, "PNG, JPEG, GIF, BMP, WebP or SVG.");
                    });
                    let (ok, cancel) = confirm_row(ui, "Place Picture");
                    if ok {
                        let a = SessionAction::InsertPicture {
                            path: path.clone(),
                            page: Some(self.page),
                            x: None,
                            y: None,
                            width: None,
                            height: None,
                            into: None,
                            link: false,
                        };
                        if let Some(o) = self.act(a) {
                            self.selection = o.created.first().copied().into_iter().collect();
                            close = true;
                        }
                    }
                    close |= cancel;
                });
            }
            Dialog::SaveTemplate { name, keep_text, keep_images } => {
                dialog_window("Save as Template").show(ctx, |ui| {
                    widgets::section(ui, ic::LAYOUT, "Template", |ui| {
                        ui.horizontal(|ui| {
                            let l = ui.label("Template name");
                            ui.add(egui::TextEdit::singleline(name).desired_width(220.0)).labelled_by(l.id);
                        });
                        ui.checkbox(keep_text, "Keep text");
                        ui.checkbox(keep_images, "Keep pictures");
                        widgets::hint(ui, "Without them, text boxes and picture frames stay as empty placeholders.");
                    });
                    let (ok, cancel) = confirm_row(ui, "Save Template");
                    let clean = name.trim().replace(['/', '\\', ':'], "-");
                    if ok && clean.is_empty() {
                        self.status = "Give the template a name".into();
                    } else if ok && let Some(dir) = self.user_templates.clone() {
                        let file = dir.join(format!("{clean}.npubt"));
                        let a = SessionAction::SaveTemplate {
                            path: file.to_string_lossy().to_string(),
                            name: name.trim().to_string(),
                            keep_text: *keep_text,
                            keep_images: *keep_images,
                        };
                        if std::fs::create_dir_all(&dir).is_ok() && self.act(a).is_some() {
                            self.status = format!("Saved template {clean}");
                            self.template_previews.retain(|k, _| !k.starts_with("file:"));
                            close = true;
                        }
                    }
                    close |= cancel;
                });
            }
            Dialog::InsertText { path, target } => {
                let target = *target;
                dialog_window("Insert Text File").show(ctx, |ui| {
                    widgets::section(ui, ic::FILE_TEXT, "Text", |ui| {
                        path_field(ui, "Text file", path, Browse::Text);
                        widgets::hint(ui, "Plain text (.txt) or Word (.docx). The text goes in at the caret.");
                    });
                    let (ok, cancel) = confirm_row(ui, "Insert Text");
                    if ok {
                        let at = Some(self.insertion_point(target));
                        if self.act(SessionAction::ImportText { target, path: path.clone(), at }).is_some() {
                            close = true;
                        }
                    }
                    close |= cancel;
                });
            }
        }
        if !close && self.dialog == Dialog::None {
            self.dialog = dialog;
        }
    }
}

/// A dialog window centred near the top of the app window.
pub(crate) fn dialog_window(title: &str) -> egui::Window<'_> {
    egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, [0.0, 96.0])
}
