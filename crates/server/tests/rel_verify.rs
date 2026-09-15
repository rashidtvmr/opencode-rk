use opencode_rk_server::rel_verify::{build_report, parse_verdict, Verdict, VerifyError};

#[test]
fn ver_t01_pass() {
    let r = build_report("suite-a", 5, 0).unwrap();
    assert_eq!(r.verdict, "pass");
    assert_eq!(r.checks, 5);
}

#[test]
fn ver_t02_fail() {
    let r = build_report("suite-a", 5, 2).unwrap();
    assert_eq!(r.verdict, "fail");
    assert_eq!(r.checks, 5);
}

#[test]
fn ver_t03_empty_suite() {
    assert!(matches!(
        build_report("", 5, 0),
        Err(VerifyError::EmptySuite)
    ));
    assert!(matches!(
        build_report("", 5, 1),
        Err(VerifyError::EmptySuite)
    ));
}

#[test]
fn ver_t04_zero_checks() {
    assert!(matches!(
        build_report("suite-a", 0, 0),
        Err(VerifyError::ZeroChecks)
    ));
    assert!(matches!(
        build_report("", 0, 0),
        Err(VerifyError::EmptySuite)
    ));
}

#[test]
fn ver_t05_parse() {
    assert_eq!(parse_verdict("pass"), Ok(Verdict::Pass));
    assert_eq!(parse_verdict("PASS"), Ok(Verdict::Pass));
    assert_eq!(parse_verdict("Fail"), Ok(Verdict::Fail));
    assert_eq!(parse_verdict("FAIL"), Ok(Verdict::Fail));
    assert!(matches!(
        parse_verdict("bogus"),
        Err(VerifyError::ZeroChecks)
    ));
    assert!(matches!(parse_verdict(""), Err(VerifyError::ZeroChecks)));
}
