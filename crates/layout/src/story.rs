//! Story flow: lines through columns and linked frames, wrapping around objects.

use crate::para::{self, G, Para, RunStyle, Seg};
use crate::{Decoration, DecorationKind, FontStore, FrameLayout, GlyphRun, Line, PGlyph, StoryLayout};
use newpub_core::{
    Align, Document, Id, LineSpacing, ListStyle, Object, ObjectKind, Rect, ResolvedPara, ShapeKind, Story, TabAlign,
    TextFrame, VAlign, WrapMode,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Narrowest line piece worth filling when text wraps around objects.
const MIN_PIECE: f64 = 18.0;
const EPS: f64 = 1e-6;
/// Space between a drop cap and the text beside it, points.
const DROP_GAP: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
enum ExKind {
    Box,
    Ellipse,
    Band,
}

#[derive(Clone, Copy, Debug)]
struct Exclusion {
    r: Rect,
    kind: ExKind,
}

impl Exclusion {
    /// Horizontal extent blocked within the band [y0, y1], if any.
    fn blocked(&self, y0: f64, y1: f64) -> Option<(f64, f64)> {
        let r = &self.r;
        if r.y >= y1 || r.bottom() <= y0 {
            return None;
        }
        match self.kind {
            ExKind::Band => Some((f64::NEG_INFINITY, f64::INFINITY)),
            ExKind::Box => Some((r.x, r.right())),
            ExKind::Ellipse => {
                let (cx, cy, rx, ry) = (r.x + r.w / 2.0, r.y + r.h / 2.0, r.w / 2.0, r.h / 2.0);
                let yn = cy.clamp(y0, y1);
                let t = ((yn - cy) / ry.max(EPS)).powi(2);
                let hw = rx * (1.0 - t).max(0.0).sqrt();
                (hw > 0.0).then_some((cx - hw, cx + hw))
            }
        }
    }
}

#[derive(Debug)]
struct FrameGeom {
    id: Id,
    /// Page y of the frame's top (for the baseline grid); None when not on a page grid (table cells).
    origin_y: Option<f64>,
    tf: TextFrame,
    /// Columns as (x0, x1) frame-local.
    cols: Vec<(f64, f64)>,
    top: f64,
    bottom: f64,
    ex: Vec<Exclusion>,
}

struct Slot {
    frame: usize,
    col: usize,
    y: f64,
    pieces: Vec<(f64, f64)>,
}

struct Flow {
    frames: Vec<FrameGeom>,
    fi: usize,
    ci: usize,
    y: f64,
    /// Nothing placed yet in the current column.
    col_empty: bool,
}

impl Flow {
    fn done(&self) -> bool {
        self.fi >= self.frames.len()
    }

    fn next_column(&mut self) {
        self.ci += 1;
        if self.ci >= self.frames[self.fi].cols.len() {
            self.ci = 0;
            self.fi += 1;
        }
        if let Some(f) = self.frames.get(self.fi) {
            self.y = f.top;
        }
        self.col_empty = true;
    }

    /// Finds room for a line of height `h`, moving through columns and frames as needed.
    fn slot(&mut self, h: f64) -> Option<Slot> {
        while !self.done() {
            let f = &self.frames[self.fi];
            let (c0, c1) = f.cols[self.ci];
            if self.y + h > f.bottom + 0.01 {
                self.next_column();
                continue;
            }
            let (y0, y1) = (self.y, self.y + h);
            let mut pieces = vec![(c0, c1)];
            let mut blocking_bottom = f64::INFINITY;
            for e in &f.ex {
                if let Some((b0, b1)) = e.blocked(y0, y1) {
                    if b1 <= c0 || b0 >= c1 {
                        continue;
                    }
                    blocking_bottom = blocking_bottom.min(e.r.bottom());
                    pieces = pieces
                        .into_iter()
                        .flat_map(|(a, b)| {
                            let mut v = vec![];
                            if b0 > a {
                                v.push((a, b0.min(b)));
                            }
                            if b1 < b {
                                v.push((b1.max(a), b));
                            }
                            v
                        })
                        .collect();
                }
            }
            pieces.retain(|(a, b)| b - a >= MIN_PIECE.min(c1 - c0));
            if pieces.is_empty() {
                // Skip below the obstruction.
                self.y = if blocking_bottom.is_finite() && blocking_bottom > self.y {
                    blocking_bottom
                } else {
                    self.y + 1.0
                };
                continue;
            }
            return Some(Slot { frame: self.fi, col: self.ci, y: self.y, pieces });
        }
        None
    }
}

fn frame_geom(doc: &Document, id: Id) -> Option<FrameGeom> {
    let obj = doc.objects.get(&id)?;
    let w = obj.rect.w;
    let h = obj.rect.h;
    // Shapes holding text (SH-06): the text area is the shape's inscribed box, text centred vertically.
    let shape_tf;
    let tf = match &obj.kind {
        ObjectKind::Text(tf) => tf,
        ObjectKind::Shape(sh) => {
            let story = sh.story?;
            let mut t = TextFrame::new(story);
            t.valign = VAlign::Middle;
            let (fx, fy): (f64, f64) = match sh.kind {
                ShapeKind::Ellipse | ShapeKind::Star { .. } | ShapeKind::Polygon { .. } => (0.1464, 0.1464),
                ShapeKind::Triangle => (0.25, 0.5),
                _ => (0.0, 0.0),
            };
            let pad = 5.76;
            t.insets = newpub_core::Insets {
                left: newpub_core::Length(w * fx + pad),
                right: newpub_core::Length(w * fx + pad),
                top: newpub_core::Length(h * fy.min(0.25) + pad),
                bottom: newpub_core::Length(h * fx + pad),
            };
            shape_tf = t;
            &shape_tf
        }
        _ => return None,
    };
    let ins = tf.insets;
    let content = Rect::new(0.0, 0.0, w, h).inset(ins.left.0, ins.top.0, ins.right.0, ins.bottom.0);
    let n = tf.columns.max(1) as f64;
    let gutter = tf.gutter.0.max(0.0);
    let cw = ((content.w - gutter * (n - 1.0)) / n).max(1.0);
    if tf.vertical {
        // Vertical text: one character per line, stacked top to bottom (a hair-wide column forces a break after every glyph).
        return Some(FrameGeom {
            id,
            origin_y: None,
            tf: tf.clone(),
            cols: vec![(content.x, content.x + 0.01)],
            top: content.y,
            bottom: content.bottom(),
            ex: vec![],
        });
    }
    let cols = (0..tf.columns.max(1)).map(|i| {
        let x0 = content.x + i as f64 * (cw + gutter);
        (x0, x0 + cw)
    });
    Some(FrameGeom {
        id,
        origin_y: (obj.rotation == 0.0).then_some(obj.rect.y),
        tf: tf.clone(),
        cols: cols.collect(),
        top: content.y,
        bottom: content.bottom(),
        ex: exclusions(doc, obj),
    })
}

/// Objects above the frame on the same page that push text away, in frame-local coords.
fn exclusions(doc: &Document, frame: &Object) -> Vec<Exclusion> {
    if frame.rotation != 0.0 {
        return vec![];
    }
    let Some(pi) = doc.page_of(frame.id) else { return vec![] };
    let list = &doc.pages[pi].objects;
    let top_id = top_level(doc, frame.id);
    let Some(pos) = list.iter().position(|x| *x == top_id) else { return vec![] };
    let mut out = vec![];
    for oid in &list[pos + 1..] {
        let Some(o) = doc.objects.get(oid) else { continue };
        if o.wrap.mode == WrapMode::None || o.hidden {
            continue;
        }
        let d = o.wrap.distance.0;
        let r =
            Rect::new(o.rect.x - frame.rect.x - d, o.rect.y - frame.rect.y - d, o.rect.w + 2.0 * d, o.rect.h + 2.0 * d);
        let kind = match (o.wrap.mode, &o.kind) {
            (WrapMode::TopBottom, _) => ExKind::Band,
            (WrapMode::Tight | WrapMode::Through, ObjectKind::Shape(s)) if s.kind == ShapeKind::Ellipse => {
                ExKind::Ellipse
            }
            _ => ExKind::Box,
        };
        out.push(Exclusion { r, kind });
    }
    out
}

fn top_level(doc: &Document, mut id: Id) -> Id {
    while let Some(p) = doc.objects.get(&id).and_then(|o| o.parent) {
        id = p;
    }
    id
}

/// Line metrics for a set of styles: (height, ascent-from-top).
fn line_metrics(styles: &[&RunStyle], ls: LineSpacing) -> (f64, f64) {
    let asc = styles.iter().map(|s| s.ascent).fold(0.0, f64::max);
    let desc = styles.iter().map(|s| s.descent).fold(0.0, f64::max);
    let gap = styles.iter().map(|s| s.gap).fold(0.0, f64::max);
    let natural = asc + desc + gap;
    match ls {
        LineSpacing::Multiple(m) => {
            let h = natural * m.max(0.1);
            (h, h - desc)
        }
        LineSpacing::Exactly(x) => (x.0, x.0 - desc),
        LineSpacing::AtLeast(x) => {
            let h = natural.max(x.0);
            (h, h - desc)
        }
    }
}

/// What a filled line contains.
struct Fill {
    segs: Vec<Seg>,
    /// Paragraph ends on this line.
    ends_para: bool,
    hard: bool,
    hyphen: bool,
}

fn next_tab(x: f64, rp: &ResolvedPara) -> (f64, TabAlign, Option<char>) {
    for t in &rp.tabs {
        if t.pos.0 > x + 0.01 {
            return (t.pos.0, t.align, t.leader);
        }
    }
    let step = 36.0;
    (((x / step).floor() + 1.0) * step, TabAlign::Left, None)
}

/// Greedily fills a line of `width` (starting at `indent` within the slot for tab purposes).
fn fill_line(p: &mut Para, story: &Story, width: f64, indent: f64, rp: &ResolvedPara, may_force: bool) -> Fill {
    let mut taken: Vec<Seg> = vec![];
    let mut x = 0.0;
    let mut hyphen = false;
    let mut hard = false;
    while let Some(mut seg) = p.segs.pop_front() {
        if seg.tab {
            let (stop, align, leader) = next_tab(indent + x, rp);
            let mut w = (stop - indent - x).max(0.0);
            if align != TabAlign::Left {
                // Width of the text up to the next tab or line end.
                let mut follow = 0.0;
                let mut before_dec = None;
                for s in p.segs.iter() {
                    if s.tab || s.hard_break {
                        if !s.tab {
                            follow += s.width() - s.trail();
                        }
                        break;
                    }
                    if align == TabAlign::Decimal && before_dec.is_none() {
                        let mut acc = 0.0;
                        for g in &s.glyphs {
                            if g.decimal {
                                before_dec = Some(follow + acc);
                                break;
                            }
                            acc += g.adv;
                        }
                    }
                    follow += s.width();
                }
                let shift = match align {
                    TabAlign::Right => follow,
                    TabAlign::Center => follow / 2.0,
                    TabAlign::Decimal => before_dec.unwrap_or(follow),
                    TabAlign::Left => 0.0,
                };
                w = (w - shift).max(0.0);
            }
            if x + w > width + EPS && !taken.is_empty() {
                p.segs.push_front(seg);
                break;
            }
            if let Some(g) = seg.glyphs.first_mut() {
                g.adv = w;
                g.leader = leader;
            }
            x += w;
            let hb = seg.hard_break;
            taken.push(seg);
            if hb {
                hard = true;
                break;
            }
            continue;
        }
        let w = seg.width();
        let trail = seg.trail();
        let extra_h = if seg.soft_hyphen {
            p.styles[seg.glyphs.last().map(|g| g.style).unwrap_or(0)].hyphen.map(|h| h.1).unwrap_or(0.0)
        } else {
            0.0
        };
        if x + w - trail + extra_h <= width + EPS {
            x += w;
            let hb = seg.hard_break;
            taken.push(seg);
            if hb {
                hard = true;
                break;
            }
            continue;
        }
        // Does not fit. Try hyphenation.
        if rp.hyphenate
            && !taken.is_empty()
            && width - x >= 0.0
            && let Some((a, b)) = split_hyphen(p, story, &seg, width - x, rp)
        {
            taken.push(a);
            p.segs.push_front(b);
            hyphen = true;
            break;
        }
        if taken.is_empty() && may_force {
            // Emergency break: as many glyphs as fit (at least one).
            let mut acc = 0.0;
            let mut n = 0;
            for g in &seg.glyphs {
                if acc + g.adv > width + EPS && n > 0 {
                    break;
                }
                acc += g.adv;
                n += 1;
            }
            if n < seg.glyphs.len() {
                let rest: Vec<G> = seg.glyphs.split_off(n);
                let cut = rest[0].chars.start;
                let tail =
                    Seg { glyphs: rest, start: cut, end: seg.end, hard_break: seg.hard_break, ..Default::default() };
                seg.end = cut;
                seg.hard_break = false;
                p.segs.push_front(tail);
            }
            taken.push(seg);
            break;
        }
        p.segs.push_front(seg);
        break;
    }
    // A break after a soft hyphen shows a hyphen.
    if !hyphen && !p.segs.is_empty() && taken.last().map(|s| s.soft_hyphen).unwrap_or(false) {
        if let Some(s) = taken.last_mut() {
            s.add_hyphen = true;
        }
        hyphen = true;
    }
    if hyphen && let Some(s) = taken.last_mut() {
        s.add_hyphen = true;
    }
    let ends_para = p.segs.is_empty();
    Fill { segs: taken, ends_para, hard, hyphen }
}

/// Splits `seg` at the last hyphenation point whose prefix (plus a hyphen) fits in `room`.
fn split_hyphen(p: &Para, story: &Story, seg: &Seg, room: f64, rp: &ResolvedPara) -> Option<(Seg, Seg)> {
    // Hyphenation zone: only hyphenate when the line would otherwise be at least this short.
    if room < rp.hyphen_zone {
        return None;
    }
    let points = para::hyphen_points(story, seg);
    for &cut in points.iter().rev() {
        let n = seg.glyphs.iter().take_while(|g| g.chars.start < cut).count();
        if n == 0 || n >= seg.glyphs.len() {
            continue;
        }
        let style = seg.glyphs[n - 1].style;
        let hw = p.styles[style].hyphen.map(|h| h.1).unwrap_or(0.0);
        let w: f64 = seg.glyphs[..n].iter().map(|g| g.adv).sum();
        if w + hw <= room + EPS {
            let a = Seg {
                glyphs: seg.glyphs[..n].to_vec(),
                start: seg.start,
                end: cut,
                add_hyphen: true,
                ..Default::default()
            };
            let b = Seg {
                glyphs: seg.glyphs[n..].to_vec(),
                start: cut,
                end: seg.end,
                hard_break: seg.hard_break,
                ..seg.clone()
            };
            let b = Seg { add_hyphen: false, ..b };
            return Some((a, b));
        }
    }
    None
}

/// Lays out one story through its frame chain.
/// Lays out one story. `page_override` sets the page for frames on a master (per-page master layout).
pub fn layout_story(
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    page_override: Option<usize>,
) -> (StoryLayout, Vec<FrameLayout>) {
    let frames: Vec<FrameGeom> = story.frames.iter().filter_map(|f| frame_geom(doc, *f)).collect();
    layout_in(doc, fonts, story, frames, page_override, None)
}

/// Lays out one story, reusing `memo` (the story's previous flow) from the first paragraph whose input
/// changed, and stopping early when the flow rejoins the previous one (PF-01). `doc_key` identifies
/// everything outside the story that text layout reads (styles, schemes, baseline grid, fonts).
pub fn layout_story_memo(
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    doc_key: u64,
    memo: &mut Option<StoryMemo>,
) -> (StoryLayout, Vec<FrameLayout>) {
    let frames: Vec<FrameGeom> = story.frames.iter().filter_map(|f| frame_geom(doc, *f)).collect();
    layout_in(doc, fonts, story, frames, None, Some((doc_key, memo)))
}

/// A story's saved flow for incremental layout: per-paragraph input keys and start states, and the frames'
/// lines before vertical alignment and continued notices.
#[derive(Clone)]
pub struct StoryMemo {
    doc_key: u64,
    frame_keys: Vec<u64>,
    keys: Vec<u64>,
    starts: Vec<usize>,
    snaps: Vec<Option<Snap>>,
    kwn: Vec<bool>,
    outs: Vec<FrameLayout>,
    overflow_at: Option<usize>,
}

/// Feeds `Debug` output straight into a hasher.
struct HashWrite<'a>(&'a mut DefaultHasher);

