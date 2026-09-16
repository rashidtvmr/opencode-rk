use opencode_rk_server::web_footer::{build_footer, FooterError};

#[test]
fn foot_t01_valid() {
    let f = build_footer("hello", "1.0.0").unwrap();
    assert_eq!(f.text, "hello");
    assert_eq!(f.version, "1.0.0");
}

#[test]
fn foot_t02_empty_text() {
    assert!(matches!(
        build_footer("", "1.0.0"),
        Err(FooterError::EmptyText)
    ));
    assert!(matches!(
        build_footer("   ", "1.0.0"),
        Err(FooterError::EmptyText)
    ));
}

#[test]
fn foot_t03_empty_version() {
    assert!(matches!(
        build_footer("hello", ""),
        Err(FooterError::EmptyVersion)
    ));
    assert!(matches!(
        build_footer("hello", "   "),
        Err(FooterError::EmptyVersion)
    ));
}

#[test]
fn foot_t04_trims() {
    let f = build_footer("  hello  ", "  1.0.0  ").unwrap();
    assert_eq!(f.text, "hello");
    assert_eq!(f.version, "1.0.0");
}

#[test]
fn foot_t05_both_empty_text_first() {
    assert!(matches!(build_footer("", ""), Err(FooterError::EmptyText)));
    assert!(matches!(
        build_footer("   ", "   "),
        Err(FooterError::EmptyText)
    ));
}
