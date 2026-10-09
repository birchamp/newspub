//! Turns the intermediate form into a newpub document: pages, frames chained per story, formatted text, pictures,
//! shapes, tables and masters.

use crate::ImportReport;
use crate::PictureDecoder;
use crate::ir::{Blip, BlipKind, CharFmt, Dash, Geom, LineSpec, Node, PageIr, ParaFmt, Pub, QStory, Rgb, Shp, Text};
use newpub_core::{
    Align, Baseline, Caps, CharAttrs, CharSpan, Color, Command, CropFrac, Document, Id, Insets, Length, LineSpacing,
    ObjectKind, PageSetup, ParaAttrs, Rect, ShapeKind, Stroke, TabAlign, TabStop, TextEffects, TextOutline, TextShadow,
    VAlign, Wrap, WrapMode,
};
use std::collections::{BTreeSet, HashMap};

const EMU_PER_PT: f64 = 12700.0;
/// Pages, objects and nesting beyond these limits are dropped with a warning.
const MAX_PAGES: usize = 500;
const MAX_OBJECTS: usize = 5000;
const MAX_PAGE_PT: f64 = 14400.0;

fn pt(emu: i64) -> f64 {
    emu as f64 / EMU_PER_PT
}

fn color(c: Rgb) -> Color {
    Color::rgb(c.0, c.1, c.2)
}

fn stroke(l: &LineSpec) -> Stroke {
    Stroke {
        color: color(l.color),
        width: Length(pt(l.width_emu).max(0.25)),
        dash: match l.dash {
            Dash::Solid => newpub_core::Dash::Solid,
            Dash::Dashed => newpub_core::Dash::Dash,
            Dash::Dotted => newpub_core::Dash::Dot,
            Dash::Mixed => newpub_core::Dash::DashDot,
            Dash::LongDashed => newpub_core::Dash::LongDash,
        },
        cap: newpub_core::LineCap::Butt,
        join: newpub_core::LineJoin::Miter,
    }
}

// ---- text ----

/// BCP-47 tag for the Windows language ids that occur in Publisher files.
fn lang_tag(lcid: u32) -> Option<String> {
    let tag = match lcid {
        0x0409 => "en-US",
        0x0809 => "en-GB",
        0x0c09 => "en-AU",
        0x1009 => "en-CA",
        0x1409 => "en-NZ",
        0x1809 => "en-IE",
        0x0407 => "de-DE",
        0x0807 => "de-CH",
        0x0c07 => "de-AT",
        0x040c => "fr-FR",
        0x0c0c => "fr-CA",
        0x080c => "fr-BE",
        0x0c0a | 0x040a => "es-ES",
        0x080a => "es-MX",
        0x0410 => "it-IT",
        0x0413 => "nl-NL",
        0x0816 => "pt-PT",
        0x0416 => "pt-BR",
        0x041d => "sv-SE",
        0x0414 => "nb-NO",
        0x0406 => "da-DK",
        0x040b => "fi-FI",
        0x0415 => "pl-PL",
        0x0419 => "ru-RU",
        0x0405 => "cs-CZ",
        0x040e => "hu-HU",
        0x0408 => "el-GR",
        0x041f => "tr-TR",
        0x0411 => "ja-JP",
        0x0412 => "ko-KR",
        0x0804 => "zh-CN",
        0x0404 => "zh-TW",
        0x0400 | 0x0000 => return None,
        _ => return None,
    };
    Some(tag.to_string())
}

struct Styler<'a> {
    text: &'a Text,
    palette: &'a [Rgb],
}