impl std::fmt::Write for HashWrite<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        s.hash(self.0);
        Ok(())
    }
}

fn debug_hash(h: &mut DefaultHasher, v: &dyn std::fmt::Debug) {
    use std::fmt::Write;
    let _ = write!(HashWrite(h), "{v:?}");
}

/// Key of everything a paragraph's own layout reads from the story: its attributes, text, runs (relative to
/// the paragraph start) and the attributes of its paragraph mark.
fn para_keys(story: &Story, ranges: &[std::ops::Range<usize>]) -> Vec<u64> {
    let runs: Vec<(std::ops::Range<usize>, &newpub_core::CharAttrs)> =
        story.runs().filter(|(r, _)| !r.is_empty()).collect();
    let mut r0 = 0;
    ranges
        .iter()
        .enumerate()
        .map(|(pi, range)| {
            let mut h = DefaultHasher::new();
            debug_hash(&mut h, &story.paras[pi]);
            crate::text::slice(story, range.clone()).hash(&mut h);
            let mark = if range.end > range.start { range.end - 1 } else { range.start.saturating_sub(1) };
            debug_hash(&mut h, &story.span_attrs_at(mark));
            while r0 < runs.len() && runs[r0].0.end <= range.start {
                r0 += 1;
            }
            for (rr, attrs) in runs[r0..].iter().take_while(|(rr, _)| rr.start < range.end) {
                (rr.start.max(range.start) - range.start, rr.end.min(range.end) - range.start).hash(&mut h);
                debug_hash(&mut h, attrs);
            }
            h.finish()
        })
        .collect()
}

