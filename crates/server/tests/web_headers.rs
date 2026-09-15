use opencode_rk_server::web_headers::{make_header, SecHeaderError};

#[test]
fn hdr_t01_valid() {
    let h = make_header("X-Frame-Options", "DENY").unwrap();
    assert_eq!(h.name, "X-Frame-Options");
    assert_eq!(h.value, "DENY");
}

#[test]
fn hdr_t02_empty_name() {
    assert!(matches!(
        make_header("", "DENY"),
        Err(SecHeaderError::EmptyName)
    ));
    assert!(matches!(
        make_header("   ", "DENY"),
        Err(SecHeaderError::EmptyName)
    ));
}

#[test]
fn hdr_t03_empty_value() {
    assert!(matches!(
        make_header("X-Frame-Options", ""),
        Err(SecHeaderError::EmptyValue)
    ));
    assert!(matches!(
        make_header("X-Frame-Options", "   "),
        Err(SecHeaderError::EmptyValue)
    ));
}

#[test]
fn hdr_t04_trims() {
    let h = make_header("  X-Frame-Options  ", "  DENY  ").unwrap();
    assert_eq!(h.name, "X-Frame-Options");
    assert_eq!(h.value, "DENY");
}

#[test]
fn hdr_t05_both_empty_name_first() {
    assert!(matches!(
        make_header("", ""),
        Err(SecHeaderError::EmptyName)
    ));
    assert!(matches!(
        make_header("   ", "   "),
        Err(SecHeaderError::EmptyName)
    ));
}
