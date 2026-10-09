//! The Styles window (Home > Styles): new paragraph and character styles from the formatting at the caret, and
//! applying or deleting the publication's styles.

use eframe::egui;
use newpub_engine::core::{Command, Id, StyleRef};

use crate::{NewpubApp, icons as ic, widgets};

#[derive(Default)]
pub struct StylesUi {
    pub open: bool,
    pub name: String,
}

impl NewpubApp {
    /// Story, range (selection, or the caret as an empty range) and the formatting there, of the text being edited.
    fn style_source(&self) -> Option<(Id, usize, usize)> {
        let frame = self.edit_target()?;
        let story = self.story_id(frame)?;
        let (a, b) = match self.caret_in(frame) {
            Some(c) => (c.range().start, c.range().end),
            None => (0, self.session.doc().story(story).map(|s| s.len()).unwrap_or(0)),
        };
        Some((story, a, b))
    }

    /// Paragraph and character formatting at the start of the source range, styles included.
    fn formatting_at(
        &self,
        story: Id,
        at: usize,
    ) -> Option<(newpub_engine::core::ParaAttrs, newpub_engine::core::CharAttrs)> {
        let doc = self.session.doc();
        let st = doc.story(story).ok()?;
        let para = st.paras.get(st.para_index_at(at))?;
        let run = st.span_attrs_at(at);
        let mut p = doc.effective_para_attrs(para);
        p.style = None;
        let mut c = doc.effective_char_attrs(para, &run);
        c.style = None;
        Some((p, c))
    }

    pub(crate) fn styles_window(&mut self, ctx: &egui::Context) {
        if !self.styles_ui.open {
            return;
        }
        let mut open = true;
        let source = self.style_source();
        let names = |key: &str, v: &serde_json::Value| -> Vec<String> {
            v.get(key)
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        let all = self.session.query(&newpub_engine::Query::Styles).unwrap_or_default();
        let (para_styles, char_styles) = (names("para", &all), names("chars", &all));
        egui::Window::new("Styles")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-560.0, 140.0))
            .show(ctx, |ui| {
                ui.set_width(300.0);
                let mut cmd = None;
                widgets::section(ui, ic::PLUS, "New style from the text at the caret", |ui| {
                    ui.horizontal(|ui| {
                        let l = ui.label("Style name");
                        ui.add(egui::TextEdit::singleline(&mut self.styles_ui.name).desired_width(180.0))
                            .labelled_by(l.id);
                    });
                    let name = self.styles_ui.name.trim().to_string();
                    let ok = !name.is_empty() && source.is_some();
                    ui.horizontal(|ui| {
                        let fmt = source.and_then(|(s, a, _)| self.formatting_at(s, a));
                        if widgets::small_button(ui, ic::TEXT_ALIGN_LEFT, "New Paragraph Style", false, ok)
                            .on_hover_text("A style with this paragraph's formatting")
                            .on_disabled_hover_text("Click in some text and give the style a name")
                            .clicked()
                            && let Some((para, chars)) = fmt.clone()
                        {
                            cmd = Some(Command::DefineParaStyle {
                                name: name.clone(),
                                based_on: None,
                                next: None,
                                para,
                                chars,
                            });
                        }
                        if widgets::small_button(ui, ic::TEXT_AA, "New Character Style", false, ok)
                            .on_hover_text("A style with the selected text's character formatting")
                            .on_disabled_hover_text("Select some text and give the style a name")
                            .clicked()
                            && let Some((_, chars)) = fmt
                        {
                            cmd = Some(Command::DefineCharStyle { name: name.clone(), based_on: None, chars });
                        }
                    });
                });
                ui.add_space(6.0);
                let target = source;
                let mut list = |ui: &mut egui::Ui, title: &str, icon: &str, items: &[String], para: bool| {
                    widgets::section(ui, icon, title, |ui| {
                        if items.is_empty() {
                            widgets::hint(ui, "None yet.");
                        }
                        for name in items {
                            ui.horizontal(|ui| {
                                ui.label(name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::icon_button(ui, ic::TRASH, &format!("Delete {name}"), false, true)
                                        .on_hover_text("Delete the style (text keeps its look as plain formatting)")
                                        .clicked()
                                    {
                                        cmd = Some(Command::DeleteStyle { style: StyleRef::Name(name.clone()) });
                                    }
                                    let apply = widgets::icon_button(
                                        ui,
                                        ic::PAINT_BRUSH,
                                        &format!("Apply {name}"),
                                        false,
                                        target.is_some(),
                                    )
                                    .on_hover_text(if para {
                                        "Apply to the paragraphs at the caret"
                                    } else {
                                        "Apply to the selected text"
                                    })
                                    .on_disabled_hover_text("Click in some text first");
                                    if apply.clicked()
                                        && let Some((story, a, b)) = target
                                    {
                                        let style = Some(StyleRef::Name(name.clone()));
                                        let (start, end) = (Some(a), Some(b));
                                        cmd = Some(if para {
                                            Command::ApplyParaStyle {
                                                target: story,
                                                start,
                                                end,
                                                style,
                                                clear_overrides: false,
                                            }
                                        } else {
                                            Command::ApplyCharStyle { target: story, start, end, style }
                                        });
                                    }
                                });
                            });
                        }
                    });
                };
                list(ui, "Paragraph styles", ic::TEXT_ALIGN_LEFT, &para_styles, true);
                ui.add_space(6.0);
                list(ui, "Character styles", ic::TEXT_AA, &char_styles, false);
                if let Some(c) = cmd {
                    self.act(c);
                }
            });
        self.styles_ui.open &= open;
    }
}
