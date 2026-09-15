use opencode_rk_sessions::ui_010::{filter_commands, PaletteError, MAX_PALETTE_ENTRIES};

fn cmds(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn ui010_t01_empty_returns_all() {
    let c = cmds(&["/init", "/build", "/test"]);
    assert_eq!(filter_commands(&c, "").unwrap(), c);
}

#[test]
fn ui010_t02_filters_substring() {
    let c = cmds(&["/init", "/commit", "/compact"]);
    assert_eq!(
        filter_commands(&c, "com").unwrap(),
        cmds(&["/commit", "/compact"])
    );
}

#[test]
fn ui010_t03_case_insensitive() {
    let c = cmds(&["/Init", "/BUILD", "/test"]);
    assert_eq!(filter_commands(&c, "init").unwrap(), cmds(&["/Init"]));
    assert_eq!(filter_commands(&c, "BUILD").unwrap(), cmds(&["/BUILD"]));
}

#[test]
fn ui010_t04_no_match_empty() {
    let c = cmds(&["/init", "/build"]);
    assert!(filter_commands(&c, "zzz").unwrap().is_empty());
}

#[test]
fn ui010_t05_overflow_rejected() {
    let c: Vec<String> = (0..MAX_PALETTE_ENTRIES + 1)
        .map(|i| format!("/cmd-{i}"))
        .collect();
    match filter_commands(&c, "") {
        Err(PaletteError::TooManyEntries { max, actual }) => {
            assert_eq!(max, MAX_PALETTE_ENTRIES);
            assert_eq!(actual, MAX_PALETTE_ENTRIES + 1);
        }
        other => panic!("expected TooManyEntries, got {other:?}"),
    }
}
