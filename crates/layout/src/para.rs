//! Paragraph itemisation: style runs → font-coverage runs → shaped glyphs → break segments.

use crate::fonts::{Face, FontStore};
use crate::shape::{ShapeOpts, has_feature, shape};
use newpub_core::{
    Baseline, Caps, CharAttrs, Color, Document, DropCap, LINE_SEP, NumberFormat, ParaAttrs, ResolvedChar, Story,
};
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

/// Resolved style of a run, in points.
#[derive(Clone)]
pub struct RunStyle {
    pub face: Arc<Face>,
    pub size: f64,
    pub color: Color,
    pub x_scale: f64,
    pub tracking: f64,
    /// Baseline shift in points (positive = up).
    pub shift: f64,
    pub underline: bool,
    pub strike: bool,
    pub synthetic_bold: bool,
    pub synthetic_italic: bool,
    pub effects: newpub_core::TextEffects,
    /// Line metrics at this size, points (descent positive).
    pub ascent: f64,
    pub descent: f64,
    pub gap: f64,
    pub hyphen: Option<(u16, f64)>,
}

impl RunStyle {
    pub fn new(fonts: &FontStore, rc: &ResolvedChar, scale: f64, face_override: Option<crate::FaceId>) -> RunStyle {
        let fid = face_override.unwrap_or_else(|| fonts.resolve(&rc.font, rc.bold, rc.italic));
        let face = fonts.face(fid);
        let base = rc.size * scale;
        let (size, shift) = match rc.baseline {
            Baseline::Normal => (base, 0.0),
            Baseline::Superscript => (base * 0.65, base * 0.33),
            Baseline::Subscript => (base * 0.65, -base * 0.14),
        };
        // Line metrics come from the un-shifted size so super/subscripts don't open up lines.
        let ascent = face.ascender * base;
        let descent = -face.descender * base;
        let gap = face.line_gap * base;
        let hyphen = {
            let g = shape(
                &face,
                "-",
                &ShapeOpts { kerning: false, ligatures: false, dlig: false, small_caps: false, extra: &[], rtl: false },
            );
            g.first().map(|g| (g.glyph, g.advance * size * rc.scale / 100.0))
        };
        RunStyle {
            synthetic_bold: rc.bold && !face.bold,
            synthetic_italic: rc.italic && !face.italic,
            effects: rc.effects.clone(),
            face,
            size,
            color: rc.color.clone(),
            x_scale: rc.scale / 100.0,
            tracking: rc.tracking / 1000.0 * size,
            shift,
            underline: rc.underline,
            strike: rc.strike,
            ascent,
            descent,
            gap,
            hyphen,
        }
    }
}

impl RunStyle {
    /// Glyph id and advance (points, scaled and tracked) of a single character in this style.
    pub fn glyph_for(&self, c: char) -> Option<(u16, f64)> {
        let opts =
            ShapeOpts { kerning: false, ligatures: false, dlig: false, small_caps: false, extra: &[], rtl: false };
        let g = shape(&self.face, &c.to_string(), &opts);
        let g = g.first()?;
        Some((g.glyph, g.advance * self.size * self.x_scale + self.tracking))
    }
}

/// A shaped glyph within a paragraph.
#[derive(Clone, Debug)]
pub struct G {
    pub glyph: u16,
    /// Advance in points, including tracking and scaling.
    pub adv: f64,
    pub dx: f64,
    pub dy: f64,
    /// Story char range of this glyph's cluster.
    pub chars: Range<usize>,
    pub style: usize,
    pub space: bool,
    pub tab: bool,
    pub decimal: bool,
    /// Leader character to fill this tab glyph's gap with (set when the tab is placed).
    pub leader: Option<char>,
    /// Displayed text of a field glyph (the story holds one FIELD_CHAR).
    pub field: Option<std::sync::Arc<str>>,
    /// Bidi embedding level (odd = right-to-left).
    pub level: u8,
}

/// Text between two break opportunities.
#[derive(Clone, Debug, Default)]
pub struct Seg {
    pub glyphs: Vec<G>,
    /// Story chars covered.
    pub start: usize,
    pub end: usize,
    /// A mandatory line break follows this segment.
    pub hard_break: bool,
    /// The segment is a single tab.
    pub tab: bool,
    /// Ends with a soft hyphen at which a break shows a hyphen.
    pub soft_hyphen: bool,
    /// The segment was split by hyphenation and must show a hyphen at line end.
    pub add_hyphen: bool,
}

