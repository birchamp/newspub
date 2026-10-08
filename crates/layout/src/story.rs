//! Story flow: lines through columns and linked frames, wrapping around objects.

use crate::para::{self, G, Para, RunStyle, Seg};
use crate::{Decoration, DecorationKind, FontStore, FrameLayout, GlyphRun, Line, PGlyph, StoryLayout};
use newpub_core::{
    Align, Document, Id, LineSpacing, Object, ObjectKind, Rect, ResolvedPara, ShapeKind, Story, TabAlign, TextFrame,
    VAlign, WrapMode,
};

/// Narrowest line piece worth filling when text wraps around objects.
const MIN_PIECE: f64 = 18.0;
const EPS: f64 = 1e-6;

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

struct FrameGeom {
    id: Id,
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
    let ObjectKind::Text(tf) = &obj.kind else { return None };
    let w = obj.rect.w;
    let h = obj.rect.h;
    let ins = tf.insets;
    let content = Rect::new(0.0, 0.0, w, h).inset(ins.left.0, ins.top.0, ins.right.0, ins.bottom.0);
    let n = tf.columns.max(1) as f64;
    let gutter = tf.gutter.0.max(0.0);
    let cw = ((content.w - gutter * (n - 1.0)) / n).max(1.0);
    let cols = (0..tf.columns.max(1)).map(|i| {
        let x0 = content.x + i as f64 * (cw + gutter);
        (x0, x0 + cw)
    });
    Some(FrameGeom {
        id,
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
        if o.wrap.mode == WrapMode::None {
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
            let (stop, align, _) = next_tab(indent + x, rp);
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
pub fn layout_story(doc: &Document, fonts: &FontStore, story: &Story) -> (StoryLayout, Vec<FrameLayout>) {
    let frames: Vec<FrameGeom> = story.frames.iter().filter_map(|f| frame_geom(doc, *f)).collect();
    let scale = frames.first().map(|f| f.tf.fit_scale).unwrap_or(1.0).clamp(0.01, 100.0);
    let mut outs: Vec<FrameLayout> = frames.iter().map(|f| FrameLayout { frame: f.id, ..Default::default() }).collect();
    let mut sl = StoryLayout { story: story.id, overflow_at: None, frames: story.frames.clone(), scale };
    if frames.is_empty() {
        sl.overflow_at = (!story.is_empty()).then_some(0);
        return (sl, outs);
    }
    let top = frames[0].top;
    let mut flow = Flow { frames, fi: 0, ci: 0, y: top, col_empty: true };
    let ranges = story.para_ranges();
    'paras: for (pi, range) in ranges.iter().enumerate() {
        let rp = doc.resolve_para(&story.paras[pi]);
        let mut p = para::build(doc, fonts, story, pi, range.clone(), scale);
        if !flow.col_empty || pi > 0 {
            flow.y += rp.space_before;
        }
        let mut first_line = true;
        loop {
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
                    let lead =
                        if first_line && lines.is_empty() { rp.indent_left + rp.indent_first } else { rp.indent_left };
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
            let Some((slot, lines, lh, asc)) = placed else {
                // Out of frames: the rest of the story overflows.
                sl.overflow_at = Some(p.segs.front().map(|s| s.start).unwrap_or(range.start));
                break 'paras;
            };
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
                out.lines.push(line);
            }
            flow.y = slot.y + lh;
            flow.col_empty = false;
            first_line = false;
            if p.segs.is_empty() {
                break;
            }
        }
        flow.y += rp.space_after;
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
    (sl, outs)
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
                    text: String::new(),
                    glyphs: vec![],
                },
                chars.start,
            ));
        }
        let (_, run, first) = cur.as_mut().expect("run started");
        let full = story.slice(*first..chars.end.max(*first));
        if full.len() > run.text.len() {
            run.text = full.to_string();
        }
        let b0 = story.slice(*first..chars.start).len();
        let b1 = story.slice(*first..chars.end).len();
        run.glyphs.push(PGlyph { id: gid, x: gx, y: gy, advance: adv, text_range: b0..b1, char_index: chars.start });
    };
    let mut deco_spans: Vec<(DecorationKind, usize, f64, f64)> = vec![];
    for (i, g) in glyphs.iter().enumerate() {
        let st = &p.styles[g.style];
        let gx = pen + g.dx;
        let gy = baseline - st.shift - g.dy;
        if !(g.tab || g.adv == 0.0 && g.chars.len() == 1 && story.slice(g.chars.clone()) == "\u{2028}") {
            push(g.style, g.glyph, g.chars.clone(), gx, gy, g.adv, &mut cur, &mut runs);
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
                    text: String::new(),
                    glyphs: vec![],
                },
                last,
            ));
        }
        if let Some((_, run, _)) = cur.as_mut() {
            let n = run.text.len();
            run.glyphs.push(PGlyph {
                id: gid,
                x: pen,
                y: baseline - s.shift,
                advance: adv,
                text_range: n..n,
                char_index: last,
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