impl Styler<'_> {
    fn char_attrs(&self, f: &CharFmt, d: &CharFmt) -> CharAttrs {
        let fonts = &self.text.fonts;
        let font =
            f.font.or(d.font).and_then(|i| fonts.get(i)).filter(|n| !n.is_empty()).or_else(|| fonts.first()).cloned();
        let mut a = CharAttrs {
            font,
            size: f.size_emu.or(d.size_emu).map(|e| Length((pt(e) * 100.0).round() / 100.0)),
            bold: Some(f.bold ^ d.bold),
            italic: Some(f.italic ^ d.italic),
            underline: Some(f.underline.or(d.underline).is_some()),
            ..Default::default()
        };
        let col = f.color.or(d.color);
        a.color = col.and_then(|c| crate::quill::text_color(self.text, c, self.palette)).map(color);
        match f.script {
            1 => a.baseline = Some(Baseline::Superscript),
            2 => a.baseline = Some(Baseline::Subscript),
            _ => {}
        }
        if f.small_caps ^ d.small_caps {
            a.caps = Some(Caps::SmallCaps);
        } else if f.all_caps ^ d.all_caps {
            a.caps = Some(Caps::AllCaps);
        }
        if let Some(s) = f.scale.or(d.scale) {
            let pct = f64::from(s) / 10.0;
            if (1.0..=1000.0).contains(&pct) && (pct - 100.0).abs() > 0.01 {
                a.scale = Some(pct);
            }
        }
        a.lang = f.lcid.or(d.lcid).and_then(lang_tag);
        let mut fx = TextEffects::default();
        if f.shadow ^ d.shadow {
            fx.shadow = Some(TextShadow { dx: 1.0, dy: 1.0, blur: 0.0, color: Color::rgb(128, 128, 128) });
        }
        if f.outline ^ d.outline {
            fx.outline = Some(TextOutline { width: 0.5, color: a.color.clone().unwrap_or(Color::BLACK) });
        }
        fx.emboss = f.emboss ^ d.emboss;
        fx.engrave = f.engrave ^ d.engrave;
        if !fx.is_empty() {
            a.effects = Some(fx);
        }
        a
    }

    fn para_attrs(&self, f: &ParaFmt, d: &ParaFmt) -> ParaAttrs {
        let mut a = ParaAttrs::default();
        let align = f.align.or(d.align).unwrap_or(0);
        a.align = Some(match align {
            1 => Align::Right,
            2 => Align::Center,
            6 => Align::Justify,
            _ => Align::Left,
        });
        if let Some(ls) = f.line_spacing.or(d.line_spacing) {
            // Odd values are exact distances (1/8 EMU); values with bit 1 are multiples of single spacing.
            if ls & 1 == 1 {
                let points = f64::from(ls - 1) / 8.0 / EMU_PER_PT;
                if points > 0.0 {
                    a.line_spacing = Some(LineSpacing::Exactly(Length(points)));
                }
            } else if ls & 2 == 2 {
                let m = f64::from(ls - 2) / 1_219_200.0;
                if m > 0.0 {
                    a.line_spacing = Some(LineSpacing::Multiple(m));
                }
            }
        }
        let len = |v: Option<i64>, w: Option<i64>| v.or(w).map(|e| Length(pt(e)));
        a.space_before = len(f.space_before, d.space_before);
        a.space_after = len(f.space_after, d.space_after);
        a.indent_left = len(f.left_indent, d.left_indent);
        a.indent_right = len(f.right_indent, d.right_indent);
        a.indent_first = len(f.first_indent, d.first_indent);
        let tabs = if f.tabs.is_empty() { &d.tabs } else { &f.tabs };
        if !tabs.is_empty() {
            a.tabs = Some(
                tabs.iter().map(|&t| TabStop { pos: Length(pt(t)), align: TabAlign::Left, leader: None }).collect(),
            );
        }
        a
    }

    /// Fills `story` from `q.chars[range]`; the last character of the range is dropped when it is a paragraph mark.
    fn fill_story(&self, story: &mut newpub_core::Story, q: &QStory, range: std::ops::Range<usize>) {
        let end = range.end.min(q.chars.len());
        let start = range.start.min(end);
        let units = &q.chars[start..end];
        let mut text = String::new();
        let mut spans: Vec<CharSpan> = Vec::new();
        let mut paras: Vec<ParaAttrs> = Vec::new();
        let none_c = CharFmt::default();
        let none_p = ParaFmt::default();
        let (mut cr, mut pr) = (0usize, 0usize);
        let n = units.len();
        let mut i = 0;
        // Paragraph attributes pending until the paragraph's mark (or the end of the range) is reached.
        while i < n {
            let g = start + i;
            while cr < q.char_runs.len() && q.char_runs[cr].0 <= g {
                cr += 1;
            }
            while pr < q.para_runs.len() && q.para_runs[pr].0 <= g {
                pr += 1;
            }
            let pfmt = q.para_runs.get(pr).map_or(&none_p, |r| &r.1);
            let dstyle = pfmt.style_index.unwrap_or(0);
            let dchar = self.text.default_chars.get(dstyle).unwrap_or(&none_c);
            let dpara = self.text.default_paras.get(dstyle).unwrap_or(&none_p);
            let cfmt = q.char_runs.get(cr).map_or(&none_c, |r| &r.1);
            let u = units[i];
            let mut step = 1;
            let ch = if (0xd800..0xdc00).contains(&u) && i + 1 < n && (0xdc00..0xe000).contains(&units[i + 1]) {
                step = 2;
                char::decode_utf16([u, units[i + 1]]).next().and_then(Result::ok)
            } else {
                char::from_u32(u32::from(u))
            };
            let is_mark = u == 0x0d;
            let last = i + step >= n;
            if is_mark && last {
                // The paragraph mark that ends the story (or cell) is not text.
                paras.push(self.para_attrs(pfmt, dpara));
                i += step;
                continue;
            }
            let out = match ch {
                Some('\r') => Some('\n'),
                Some('\u{b}') => Some(newpub_core::LINE_SEP),
                Some('\t') => Some('\t'),
                Some(c) if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{fffc}' => None,
                Some(c) => Some(c),
                None => Some('\u{fffd}'),
            };
            if let Some(c) = out {
                text.push(c);
                let attrs = self.char_attrs(cfmt, dchar);
                match spans.last_mut() {
                    Some(s) if s.attrs == attrs => s.len += 1,
                    _ => spans.push(CharSpan { len: 1, attrs }),
                }
                if c == '\n' {
                    paras.push(self.para_attrs(pfmt, dpara));
                }
            }
            i += step;
            if last && !is_mark {
                paras.push(self.para_attrs(pfmt, dpara));
            }
        }
        if paras.is_empty() {
            paras.push(ParaAttrs::default());
        }
        story.text = text;
        story.chars = spans;
        story.paras = paras;
        story.normalize();
    }
}

