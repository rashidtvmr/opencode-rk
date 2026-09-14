use opencode_rk_sessions::auto_compact::{
    evaluate_compact_policy, record_compaction_result, CompactAction, CompactPolicyInput,
    CompactPolicyState, TokenWarningState, AUTOCOMPACT_BUFFER, MANUAL_COMPACT_BUFFER,
    MAX_CONSECUTIVE_FAILURES, MAX_OUTPUT_TOKENS_FOR_SUMMARY, WARNING_BUFFER,
};

fn input(token_count: u32, effective_context_window: u32) -> CompactPolicyInput {
    CompactPolicyInput {
        token_count,
        effective_context_window,
        manual_requested: false,
        disable_compact: false,
        disable_auto_compact: false,
    }
}

#[test]
fn sess_020_t01_adopts_compaction_constants_and_buffers() {
    assert_eq!(MAX_OUTPUT_TOKENS_FOR_SUMMARY, 20_000);
    assert_eq!(AUTOCOMPACT_BUFFER, 13_000);
    assert_eq!(WARNING_BUFFER, 20_000);
    assert_eq!(MANUAL_COMPACT_BUFFER, 3_000);
    assert_eq!(MAX_CONSECUTIVE_FAILURES, 3);
}

#[test]
fn sess_020_t02_auto_compacts_proactively_at_effective_context_minus_buffer() {
    let state = CompactPolicyState::default();
    let below_threshold = evaluate_compact_policy(&input(86_999, 100_000), &state);
    let at_threshold = evaluate_compact_policy(&input(87_000, 100_000), &state);

    assert_eq!(below_threshold.action, CompactAction::None);
    assert_eq!(at_threshold.action, CompactAction::AutoCompact);
}

#[test]
fn sess_020_t03_warning_state_tracks_remaining_context_budget() {
    let state = CompactPolicyState::default();

    let normal = evaluate_compact_policy(&input(79_999, 100_000), &state);
    let warning = evaluate_compact_policy(&input(80_000, 100_000), &state);
    let error = evaluate_compact_policy(&input(100_000, 100_000), &state);

    assert_eq!(normal.warning, TokenWarningState::Normal);
    assert_eq!(warning.warning, TokenWarningState::Warning);
    assert_eq!(error.warning, TokenWarningState::Error);
}

#[test]
fn sess_020_t04_three_failures_open_breaker_and_success_resets_it() {
    let mut state = CompactPolicyState::default();
    let threshold_input = input(87_000, 100_000);

    for expected_failures in 1..=MAX_CONSECUTIVE_FAILURES {
        record_compaction_result(&mut state, false);
        assert_eq!(state.consecutive_failures, expected_failures);
    }

    let blocked = evaluate_compact_policy(&threshold_input, &state);
    assert_eq!(blocked.action, CompactAction::None);
    assert!(blocked.breaker_open);

    record_compaction_result(&mut state, true);
    assert_eq!(state.consecutive_failures, 0);

    let recovered = evaluate_compact_policy(&threshold_input, &state);
    assert_eq!(recovered.action, CompactAction::AutoCompact);
    assert!(!recovered.breaker_open);
}

#[test]
fn sess_020_t05_explicit_kill_switch_inputs_disable_compaction_without_global_state() {
    let state = CompactPolicyState::default();

    let mut disable_auto = input(87_000, 100_000);
    disable_auto.disable_auto_compact = true;
    assert_eq!(
        evaluate_compact_policy(&disable_auto, &state).action,
        CompactAction::None
    );

    disable_auto.manual_requested = true;
    assert_eq!(
        evaluate_compact_policy(&disable_auto, &state).action,
        CompactAction::ManualCompact,
        "disabling auto compact must preserve an explicit manual compact request"
    );

    let mut disable_all = input(87_000, 100_000);
    disable_all.manual_requested = true;
    disable_all.disable_compact = true;
    assert_eq!(
        evaluate_compact_policy(&disable_all, &state).action,
        CompactAction::None,
        "the compact kill switch must suppress both automatic and manual compaction"
    );
}