fn frame_key(f: &FrameGeom) -> u64 {
    let mut h = DefaultHasher::new();
    debug_hash(&mut h, f);
    h.finish()
}

/// A saved line moved to char offset `delta` and paragraph offset `dp` (paragraphs after an edit).
fn shift_line(l: &Line, delta: isize, dp: isize) -> Line {
    let mv = |x: usize, d: isize| x.saturating_add_signed(d);
    let mut l = l.clone();
    l.char_range = mv(l.char_range.start, delta)..mv(l.char_range.end, delta);
    l.para = mv(l.para, dp);
    for r in &mut l.runs {
        for g in &mut r.glyphs {
            g.char_index = mv(g.char_index, delta);
        }
    }
    l
}

/// Lays out a table cell's story in the cell's content box (table-local coordinates); the result's
/// frame id is the cell's story id. `max_height` overrides the box height (to measure natural height).
pub fn layout_cell(
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    cell: &newpub_core::table::Cell,
    rect: Rect,
    page: Option<usize>,
) -> (StoryLayout, Vec<FrameLayout>) {
    let mut tf = TextFrame::new(story.id);
    tf.insets = cell.insets;
    tf.valign = cell.valign;
    let ins = cell.insets;
    let content = rect.inset(ins.left.0, ins.top.0, ins.right.0, ins.bottom.0);
    let geom = FrameGeom {
        id: story.id,
        origin_y: None,
        tf,
        cols: vec![(content.x, content.x + content.w.max(1.0))],
        top: content.y,
        bottom: content.bottom(),
        ex: vec![],
    };
    layout_in(doc, fonts, story, vec![geom], page, None)
}

