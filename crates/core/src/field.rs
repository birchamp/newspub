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
    /// Today's date when the publication is laid out, printed or exported.
    Date(DateFormat),
}

/// How a date field shows the date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    /// "May 1, 2026"
    #[default]
    Long,
    /// "5/1/2026"
    Short,
    /// "2026-05-01"
    Iso,
    /// "1 May 2026"
    DayMonthYear,
}

/// Today's (year, month, day) in UTC. `NEWPUB_TODAY=YYYY-MM-DD` fixes it (journeys, reproducible output).
pub fn today() -> (i64, u32, u32) {
    if let Some((y, m, d)) = std::env::var("NEWPUB_TODAY").ok().and_then(|s| {
        let mut it = s.trim().split('-').map(|p| p.parse::<i64>().ok());
        Some((it.next()??, it.next()?? as u32, it.next()?? as u32))
    }) {
        return (y, m, d);
    }
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0);
    civil_from_days(days)
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian (H. Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A date in one of the field formats.
pub fn format_date((y, m, d): (i64, u32, u32), f: DateFormat) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month = MONTHS[(m.clamp(1, 12) - 1) as usize];
    match f {
        DateFormat::Long => format!("{month} {d}, {y}"),
        DateFormat::Short => format!("{m}/{d}/{y}"),
        DateFormat::Iso => format!("{y:04}-{m:02}-{d:02}"),
        DateFormat::DayMonthYear => format!("{d} {month} {y}"),
    }
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
