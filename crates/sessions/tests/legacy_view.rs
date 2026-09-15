use opencode_rk_sessions::legacy_view::{project_legacy, LegacyViewError, MAX_LEGACY_SESSIONS};

#[test]
fn legacy_t01_projects_in_order() {
    let input = [
        ("a", "Alpha", false),
        ("b", "Beta", true),
        ("c", "Gamma", false),
    ];
    let got = project_legacy(&input).unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].id, "a");
    assert_eq!(got[0].title, "Alpha");
    assert!(!got[0].archived);
    assert_eq!(got[1].id, "b");
    assert_eq!(got[1].title, "Beta");
    assert!(got[1].archived);
    assert_eq!(got[2].id, "c");
    assert_eq!(got[2].title, "Gamma");
    assert!(!got[2].archived);
}

#[test]
fn legacy_t02_archived_flag_kept() {
    let input = [("x", "X", true), ("y", "Y", false)];
    let got = project_legacy(&input).unwrap();
    assert!(got[0].archived);
    assert!(!got[1].archived);
}

#[test]
fn legacy_t03_empty_id_rejected() {
    let input = [("ok", "Ok", false), ("", "NoId", false)];
    assert!(matches!(
        project_legacy(&input),
        Err(LegacyViewError::EmptyId)
    ));
}

#[test]
fn legacy_t04_empty_title_rejected() {
    let input = [("ok", "", false)];
    assert!(matches!(
        project_legacy(&input),
        Err(LegacyViewError::EmptyTitle)
    ));
}

#[test]
fn legacy_t05_overflow_rejected() {
    let owned: Vec<(String, String, bool)> = (0..MAX_LEGACY_SESSIONS + 1)
        .map(|i| (format!("id-{i}"), format!("t-{i}"), false))
        .collect();
    let refs: Vec<(&str, &str, bool)> = owned
        .iter()
        .map(|(id, title, archived)| (id.as_str(), title.as_str(), *archived))
        .collect();
    match project_legacy(&refs) {
        Err(LegacyViewError::TooManySessions { max, actual }) => {
            assert_eq!(max, MAX_LEGACY_SESSIONS);
            assert_eq!(actual, MAX_LEGACY_SESSIONS + 1);
        }
        other => panic!("expected TooManySessions, got {other:?}"),
    }
}
