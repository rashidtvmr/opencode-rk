use opencode_rk_providers::recording::{MAX_RECORDED_CALLS, RecordedCall, RecordingError, summarize_calls};

fn call(provider: &str, input: u64, output: u64, failed: bool) -> RecordedCall {
    RecordedCall {
        provider_id: provider.to_string(),
        model_id: "m".to_string(),
        input_tokens: input,
        output_tokens: output,
        duration_ms: 1,
        failed,
    }
}

#[test]
fn recording_t01_empty_is_zero_summary() {
    let s = summarize_calls(&[]).expect("empty ok");
    assert_eq!(s.calls, 0);
    assert_eq!(s.total_input, 0);
    assert_eq!(s.total_output, 0);
    assert_eq!(s.failures, 0);
}

#[test]
fn recording_t02_totals_accumulate() {
    let calls = vec![call("a", 10, 20, false), call("a", 5, 7, false)];
    let s = summarize_calls(&calls).expect("ok");
    assert_eq!(s.calls, 2);
    assert_eq!(s.total_input, 15);
    assert_eq!(s.total_output, 27);
    assert_eq!(s.failures, 0);
}

#[test]
fn recording_t03_failures_counted() {
    let calls = vec![call("a", 1, 1, true), call("a", 1, 1, false), call("a", 1, 1, true)];
    let s = summarize_calls(&calls).expect("ok");
    assert_eq!(s.calls, 3);
    assert_eq!(s.failures, 2);
}

#[test]
fn recording_t04_empty_provider_rejected() {
    let calls = vec![call("", 1, 1, false)];
    let err = summarize_calls(&calls).expect_err("empty provider");
    assert!(matches!(err, RecordingError::EmptyProviderId));
}

#[test]
fn recording_t05_overflow_rejected() {
    let calls: Vec<RecordedCall> = (0..MAX_RECORDED_CALLS + 1).map(|_| call("a", 1, 1, false)).collect();
    let err = summarize_calls(&calls).expect_err("overflow");
    assert!(matches!(err, RecordingError::TooManyCalls { max, actual } if max == MAX_RECORDED_CALLS && actual == MAX_RECORDED_CALLS + 1));
}