fn layout_in(
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    frames: Vec<FrameGeom>,
    page_override: Option<usize>,
    memo: Option<(u64, &mut Option<StoryMemo>)>,
) -> (StoryLayout, Vec<FrameLayout>) {
    let scale = frames.first().map(|f| f.tf.fit_scale).unwrap_or(1.0).clamp(0.01, 100.0);
    let mut outs: Vec<FrameLayout> = frames.iter().map(|f| FrameLayout { frame: f.id, ..Default::default() }).collect();
    let mut sl =
        StoryLayout { story: story.id, overflow_at: None, frames: frames.iter().map(|f| f.id).collect(), scale };
    if frames.is_empty() {
        sl.overflow_at = (!story.is_empty()).then_some(0);
        return (sl, outs);
    }
    // Continued notices: reserve one notice line at the bottom ("on") or top ("from") of frames that ask for it.
    let mut frames = frames;
    let n_frames = frames.len();
    let notice = NoticeStyle::new(doc, fonts, story);
    for (i, f) in frames.iter_mut().enumerate() {
        if f.tf.continued_on && i + 1 < n_frames {
            f.bottom -= notice.height;
        }
        if f.tf.continued_from && i > 0 {
            f.top += notice.height;
        }
    }
    let top = frames[0].top;
    let mut flow = Flow { frames, fi: 0, ci: 0, y: top, col_empty: true };
    let ranges = story.para_ranges();
    // Number of the previous paragraph when it was numbered.
    let mut prev_number: Option<u32> = None;
    // Paragraph-level keeps (TY-15): snapshots let a paragraph be rolled back and pushed to the next column.
    let n_paras = ranges.len();
    let mut snaps: Vec<Option<Snap>> = vec![None; n_paras];
    let mut pushed = vec![false; n_paras];
    let mut widow_limit: Vec<Option<usize>> = vec![None; n_paras];
    let mut para_pos: Vec<Vec<(usize, usize)>> = vec![vec![]; n_paras];
    let mut kwn = vec![false; n_paras];
    let mut pi = 0;
    // Incremental layout (PF-01). Stories with fields depend on their pages and are always laid out in full.
    let mut memo = memo.filter(|_| !story.text.contains(newpub_core::field::FIELD_CHAR));
    let (doc_key, keys, frame_keys) = match &memo {
        Some((dk, _)) => (*dk, para_keys(story, &ranges), flow.frames.iter().map(frame_key).collect()),
        None => (0, vec![], vec![]),
    };
    let old = memo.as_mut().and_then(|(_, slot)| slot.take()).filter(|m| m.doc_key == doc_key);
    // New paragraph index from which the paragraphs match the previous flow's last ones (same frames only).
    let mut tail_from: Option<usize> = None;
    if let Some(o) = &old {
        let n_old = o.keys.len();
        let k = keys.iter().zip(&o.keys).take_while(|(a, b)| a == b).count();
        let g = frame_keys.iter().zip(&o.frame_keys).take_while(|(a, b)| a == b).count();
        let same_frames = g == frame_keys.len() && g == o.frame_keys.len();
        if k == n_paras && k == n_old && same_frames {
            // Nothing changed.
            outs = o.outs.clone();
            sl.overflow_at = o.overflow_at;
            snaps = o.snaps.clone();
            kwn = o.kwn.clone();
            pi = n_paras;
        } else {
            // Resume at the first changed paragraph, or earlier: while keep-with-next ties a paragraph to the one
            // before it (that one may move), and while the start state is unknown or lies in a changed frame.
            let mut j = k.min(n_old.saturating_sub(1)).min(n_paras - 1);
            while j > 0 && (o.kwn[j - 1] || o.snaps[j].as_ref().is_none_or(|s| s.fi >= g)) {
                j -= 1;
            }
            if j > 0
                && let Some(sn) = &o.snaps[j]
            {
                outs = flow
                    .frames
                    .iter()
                    .enumerate()
                    .map(|(f, fg)| match o.outs.get(f) {
                        Some(out) if f < g => out.clone(),
                        _ => FrameLayout { frame: fg.id, ..Default::default() },
                    })
                    .collect();
                sn.restore(&mut flow, &mut outs, &mut prev_number, &mut sl);
                snaps[..j].clone_from_slice(&o.snaps[..j]);
                kwn[..j].copy_from_slice(&o.kwn[..j]);
                pi = j;
            }
            if same_frames {
                let m = keys.iter().rev().zip(o.keys.iter().rev()).take_while(|(a, b)| a == b).count();
                tail_from = Some(n_paras - m.min(n_paras.min(n_old) - k));
            }
        }
    }
    'paras: while pi < n_paras {
        // Rejoining the previous flow: an unchanged paragraph starting in the same state lays out as before, and so
        // does everything after it (nothing later reaches back past it), so the saved lines are reused, shifted.
        if let (Some(from), Some(o)) = (tail_from, &old)
            && pi >= from
            && pi > 0
            && !pushed[pi]
            && widow_limit[pi].is_none()
            && !kwn[pi - 1]
        {
            let oi = pi + o.keys.len() - n_paras;
            if oi > 0
                && !o.kwn[oi - 1]
                && let Some(os) = &o.snaps[oi]
                && os.same_state(&flow, prev_number)
            {
                let delta = ranges[pi].start as isize - o.starts[oi] as isize;
                let dp = pi as isize - oi as isize;
                let cur: Vec<(usize, usize)> = outs.iter().map(|x| (x.lines.len(), x.decorations.len())).collect();
                for (f, out) in outs.iter_mut().enumerate() {
                    let (l0, d0) = os.lens[f];
                    out.lines.extend(o.outs[f].lines[l0..].iter().map(|l| shift_line(l, delta, dp)));
                    out.decorations.extend_from_slice(&o.outs[f].decorations[d0..]);
                }
                sl.overflow_at = o.overflow_at.map(|x| x.saturating_add_signed(delta));
                for q in oi..o.keys.len() {
                    let nq = q - oi + pi;
                    snaps[nq] = o.snaps[q].as_ref().map(|s| s.rebased(&os.lens, &cur));
                    kwn[nq] = o.kwn[q];
                }
                break 'paras;
            }
        }
        let range = &ranges[pi];
        let rp = doc.resolve_para(&story.paras[pi]);
        kwn[pi] = rp.keep_with_next;
        let snap = Snap::take(&flow, &outs, prev_number);
        let started_top = flow.col_empty;
        snaps[pi] = Some(snap);
        if pushed[pi] && !flow.col_empty && !flow.done() {
            flow.next_column();
        }
        para_pos[pi].clear();
        let cur_page = page_override.or_else(|| flow.frames.get(flow.fi).and_then(|f| doc.page_of(f.id)));
        let mut p = para::build(doc, fonts, story, pi, range.clone(), scale, cur_page);
        if !flow.col_empty || pi > 0 {
            flow.y += rp.space_before;
        }
        let mut first_line = true;
        // Generated list marker.
        let marker_text = match &rp.list {
            ListStyle::None => {
                prev_number = None;
                None
            }
            ListStyle::Bullet { bullet, .. } => {
                prev_number = None;
                Some(bullet.to_string())
            }
            ListStyle::Numbered { format, start, suffix, .. } => {
                let n = prev_number.map(|n| n + 1).unwrap_or(*start);
                prev_number = Some(n);
                Some(format!("{}{}", para::format_number(*format, n), suffix))
            }
        };
        let list_indent = match &rp.list {
            ListStyle::None => None,
            ListStyle::Bullet { indent, .. } | ListStyle::Numbered { indent, .. } => Some(indent.0.max(0.0)),
        };
        let marker = marker_text.and_then(|t| para::shape_marker(&mut p, doc, fonts, story, pi, scale, &t));
        // Drop cap: sized from the body line pitch and cap height.
        let drop = match &rp.drop_cap {
            Some(dc) if dc.lines > 0 && dc.chars > 0 && range.end > range.start => {
                let body = match p.segs.front() {
                    Some(sg) if !sg.glyphs.is_empty() => &p.styles[sg.glyphs[0].style],
                    _ => &p.styles[p.mark],
                };
                let (pitch, _) = line_metrics(&[body], rp.line_spacing);
                let body_cap = body.face.cap_height * body.size;
                let body_cap = if body_cap > 0.1 { body_cap } else { body.size * 0.7 };
                para::drop_cap(&mut p, doc, fonts, story, pi, scale, dc, pitch, body_cap)
            }
            _ => None,
        };
        let drop_lines = rp.drop_cap.as_ref().map(|d| d.lines as usize).unwrap_or(0);
        let mut emitted = 0usize;
        loop {
            // Widow control: stop the paragraph's first column early so its last line has company.
            if let (Some(lim), Some(first)) = (widow_limit[pi], para_pos[pi].first().copied())
                && (flow.fi, flow.ci) == first
                && para_pos[pi].iter().filter(|p| **p == first).count() >= lim
            {
                flow.next_column();
            }
            // Estimate the line height from the next segment's style (or the paragraph mark).
            let est_styles: Vec<&RunStyle> = match p.segs.front() {
                Some(s) if !s.glyphs.is_empty() => {
                    let mut v: Vec<&RunStyle> = s.glyphs.iter().map(|g| &p.styles[g.style]).collect();
                    v.dedup_by(|a, b| std::ptr::eq(*a, *b));
                    v
                }
                _ => vec![&p.styles[p.mark]],
            };
            let (mut h, _) = line_metrics(&est_styles, rp.line_spacing);
            let mut attempt = 0;
            let placed = loop {
                let Some(slot) = flow.slot(h) else { break None };
                let backup = p.segs.clone();
                let mut lines: Vec<(usize, Fill, f64, f64, f64)> = vec![]; // (col, fill, x, width, indent)
                let npieces = slot.pieces.len();
                for (k, &(a, b)) in slot.pieces.iter().enumerate() {
                    if p.segs.is_empty() && !lines.is_empty() {
                        break;
                    }
                    let line_no = emitted + lines.len();
                    let mut lead = match list_indent {
                        Some(ind) => rp.indent_left + ind,
                        None if first_line && lines.is_empty() => rp.indent_left + rp.indent_first,
                        None => rp.indent_left,
                    };
                    if let Some(d) = &drop
                        && line_no < drop_lines
                    {
                        lead += d.width + DROP_GAP;
                    }
                    let li = if k == 0 { lead } else { 0.0 };
                    let ri = if k + 1 == npieces { rp.indent_right } else { 0.0 };
                    let w = (b - a - li - ri).max(0.0);
                    // Only a full-width piece may break inside a word; narrow wrap pieces are skipped instead.
                    let (c0, c1) = flow.frames[slot.frame].cols[slot.col];
                    let full_width = (b - a) >= (c1 - c0) - 0.5;
                    let fill = fill_line(&mut p, story, w, li, &rp, full_width);
                    if fill.segs.is_empty() && !p.segs.is_empty() {
                        continue; // piece too narrow for the next word
                    }
                    let hard = fill.hard;
                    lines.push((slot.col, fill, a + li, w, li));
                    if hard {
                        break;
                    }
                }
                if lines.is_empty() {
                    // Nothing fit in any piece at this y: move down a little and retry.
                    p.segs = backup;
                    flow.y += 1.0_f64.max(h / 4.0);
                    continue;
                }
                // Real line height from what was placed.
                let mut used: Vec<&RunStyle> = lines
                    .iter()
                    .flat_map(|l| l.1.segs.iter().flat_map(|s| s.glyphs.iter().map(|g| &p.styles[g.style])))
                    .collect();
                if used.is_empty() {
                    used.push(&p.styles[p.mark]);
                }
                let (h2, asc) = line_metrics(&used, rp.line_spacing);
                if h2 > h + 0.01 && attempt == 0 {
                    p.segs = backup;
                    h = h2;
                    attempt += 1;
                    continue;
                }
                break Some((slot, lines, h2.max(h), asc + (h.max(h2) - h2)));
            };
            let Some((mut slot, lines, lh, asc)) = placed else {
                // Out of frames: the rest of the story overflows.
                sl.overflow_at = Some(p.segs.front().map(|s| s.start).unwrap_or(range.start));
                // Later paragraphs were not placed (snapshots left from before a rollback are stale).
                for s in &mut snaps[pi + 1..] {
                    *s = None;
                }
                break 'paras;
            };
            // Baseline grid (TY-14): move the line down so its baseline sits on the page's grid.
            if rp.align_to_baseline
                && let (Some(grid), Some(oy)) = (&doc.baseline_grid, flow.frames[slot.frame].origin_y)
                && grid.spacing.0 > 0.0
            {
                let (sp, off) = (grid.spacing.0, grid.offset.0);
                let bp = oy + slot.y + asc;
                let snapped = off + ((bp - off) / sp - 1e-9).ceil() * sp;
                slot.y += (snapped - bp).max(0.0);
            }
            let out = &mut outs[slot.frame];
            for (col, fill, x, w, _) in lines {
                let justify = match rp.align {
                    Align::Justify => !(fill.ends_para || fill.hard),
                    Align::JustifyAll => true,
                    _ => false,
                };
                let line = emit_line(
                    story,
                    &p,
                    pi,
                    &fill,
                    col,
                    x,
                    w,
                    slot.y,
                    lh,
                    slot.y + asc,
                    rp.align,
                    justify,
                    &mut out.decorations,
                );
                let mut line = line;
                if emitted == 0 && first_line {
                    // Generated material belongs to the paragraph's first line only.
                    let mut lead_runs: Vec<GlyphRun> = vec![];
                    if let Some(m) = &marker {
                        let mx = x - (rp.indent_left + list_indent.unwrap_or(0.0)) + rp.indent_left + rp.indent_first;
                        lead_runs.push(marker_run(&p, m, range.start, mx, line.baseline));
                    }
                    if let Some(d) = &drop {
                        let dx = x - d.width - DROP_GAP;
                        let by = line.baseline + (drop_lines as f64 - 1.0) * lh;
                        lead_runs.push(drop_run(story, &p, d, dx, by));
                        line.char_range.start = range.start;
                    }
                    if !lead_runs.is_empty() {
                        lead_runs.append(&mut line.runs);
                        line.runs = lead_runs;
                    }
                }
                out.lines.push(line);
                emitted += 1;
                para_pos[pi].push((slot.frame, col));
            }
            flow.y = slot.y + lh;
            flow.col_empty = false;
            first_line = false;
            if p.segs.is_empty() {
                break;
            }
        }
        // Check keep rules; on a violation roll back and lay out again.
        let pos = &para_pos[pi];
        let distinct = {
            let mut d = pos.clone();
            d.dedup();
            d.len()
        };
        let count_at = |p: Option<&(usize, usize)>| p.map(|p| pos.iter().filter(|q| *q == p).count()).unwrap_or(0);
        let (first_n, last_n) = (count_at(pos.first()), count_at(pos.last()));
        if distinct > 1
            && !started_top
            && !pushed[pi]
            && (rp.keep_together || (rp.widow_control && pos.len() >= 2 && first_n == 1))
        {
            // Keep lines together / orphan: start the paragraph in the next column.
            pushed[pi] = true;
            if let Some(sn) = &snaps[pi] {
                sn.restore(&mut flow, &mut outs, &mut prev_number, &mut sl);
            }
            continue;
        }
        if distinct > 1
            && rp.widow_control
            && pos.len() >= 3
            && last_n == 1
            && first_n >= 2
            && widow_limit[pi].is_none()
        {
            // Widow: move one more line to the next column.
            widow_limit[pi] = Some(first_n - 1);
            if let Some(sn) = &snaps[pi] {
                sn.restore(&mut flow, &mut outs, &mut prev_number, &mut sl);
            }
            continue;
        }
        if pi > 0
            && kwn[pi - 1]
            && !pushed[pi - 1]
            && !para_pos[pi - 1].is_empty()
            && para_pos[pi - 1].last() != pos.first()
        {
            // Keep with next: the previous paragraph moves to the column where this one starts.
            pushed[pi - 1] = true;
            if let Some(sn) = &snaps[pi - 1] {
                sn.restore(&mut flow, &mut outs, &mut prev_number, &mut sl);
            }
            pi -= 1;
            continue;
        }
        flow.y += rp.space_after;
        pi += 1;
    }
    if let Some((_, slot)) = memo {
        *slot = Some(StoryMemo {
            doc_key,
            frame_keys,
            keys,
            starts: ranges.iter().map(|r| r.start).collect(),
            snaps,
            kwn,
            outs: outs.clone(),
            overflow_at: sl.overflow_at,
        });
    }
    // Char ranges, overflow flags, vertical alignment.
    for (i, out) in outs.iter_mut().enumerate() {
        let s = out.lines.first().map(|l| l.char_range.start);
        let e = out.lines.last().map(|l| l.char_range.end);
        out.char_range = match (s, e) {
            (Some(s), Some(e)) => s..e,
            _ => 0..0,
        };
        out.overflow = sl.overflow_at.is_some() && i + 1 == story.frames.len();
        let fg = &flow.frames[i];
        if fg.tf.valign != VAlign::Top && !out.lines.is_empty() {
            let used = out.lines.iter().map(|l| l.top + l.height).fold(0.0, f64::max) - fg.top;
            let free = (fg.bottom - fg.top - used).max(0.0);
            let dy = if fg.tf.valign == VAlign::Middle { free / 2.0 } else { free };
            for l in &mut out.lines {
                l.top += dy;
                l.baseline += dy;
                for r in &mut l.runs {
                    for g in &mut r.glyphs {
                        g.y += dy;
                    }
                }
            }
            for d in &mut out.decorations {
                d.y += dy;
            }
        }
    }
    add_continued_notices(doc, fonts, &notice, &flow.frames, &mut outs, sl.overflow_at.is_some());
    (sl, outs)
}