impl Seg {
    pub fn width(&self) -> f64 {
        self.glyphs.iter().map(|g| g.adv).sum()
    }
    /// Width of trailing whitespace glyphs (not counted at line end).
    pub fn trail(&self) -> f64 {
        self.glyphs.iter().rev().take_while(|g| g.space).map(|g| g.adv).sum()
    }
}

pub struct Para {
    pub styles: Vec<RunStyle>,
    pub segs: VecDeque<Seg>,
    /// Style of the paragraph mark (used for empty paragraphs).
    pub mark: usize,
    pub range: Range<usize>,
}

/// Builds a paragraph. `page` is the page the text is being laid out on (for page-number fields).
pub fn build(
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    pi: usize,
    range: Range<usize>,
    scale: f64,
    page: Option<usize>,
) -> Para {
    let pattrs: &ParaAttrs = &story.paras[pi];
    let mut styles: Vec<RunStyle> = vec![];
    let mut glyphs: Vec<G> = vec![];

    // Paragraph mark style: attrs of the last char of the paragraph (or the char before it).
    let mark_attrs: CharAttrs =
        story.span_attrs_at(if range.end > range.start { range.end - 1 } else { range.start.saturating_sub(1) });
    let mark_rc = doc.resolve_char(pattrs, &mark_attrs);
    styles.push(RunStyle::new(fonts, &mark_rc, scale, None));
    let mark = 0;
    let levels = bidi_levels(story.slice(range.clone()), doc.resolve_para(pattrs).rtl);

    for (rr, attrs) in story.runs() {
        let (s, e) = (rr.start.max(range.start), rr.end.min(range.end));
        if s >= e {
            continue;
        }
        let rc = doc.resolve_char(pattrs, attrs);
        if let Some(field) = &attrs.field {
            // Each field char shows computed text; all its glyphs map to that one story char.
            let shown: std::sync::Arc<str> = doc.field_text(field, page).into();
            let si = styles.len();
            styles.push(RunStyle::new(fonts, &rc, scale, None));
            for ci in s..e {
                let start = glyphs.len();
                let orig: Vec<char> = shown.chars().collect();
                shape_chunk(&mut glyphs, &styles[si], si, &shown, ci, &rc, false, &orig, &[]);
                for g in &mut glyphs[start..] {
                    g.chars = ci..ci + 1;
                    g.field = Some(shown.clone());
                }
            }
            continue;
        }
        let text: Vec<char> = story.slice(s..e).chars().collect();
        // Caps transform, 1:1 per char so indices stay aligned.
        let disp: Vec<char> = text
            .iter()
            .map(|&c| match rc.caps {
                Caps::AllCaps => single_upper(c),
                _ => c,
            })
            .collect();
        let primary = fonts.resolve(&rc.font, rc.bold, rc.italic);
        let pface = fonts.face(primary);
        let small_caps = rc.caps == Caps::SmallCaps;
        let native_smcp = small_caps && has_feature(&pface, "smcp");
        // Split by font coverage.
        let mut i = 0;
        while i < disp.len() {
            let needs_fallback =
                |c: char| !c.is_whitespace() && !c.is_control() && c != '\u{AD}' && !pface.has_glyph(c);
            let fb = if needs_fallback(disp[i]) { fonts.fallback_for(disp[i], rc.bold, rc.italic) } else { None };
            let mut j = i + 1;
            while j < disp.len() {
                let c = disp[j];
                let nf = needs_fallback(c);
                let same = if fb.is_some() { nf && fonts.fallback_for(c, rc.bold, rc.italic) == fb } else { !nf };
                if !same {
                    break;
                }
                j += 1;
            }
            // Synthetic small caps: lowercase letters become smaller capitals.
            let mut k = i;
            while k < j {
                let synth = small_caps && !native_smcp && disp[k].is_lowercase();
                let mut m = k + 1;
                while m < j && (small_caps && !native_smcp && disp[m].is_lowercase()) == synth {
                    m += 1;
                }
                let mut rc2 = rc.clone();
                let chunk: String = if synth {
                    rc2.size *= 0.75;
                    disp[k..m].iter().map(|&c| single_upper(c)).collect()
                } else {
                    disp[k..m].iter().collect()
                };
                let si = styles.len();
                styles.push(RunStyle::new(fonts, &rc2, scale, fb));
                let lv: &[u8] = if levels.is_empty() { &[] } else { &levels[s + k - range.start..s + m - range.start] };
                shape_chunk(&mut glyphs, &styles[si], si, &chunk, s + k, &rc2, native_smcp, &text[k..m], lv);
                k = m;
            }
            i = j;
        }
    }

    let segs = segment(story, range.clone(), glyphs);
    Para { styles, segs, mark, range }
}

