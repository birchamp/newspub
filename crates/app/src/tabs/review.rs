//! The Review ribbon tab.

use crate::widgets::{self, group, ribbon_button};
use crate::{NewpubApp, icons as ic, labeled_field};
use newpub_engine::core::{Command, Id, ObjectKind};
use newpub_engine::{Query, SessionAction};

/// State of the Review tab's windows.
#[derive(Default)]
pub(crate) struct ReviewState {
    spelling_open: bool,
    find_open: bool,
    a11y_open: bool,
    find: String,
    replace: String,
    match_case: bool,
    whole_word: bool,
    /// Index of the last hit visited by Find Next.
    next_hit: usize,
}

struct Misspelling {
    story: Id,
    start: usize,
    end: usize,
    word: String,
    suggestions: Vec<String>,
}

struct Issue {
    object: Id,
    page: usize,
    message: String,
}

impl NewpubApp {
    fn misspellings(&mut self) -> Vec<Misspelling> {
        let Ok(v) = self.session.query(&Query::Spelling) else { return vec![] };
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| {
                        Some(Misspelling {
                            story: Id(m["story"].as_u64()?),
                            start: m["start"].as_u64()? as usize,
                            end: m["end"].as_u64()? as usize,
                            word: m["word"].as_str()?.to_string(),
                            suggestions: m["suggestions"]
                                .as_array()?
                                .iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn a11y_issues(&mut self) -> Vec<Issue> {
        let Ok(v) = self.session.query(&Query::AccessibilityCheck) else { return vec![] };
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| {
                        Some(Issue {
                            object: Id(i["object"].as_u64()?),
                            page: i["page"].as_u64().unwrap_or(0) as usize,
                            message: i["message"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The text frame (and its page) showing a story.
    fn frame_of_story(&self, story: Id) -> Option<(Id, usize)> {
        let d = self.session.doc();
        d.pages.iter().enumerate().find_map(|(pi, p)| {
            p.objects.iter().find_map(|id| match d.objects.get(id).map(|o| &o.kind) {
                Some(ObjectKind::Text(t)) if t.story == story => Some((*id, pi)),
                _ => None,
            })
        })
    }

    pub(crate) fn review_tab(&mut self, ui: &mut egui::Ui) {
        group(ui, "Proofing", |ui| {
            if ribbon_button(ui, ic::LIST_CHECKS, "Spelling", self.review_ui.spelling_open, true).clicked() {
                self.review_ui.spelling_open = !self.review_ui.spelling_open;
            }
            if ribbon_button(ui, ic::MAGNIFYING_GLASS, "Find and Replace", self.review_ui.find_open, true).clicked() {
                self.review_ui.find_open = !self.review_ui.find_open;
            }
        });
        group(ui, "Accessibility", |ui| {
            if ribbon_button(ui, ic::WHEELCHAIR, "Accessibility Checker", self.review_ui.a11y_open, true).clicked() {
                self.review_ui.a11y_open = !self.review_ui.a11y_open;
            }
        });
    }

    pub(crate) fn review_windows(&mut self, ctx: &egui::Context) {
        if self.review_ui.spelling_open {
            let mut open = true;
            let errors = self.misspellings();
            egui::Window::new("Spelling")
                .default_pos(window_pos(ctx))
                .open(&mut open)
                .collapsible(false)
                .default_width(260.0)
                .show(ctx, |ui| {
                    if errors.is_empty() {
                        ui.label("No spelling errors");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, m) in errors.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.separator();
                                ui.label(&m.word);
                                ui.horizontal_wrapped(|ui| {
                                    for s in &m.suggestions {
                                        if ui.button(s).clicked() {
                                            self.act(Command::ReplaceText {
                                                target: m.story,
                                                start: m.start,
                                                end: m.end,
                                                text: s.clone(),
                                            });
                                        }
                                    }
                                });
                                ui.horizontal(|ui| {
                                    if ui.small_button("Ignore").clicked() {
                                        self.act(SessionAction::IgnoreWord { word: m.word.clone() });
                                    }
                                    if ui.small_button("Add to Dictionary").clicked() {
                                        self.act(Command::AddToDictionary { word: m.word.clone() });
                                    }
                                });
                            });
                        }
                    });
                });
            self.review_ui.spelling_open = open;
        }
        if self.review_ui.find_open {
            let mut open = true;
            egui::Window::new("Find and Replace Text")
                .default_pos(window_pos(ctx))
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    labeled_field(ui, "Find what", &mut self.review_ui.find);
                    labeled_field(ui, "Replace with", &mut self.review_ui.replace);
                    ui.checkbox(&mut self.review_ui.match_case, "Match case");
                    ui.checkbox(&mut self.review_ui.whole_word, "Whole words");
                    ui.horizontal(|ui| {
                        if widgets::secondary_button(ui, "Find Next").clicked() {
                            self.find_next();
                        }
                        if widgets::primary_button(ui, "Replace All").clicked() {
                            let r = &self.review_ui;
                            let (find, replace) = (r.find.clone(), r.replace.clone());
                            let (match_case, whole_word) = (r.match_case, r.whole_word);
                            let count = self.find_hits().len();
                            if self.act(SessionAction::ReplaceAll { find, replace, match_case, whole_word }).is_some() {
                                self.status =
                                    format!("Replaced {count} occurrence{}", if count == 1 { "" } else { "s" });
                            }
                        }
                    });
                });
            self.review_ui.find_open = open;
        }
        if self.review_ui.a11y_open {
            let mut open = true;
            let issues = self.a11y_issues();
            egui::Window::new("Accessibility")
                .default_pos(window_pos(ctx))
                .open(&mut open)
                .collapsible(false)
                .default_width(260.0)
                .show(ctx, |ui| {
                    if issues.is_empty() {
                        ui.label("No issues found");
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, issue) in issues.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                if ui.add(egui::Label::new(&issue.message).sense(egui::Sense::click())).clicked() {
                                    self.selection = vec![issue.object];
                                    self.page = issue.page;
                                }
                            });
                        }
                    });
                });
            self.review_ui.a11y_open = open;
        }
    }

    fn find_hits(&mut self) -> Vec<(Id, usize, usize)> {
        let r = &self.review_ui;
        if r.find.is_empty() {
            return vec![];
        }
        let q = Query::Find { text: r.find.clone(), match_case: r.match_case, whole_word: r.whole_word };
        let Ok(v) = self.session.query(&q) else { return vec![] };
        let hit = |h: &serde_json::Value| {
            Some((Id(h["story"].as_u64()?), h["start"].as_u64()? as usize, h["end"].as_u64()? as usize))
        };
        v.as_array().map(|a| a.iter().filter_map(hit).collect()).unwrap_or_default()
    }

    /// Selects the text of the next hit, editing its frame.
    fn find_next(&mut self) {
        let hits = self.find_hits();
        if hits.is_empty() {
            self.status = "Not found".into();
            return;
        }
        let i = self.review_ui.next_hit % hits.len();
        self.review_ui.next_hit = i + 1;
        let (story, start, end) = hits[i];
        if let Some((frame, page)) = self.frame_of_story(story) {
            self.page = page;
            self.place_caret(frame, start, false);
            self.place_caret(frame, end, true);
            self.status = format!("Match {} of {}", i + 1, hits.len());
        }
    }
}

/// Default window position: below the ribbon, towards the right, clear of the tab buttons.
fn window_pos(ctx: &egui::Context) -> egui::Pos2 {
    egui::pos2((ctx.content_rect().right() - 340.0).max(20.0), 200.0)
}