/// Layout state at the start of a paragraph.
#[derive(Clone)]
struct Snap {
    fi: usize,
    ci: usize,
    y: f64,
    col_empty: bool,
    lens: Vec<(usize, usize)>,
    prev_number: Option<u32>,
}

impl Snap {
    fn take(flow: &Flow, outs: &[FrameLayout], prev_number: Option<u32>) -> Snap {
        Snap {
            fi: flow.fi,
            ci: flow.ci,
            y: flow.y,
            col_empty: flow.col_empty,
            lens: outs.iter().map(|o| (o.lines.len(), o.decorations.len())).collect(),
            prev_number,
        }
    }

    fn same_state(&self, flow: &Flow, prev_number: Option<u32>) -> bool {
        self.fi == flow.fi
            && self.ci == flow.ci
            && self.y == flow.y
            && self.col_empty == flow.col_empty
            && self.prev_number == prev_number
    }

    /// This snapshot with line counts moved from `from` to `to` (saved lines appended after an edit).
    fn rebased(&self, from: &[(usize, usize)], to: &[(usize, usize)]) -> Snap {
        let mut s = self.clone();
        for ((l, f), t) in s.lens.iter_mut().zip(from).zip(to) {
            *l = (l.0 - f.0 + t.0, l.1 - f.1 + t.1);
        }
        s
    }