// ---- document ----

/// A text frame waiting for its story.
struct PendingFrame {
    chain_pos: u32,
    seq: u32,
    frame: Id,
    story: Id,
}

struct Builder<'a> {
    doc: Document,
    p: &'a Pub,
    report: &'a mut ImportReport,
    decode: PictureDecoder<'a>,
    assets: HashMap<usize, Option<Id>>,
    frames: Vec<(u32, Vec<PendingFrame>)>,
    tables: Vec<(Id, Vec<Id>, u32)>,
    objects: usize,
    over_limit: bool,
    skipped: HashMap<String, usize>,
    fonts_used: BTreeSet<String>,
}

/// Where new objects go.
#[derive(Clone, Copy)]
enum Target {
    Page(usize),
    Master(Id),
}

impl Builder<'_> {
    fn skip(&mut self, what: impl Into<String>) {
        *self.skipped.entry(what.into()).or_insert(0) += 1;
    }

    fn rect_of(&self, s: &Shp) -> Rect {
        let w = self.doc.setup.width.0;
        let h = self.doc.setup.height.0;
        let x0 = w / 2.0 + pt(s.rect[0]);
        let y0 = h / 2.0 + pt(s.rect[1]);
        Rect::new(x0, y0, pt(s.rect[2] - s.rect[0]).max(0.0), pt(s.rect[3] - s.rect[1]).max(0.0))
    }

    fn asset_for(&mut self, blip_index: usize) -> Option<Id> {
        if let Some(a) = self.assets.get(&blip_index) {
            return *a;
        }
        let id = blip_index
            .checked_sub(1)
            .and_then(|i| self.p.blips.get(i))
            .and_then(|b| b.as_ref())
            .and_then(|b| self.picture_asset(b, blip_index));
        self.assets.insert(blip_index, id);
        id
    }

    fn picture_asset(&mut self, b: &Blip, index: usize) -> Option<Id> {
        let bytes: Vec<u8> = match b.kind {
            BlipKind::Dib => dib_to_bmp(&b.data)?,
            BlipKind::Other => return None,
            _ => b.data.clone(),
        };
        let pic = (self.decode)(&bytes)?;
        let name = format!("picture{index}");
        Some(self.doc.add_asset(&name, &pic.mime, std::sync::Arc::from(pic.bytes), pic.px_w, pic.px_h))
    }

    fn count(&mut self) -> bool {
        if self.objects >= MAX_OBJECTS {
            if !self.over_limit {
                self.over_limit = true;
            }
            self.skip("objects beyond the first 5000");
            return false;
        }
        self.objects += 1;
        true
    }

    fn place_nodes(&mut self, nodes: &[Node], target: Target) -> Vec<Id> {
        let mut ids = Vec::new();
        for n in nodes {
            match n {
                Node::Shape(s) => ids.extend(self.place_shape(s, target)),
                Node::Group { kids, .. } => {
                    let inner = self.place_nodes(kids, target);
                    if inner.len() >= 2
                        && let Ok(a) = self.doc.apply(&Command::Group { ids: inner.clone() })
                    {
                        ids.extend(a.created);
                        continue;
                    }
                    ids.extend(inner);
                }
            }
        }
        ids
    }

    fn page_arg(t: Target) -> (Option<usize>, Option<Id>) {
        match t {
            Target::Page(i) => (Some(i), None),
            Target::Master(m) => (None, Some(m)),
        }
    }

    fn place_shape(&mut self, s: &Shp, target: Target) -> Option<Id> {
        let (page, master) = Self::page_arg(target);
        match &s.geom {
            Geom::WordArt => {
                self.skip("WordArt objects");
                return None;
            }
            Geom::Unsupported(t) => {
                self.skip(format!("autoshapes of type {t}"));
                return None;
            }
            _ => {}
        }
        if !self.count() {
            return None;
        }
        let rect = self.rect_of(s);
        let id = match &s.geom {
            Geom::TextBox => self.place_text_box(s, rect, page, master)?,
            Geom::Table => self.place_table(s, rect, page, master)?,
            Geom::Picture => self.place_picture(s, rect, page, master)?,
            _ => self.place_autoshape(s, rect, page, master)?,
        };
        if let Some(o) = self.doc.objects.get_mut(&id) {
            o.rotation = s.rotation;
            o.flip_h = s.flip_h;
            o.flip_v = s.flip_v;
        }
        Some(id)
    }

    fn place_text_box(&mut self, s: &Shp, rect: Rect, page: Option<usize>, master: Option<Id>) -> Option<Id> {
        let (frame, story) = self.doc.create_text_frame(page, master, rect).ok()?;
        if let Some(o) = self.doc.objects.get_mut(&frame)
            && let ObjectKind::Text(t) = &mut o.kind
        {
            t.insets = Insets {
                left: Length(pt(s.insets[0])),
                top: Length(pt(s.insets[1])),
                right: Length(pt(s.insets[2])),
                bottom: Length(pt(s.insets[3])),
            };
            t.valign = match s.valign {
                1 => VAlign::Middle,
                2 => VAlign::Bottom,
                _ => VAlign::Top,
            };
            if s.columns > 1 {
                t.columns = s.columns;
                if s.column_gap > 0 {
                    t.gutter = Length(pt(s.column_gap));
                }
            }
            t.fill = s.fill.map(color);
            t.stroke = s.line.as_ref().map(stroke);
        }
        match s.text {
            Some(tb) => {
                let list = match self.frames.iter().position(|(id, _)| *id == tb.text_id) {
                    Some(i) => i,
                    None => {
                        self.frames.push((tb.text_id, Vec::new()));
                        self.frames.len() - 1
                    }
                };
                self.frames[list].1.push(PendingFrame { chain_pos: tb.chain_pos, seq: s.seq, frame, story });
            }
            None => {
                // A text box without a story: stays empty.
            }
        }
        Some(frame)
    }

    fn place_picture(&mut self, s: &Shp, rect: Rect, page: Option<usize>, master: Option<Id>) -> Option<Id> {
        let asset = s.image.and_then(|i| self.asset_for(i));
        let Some(asset) = asset else {
            self.skip("pictures in a format that cannot be decoded");
            return None;
        };
        let id = self.doc.apply(&Command::AddImage { page, master, rect, asset: Some(asset) }).ok()?.created[0];
        if let Some(o) = self.doc.objects.get_mut(&id) {
            o.wrap = Wrap { mode: WrapMode::None, distance: Length(0.0) };
            if let ObjectKind::Image(im) = &mut o.kind {
                im.crop = CropFrac { left: s.crop[0], top: s.crop[1], right: s.crop[2], bottom: s.crop[3] };
                im.stroke = s.line.as_ref().map(stroke);
            }
        }
        Some(id)
    }

    fn place_autoshape(&mut self, s: &Shp, rect: Rect, page: Option<usize>, master: Option<Id>) -> Option<Id> {
        let short = rect.w.min(rect.h);
        let path = |pts: &[[f64; 2]]| ShapeKind::Path { points: pts.to_vec(), closed: true };
        let kind = match &s.geom {
            Geom::Rect => ShapeKind::Rect,
            Geom::RoundRect => ShapeKind::RoundRect { radius: Length(short * s.corner) },
            Geom::Ellipse => ShapeKind::Ellipse,
            Geom::Line => ShapeKind::Line,
            Geom::Triangle => ShapeKind::Triangle,
            Geom::RightTriangle => path(&[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0]]),
            Geom::Diamond => path(&[[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]]),
            Geom::Polygon(5) => path(&[[0.5, 0.0], [1.0, 0.382], [0.809, 1.0], [0.191, 1.0], [0.0, 0.382]]),
            Geom::Polygon(6) => path(&[[0.25, 0.0], [0.75, 0.0], [1.0, 0.5], [0.75, 1.0], [0.25, 1.0], [0.0, 0.5]]),
            Geom::Polygon(8) => path(&[
                [0.293, 0.0],
                [0.707, 0.0],
                [1.0, 0.293],
                [1.0, 0.707],
                [0.707, 1.0],
                [0.293, 1.0],
                [0.0, 0.707],
                [0.0, 0.293],
            ]),
            Geom::Polygon(n) => ShapeKind::Polygon { sides: *n },
            Geom::Star(5) => ShapeKind::Star { points: 5, inner: 0.382 },
            Geom::Star(n) => ShapeKind::Star { points: *n, inner: 0.75 },
            Geom::Arrow => ShapeKind::Arrow,
            _ => ShapeKind::Rect,
        };
        let fill = if matches!(s.geom, Geom::Line) { None } else { s.fill.map(color) };
        let line = s.line.as_ref().map(stroke);
        let id = self.doc.apply(&Command::AddShape { page, master, rect, kind, fill, stroke: line }).ok()?.created[0];
        if let Some(tb) = s.text {
            // A shape that holds text: the story lives inside the shape.
            if let Ok(a) = self.doc.apply(&Command::AddShapeText { id })
                && let Some(&story) = a.created.first()
            {
                let list = match self.frames.iter().position(|(t, _)| *t == tb.text_id) {
                    Some(i) => i,
                    None => {
                        self.frames.push((tb.text_id, Vec::new()));
                        self.frames.len() - 1
                    }
                };
                self.frames[list].1.push(PendingFrame { chain_pos: tb.chain_pos, seq: s.seq, frame: id, story });
            }
        }
        Some(id)
    }

    fn place_table(&mut self, s: &Shp, rect: Rect, page: Option<usize>, master: Option<Id>) -> Option<Id> {
        let t = s.table.as_ref()?;
        let applied = self.doc.apply(&Command::AddTable { page, master, rect, rows: t.rows, cols: t.cols }).ok()?;
        let id = applied.created[0];
        let cell_stories: Vec<Id> = applied.created[1..].to_vec();
        if let Some(o) = self.doc.objects.get_mut(&id)
            && let ObjectKind::Table(tb) = &mut o.kind
        {
            for (c, w) in tb.col_widths.iter_mut().zip(&t.col_widths) {
                *c = Length(pt(*w).max(1.0));
            }
            for (r, h) in tb.row_heights.iter_mut().zip(&t.row_heights) {
                *r = Length(pt(*h).max(1.0));
            }
            tb.min_row_heights = tb.row_heights.clone();
            o.rect.w = tb.col_widths.iter().map(|w| w.0).sum();
            o.rect.h = tb.row_heights.iter().map(|h| h.0).sum();
            // Merged cells.
            let cols = tb.cols();
            let mut anchors = Vec::new();
            for &(r0, r1, c0, c1) in &t.cells {
                let (r1, c1) = (r1.max(r0).min(t.rows - 1), c1.max(c0).min(t.cols - 1));
                if r0 >= t.rows || c0 >= t.cols {
                    anchors.push(None);
                    continue;
                }
                anchors.push(Some(r0 * cols + c0));
                if r1 > r0 || c1 > c0 {
                    for r in r0..=r1 {
                        for c in c0..=c1 {
                            if (r, c) != (r0, c0) {
                                tb.cells[r * cols + c].covered = true;
                            }
                        }
                    }
                    let cell = &mut tb.cells[r0 * cols + c0];
                    cell.rowspan = (r1 - r0 + 1) as u32;
                    cell.colspan = (c1 - c0 + 1) as u32;
                }
            }
            let ordered: Vec<Id> =
                anchors.iter().map(|a| a.and_then(|i| cell_stories.get(i).copied()).unwrap_or(Id(0))).collect();
            self.tables.push((id, ordered, s.text.map_or(u32::MAX, |t| t.text_id)));
        }
        if s.text.is_none() {
            self.skip("tables without text");
        }
        Some(id)
    }

    /// Distributes the Quill stories over the frames, cells and shapes that show them.
    fn fill_text(&mut self) {
        let styler = Styler { text: &self.p.text, palette: &self.p.palette };
        let by_id: HashMap<u32, &QStory> = self.p.text.stories.iter().map(|q| (q.text_id, q)).collect();
        let mut used: BTreeSet<u32> = BTreeSet::new();
        for (text_id, list) in &mut self.frames {
            list.sort_by_key(|f| (f.chain_pos, f.seq));
            let Some(q) = by_id.get(text_id) else {
                self.report.warnings.push(format!("text id {text_id} has no text; its frames stay empty"));
                continue;
            };
            used.insert(*text_id);
            let Some(head) = list.first() else { continue };
            let head_story = head.story;
            if let Ok(st) = self.doc.story_mut(head_story) {
                styler.fill_story(st, q, 0..q.chars.len());
            }
            // Link the rest of the chain to the head story.
            for f in list.iter().skip(1) {
                if f.story == head_story {
                    continue;
                }
                let is_text = matches!(self.doc.objects.get(&f.frame).map(|o| &o.kind), Some(ObjectKind::Text(_)));
                if !is_text {
                    // Shapes holding text each keep their own copy of the story.
                    if let Ok(st) = self.doc.story_mut(f.story) {
                        styler.fill_story(st, q, 0..q.chars.len());
                    }
                    continue;
                }
                let last =
                    self.doc.stories.get(&head_story).and_then(|s| s.frames.last().copied()).unwrap_or(list[0].frame);
                let _ = last;
                self.doc.stories.remove(&f.story);
                if let Some(st) = self.doc.stories.get_mut(&head_story) {
                    st.frames.push(f.frame);
                }
                if let Some(o) = self.doc.objects.get_mut(&f.frame)
                    && let ObjectKind::Text(t) = &mut o.kind
                {
                    t.story = head_story;
                }
            }
        }
        // Tables: one story per cell.
        for (table, cells, text_id) in std::mem::take(&mut self.tables) {
            let Some(q) = by_id.get(&text_id) else { continue };
            used.insert(text_id);
            let Some(ends) = &q.cell_ends else {
                self.skip("tables whose cell boundaries are unknown");
                continue;
            };
            let _ = table;
            let mut start = 0usize;
            for (n, &end) in ends.iter().enumerate() {
                let range = start..end.max(start);
                start = end + 1;
                if let Some(&sid) = cells.get(n)
                    && sid != Id(0)
                    && let Ok(st) = self.doc.story_mut(sid)
                {
                    styler.fill_story(st, q, range);
                }
            }
        }
        let unused = self.p.text.stories.iter().filter(|q| !used.contains(&q.text_id) && !q.chars.is_empty()).count();
        if unused > 0 {
            *self.skipped.entry("stories that no text box shows".into()).or_insert(0) += unused;
        }
        for st in self.doc.stories.values() {
            for (_, a) in st.runs() {
                if let Some(f) = &a.font {
                    self.fonts_used.insert(f.clone());
                }
            }
        }
    }
}