fn single_upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// Shapes a chunk of one style and font. `levels` (bidi embedding level per char, may be empty = all
/// left-to-right) splits it into direction runs; glyphs are emitted in logical order with their level.
#[allow(clippy::too_many_arguments)]
fn shape_chunk(
    out: &mut Vec<G>,
    st: &RunStyle,
    si: usize,
    text: &str,
    first_char: usize,
    rc: &ResolvedChar,
    smcp: bool,
    orig: &[char],
    levels: &[u8],
) {
    if levels.is_empty() || levels.iter().all(|l| *l == 0) {
        shape_piece(out, st, si, text, first_char, rc, smcp, orig, 0);
        return;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let lvl = levels.get(i).copied().unwrap_or(0);
        let mut j = i + 1;
        while j < chars.len() && levels.get(j).copied().unwrap_or(0) == lvl {
            j += 1;
        }
        let piece: String = chars[i..j].iter().collect();
        shape_piece(out, st, si, &piece, first_char + i, rc, smcp, &orig[i.min(orig.len())..j.min(orig.len())], lvl);
        i = j;
    }
}

#[allow(clippy::too_many_arguments)]
fn shape_piece(
    out: &mut Vec<G>,
    st: &RunStyle,
    si: usize,
    text: &str,
    first_char: usize,
    rc: &ResolvedChar,
    smcp: bool,
    orig: &[char],
    level: u8,
) {
    let rtl = level % 2 == 1;
    let start = out.len();
    let opts = ShapeOpts {
        kerning: rc.kerning,
        ligatures: rc.ligatures,
        dlig: rc.dlig,
        small_caps: smcp,
        extra: &rc.features,
        rtl,
    };
    let shaped = shape(&st.face, text, &opts);
    // byte offset → char index within chunk
    let mut b2c = vec![0usize; text.len() + 1];
    for (ci, (b, _)) in text.char_indices().enumerate() {
        b2c[b] = ci;
    }
    b2c[text.len()] = text.chars().count();
    let mut clusters: Vec<usize> = shaped.iter().map(|g| g.cluster).collect();
    clusters.sort_unstable();
    clusters.dedup();
    let next_cluster = |c: usize| clusters.iter().copied().find(|&x| x > c).unwrap_or(text.len());
    let k = st.size * st.x_scale;
    for g in shaped {
        let c0 = b2c[g.cluster];
        let c1 = b2c[next_cluster(g.cluster)];
        let ch = orig.get(c0).copied().unwrap_or(' ');
        let is_tab = ch == '\t';
        let invisible = ch == LINE_SEP || ch == '\u{AD}';
        let adv = if invisible { 0.0 } else { g.advance * k + st.tracking };
        out.push(G {
            glyph: g.glyph,
            adv,
            dx: g.dx * k,
            dy: g.dy * st.size,
            chars: first_char + c0..first_char + c1.max(c0 + 1),
            style: si,
            space: ch == ' ' || ch == '\u{3000}' || is_tab,
            tab: is_tab,
            decimal: ch == '.',
            leader: None,
            field: None,
            level,
        });
    }
    if rtl {
        // rustybuzz returns right-to-left glyphs in visual order; store them logically.
        out[start..].reverse();
    }
}

/// Bidi embedding level per char of a paragraph (empty when the paragraph is plain left-to-right).
pub fn bidi_levels(text: &str, rtl_base: bool) -> Vec<u8> {
    use unicode_bidi::{BidiClass, BidiInfo, Level};
    let has_rtl =
        text.chars().any(|c| matches!(unicode_bidi::bidi_class(c), BidiClass::R | BidiClass::AL | BidiClass::AN));
    if !rtl_base && !has_rtl {
        return vec![];
    }
    let base = if rtl_base { Level::rtl() } else { Level::ltr() };
    let info = BidiInfo::new(text, Some(base));
    text.char_indices().map(|(b, _)| info.levels[b].number()).collect()
}

