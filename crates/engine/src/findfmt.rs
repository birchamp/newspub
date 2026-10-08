//! Find and replace formatting and special characters (FR-03).

use crate::{EngineError, Outcome, Query, Session, SessionAction, find_matches};
use newpub_core::attrs::{CharAttrs, ParaAttrs};
use newpub_core::{Command, Document, Story};
use serde_json::{Value, json};
use std::ops::Range;

/// Expands the search/replace codes (^p ^l ^t ^s ^- ^^) into the characters they stand for.
fn expand_codes(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '^' {
            out.push(c);
            continue;
        }
        let rep = match it.peek() {
            Some('p') => '\n',
            Some('l') => '\u{2028}',
            Some('t') => '\t',
            Some('s') => '\u{00A0}',
            Some('-') => '\u{00AD}',
            Some('^') => '^',
            _ => {
                out.push('^');
                continue;
            }
        };
        it.next();
        out.push(rep);
    }
    out
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Per-char flags: does the char's resolved formatting satisfy every field set in `fmt`?
fn char_matches(doc: &Document, story: &Story, fmt: &CharAttrs) -> Vec<bool> {
    let want = doc.resolve_char(&ParaAttrs::default(), fmt);
    let mut out = vec![false; story.len()];
    let mut para_starts: Vec<usize> = story.para_ranges().into_iter().map(|r| r.start).collect();
    para_starts.push(usize::MAX);
    for (range, attrs) in story.runs() {
        // A run may span paragraphs: split it at paragraph starts.
        let mut seg_start = range.start;
        while seg_start < range.end {
            let pi = para_starts.iter().rposition(|&s| s <= seg_start).unwrap_or(0).min(story.paras.len() - 1);
            let next_para = para_starts.get(pi + 1).copied().unwrap_or(usize::MAX);
            let seg_end = range.end.min(next_para);
            let have = doc.resolve_char(&story.paras[pi], attrs);
            let ok = fmt.font.as_ref().is_none_or(|_| have.font == want.font)
                && fmt.size.is_none_or(|_| same(have.size, want.size))
                && fmt.bold.is_none_or(|v| have.bold == v)
                && fmt.italic.is_none_or(|v| have.italic == v)
                && fmt.underline.is_none_or(|v| have.underline == v)
                && fmt.strike.is_none_or(|v| have.strike == v)
                && fmt.color.as_ref().is_none_or(|_| have.color == want.color)
                && fmt.tracking.is_none_or(|v| same(have.tracking, v))
                && fmt.scale.is_none_or(|v| same(have.scale, v))
                && fmt.kerning.is_none_or(|v| have.kerning == v)
                && fmt.ligatures.is_none_or(|v| have.ligatures == v)
                && fmt.dlig.is_none_or(|v| have.dlig == v)
                && fmt.features.as_ref().is_none_or(|v| have.features == *v)
                && fmt.baseline.is_none_or(|v| have.baseline == v)
                && fmt.caps.is_none_or(|v| have.caps == v)
                && fmt.lang.as_ref().is_none_or(|v| have.lang == *v);
            if ok {
                for f in &mut out[seg_start..seg_end] {
                    *f = true;
                }
            }
            seg_start = seg_end;
        }
    }
    out
}

/// Matches in one story: text and/or format.
fn story_matches(
    doc: &Document,
    story: &Story,
    text: Option<&str>,
    format: Option<&CharAttrs>,
    match_case: bool,
    whole_word: bool,
) -> Vec<Range<usize>> {
    let flags = format.map(|f| char_matches(doc, story, f));
    match text {
        Some(t) => find_matches(&story.text, t, match_case, whole_word)
            .into_iter()
            .filter(|&(s, e)| flags.as_ref().is_none_or(|f| f[s..e].iter().all(|&b| b)))
            .map(|(s, e)| s..e)
            .collect(),
        None => {
            let Some(flags) = flags else { return vec![] };
            let mut out = Vec::new();
            let mut start = None;
            for (i, c) in story.text.chars().enumerate() {
                let hit = flags[i] && c != '\n';
                match (hit, start) {
                    (true, None) => start = Some(i),
                    (false, Some(s)) => {
                        out.push(s..i);
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(s) = start {
                out.push(s..flags.len());
            }
            out
        }
    }
}

/// Normalises an optional pattern: codes expanded, empty means "no text".
fn pattern(s: &Option<String>) -> Option<String> {
    s.as_deref().map(expand_codes).filter(|s| !s.is_empty())
}

fn format_opt(f: Option<&CharAttrs>) -> Option<&CharAttrs> {
    f.filter(|f| !f.is_empty())
}

impl Session {
    pub(crate) fn findfmt_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        let SessionAction::ReplaceAdvanced { find, find_format, replace, replace_format, match_case, whole_word } = a
        else {
            return Err(EngineError::Other(format!("{a:?} is not a find/replace action")));
        };
        let text = pattern(find);
        let fmt = format_opt(find_format.as_ref());
        if text.is_none() && fmt.is_none() {
            return Err(EngineError::Nothing("find"));
        }
        let repl = replace.as_deref().map(expand_codes);
        let rfmt = format_opt(replace_format.as_ref());
        let mut d = self.doc.clone();
        for (sid, story) in &self.doc.stories {
            let matches = story_matches(&self.doc, story, text.as_deref(), fmt, *match_case, *whole_word);
            for r in matches.into_iter().rev() {
                let mut end = r.end;
                if let Some(t) = &repl {
                    d.apply(&Command::ReplaceText { target: *sid, start: r.start, end: r.end, text: t.clone() })?;
                    end = r.start + t.chars().count();
                }
                if let Some(attrs) = rfmt
                    && end > r.start
                {
                    d.apply(&Command::FormatChars {
                        target: *sid,
                        start: Some(r.start),
                        end: Some(end),
                        attrs: attrs.clone(),
                    })?;
                }
            }
        }
        self.commit(d, None);
        Ok(Outcome::default())
    }

    pub(crate) fn findfmt_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        let Query::FindAdvanced { text, format, match_case, whole_word } = q else {
            return Err(EngineError::Other(format!("query {q:?} is not a find query")));
        };
        let text = pattern(text);
        let fmt = format_opt(format.as_deref());
        if text.is_none() && fmt.is_none() {
            return Err(EngineError::Nothing("find"));
        }
        let mut out = Vec::new();
        for (sid, story) in &self.doc.stories {
            for r in story_matches(&self.doc, story, text.as_deref(), fmt, *match_case, *whole_word) {
                out.push(json!({"story": sid, "start": r.start, "end": r.end}));
            }
        }
        Ok(Value::Array(out))
    }
}
