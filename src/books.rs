use anyhow::{bail, Result};
use strsim::jaro_winkler;

pub const CANONICAL_BOOKS: &[(u32, &str)] = &[
    (1, "Genesis"),
    (2, "Exodus"),
    (3, "Leviticus"),
    (4, "Numbers"),
    (5, "Deuteronomy"),
    (6, "Joshua"),
    (7, "Judges"),
    (8, "Ruth"),
    (9, "1 Samuel"),
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

const ALIASES: &[(u32, &[&str])] = &[
    (1, &["gen", "ge", "gn"]),
    (2, &["exo", "ex", "exod"]),
    (3, &["lev", "le", "lv"]),
    (4, &["num", "nu", "nm", "nb"]),
    (5, &["deut", "deu", "dt"]),
    (6, &["josh", "jos", "jsh"]),
    (7, &["judg", "jdg", "jg", "jdgs"]),
    (8, &["rut", "ru"]),
    (9, &["1sam", "1sa", "1sm", "1s"]),
    (10, &["2sam", "2sa", "2sm", "2s"]),
    (11, &["1kgs", "1ki", "1k", "1kin"]),
    (12, &["2kgs", "2ki", "2k", "2kin"]),
    (13, &["1chr", "1ch", "1chron"]),
    (14, &["2chr", "2ch", "2chron"]),
    (15, &["ezr"]),
    (16, &["neh", "ne"]),
    (17, &["est", "esth"]),
    (18, &["jb"]),
    (19, &["ps", "psa", "psalm", "pss", "psm"]),
    (20, &["prov", "pro", "prv", "pr"]),
    (21, &["eccl", "eccles", "ecc", "ec", "qoh"]),
    (22, &["song", "sos", "ss", "songofsongs", "canticles", "sg"]),
    (23, &["isa", "is"]),
    (24, &["jer", "je", "jr"]),
    (25, &["lam", "la"]),
    (26, &["ezek", "eze", "ezk"]),
    (27, &["dan", "da", "dn"]),
    (28, &["hos", "ho"]),
    (29, &["joe", "jl"]),
    (30, &["am"]),
    (31, &["obad", "ob"]),
    (32, &["jon", "jnh"]),
    (33, &["mic", "mi"]),
    (34, &["nah", "na"]),
    (35, &["hab", "hb"]),
    (36, &["zeph", "zep", "zp"]),
    (37, &["hag", "hg"]),
    (38, &["zech", "zec", "zc"]),
    (39, &["mal", "ml"]),
    (40, &["matt", "mat", "mt"]),
    (41, &["mrk", "mar", "mk", "mr"]),
    (42, &["luk", "lk"]),
    (43, &["joh", "jhn", "jn"]),
    (44, &["act", "ac"]),
    (45, &["rom", "ro", "rm"]),
    (46, &["1cor", "1co"]),
    (47, &["2cor", "2co"]),
    (48, &["gal", "ga"]),
    (49, &["eph", "ephes"]),
    (50, &["phil", "php", "pp"]),
    (51, &["col", "co"]),
    (52, &["1thess", "1thes", "1th"]),
    (53, &["2thess", "2thes", "2th"]),
    (54, &["1tim", "1ti"]),
    (55, &["2tim", "2ti"]),
    (56, &["tit"]),
    (57, &["philem", "phm", "pm"]),
    (58, &["heb"]),
    (59, &["jas", "jm"]),
    (60, &["1pet", "1pe", "1pt", "1p"]),
    (61, &["2pet", "2pe", "2pt", "2p"]),
    (62, &["1jn", "1jo", "1joh", "1jhn"]),
    (63, &["2jn", "2jo", "2joh", "2jhn"]),
    (64, &["3jn", "3jo", "3joh", "3jhn"]),
    (65, &["jud", "jde"]),
    (66, &["rev", "re", "revelations", "apocalypse"]),
];

fn compact(input: &str) -> String {
    let normalized = normalize_ordinals(input);
    let mut words = normalized.split_whitespace();
    let mut out = String::new();
    if let Some(first) = words.next() {
        match first {
            "i" | "ii" | "iii" if normalized.split_whitespace().count() > 1 => {
                out.push_str(&first.len().to_string());
            }
            other => out.push_str(other),
        }
    }
    for w in words {
        out.push_str(w);
    }
    out.retain(|c| c.is_alphanumeric());
    out
}

fn resolve_alias(input: &str) -> Option<(u32, &'static str)> {
    let key = compact(&input.to_lowercase());
    let num = CANONICAL_BOOKS
        .iter()
        .find(|(_, name)| compact(&name.to_lowercase()) == key)
        .map(|&(n, _)| n)
        .or_else(|| {
            ALIASES
                .iter()
                .find(|(_, names)| names.contains(&key.as_str()))
                .map(|&(n, _)| n)
        })?;
    CANONICAL_BOOKS.iter().copied().find(|&(n, _)| n == num)
}

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

    if let Some(found) = resolve_alias(input) {
        return Ok(found);
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
    fn abbreviations_resolve_to_the_right_book() {
        for (input, want) in [
            ("Mt", "Matthew"),
            ("Mk", "Mark"),
            ("Jas", "James"),
            ("1Cor", "1 Corinthians"),
            ("Phil", "Philippians"),
            ("Philem", "Philemon"),
            ("Jn", "John"),
            ("Jon", "Jonah"),
            ("I Kings", "1 Kings"),
            ("II Kings", "2 Kings"),
            ("1 Jn", "1 John"),
            ("Ps.", "Psalms"),
            ("Song of Songs", "Song of Solomon"),
            ("1corinthians", "1 Corinthians"),
        ] {
            assert_eq!(resolve_book(input).unwrap().1, want, "{input}");
        }
    }

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