/// Splits glyphs at UAX #14 break opportunities (and around tabs).
fn segment(story: &Story, range: Range<usize>, glyphs: Vec<G>) -> VecDeque<Seg> {
    let text = story.slice(range.clone());
    let chars: Vec<char> = text.chars().collect();
    let mut b2c = vec![0usize; text.len() + 1];
    for (ci, (b, _)) in text.char_indices().enumerate() {
        b2c[b] = ci;
    }
    b2c[text.len()] = chars.len();
    // Boundaries (relative char index where a new segment starts) and whether the break is mandatory.
    let mut bounds: Vec<(usize, bool)> = vec![];
    for (b, op) in unicode_linebreak::linebreaks(text) {
        let ci = b2c[b];
        if ci >= chars.len() {
            continue;
        }
        bounds.push((ci, matches!(op, unicode_linebreak::BreakOpportunity::Mandatory)));
    }
    for (i, &c) in chars.iter().enumerate() {
        if c == '\t' {
            bounds.push((i, false));
            if i + 1 < chars.len() {
                bounds.push((i + 1, false));
            }
        }
    }
    bounds.sort_by_key(|b| b.0);
    bounds.dedup_by(|a, b| {
        if a.0 == b.0 {
            b.1 |= a.1;
            true
        } else {
            false
        }
    });
    let mut segs: VecDeque<Seg> = VecDeque::new();
    let mut start = 0usize;
    let mut cuts: Vec<(usize, bool)> = bounds.into_iter().filter(|b| b.0 > 0).collect();
    cuts.push((chars.len(), false));
    let mut gi = 0;
    for (end, mand) in cuts {
        if end <= start {
            continue;
        }
        let mut seg = Seg { start: range.start + start, end: range.start + end, ..Default::default() };
        while gi < glyphs.len() && glyphs[gi].chars.start < range.start + end {
            seg.glyphs.push(glyphs[gi].clone());
            gi += 1;
        }
        seg.tab = end - start == 1 && chars[start] == '\t';
        seg.soft_hyphen = chars[end - 1] == '\u{AD}';
        seg.hard_break = mand;
        segs.push_back(seg);
        start = end;
    }
    segs
}

/// Hyphenation break points (story char indices) inside a segment, for en-US.
pub fn hyphen_points(story: &Story, seg: &Seg) -> Vec<usize> {
    use hyphenation::{Hyphenator, Language, Load, Standard};
    use std::sync::OnceLock;
    static DICT: OnceLock<Option<Standard>> = OnceLock::new();
    let Some(dict) = DICT.get_or_init(|| Standard::from_embedded(Language::EnglishUS).ok()) else {
        return vec![];
    };
    let word: String = story.slice(seg.start..seg.end).chars().take_while(|c| c.is_alphabetic()).collect();
    if word.chars().count() < 5 {
        return vec![];
    }
    let lower = word.to_lowercase();
    if lower.len() != word.len() {
        return vec![];
    }
    let h = dict.hyphenate(&lower);
    h.breaks.iter().map(|&b| seg.start + word[..b].chars().count()).collect()
}

/// Generated list marker, shaped in the paragraph's first run style.
pub struct Marker {
    pub style: usize,
    pub text: String,
    pub glyphs: Vec<G>,
}

/// Shaped drop-cap glyphs (they keep their story char indices).
pub struct DropGlyphs {
    pub glyphs: Vec<G>,
    /// Story chars covered.
    pub chars: Range<usize>,
    pub width: f64,
}

/// Style for generated text (markers) at the start of a paragraph.
fn first_rc(doc: &Document, story: &Story, pi: usize, range: &Range<usize>) -> ResolvedChar {
    let attrs = story.span_attrs_at(range.start);
    doc.resolve_char(&story.paras[pi], &attrs)
}

/// Shapes marker `text` for the paragraph at `pi`; the style is appended to `p.styles`.
pub fn shape_marker(
    p: &mut Para,
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    pi: usize,
    scale: f64,
    text: &str,
) -> Option<Marker> {
    let rc = first_rc(doc, story, pi, &p.range);
    let chars: Vec<char> = text.chars().collect();
    let primary = fonts.face(fonts.resolve(&rc.font, rc.bold, rc.italic));
    let fb = chars.iter().find(|&&c| !primary.has_glyph(c)).and_then(|&c| fonts.fallback_for(c, rc.bold, rc.italic));
    let si = p.styles.len();
    p.styles.push(RunStyle::new(fonts, &rc, scale, fb));
    let mut glyphs = vec![];
    shape_chunk(&mut glyphs, &p.styles[si], si, text, 0, &rc, false, &chars, &[]);
    (!glyphs.is_empty()).then(|| Marker { style: si, text: text.to_string(), glyphs })
}

