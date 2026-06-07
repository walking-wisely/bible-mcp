use bible_mcp::books::resolve_book;

#[test]
fn gen_variants() {
    for input in &["Genesis", "genesis", "Gen", "gen"] {
        let (num, name) = resolve_book(input).unwrap();
        assert_eq!(num, 1, "failed on '{input}'");
        assert_eq!(name, "Genesis");
    }
}

#[test]
fn numbered_books() {
    assert_eq!(resolve_book("1 Kings").unwrap().0, 11);
    assert_eq!(resolve_book("2 Kings").unwrap().0, 12);
    assert_eq!(resolve_book("1 Corinthians").unwrap().0, 46);
    assert_eq!(resolve_book("Revelation").unwrap().0, 66);
}

#[test]
fn fuzzy_variants() {
    assert_eq!(resolve_book("Psalms").unwrap().0, 19);
    assert_eq!(resolve_book("Psalm").unwrap().0, 19);
    assert_eq!(resolve_book("Rev").unwrap().0, 66);
    assert_eq!(resolve_book("Matt").unwrap().0, 40);
}

#[test]
fn completely_unknown_fails() {
    assert!(resolve_book("Hezekiah").is_err());
    assert!(resolve_book("xyz123").is_err());
}
