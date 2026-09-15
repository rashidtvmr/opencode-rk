use opencode_rk_tools::compat_commands::{CompatError, MAX_ALIASES, resolve_alias};

#[test]
fn compat_t01_known_alias_resolves() {
    let out = resolve_alias(&[("old-cmd", "new-cmd")], "old-cmd").expect("known alias resolves");
    assert_eq!(out, "new-cmd");
}

#[test]
fn compat_t02_canonical_passthrough_when_listed() {
    // Canonical value alone is not a match: no guessing.
    let err =
        resolve_alias(&[("old-cmd", "new-cmd")], "new-cmd").expect_err("canonical must be Unknown");
    assert!(matches!(err, CompatError::UnknownAlias { .. }));
}

#[test]
fn compat_t03_unknown_rejected() {
    let err = resolve_alias(&[("old-cmd", "new-cmd")], "nope").expect_err("unknown rejected");
    assert!(matches!(err, CompatError::UnknownAlias { .. }));
}

#[test]
fn compat_t04_empty_rejected() {
    let err = resolve_alias(&[("old-cmd", "new-cmd")], "").expect_err("empty rejected");
    assert!(matches!(err, CompatError::EmptyName));
}

#[test]
fn compat_t05_overflow_rejected() {
    let pairs: Vec<(String, String)> = (0..MAX_ALIASES + 1)
        .map(|i| (format!("old-{i}"), format!("new-{i}")))
        .collect();
    let refs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let err = resolve_alias(&refs, "old-0").expect_err("overflow rejected");
    assert!(matches!(err, CompatError::TooManyAliases { .. }));
}
