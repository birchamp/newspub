//! Stories: attributed strings shared by a chain of linked text frames.
//!
//! Invariants (kept by [`Story::normalize`]):
//! - `chars` covers `text` exactly: the sum of span lengths equals `text.chars().count()`.
//! - `paras.len()` equals the number of `'\n'` in `text` plus one.
//! - No empty spans, and no two adjacent spans with equal attrs.

use crate::attrs::{CharAttrs, ParaAttrs};
use crate::{CoreError, Id};
use serde::{Deserialize, Serialize};
use std::ops::Range;

pub const PARA_SEP: char = '\n';
/// Explicit line break inside a paragraph (Shift+Enter).
pub const LINE_SEP: char = '\u{2028}';

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharSpan {
    pub len: usize,
    #[serde(default, skip_serializing_if = "CharAttrs::is_empty")]
    pub attrs: CharAttrs,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub id: Id,
    pub text: String,
    pub chars: Vec<CharSpan>,
    pub paras: Vec<ParaAttrs>,
    /// Ordered chain of text frames that display this story.
    pub frames: Vec<Id>,
}

/// Converts a char index into a byte index in `s` (clamped to the end).
pub fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

impl Story {
    pub fn new(id: Id) -> Story {
        Story { id, text: String::new(), chars: vec![], paras: vec![ParaAttrs::default()], frames: vec![] }
    }

    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Char ranges of each paragraph (excluding the separator).
    pub fn para_ranges(&self) -> Vec<Range<usize>> {
        let mut out = Vec::with_capacity(self.paras.len());
        let mut start = 0;
        for (i, c) in self.text.chars().enumerate() {
            if c == PARA_SEP {
                out.push(start..i);
                start = i + 1;
            }
        }
        out.push(start..self.len());
        out
    }

    /// Index of the paragraph containing char index `at`.
    pub fn para_index_at(&self, at: usize) -> usize {
        self.text.chars().take(at).filter(|&c| c == PARA_SEP).count()
    }

    /// Char attrs (run overrides only) at char index `at`, i.e. of the char at `at`,
    /// or of the char before it when `at` is the end.
    pub fn span_attrs_at(&self, at: usize) -> CharAttrs {
        let mut pos = 0;
        let mut last = None;
        for s in &self.chars {
            if at < pos + s.len {
                return s.attrs.clone();
            }
            pos += s.len;
            last = Some(&s.attrs);
        }
        last.cloned().unwrap_or_default()
    }

    fn check_range(&self, r: &Range<usize>) -> Result<(), CoreError> {
        if r.start > r.end || r.end > self.len() {
            return Err(CoreError::BadRange { start: r.start, end: r.end, len: self.len() });
        }
        Ok(())
    }

