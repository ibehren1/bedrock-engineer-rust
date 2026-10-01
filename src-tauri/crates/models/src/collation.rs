//! Approximation of JS `String.prototype.localeCompare` (ICU root / `en` collation) for the
//! ASCII strings used as model display names.
//!
//! ICU orders by primary weight first (case-insensitive; whitespace < punctuation < symbols <
//! digits < letters), then by case (lowercase before uppercase). Characters outside ASCII sort
//! after all ASCII by code point. The tests check this against the order the TS code produced
//! (`regionOrder` in `data/models.json`).

use std::cmp::Ordering;

/// CLDR root order of ASCII punctuation and symbols (before digits and letters).
const PUNCT_ORDER: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn primary(c: char) -> (u8, u32) {
    if c.is_ascii_whitespace() {
        return (0, c as u32);
    }
    if let Some(pos) = PUNCT_ORDER.find(c) {
        return (1, pos as u32);
    }
    if c.is_ascii_digit() {
        return (2, c as u32);
    }
    if c.is_ascii_alphabetic() {
        return (3, c.to_ascii_lowercase() as u32);
    }
    (4, c as u32)
}

/// Compares two strings the way `a.localeCompare(b)` does for ASCII display names.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let primary_cmp = a.chars().map(primary).cmp(b.chars().map(primary));
    if primary_cmp != Ordering::Equal {
        return primary_cmp;
    }
    // Tertiary level: lowercase sorts before uppercase.
    let case_key = |c: char| u8::from(c.is_ascii_uppercase());
    a.chars()
        .map(case_key)
        .cmp(b.chars().map(case_key))
        .then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_like_icu() {
        assert_eq!(locale_compare("a", "B"), Ordering::Less);
        assert_eq!(locale_compare("a", "A"), Ordering::Less);
        assert_eq!(locale_compare("GPT-6 Sol", "GPT-6.1 Sol"), Ordering::Less);
        assert_eq!(locale_compare("X (US)", "X-1"), Ordering::Less);
        assert_eq!(locale_compare("X-1", "X(1"), Ordering::Less);
        assert_eq!(locale_compare("Z1", "Za"), Ordering::Less);
        assert_eq!(locale_compare("same", "same"), Ordering::Equal);
    }
}