    fn restore(&self, flow: &mut Flow, outs: &mut [FrameLayout], prev_number: &mut Option<u32>, sl: &mut StoryLayout) {
        flow.fi = self.fi;
        flow.ci = self.ci;
        flow.y = self.y;
        flow.col_empty = self.col_empty;
        for (o, (l, d)) in outs.iter_mut().zip(&self.lens) {
            o.lines.truncate(*l);
            o.decorations.truncate(*d);
        }
        *prev_number = self.prev_number;
        sl.overflow_at = None;
    }
}

/// Style of "(Continued on page N)" notices: the story's font, italic, 9 pt.
struct NoticeStyle {
    face: crate::FaceId,
    size: f64,
    height: f64,
    ascent: f64,
    color: newpub_core::Color,
}

impl NoticeStyle {
    fn new(doc: &Document, fonts: &FontStore, story: &Story) -> NoticeStyle {
        let rc = doc.resolve_char(&story.paras[0], &story.span_attrs_at(0));
        let face = fonts.resolve(&rc.font, false, true);
        let f = fonts.face(face);
        let size = 9.0;
        NoticeStyle { face, size, height: f.line_height() * size, ascent: f.ascender * size, color: rc.color }
    }

    fn run(&self, fonts: &FontStore, text: &str, x: f64, baseline: f64, char_index: usize) -> (GlyphRun, f64) {
        let face = fonts.face(self.face);
        let opts = crate::shape::ShapeOpts {
            kerning: true,
            ligatures: true,
            dlig: false,
            small_caps: false,
            extra: &[],
            rtl: false,
        };
        let shaped = crate::shape::shape(&face, text, &opts);
        let mut pen = x;
        let mut glyphs = vec![];
        for (i, g) in shaped.iter().enumerate() {
            let end = shaped.get(i + 1).map(|n| n.cluster).unwrap_or(text.len()).max(g.cluster);
            glyphs.push(PGlyph {
                id: g.glyph,
                x: pen + g.dx * self.size,
                y: baseline - g.dy * self.size,
                advance: g.advance * self.size,
                text_range: g.cluster..end,
                char_index,
                generated: true,
            });
            pen += g.advance * self.size;
        }
        let run = GlyphRun {
            face: self.face,
            size: self.size,
            color: self.color.clone(),
            x_scale: 1.0,
            synthetic_bold: false,
            synthetic_italic: !face.italic,
            effects: Default::default(),
            text: text.to_string(),
            glyphs,
        };
        (run, pen - x)
    }
}

