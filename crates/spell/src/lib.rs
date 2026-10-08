//! newpub-spell: Hunspell-compatible spell checking (SP-01..SP-03).

use spellbook::Dictionary;
use std::sync::OnceLock;

const EN_US_AFF: &str = include_str!("../../../assets/dict/en_US.aff");
const EN_US_DIC: &str = include_str!("../../../assets/dict/en_US.dic");

/// Canonical tags of the installed dictionaries.
pub const INSTALLED: &[&str] = &["en-US"];

/// A loaded dictionary.
pub struct Checker {
    dict: Dictionary,
}

impl Checker {
    /// Is `word` spelled correctly?
    pub fn check(&self, word: &str) -> bool {
        self.dict.check(word)
    }

    /// Suggested corrections for `word` (possibly empty).
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out
    }
}

/// Maps a language tag to the canonical tag of an installed dictionary, if any.
pub fn canonical_language(lang: &str) -> Option<&'static str> {
    match lang.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "en" | "en-us" => Some("en-US"),
        _ => None,
    }
}

/// The checker for a language tag ("en-US", "en", "en-us"), loaded lazily once. `None` if not installed.
pub fn checker(lang: &str) -> Option<&'static Checker> {
    static EN_US: OnceLock<Option<Checker>> = OnceLock::new();
    match canonical_language(lang)? {
        "en-US" => {
            EN_US.get_or_init(|| Dictionary::new(EN_US_AFF, EN_US_DIC).ok().map(|dict| Checker { dict })).as_ref()
        }
        _ => None,
    }
}
