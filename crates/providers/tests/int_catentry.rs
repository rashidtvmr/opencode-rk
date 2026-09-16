use opencode_rk_providers::int_catentry::{make_entry, CatEntryError};

#[test]
fn ce_t01_valid() {
    let e = make_entry("a1", "Title One").unwrap();
    assert_eq!(e.id, "a1");
    assert_eq!(e.title, "Title One");
}

#[test]
fn t02_empty_id() {
    assert!(matches!(make_entry("", "T"), Err(CatEntryError::EmptyId)));
    assert!(matches!(
        make_entry("   ", "T"),
        Err(CatEntryError::EmptyId)
    ));
}

#[test]
fn t03_empty_title() {
    assert!(matches!(
        make_entry("a1", ""),
        Err(CatEntryError::EmptyTitle)
    ));
    assert!(matches!(
        make_entry("a1", "   "),
        Err(CatEntryError::EmptyTitle)
    ));
}

#[test]
fn t04_trims() {
    let e = make_entry("  a1  ", "  Title One  ").unwrap();
    assert_eq!(e.id, "a1");
    assert_eq!(e.title, "Title One");
}

#[test]
fn t05_both_empty_id_first() {
    assert!(matches!(make_entry("", ""), Err(CatEntryError::EmptyId)));
    assert!(matches!(
        make_entry("   ", "   "),
        Err(CatEntryError::EmptyId)
    ));
}
