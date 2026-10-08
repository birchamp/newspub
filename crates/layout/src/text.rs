//! Fast story slicing during a layout pass. `Story::slice` converts char indices to byte offsets by scanning
//! from the start, which made layout quadratic in story length; here each story's offset table is built once
//! per pass. Tables are keyed by the text buffer's address and length and are dropped when the outermost pass
//! ends, so a table never outlives the immutable document it was built from.

use newpub_core::Story;
use std::cell::RefCell;
use std::ops::Range;

struct Memo {
    depth: usize,
    tables: Vec<(usize, usize, Vec<usize>)>,
}

thread_local! {
    static MEMO: RefCell<Memo> = const { RefCell::new(Memo { depth: 0, tables: Vec::new() }) };
}

/// Scope of one layout pass (re-entrant).
pub(crate) struct Pass;

impl Pass {
    pub(crate) fn begin() -> Pass {
        MEMO.with(|m| m.borrow_mut().depth += 1);
        Pass
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        MEMO.with(|m| {
            let mut m = m.borrow_mut();
            m.depth = m.depth.saturating_sub(1);
            if m.depth == 0 {
                m.tables.clear();
            }
        });
    }
}

/// `story.slice(r)` in O(1) after the first call for that story in the current pass.
pub(crate) fn slice(story: &Story, r: Range<usize>) -> &str {
    let text = story.text.as_str();
    let key = (text.as_ptr() as usize, text.len());
    let bytes = MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.depth == 0 {
            return None;
        }
        let i = match m.tables.iter().position(|t| (t.0, t.1) == key) {
            Some(i) => i,
            None => {
                let mut off: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
                off.push(text.len());
                m.tables.push((key.0, key.1, off));
                m.tables.len() - 1
            }
        };
        let off = &m.tables[i].2;
        let at = |c: usize| off[c.min(off.len() - 1)];
        Some((at(r.start), at(r.end.max(r.start))))
    });
    match bytes {
        Some((b0, b1)) => &text[b0..b1],
        None => story.slice(r),
    }
}