/// Adds "(Continued on page N)" at the bottom and "(Continued from page N)" at the top of frames that request them,
/// when the story really continues to (or comes from) another frame on a different page.
fn add_continued_notices(
    doc: &Document,
    fonts: &FontStore,
    notice: &NoticeStyle,
    frames: &[FrameGeom],
    outs: &mut [FrameLayout],
    _overflows: bool,
) {
    let has_text: Vec<bool> = outs.iter().map(|o| o.lines.iter().any(|l| !l.char_range.is_empty())).collect();
    let page_of = |i: usize| doc.page_of(frames[i].id);
    for i in 0..frames.len() {
        if !has_text[i] {
            continue;
        }
        let f = &frames[i];
        let (x0, x1) = (f.cols.first().map(|c| c.0).unwrap_or(0.0), f.cols.last().map(|c| c.1).unwrap_or(0.0));
        if f.tf.continued_on
            && let Some(j) = (i + 1..frames.len()).find(|&j| has_text[j])
            && let Some(pj) = page_of(j).filter(|pj| Some(*pj) != page_of(i))
        {
            let text = format!("(Continued on page {})", doc.page_label(pj));
            let baseline = f.bottom + notice.ascent;
            let at = outs[i].char_range.end;
            let (run, w) = notice.run(fonts, &text, 0.0, baseline, at);
            let x = (x1 - w).max(x0);
            let run = shift_run(run, x);
            outs[i].lines.push(Line {
                column: f.cols.len().saturating_sub(1),
                x,
                width: x1 - x0,
                top: f.bottom,
                height: notice.height,
                baseline,
                char_range: at..at,
                para: usize::MAX,
                runs: vec![run],
                hyphenated: false,
            });
        }
        if f.tf.continued_from
            && let Some(k) = (0..i).rev().find(|&k| has_text[k])
            && let Some(pk) = page_of(k).filter(|pk| Some(*pk) != page_of(i))
        {
            let text = format!("(Continued from page {})", doc.page_label(pk));
            let top = f.top - notice.height;
            let baseline = top + notice.ascent;
            let at = outs[i].char_range.start;
            let (run, _) = notice.run(fonts, &text, x0, baseline, at);
            outs[i].lines.insert(
                0,
                Line {
                    column: 0,
                    x: x0,
                    width: x1 - x0,
                    top,
                    height: notice.height,
                    baseline,
                    char_range: at..at,
                    para: usize::MAX,
                    runs: vec![run],
                    hyphenated: false,
                },
            );
        }
    }
}

fn shift_run(mut run: GlyphRun, dx: f64) -> GlyphRun {
    for g in &mut run.glyphs {
        g.x += dx;
    }
    run
}

