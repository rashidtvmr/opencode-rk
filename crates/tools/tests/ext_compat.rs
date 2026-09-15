use opencode_rk_tools::ext_compat::{CompatError, CompatVerdict, check_compat, verdict_label};

#[test]
fn compat2_t01_ok() {
    assert_eq!(check_compat("tool-a", false), Ok(CompatVerdict::Ok));
}

#[test]
fn compat2_t02_shimmed() {
    assert_eq!(check_compat("tool-a", true), Ok(CompatVerdict::Shimmed));
}

#[test]
fn compat2_t03_empty() {
    assert_eq!(check_compat("", false), Err(CompatError::EmptyName));
    assert_eq!(check_compat("", true), Err(CompatError::EmptyName));
}

#[test]
fn compat2_t04_labels() {
    assert_eq!(verdict_label(&CompatVerdict::Ok), "ok");
    assert_eq!(verdict_label(&CompatVerdict::Shimmed), "shimmed");
    assert_eq!(verdict_label(&CompatVerdict::Blocked), "blocked");
}

#[test]
fn compat2_t05_whitespace_empty() {
    assert_eq!(check_compat("   ", false), Err(CompatError::EmptyName));
    assert_eq!(check_compat(" \t\n ", true), Err(CompatError::EmptyName));
}
