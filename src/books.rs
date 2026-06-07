use anyhow::{bail, Result};
use strsim::jaro_winkler;

pub const CANONICAL_BOOKS: &[(u32, &str)] = &[
    (1,  "Genesis"),
    (2,  "Exodus"),
    (3,  "Leviticus"),
    (4,  "Numbers"),
    (5,  "Deuteronomy"),
    (6,  "Joshua"),
    (7,  "Judges"),
    (8,  "Ruth"),
    (9,  "1 Samuel"),
    (10, "2 Samuel"),
    (11, "1 Kings"),
    (12, "2 Kings"),
    (13, "1 Chronicles"),
    (14, "2 Chronicles"),
    (15, "Ezra"),
    (16, "Nehemiah"),
    (17, "Esther"),
    (18, "Job"),
    (19, "Psalms"),
    (20, "Proverbs"),
    (21, "Ecclesiastes"),
    (22, "Song of Solomon"),
    (23, "Isaiah"),
    (24, "Jeremiah"),
    (25, "Lamentations"),
    (26, "Ezekiel"),
    (27, "Daniel"),
    (28, "Hosea"),
    (29, "Joel"),
    (30, "Amos"),
    (31, "Obadiah"),
    (32, "Jonah"),
    (33, "Micah"),
    (34, "Nahum"),
    (35, "Habakkuk"),
    (36, "Zephaniah"),
    (37, "Haggai"),
    (38, "Zechariah"),
    (39, "Malachi"),
    (40, "Matthew"),
    (41, "Mark"),
    (42, "Luke"),
    (43, "John"),
    (44, "Acts"),
    (45, "Romans"),
    (46, "1 Corinthians"),
    (47, "2 Corinthians"),
    (48, "Galatians"),
    (49, "Ephesians"),
    (50, "Philippians"),
    (51, "Colossians"),
    (52, "1 Thessalonians"),
    (53, "2 Thessalonians"),
    (54, "1 Timothy"),
    (55, "2 Timothy"),
    (56, "Titus"),
    (57, "Philemon"),
    (58, "Hebrews"),
    (59, "James"),
    (60, "1 Peter"),
    (61, "2 Peter"),
    (62, "1 John"),
    (63, "2 John"),
    (64, "3 John"),
    (65, "Jude"),
    (66, "Revelation"),
];

const MATCH_THRESHOLD: f64 = 0.80;

/// Normalize ordinal words to numbers so "First Kings" → "1 Kings".
fn normalize_ordinals(s: &str) -> String {
    s.to_lowercase()
        .replace("first ", "1 ")
        .replace("second ", "2 ")
        .replace("third ", "3 ")
        .replace("1st ", "1 ")
        .replace("2nd ", "2 ")
        .replace("3rd ", "3 ")
}

/// Return `(book_num, canonical_name)` or an error with a helpful message.
pub fn resolve_book(input: &str) -> Result<(u32, &'static str)> {
    let input_lower = input.to_lowercase();
    let input_norm = normalize_ordinals(input);

    // exact match first (case-insensitive, including normalized form)
    for &(num, name) in CANONICAL_BOOKS {
        let name_lower = name.to_lowercase();
        if name_lower == input_lower || name_lower == input_norm {
            return Ok((num, name));
        }
    }

    // fuzzy match via jaro_winkler against both raw and normalized forms
    let best = CANONICAL_BOOKS
        .iter()
        .map(|&(num, name)| {
            let name_lower = name.to_lowercase();
            let s1 = jaro_winkler(&input_lower, &name_lower);
            let s2 = jaro_winkler(&input_norm, &name_lower);
            (s1.max(s2), num, name)
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    match best {
        Some((score, num, name)) if score >= MATCH_THRESHOLD => Ok((num, name)),
        _ => bail!("Unknown book: '{}'. Try a standard Bible book name.", input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        let (num, name) = resolve_book("Genesis").unwrap();
        assert_eq!(num, 1);
        assert_eq!(name, "Genesis");
    }

    #[test]
    fn case_insensitive() {
        let (num, name) = resolve_book("genesis").unwrap();
        assert_eq!(num, 1);
        assert_eq!(name, "Genesis");
    }

    #[test]
    fn abbreviated_gen() {
        let (num, _) = resolve_book("Gen").unwrap();
        assert_eq!(num, 1);
    }

    #[test]
    fn fuzzy_first_kings() {
        let (num, name) = resolve_book("First Kings").unwrap();
        assert_eq!(num, 11);
        assert_eq!(name, "1 Kings");
    }

    #[test]
    fn unknown_book_returns_error() {
        assert!(resolve_book("Hezekiah").is_err());
    }

    #[test]
    fn revelation() {
        let (num, _) = resolve_book("Rev").unwrap();
        assert_eq!(num, 66);
    }
}