    /// Inserts `text` at char index `at`. New chars take `attrs` if given, otherwise the
    /// attrs of the preceding char (typing continues the current formatting).
    pub fn insert(&mut self, at: usize, text: &str, attrs: Option<CharAttrs>) -> Result<(), CoreError> {
        self.check_range(&(at..at))?;
        if text.is_empty() {
            return Ok(());
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let n = text.chars().count();
        let attrs = attrs.unwrap_or_else(|| self.span_attrs_at(at.saturating_sub(1)));
        // Paragraph attrs: a new paragraph inherits the attrs of the one it splits.
        let pi = self.para_index_at(at);
        let new_paras = text.matches(PARA_SEP).count();
        let pattrs = self.paras[pi].clone();
        for _ in 0..new_paras {
            self.paras.insert(pi + 1, pattrs.clone());
        }
        let b = byte_at(&self.text, at);
        self.text.insert_str(b, &text);
        self.splice_spans(at..at, vec![CharSpan { len: n, attrs }]);
        self.normalize();
        Ok(())
    }

    /// Deletes the chars in `r`. Paragraph attrs of the first paragraph win when paragraphs merge.
    pub fn delete(&mut self, r: Range<usize>) -> Result<(), CoreError> {
        self.check_range(&r)?;
        if r.is_empty() {
            return Ok(());
        }
        let first = self.para_index_at(r.start);
        let removed: usize = self.text.chars().skip(r.start).take(r.len()).filter(|&c| c == PARA_SEP).count();
        self.paras.drain(first + 1..first + 1 + removed);
        let (b0, b1) = (byte_at(&self.text, r.start), byte_at(&self.text, r.end));
        self.text.replace_range(b0..b1, "");
        self.splice_spans(r, vec![]);
        self.normalize();
        Ok(())
    }

    /// Applies `patch` over the run attrs in `r`.
    pub fn format_chars(&mut self, r: Range<usize>, patch: &CharAttrs) -> Result<(), CoreError> {
        self.check_range(&r)?;
        self.map_spans(r, |a| a.overlay(patch));
        Ok(())
    }

    /// Replaces the run attrs in `r` with `attrs` (used to clear overrides).
    pub fn set_chars(&mut self, r: Range<usize>, attrs: &CharAttrs) -> Result<(), CoreError> {
        self.check_range(&r)?;
        self.map_spans(r, |a| *a = attrs.clone());
        Ok(())
    }

    /// Applies `patch` to every paragraph touching `r`.
    pub fn format_paras(&mut self, r: Range<usize>, patch: &ParaAttrs) -> Result<(), CoreError> {
        self.check_range(&r)?;
        let (a, b) = (self.para_index_at(r.start), self.para_index_at(r.end));
        for p in &mut self.paras[a..=b] {
            p.overlay(patch);
        }
        Ok(())
    }

    fn map_spans(&mut self, r: Range<usize>, f: impl Fn(&mut CharAttrs)) {
        if r.is_empty() {
            return;
        }
        let mut pieces = self.take_spans(r.clone());
        for p in &mut pieces {
            f(&mut p.attrs);
        }
        self.splice_spans(r.start..r.start, pieces);
        self.normalize();
    }

    /// Removes and returns the spans covering `r`, leaving the rest in place.
    fn take_spans(&mut self, r: Range<usize>) -> Vec<CharSpan> {
        let mut out = vec![];
        let mut keep = vec![];
        let mut pos = 0;
        for s in self.chars.drain(..) {
            let (s0, s1) = (pos, pos + s.len);
            pos = s1;
            let (i0, i1) = (s0.max(r.start), s1.min(r.end));
            if i0 >= i1 {
                keep.push(s);
                continue;
            }
            if s0 < i0 {
                keep.push(CharSpan { len: i0 - s0, attrs: s.attrs.clone() });
            }
            out.push(CharSpan { len: i1 - i0, attrs: s.attrs.clone() });
            if i1 < s1 {
                keep.push(CharSpan { len: s1 - i1, attrs: s.attrs });
            }
        }
        self.chars = keep;
        out
    }

    /// Replaces the spans covering `r` with `new`.
    fn splice_spans(&mut self, r: Range<usize>, new: Vec<CharSpan>) {
        self.take_spans(r.clone());
        // Find the span index where position r.start begins, splitting if needed.
        let mut pos = 0;
        let mut idx = self.chars.len();
        for (i, s) in self.chars.iter().enumerate() {
            if pos == r.start {
                idx = i;
                break;
            }
            if pos + s.len > r.start {
                let left = r.start - pos;
                let right = CharSpan { len: s.len - left, attrs: s.attrs.clone() };
                self.chars[i].len = left;
                self.chars.insert(i + 1, right);
                idx = i + 1;
                break;
            }
            pos += s.len;
        }
        for (k, s) in new.into_iter().enumerate() {
            self.chars.insert(idx + k, s);
        }
    }

    /// Restores the invariants listed in the module docs.
    pub fn normalize(&mut self) {
        self.chars.retain(|s| s.len > 0);
        let mut merged: Vec<CharSpan> = Vec::with_capacity(self.chars.len());
        for s in self.chars.drain(..) {
            match merged.last_mut() {
                Some(last) if last.attrs == s.attrs => last.len += s.len,
                _ => merged.push(s),
            }
        }
        self.chars = merged;
        let n = self.len();
        let covered: usize = self.chars.iter().map(|s| s.len).sum();
        if covered < n {
            let attrs = self.chars.last().map(|s| s.attrs.clone()).unwrap_or_default();
            self.chars.push(CharSpan { len: n - covered, attrs });
        } else if covered > n {
            let mut excess = covered - n;
            while excess > 0 {
                let Some(last) = self.chars.last_mut() else { break };
                let d = excess.min(last.len);
                last.len -= d;
                excess -= d;
                if last.len == 0 {
                    self.chars.pop();
                }
            }
        }
        let want = self.text.matches(PARA_SEP).count() + 1;
        let fill = self.paras.last().cloned().unwrap_or_default();
        self.paras.resize(want, fill);
    }

    /// Iterates `(char_range, attrs)` for each run.
    pub fn runs(&self) -> impl Iterator<Item = (Range<usize>, &CharAttrs)> {
        let mut pos = 0;
        self.chars.iter().map(move |s| {
            let r = pos..pos + s.len;
            pos += s.len;
            (r, &s.attrs)
        })
    }

    /// Substring by char range.
    pub fn slice(&self, r: Range<usize>) -> &str {
        let (b0, b1) = (byte_at(&self.text, r.start), byte_at(&self.text, r.end));
        &self.text[b0..b1]
    }
}
