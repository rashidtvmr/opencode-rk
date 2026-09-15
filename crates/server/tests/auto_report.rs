use opencode_rk_server::auto_report::{build_report, ReportError, MAX_REPORT_LINES};

#[test]
fn rep_t01_valid() {
    let r = build_report("Weekly", &["a", "b"]).unwrap();
    assert_eq!(r.title, "Weekly");
    assert_eq!(r.lines, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn rep_t02_empty_title() {
    assert!(matches!(
        build_report("", &["a"]),
        Err(ReportError::EmptyTitle)
    ));
    assert!(matches!(
        build_report("   ", &["a"]),
        Err(ReportError::EmptyTitle)
    ));
    assert!(matches!(
        build_report("\t\n ", &[]),
        Err(ReportError::EmptyTitle)
    ));
}

#[test]
fn rep_t03_overflow() {
    let owned: Vec<String> = (0..MAX_REPORT_LINES + 1).map(|i| format!("l{i}")).collect();
    let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
    match build_report("T", &refs) {
        Err(ReportError::TooManyLines { max, actual }) => {
            assert_eq!(max, MAX_REPORT_LINES);
            assert_eq!(max, 500);
            assert_eq!(actual, MAX_REPORT_LINES + 1);
        }
        other => panic!("expected TooManyLines, got {other:?}"),
    }
}

#[test]
fn rep_t04_trims() {
    let r = build_report("  hi  ", &["x"]).unwrap();
    assert_eq!(r.title, "hi");
}

#[test]
fn rep_t05_empty_lines_ok() {
    let r = build_report("T", &[]).unwrap();
    assert_eq!(r.title, "T");
    assert!(r.lines.is_empty());
}
