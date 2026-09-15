use opencode_rk_foundation::runtime_report::{summarize, ReportError, RuntimeSample, MAX_SAMPLES};

#[test]
fn rep_t01_empty_healthy() {
    let report = summarize(&[]).expect("empty must succeed");
    assert_eq!(report.total_tasks, 0);
    assert_eq!(report.total_errors, 0);
    assert!(report.healthy);
}

#[test]
fn rep_t02_totals() {
    let samples = [
        RuntimeSample { tasks: 3, errors: 0 },
        RuntimeSample { tasks: 7, errors: 1 },
    ];
    let report = summarize(&samples).expect("totals must succeed");
    assert_eq!(report.total_tasks, 10);
    assert_eq!(report.total_errors, 1);
    assert!(!report.healthy);
}

#[test]
fn rep_t03_unhealthy() {
    let samples = [RuntimeSample { tasks: 0, errors: 2 }];
    let report = summarize(&samples).expect("unhealthy must succeed");
    assert!(!report.healthy);
}

#[test]
fn rep_t04_saturating() {
    let samples = [
        RuntimeSample { tasks: u64::MAX, errors: u64::MAX },
        RuntimeSample { tasks: 1, errors: 1 },
    ];
    let report = summarize(&samples).expect("saturating must succeed");
    assert_eq!(report.total_tasks, u64::MAX);
    assert_eq!(report.total_errors, u64::MAX);
    assert!(!report.healthy);
}

#[test]
fn rep_t05_overflow() {
    let samples = vec![RuntimeSample { tasks: 1, errors: 0 }; MAX_SAMPLES + 1];
    assert_eq!(
        summarize(&samples).unwrap_err(),
        ReportError::TooManySamples { max: MAX_SAMPLES, actual: MAX_SAMPLES + 1 }
    );
}
