//! TOOL-020 frozen tests: failure-only RTK filters (mirror RTK-TEST-T01..T05).
//!
//! Included via path so the owning module needs no shared-file wiring;
//! the integrator re-exports it from `lib.rs` per PLAN.md section 5.

#[path = "../src/rtk_testfilter.rs"]
mod rtk_testfilter;

use rtk_testfilter::{FilterOpts, TestTool, filter_test_output, MAX_KEPT_BYTES};

fn opts() -> FilterOpts {
    FilterOpts::default()
}

#[test]
fn tool020_t01_allpass_verdict_only() {
    let mut log = String::from("collected 5 items\n");
    for i in 0..200 {
        log.push_str(&format!("tests/test_calc.py::test_case_{i:03} PASSED\n"));
    }
    log.push_str("5 passed in 1.23s\n");
    let out = filter_test_output(TestTool::Pytest, &log, 0, &opts());
    assert!(out.text.contains("5 passed"), "missing verdict: {:?}", out.text);
    assert!(out.verdict.contains("5 passed"), "verdict field empty: {:?}", out.verdict);
    for line in out.text.lines() {
        assert!(!line.contains("PASSED"), "leaked passing line: {line}");
        assert!(
            !line.trim_end().ends_with("ok"),
            "leaked per-test ok line: {line}"
        );
    }
    assert_eq!(out.exit, 0);
    assert!(!out.truncated);
}

#[test]
fn tool020_t02_failure_shows_failing_test_plus_diff() {
    let log = [
        "collected 2 items",
        "tests/test_auth.py::test_pass_login PASSED",
        "tests/test_auth.py::test_x FAILED",
        "________________ test_x ________________",
        "",
        "    def test_x():",
        ">       assert 1 == 2",
        "E       assert 1 == 2",
        "",
        "1 failed, 1 passed in 0.45s",
    ]
    .join("\n")
        + "\n";
    let out = filter_test_output(TestTool::Pytest, &log, 1, &opts());
    assert!(out.text.contains("test_x"), "missing failing id: {:?}", out.text);
    assert!(
        out.text.contains("assert 1 == 2"),
        "missing diff: {:?}",
        out.text
    );
    assert!(
        !out.text.contains("test_pass_login"),
        "leaked passing id: {:?}",
        out.text
    );
    assert_eq!(out.exit, 1);
    assert!(!out.truncated);
}

#[test]
fn tool020_t03_exit_code_preserved() {
    let cases: &[(TestTool, &str, i32)] = &[
        (TestTool::Pytest, "FAILED tests/test_a.py::test_x\n1 failed in 0.3s\n", 1),
        (
            TestTool::CargoTest,
            "test foo::bar FAILED\ntest result: FAILED. 0 passed; 1 failed;\n",
            101,
        ),
        (
            TestTool::Tsc,
            "src/a.ts(1,1): error TS2322: Type X\nFound 1 error in 1 file.\n",
            2,
        ),
        (
            TestTool::Lint,
            "src/a.ts\n  1:1  error  msg  rule\n1 problem (1 error, 0 warnings)\n",
            1,
        ),
    ];
    for (tool, log, exit) in cases {
        let out = filter_test_output(*tool, log, *exit, &opts());
        assert_eq!(out.exit, *exit, "exit remapped for {tool:?}");
        assert_ne!(out.exit, 0, "non-zero mapped to zero for {tool:?}");
        assert!(!out.text.is_empty(), "empty output for {tool:?}");
    }
}

#[test]
fn tool020_t04_verdict_never_filtered() {
    let failing: &[(TestTool, &str, &str, i32)] = &[
        (
            TestTool::Pytest,
            "tests/test_a.py::test_x FAILED\n1 failed, 1 passed in 0.45s\n",
            "1 failed, 1 passed in 0.45s",
            1,
        ),
        (
            TestTool::CargoTest,
            "running 4 tests\ntest a::t1 ... ok\ntest a::t2 ... FAILED\nfailures:\n    a::t2\ntest result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n",
            "test result: FAILED",
            101,
        ),
        (
            TestTool::Tsc,
            "src/a.ts(2,5): error TS2322: bad assign.\nsrc/b.ts(1,1): error TS1005: ';' expected.\nFound 3 errors in 2 files.\n",
            "Found 3 errors",
            2,
        ),
        (
            TestTool::Lint,
            "src/a.ts\n  2:3  error  Unexpected var  no-var\n1 problem (1 error, 0 warnings)\n",
            "1 problem",
            1,
        ),
    ];
    for (tool, log, verdict, exit) in failing {
        let out = filter_test_output(*tool, log, *exit, &opts());
        assert!(
            out.text.contains(*verdict),
            "missing verdict {verdict:?} for {tool:?}: {:?}",
            out.text
        );
    }
    let passing: &[(TestTool, &str, &str)] = &[
        (TestTool::Pytest, "5 passed in 1.23s\n", "5 passed"),
        (
            TestTool::CargoTest,
            "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s\n",
            "test result: ok",
        ),
        (TestTool::Tsc, "", "ok (exit 0)"),
        (TestTool::Lint, "0 problems\n", "0 problems"),
    ];
    for (tool, log, verdict) in passing {
        let out = filter_test_output(*tool, log, 0, &opts());
        assert!(
            out.text.contains(*verdict),
            "missing all-pass verdict {verdict:?} for {tool:?}: {:?}",
            out.text
        );
    }
}

#[test]
fn tool020_t05_overbudget_truncates_with_marker() {
    let mut log = String::new();
    for i in 0..6000 {
        log.push_str(&format!(
            "FAILED tests/test_big.py::test_huge_fail[{i}] E AssertionError: assert {i} == {i}XXXX padding padding padding\n"
        ));
    }
    log.push_str("1 failed in 12.30s\n");
    assert!(log.len() > 500 * 1024, "fixture under 500 KiB: {}", log.len());
    let out = filter_test_output(TestTool::Pytest, &log, 1, &opts());
    assert!(
        out.text.len() <= MAX_KEPT_BYTES + 1024,
        "over budget: {}",
        out.text.len()
    );
    assert!(out.truncated);
    let last = out.text.lines().last().unwrap_or_default();
    assert!(
        last.contains("[rtk: truncated"),
        "marker not on tail line: {last:?}"
    );
    assert!(
        out.text.contains("1 failed in 12.30s"),
        "verdict lost under truncation"
    );
    assert!(
        out.text.contains("test_huge_fail"),
        "failing id lost under truncation"
    );
}
