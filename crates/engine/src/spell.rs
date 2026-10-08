//! Spell checking (SP-01..SP-03).

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::Id;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Session state owned by this module.
#[derive(Default)]
pub struct State {
    /// Words ignored for this session (lowercase).
    pub ignored: Vec<String>,
}

struct Found {
    story: Id,
    start: usize,
    end: usize,
    word: String,
    lang: String,
}

fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

impl Session {
    pub(crate) fn spell_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        match a {
            SessionAction::IgnoreWord { word } => {
                let w = word.trim().to_lowercase();
                if !w.is_empty() && !self.spell.ignored.contains(&w) {
                    self.spell.ignored.push(w);
                }
                Ok(Outcome::default())
            }
            other => Err(EngineError::Other(format!("{other:?} is not a spelling action"))),
        }
    }

    /// All words of all stories with their resolved language, in story-id then position order.
    fn spell_words(&self) -> Vec<Found> {
        let doc = &self.doc;
        let mut out = Vec::new();
        for (id, story) in &doc.stories {
            let mut para_starts = vec![0usize];
            for (i, c) in story.text.chars().enumerate() {
                if c == '\n' {
                    para_starts.push(i + 1);
                }
            }
            // Resolved language of every char.
            let mut langs: Vec<String> = Vec::with_capacity(story.len());
            for (range, attrs) in story.runs() {
                for i in range {
                    let p = para_starts.partition_point(|&s| s <= i).saturating_sub(1);
                    let lang = match story.paras.get(p) {
                        Some(pa) => doc.resolve_char(pa, attrs).lang,
                        None => "en-US".to_string(),
                    };
                    langs.push(lang);
                }
            }
            let chars: Vec<char> = story.text.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if !chars[i].is_alphanumeric() {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric()
                        || (is_apostrophe(chars[i]) && chars.get(i + 1).is_some_and(|n| n.is_alphabetic())))
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                if word.chars().any(|c| c.is_numeric()) {
                    continue;
                }
                let lang = langs.get(start).cloned().unwrap_or_else(|| "en-US".to_string());
                out.push(Found { story: *id, start, end: i, word, lang });
            }
        }
        out
    }

    pub(crate) fn spell_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        let words = self.spell_words();
        match q {
            Query::Spelling => {
                let mut res = Vec::new();
                for f in &words {
                    if f.lang == "zxx" {
                        continue;
                    }
                    let Some(checker) = newpub_spell::checker(&f.lang) else { continue };
                    let lower = f.word.to_lowercase();
                    let known = self.doc.custom_words.iter().any(|c| c.to_lowercase() == lower)
                        || self.spell.ignored.contains(&lower);
                    // Typographic apostrophes are looked up as ASCII.
                    let plain = f.word.replace('\u{2019}', "'");
                    if known || checker.check(&plain) || checker.check(&plain.to_lowercase()) {
                        continue;
                    }
                    let mut suggestions = checker.suggest(&plain);
                    suggestions.truncate(8);
                    res.push(json!({
                        "story": f.story, "start": f.start, "end": f.end, "word": f.word,
                        "suggestions": suggestions,
                    }));
                }
                Ok(Value::Array(res))
            }
            Query::SpellingLanguages => {
                let missing: BTreeSet<&str> = words
                    .iter()
                    .map(|f| f.lang.as_str())
                    .filter(|l| *l != "zxx" && newpub_spell::canonical_language(l).is_none())
                    .collect();
                Ok(json!({ "installed": newpub_spell::INSTALLED, "missing": missing }))
            }
            other => Err(EngineError::Other(format!("query {other:?} is not a spelling query"))),
        }
    }
}