#[allow(clippy::too_many_arguments)]
fn emit_line(
    story: &Story,
    p: &Para,
    pi: usize,
    fill: &Fill,
    col: usize,
    x: f64,
    width: f64,
    top: f64,
    height: f64,
    baseline: f64,
    align: Align,
    justify: bool,
    decos: &mut Vec<Decoration>,
) -> Line {
    // Flatten glyphs; drop trailing whitespace from the measured width.
    let glyphs: Vec<&G> = fill.segs.iter().flat_map(|s| s.glyphs.iter()).collect();
    let mut hyphen: Option<(usize, u16, f64)> = None;
    if fill.hyphen
        && let Some(last) = glyphs.last()
    {
        let st = last.style;
        if let Some((gid, adv)) = p.styles[st].hyphen {
            hyphen = Some((st, gid, adv));
        }
    }
    let total: f64 = glyphs.iter().map(|g| g.adv).sum::<f64>() + hyphen.map(|h| h.2).unwrap_or(0.0);
    let trail: f64 =
        if hyphen.is_some() { 0.0 } else { glyphs.iter().rev().take_while(|g| g.space && !g.tab).map(|g| g.adv).sum() };
    let natural = total - trail;
    let free = (width - natural).max(0.0);
    // Spaces that stretch: inner spaces after the last tab.
    let ntrail = if hyphen.is_some() { 0 } else { glyphs.iter().rev().take_while(|g| g.space && !g.tab).count() };
    let last_tab = glyphs.iter().rposition(|g| g.tab);
    let stretch: Vec<bool> = glyphs
        .iter()
        .enumerate()
        .map(|(i, g)| g.space && !g.tab && i < glyphs.len() - ntrail && last_tab.map(|t| i > t).unwrap_or(true))
        .collect();
    let nstretch = stretch.iter().filter(|s| **s).count();
    let (mut pen, per_space) = if justify && nstretch > 0 {
        (0.0, free / nstretch as f64)
    } else {
        let off = match align {
            Align::Center => free / 2.0,
            Align::Right => free,
            _ => 0.0,
        };
        (off, 0.0)
    };
    pen += x;
    let mut runs: Vec<GlyphRun> = vec![];
    let mut cur: Option<(usize, GlyphRun, usize)> = None; // (style, run, first char)
    let flush = |cur: &mut Option<(usize, GlyphRun, usize)>, runs: &mut Vec<GlyphRun>| {
        if let Some((_, r, _)) = cur.take() {
            runs.push(r);
        }
    };
    let start_char = fill.segs.first().map(|s| s.start).unwrap_or(p.range.start);
    let end_char = fill.segs.last().map(|s| s.end).unwrap_or(p.range.start);
    // Run text is the story slice of the run's chars; glyph ranges are byte offsets into it.
    let push = |style: usize,
                gid: u16,
                chars: std::ops::Range<usize>,
                gx: f64,
                gy: f64,
                adv: f64,
                ghost: bool,
                field: Option<&str>,
                cur: &mut Option<(usize, GlyphRun, usize)>,
                runs: &mut Vec<GlyphRun>| {
        if cur.as_ref().map(|c| c.0 != style).unwrap_or(true) {
            flush(cur, runs);
            let st = &p.styles[style];
            *cur = Some((
                style,
                GlyphRun {
                    face: st.face.id,
                    size: st.size,
                    color: st.color.clone(),
                    x_scale: st.x_scale,
                    synthetic_bold: st.synthetic_bold,
                    synthetic_italic: st.synthetic_italic,
                    effects: st.effects.clone(),
                    text: String::new(),
                    glyphs: vec![],
                },
                chars.start,
            ));
        }
        let (_, run, last) = cur.as_mut().expect("run started");
        // Run text is appended per cluster: glyphs of one cluster share its text range; ghost glyphs
        // (tab leaders) map to no text; field glyphs map to the field's displayed text.
        let range = if ghost {
            run.text.len()..run.text.len()
        } else if let Some(prev) = run
            .glyphs
            .last()
            .filter(|g| g.char_index == chars.start && *last == chars.start && !g.text_range.is_empty())
        {
            prev.text_range.clone()
        } else {
            let t = match field {
                Some(f) => f.to_string(),
                None => crate::text::slice(story, chars.clone()).to_string(),
            };
            let b0 = run.text.len();
            run.text.push_str(&t);
            b0..run.text.len()
        };
        *last = chars.start;
        run.glyphs.push(PGlyph {
            id: gid,
            x: gx,
            y: gy,
            advance: adv,
            text_range: range,
            char_index: chars.start,
            generated: false,
        });
    };
    let mut deco_spans: Vec<(DecorationKind, usize, f64, f64)> = vec![];
    // Visual order (UAX #9 rule L2): reverse runs at each level from the highest down to the lowest odd level.
    let mut order: Vec<usize> = (0..glyphs.len()).collect();
    let max_level = glyphs.iter().map(|g| g.level).max().unwrap_or(0);
    if max_level > 0 {
        let min_odd = glyphs.iter().map(|g| g.level).filter(|l| l % 2 == 1).min().unwrap_or(1);
        for lvl in (min_odd.max(1)..=max_level).rev() {
            let mut k = 0;
            while k < order.len() {
                if glyphs[order[k]].level >= lvl {
                    let mut e = k;
                    while e < order.len() && glyphs[order[e]].level >= lvl {
                        e += 1;
                    }
                    order[k..e].reverse();
                    k = e;
                } else {
                    k += 1;
                }
            }
        }
    }
    for &i in &order {
        let g = glyphs[i];
        let st = &p.styles[g.style];
        let gx = pen + g.dx;
        let gy = baseline - st.shift - g.dy;
        if !(g.tab || g.adv == 0.0 && g.chars.len() == 1 && crate::text::slice(story, g.chars.clone()) == "\u{2028}") {
            push(g.style, g.glyph, g.chars.clone(), gx, gy, g.adv, false, g.field.as_deref(), &mut cur, &mut runs);
        }
        if g.tab
            && g.adv > 0.0
            && let Some(lc) = g.leader
            && let Some((lid, la)) = st.glyph_for(lc)
            && la > 0.01
        {
            // Leaders sit on a grid of their own advance and keep half an advance clear of the next text.
            let end = pen + g.adv;
            let mut k = (pen / la - 1e-6).ceil();
            while (k + 1.0) * la <= end - la / 2.0 + 1e-6 {
                push(g.style, lid, g.chars.start..g.chars.start, k * la, gy, la, true, None, &mut cur, &mut runs);
                k += 1.0;
            }
        }
        let adv = g.adv + if stretch[i] { per_space } else { 0.0 };
        let in_trail = i >= glyphs.len() - ntrail;
        if !in_trail {
            if st.underline {
                deco_spans.push((DecorationKind::Underline, g.style, pen, pen + adv));
            }
            if st.strike {
                deco_spans.push((DecorationKind::Strike, g.style, pen, pen + adv));
            }
        }
        pen += adv;
    }
    if let Some((st, gid, adv)) = hyphen {
        let last = glyphs.last().map(|g| g.chars.end).unwrap_or(end_char);
        let s = &p.styles[st];
        // The hyphen glyph maps to no source text.
        if cur.as_ref().map(|c| c.0 != st).unwrap_or(true) {
            flush(&mut cur, &mut runs);
            cur = Some((
                st,
                GlyphRun {
                    face: s.face.id,
                    size: s.size,
                    color: s.color.clone(),
                    x_scale: s.x_scale,
                    synthetic_bold: s.synthetic_bold,
                    synthetic_italic: s.synthetic_italic,
                    effects: s.effects.clone(),
                    text: String::new(),
                    glyphs: vec![],
                },
                last,
            ));
        }
        if let Some((_, run, _)) = cur.as_mut() {
            // The hyphen is not in the story; it maps to a "-" appended to the run text.
            let n = run.text.len();
            run.text.push('-');
            run.glyphs.push(PGlyph {
                id: gid,
                x: pen,
                y: baseline - s.shift,
                advance: adv,
                text_range: n..n + 1,
                char_index: last,
                generated: false,
            });
        }
    }
    flush(&mut cur, &mut runs);
    // Merge decoration spans per kind and style.
    for (kind, style, x0, x1) in deco_spans {
        let st = &p.styles[style];
        let (y, th) = match kind {
            DecorationKind::Underline => {
                (baseline - st.face.underline_pos * st.size, st.face.underline_thickness * st.size)
            }
            DecorationKind::Strike => {
                (baseline - st.shift - st.face.strikeout_pos * st.size, st.face.underline_thickness * st.size)
            }
        };
        if let Some(d) = decos.last_mut()
            && d.kind == kind
            && (d.x1 - x0).abs() < 0.01
            && (d.y - y).abs() < 0.01
            && d.color == st.color
        {
            d.x1 = x1;
            continue;
        }
        decos.push(Decoration { kind, x0, x1, y, thickness: th.max(0.25), color: st.color.clone() });
    }
    Line {
        column: col,
        x,
        width,
        top,
        height,
        baseline,
        char_range: start_char..end_char,
        para: pi,
        runs,
        hyphenated: fill.hyphen,
    }
}

fn glyph_run(p: &Para, style: usize, text: String, glyphs: Vec<PGlyph>) -> GlyphRun {
    let st = &p.styles[style];
    GlyphRun {
        face: st.face.id,
        size: st.size,
        color: st.color.clone(),
        x_scale: st.x_scale,
        synthetic_bold: st.synthetic_bold,
        synthetic_italic: st.synthetic_italic,
        effects: st.effects.clone(),
        text,
        glyphs,
    }
}

/// Byte offset of char index `ci` in `text`.
fn byte_of(text: &str, ci: usize) -> usize {
    text.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(text.len())
}

/// Run for a generated list marker. Its glyphs carry the paragraph start as char index but are `generated`.
fn marker_run(p: &Para, m: &para::Marker, para_start: usize, x: f64, baseline: f64) -> GlyphRun {
    let st = &p.styles[m.style];
    let mut pen = x;
    let mut glyphs = vec![];
    for g in &m.glyphs {
        glyphs.push(PGlyph {
            id: g.glyph,
            x: pen + g.dx,
            y: baseline - st.shift - g.dy,
            advance: g.adv,
            text_range: byte_of(&m.text, g.chars.start)..byte_of(&m.text, g.chars.end),
            char_index: para_start,
            generated: true,
        });
        pen += g.adv;
    }
    glyph_run(p, m.style, m.text.clone(), glyphs)
}

/// Run for the dropped capital(s); glyphs keep their story char indices.
fn drop_run(story: &Story, p: &Para, d: &para::DropGlyphs, x: f64, baseline: f64) -> GlyphRun {
    let text = crate::text::slice(story, d.chars.clone()).to_string();
    let style = d.glyphs.first().map(|g| g.style).unwrap_or(0);
    let mut pen = x;
    let mut glyphs = vec![];
    for g in &d.glyphs {
        glyphs.push(PGlyph {
            id: g.glyph,
            x: pen + g.dx,
            y: baseline - g.dy,
            advance: g.adv,
            text_range: byte_of(&text, g.chars.start - d.chars.start)..byte_of(&text, g.chars.end - d.chars.start),
            char_index: g.chars.start,
            generated: false,
        });
        pen += g.adv;
    }
    glyph_run(p, style, text, glyphs)
}