/// Wraps a device-independent bitmap in a bitmap file header.
fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let header = u32::from_le_bytes(dib.get(0..4)?.try_into().ok()?) as usize;
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let colors_used = u32::from_le_bytes(dib.get(32..36)?.try_into().ok()?) as usize;
    let palette = if colors_used > 0 {
        colors_used
    } else if bits <= 8 {
        1usize << bits
    } else {
        0
    };
    let offset = 14usize.checked_add(header)?.checked_add(palette.checked_mul(4)?)?;
    let total = dib.len().checked_add(14)?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

pub(crate) fn build(p: &Pub, decode: PictureDecoder, report: &mut ImportReport) -> Result<Document, String> {
    let (w, h) = (p.width_emu as f64 / EMU_PER_PT, p.height_emu as f64 / EMU_PER_PT);
    if !(1.0..=MAX_PAGE_PT).contains(&w) || !(1.0..=MAX_PAGE_PT).contains(&h) {
        return Err(format!("the page size {w:.0} x {h:.0} pt is not valid"));
    }
    let setup = PageSetup {
        width: Length(w),
        height: Length(h),
        margins: Insets::uniform(36.0f64.min(w / 4.0).min(h / 4.0)),
        facing: false,
        bleed: Length(0.0),
    };
    let npages = p.pages.len().clamp(1, MAX_PAGES);
    if p.pages.len() > MAX_PAGES {
        report
            .warnings
            .push(format!("{} pages beyond the first {MAX_PAGES} were not imported", p.pages.len() - MAX_PAGES));
    }
    let doc = Document::new(setup, npages);
    let mut b = Builder {
        doc,
        p,
        report,
        decode,
        assets: HashMap::new(),
        frames: Vec::new(),
        tables: Vec::new(),
        objects: 0,
        over_limit: false,
        skipped: HashMap::new(),
        fonts_used: BTreeSet::new(),
    };
    // Pages first, so that the stories of the pages get the lowest ids; masters after.
    for (i, page) in p.pages.iter().take(npages).enumerate() {
        b.place_nodes(&page.nodes, Target::Page(i));
    }
    let mut master_ids: Vec<Option<Id>> = Vec::new();
    for (i, m) in p.masters.iter().enumerate() {
        if m.nodes.is_empty() {
            master_ids.push(None);
            continue;
        }
        let name = format!("Master {}", (b'A' + (i % 26) as u8) as char);
        match b.doc.apply(&Command::AddMaster { name }) {
            Ok(a) => {
                let mid = a.created[0];
                b.place_nodes(&m.nodes, Target::Master(mid));
                master_ids.push(Some(mid));
            }
            Err(_) => master_ids.push(None),
        }
    }
    for (i, page) in p.pages.iter().take(npages).enumerate() {
        if let Some(Some(mid)) = page.master.map(|m| master_ids.get(m).copied().flatten()) {
            let _ = b.doc.apply(&Command::ApplyMaster { pages: Some(vec![i]), master: Some(mid) });
        }
    }
    b.fill_text();
    let _: Option<&PageIr> = None;

    let Builder { doc, skipped, fonts_used, frames, .. } = b;
    report.frames_placed = doc.objects.values().filter(|o| matches!(o.kind, ObjectKind::Text(_))).count();
    report.fonts = fonts_used.into_iter().collect();
    let _ = frames;
    let mut skipped: Vec<(String, usize)> = skipped.into_iter().collect();
    for (k, v) in &p.skipped {
        skipped.push(((*k).to_string(), *v));
    }
    skipped.sort();
    for (what, n) in skipped {
        report.warnings.push(format!("{n} {what} not imported"));
    }
    Ok(doc)
}
