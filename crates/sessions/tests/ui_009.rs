use opencode_rk_sessions::ui_009::{parse_theme, theme_label, Theme, ThemeError};

#[test]
fn ui009_t01_parses_dark() {
    assert_eq!(parse_theme("dark").unwrap(), Theme::Dark);
    assert_eq!(parse_theme("light").unwrap(), Theme::Light);
    assert_eq!(parse_theme("system").unwrap(), Theme::System);
}

#[test]
fn ui009_t02_case_insensitive() {
    assert_eq!(parse_theme("  DARK  ").unwrap(), Theme::Dark);
    assert_eq!(parse_theme("Light").unwrap(), Theme::Light);
    assert_eq!(parse_theme("SYSTEM").unwrap(), Theme::System);
}

#[test]
fn ui009_t03_labels() {
    assert_eq!(theme_label(&Theme::Light), "Light");
    assert_eq!(theme_label(&Theme::Dark), "Dark");
    assert_eq!(theme_label(&Theme::System), "System");
}

#[test]
fn ui009_t04_unknown_rejected() {
    match parse_theme("midnight") {
        Err(ThemeError::UnknownTheme { name }) => assert_eq!(name, "midnight"),
        other => panic!("expected UnknownTheme, got {other:?}"),
    }
}

#[test]
fn ui009_t05_empty_rejected() {
    assert!(matches!(
        parse_theme(""),
        Err(ThemeError::UnknownTheme { .. })
    ));
    assert!(matches!(
        parse_theme("   "),
        Err(ThemeError::UnknownTheme { .. })
    ));
}
