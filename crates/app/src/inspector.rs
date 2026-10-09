//! The right-hand inspector: formatting of the selected text and properties of the selected objects.

use crate::{Fields, NewpubApp, fmt_len, fmt_num, icons, labeled_field_state, widgets};
use egui::Color32;
use newpub_engine::core::{
    self as core, Align, CharAttrs, Command, Length, ObjectKind, ObjectPatch, ParaAttrs, TextFramePatch,
};

impl NewpubApp {
    pub(crate) fn format_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Format");
        let first = self.selection.first().copied();
        if self.fields.owner != first {
            self.fields = Fields { owner: first, ..Default::default() };
        }
        let Some(frame) = self.selected_text_frame() else {
            if first.is_none() {
                widgets::hint(ui, "Select a text box to format text.");
            } else {
                widgets::section(ui, icons::RULER, "Position and size", |ui| self.object_panel(ui));
            }
            return;
        };
        let doc = self.session.doc();
        let Ok(sid) = doc.story_of(frame) else { return };
        let Ok(story) = doc.story(sid) else { return };
        let rc = doc.resolve_char(&story.paras[0], &story.span_attrs_at(0));
        let mut font = rc.font.clone();
        let families = self.session.fonts().families();
        let combo = egui::ComboBox::from_id_salt("font").selected_text(&font).show_ui(ui, |ui| {
            for f in &families {
                ui.selectable_value(&mut font, f.clone(), f);
            }
        });
        combo.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "Font"));
        let mut patch = CharAttrs::default();
        if font != rc.font {
            patch.font = Some(font);
        }
        ui.horizontal(|ui| {
            if ui.selectable_label(rc.bold, "Bold").clicked() {
                patch.bold = Some(!rc.bold);
            }
            if ui.selectable_label(rc.italic, "Italic").clicked() {
                patch.italic = Some(!rc.italic);
            }
            if ui.selectable_label(rc.underline, "Underline").clicked() {
                patch.underline = Some(!rc.underline);
            }
        });
        if !patch.is_empty() {
            self.act(Command::FormatChars { target: frame, start: None, end: None, attrs: patch });
        }
        // Font size: typing applies.
        let mut size_text = std::mem::take(&mut self.fields.size);
        let (changed, focused) = labeled_field_state(ui, "Font size", &mut size_text);
        if changed {
            if let Some(v) = core::units::parse_length(&size_text).filter(|v| (1.0..=999.0).contains(v))
                && (v - rc.size).abs() > 1e-9
            {
                let attrs = CharAttrs { size: Some(Length(v)), ..Default::default() };
                self.act_run(
                    format!("size:{frame}"),
                    Command::FormatChars { target: frame, start: None, end: None, attrs },
                );
            }
        } else if !focused {
            size_text = fmt_num(rc.size);
        }
        self.fields.size = size_text;
        ui.label("Alignment");
        let rp = self
            .session
            .doc()
            .resolve_para(&self.session.doc().story(sid).map(|s| s.paras[0].clone()).unwrap_or_default());
        ui.horizontal_wrapped(|ui| {
            for (a, label) in [
                (Align::Left, "Align Left"),
                (Align::Center, "Center"),
                (Align::Right, "Align Right"),
                (Align::Justify, "Justify"),
            ] {
                if ui.selectable_label(rp.align == a, label).clicked() {
                    self.act(Command::FormatParas {
                        target: frame,
                        start: None,
                        end: None,
                        attrs: ParaAttrs { align: Some(a), ..Default::default() },
                    });
                }
            }
        });
        ui.add_space(8.0);
        widgets::section(ui, icons::RULER, "Position and size", |ui| self.object_panel(ui));
        let overflow = self
            .session
            .query(&newpub_engine::Query::Overflow { target: frame })
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if overflow {
            ui.colored_label(Color32::from_rgb(200, 60, 40), "Text overflow");
        }
    }

    /// Position, size and (for text frames) columns of the first selected object.
    fn object_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selection.first().copied() else { return };
        let Some(obj) = self.session.doc().objects.get(&id) else { return };
        let (rect, columns) = (obj.rect, if let ObjectKind::Text(t) = &obj.kind { Some(t.columns) } else { None });
        ui.label(format!("Object {id}"));
        let mut new_rect = rect;
        let mut edited = false;
        let model = [rect.x, rect.y, rect.w, rect.h];
        let mut bufs = [
            std::mem::take(&mut self.fields.x),
            std::mem::take(&mut self.fields.y),
            std::mem::take(&mut self.fields.w),
            std::mem::take(&mut self.fields.h),
        ];
        for (i, label) in ["X position", "Y position", "Width", "Height"].into_iter().enumerate() {
            let (changed, focused) = labeled_field_state(ui, label, &mut bufs[i]);
            if changed {
                // Any unit is accepted; partial input such as "2i" is ignored until it parses.
                if let Some(v) =
                    core::units::parse_length(&bufs[i]).filter(|v| *v >= if i < 2 { f64::MIN } else { 0.1 })
                {
                    match i {
                        0 => new_rect.x = v,
                        1 => new_rect.y = v,
                        2 => new_rect.w = v,
                        _ => new_rect.h = v,
                    }
                    edited = true;
                }
            } else if !focused {
                bufs[i] = fmt_len(model[i]);
            }
        }
        let [x, y, w, h] = bufs;
        (self.fields.x, self.fields.y, self.fields.w, self.fields.h) = (x, y, w, h);
        if edited && new_rect != rect {
            let patch = ObjectPatch { rect: Some(new_rect), ..Default::default() };
            self.act_run(format!("geometry:{id}"), Command::SetObject { id, patch });
        }
        if let Some(cols) = columns {
            let mut text = std::mem::take(&mut self.fields.columns);
            let (changed, focused) = labeled_field_state(ui, "Columns", &mut text);
            if changed {
                if let Some(n) = text.trim().parse::<u32>().ok().filter(|n| (1..=20).contains(n))
                    && n != cols
                {
                    let patch = TextFramePatch { columns: Some(n), ..Default::default() };
                    self.act_run(format!("columns:{id}"), Command::SetTextFrame { id, patch });
                }
            } else if !focused {
                text = cols.to_string();
            }
            self.fields.columns = text;
        }
    }
}
