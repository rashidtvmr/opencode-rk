use opencode_rk_sessions::share_policy::{
    parse_visibility, visibility_label, SharePolicyError, Visibility,
};

#[test]
fn sharepol_t01_parses() {
    assert_eq!(parse_visibility("private").unwrap(), Visibility::Private);
    assert_eq!(parse_visibility("link").unwrap(), Visibility::Link);
    assert_eq!(
        parse_visibility("workspace").unwrap(),
        Visibility::Workspace
    );
}

#[test]
fn sharepol_t02_case_insensitive() {
    assert_eq!(parse_visibility("PRIVATE").unwrap(), Visibility::Private);
    assert_eq!(parse_visibility(" Link ").unwrap(), Visibility::Link);
    assert_eq!(
        parse_visibility("WORKSPACE").unwrap(),
        Visibility::Workspace
    );
}

#[test]
fn sharepol_t03_labels() {
    assert_eq!(visibility_label(&Visibility::Private), "Private");
    assert_eq!(visibility_label(&Visibility::Link), "Link");
    assert_eq!(visibility_label(&Visibility::Workspace), "Workspace");
}

#[test]
fn sharepol_t04_unknown_rejected() {
    assert!(matches!(
        parse_visibility("public"),
        Err(SharePolicyError::UnknownVisibility { .. })
    ));
}

#[test]
fn sharepol_t05_empty_rejected() {
    assert!(matches!(
        parse_visibility(""),
        Err(SharePolicyError::UnknownVisibility { .. })
    ));
    assert!(matches!(
        parse_visibility("   "),
        Err(SharePolicyError::UnknownVisibility { .. })
    ));
}