/// Shapes the drop cap and removes the dropped chars from the paragraph's segments.
/// `pitch` is the body line pitch and `body_cap` the body cap height, both in points.
#[allow(clippy::too_many_arguments)]
pub fn drop_cap(
    p: &mut Para,
    doc: &Document,
    fonts: &FontStore,
    story: &Story,
    pi: usize,
    scale: f64,
    dc: &DropCap,
    pitch: f64,
    body_cap: f64,
) -> Option<DropGlyphs> {
    let range = p.range.clone();
    let wanted = story.slice(range.clone()).chars().take(dc.chars as usize).take_while(|c| !c.is_whitespace()).count();
    if wanted == 0 || dc.lines == 0 {
        return None;
    }
    // Whole glyphs are removed: a ligature crossing the cut takes its whole cluster along.
    let mut cutoff = range.start + wanted;
    for s in p.segs.iter() {
        for g in &s.glyphs {
            if g.chars.start < cutoff {
                cutoff = cutoff.max(g.chars.end.min(range.end));
            }
        }
        if s.end >= cutoff {
            break;
        }
    }
    let text = story.slice(range.start..cutoff).to_string();
    let chars: Vec<char> = text.chars().collect();
    let mut rc = first_rc(doc, story, pi, &range);
    rc.baseline = Baseline::Normal;
    rc.caps = Caps::Normal;
    rc.tracking = 0.0;
    if let Some(f) = &dc.font {
        rc.font = f.clone();
    }
    if let Some(c) = &dc.color {
        rc.color = c.clone();
    }
    let face = fonts.face(fonts.resolve(&rc.font, rc.bold, rc.italic));
    let cap_em = if face.cap_height > 0.1 { face.cap_height } else { 0.7 };
    let target = (dc.lines as f64 - 1.0) * pitch + body_cap;
    rc.size = target / cap_em / scale;
    let si = p.styles.len();
    p.styles.push(RunStyle::new(fonts, &rc, scale, None));
    let mut glyphs = vec![];
    shape_chunk(&mut glyphs, &p.styles[si], si, &text, range.start, &rc, false, &chars, &[]);
    if glyphs.is_empty() {
        p.styles.pop();
        return None;
    }
    let width = glyphs.iter().map(|g| g.adv).sum();
    // Remove the dropped glyphs from the body.
    for s in p.segs.iter_mut() {
        s.glyphs.retain(|g| g.chars.start >= cutoff);
        s.start = s.start.max(cutoff).min(s.end);
    }
    p.segs.retain(|s| !s.glyphs.is_empty() || s.hard_break || s.start < s.end);
    Some(DropGlyphs { glyphs, chars: range.start..cutoff, width })
}

/// Formats list number `n` (1-based) in `fmt`.
pub fn format_number(fmt: NumberFormat, n: u32) -> String {
    let alpha = |upper: bool| -> String {
        let mut v = vec![];
        let mut k = n;
        while k > 0 {
            k -= 1;
            let c = (b'a' + (k % 26) as u8) as char;
            v.push(if upper { c.to_ascii_uppercase() } else { c });
            k /= 26;
        }
        v.iter().rev().collect()
    };
    let roman = |upper: bool| -> String {
        const T: [(u32, &str); 13] = [
            (1000, "m"),
            (900, "cm"),
            (500, "d"),
            (400, "cd"),
            (100, "c"),
            (90, "xc"),
            (50, "l"),
            (40, "xl"),
            (10, "x"),
            (9, "ix"),
            (5, "v"),
            (4, "iv"),
            (1, "i"),
        ];
        let mut k = n;
        let mut out = String::new();
        for (v, s) in T {
            while k >= v {
                out.push_str(s);
                k -= v;
            }
        }
        if upper { out.to_uppercase() } else { out }
    };
    match fmt {
        _ if n == 0 || (matches!(fmt, NumberFormat::LowerRoman | NumberFormat::UpperRoman) && n >= 4000) => {
            n.to_string()
        }
        NumberFormat::Decimal => n.to_string(),
        NumberFormat::LowerAlpha => alpha(false),
        NumberFormat::UpperAlpha => alpha(true),
        NumberFormat::LowerRoman => roman(false),
        NumberFormat::UpperRoman => roman(true),
    }
}
