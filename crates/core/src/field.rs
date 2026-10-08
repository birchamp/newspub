//! Fields: single story characters (U+FFFC) whose displayed text is computed at layout time
//! (page numbers, page count, dates, mail-merge fields). The field kind is stored in the char's
//! `CharAttrs::field`. Sections control page-number labels.

use crate::Id;
use crate::attrs::NumberFormat;
use serde::{Deserialize, Serialize};

/// The story character that stands for a field.
pub const FIELD_CHAR: char = '\u{FFFC}';

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    /// Label of the page showing the field (respects sections).
    PageNumber,
    /// Total number of pages.
    PageCount,
    /// Number of pages in the current section.
    SectionPageCount,
    /// Mail-merge field by column name.
    Merge(String),
    /// Business information field by key (BB-05), e.g. `organization`, `phone`.
    Business(String),
}

/// A section starts at a page and restarts page labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// First page of the section (page id, so it survives page moves).
    pub page: Id,
    #[serde(default = "one")]
    pub start_at: u32,
    #[serde(default = "decimal")]
    pub format: NumberFormat,
}

fn one() -> u32 {
    1
}
fn decimal() -> NumberFormat {
    NumberFormat::Decimal
}

/// Formats `n` in a number format (1 → "1", "a", "A", "i", "I").
pub fn format_number(n: u32, f: NumberFormat) -> String {
    match f {
        NumberFormat::Decimal => n.to_string(),
        NumberFormat::LowerAlpha => alpha(n).to_lowercase(),
        NumberFormat::UpperAlpha => alpha(n),
        NumberFormat::LowerRoman => roman(n).to_lowercase(),
        NumberFormat::UpperRoman => roman(n),
    }
}

/// 1 → A, 26 → Z, 27 → AA (Publisher/Word style repeats the letter).
fn alpha(n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let letter = (b'A' + ((n - 1) % 26) as u8) as char;
    std::iter::repeat_n(letter, ((n - 1) / 26 + 1) as usize).collect()
}

fn roman(mut n: u32) -> String {
    if n == 0 || n >= 4000 {
        return n.to_string();
    }
    let table = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}
