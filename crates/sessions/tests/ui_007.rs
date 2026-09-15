use opencode_rk_sessions::ui_007::{effort_label, parse_effort, Effort, EffortError};

#[test]
fn ui007_t01_parses_low() {
    assert!(matches!(parse_effort("low"), Ok(Effort::Low)));
    assert!(matches!(parse_effort("medium"), Ok(Effort::Medium)));
    assert!(matches!(parse_effort("high"), Ok(Effort::High)));
}

#[test]
fn ui007_t02_case_insensitive() {
    assert!(matches!(parse_effort("LOW"), Ok(Effort::Low)));
    assert!(matches!(parse_effort("Medium"), Ok(Effort::Medium)));
    assert!(matches!(parse_effort("  HIGH  "), Ok(Effort::High)));
}

#[test]
fn ui007_t03_labels() {
    assert_eq!(effort_label(&Effort::Low), "Low");
    assert_eq!(effort_label(&Effort::Medium), "Medium");
    assert_eq!(effort_label(&Effort::High), "High");
}

#[test]
fn ui007_t04_unknown_rejected() {
    match parse_effort("ultra") {
        Err(EffortError::UnknownEffort { name }) => assert_eq!(name, "ultra"),
        other => panic!("expected UnknownEffort, got {other:?}"),
    }
}

#[test]
fn ui007_t05_empty_rejected() {
    assert!(matches!(
        parse_effort(""),
        Err(EffortError::UnknownEffort { .. })
    ));
    assert!(matches!(
        parse_effort("   "),
        Err(EffortError::UnknownEffort { .. })
    ));
}
