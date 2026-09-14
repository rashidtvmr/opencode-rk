use opencode_rk_providers::rate_limit::{
    plan_account_failure_lock, AccountFailureDecision, AccountFailurePatch, AccountLockScope,
    GenericFallbackOutcome, MAX_FAILURE_REASON_CHARS, MAX_RATE_LIMIT_COOLDOWN_MS,
};

fn generic(should_fallback: bool, cooldown_ms: u64, backoff_level: u32) -> GenericFallbackOutcome {
    GenericFallbackOutcome {
        should_fallback,
        cooldown_ms,
        backoff_level,
    }
}

fn patch(decision: AccountFailureDecision) -> AccountFailurePatch {
    match decision {
        AccountFailureDecision::Update(patch) => patch,
        AccountFailureDecision::Noop => panic!("expected an account failure patch"),
    }
}

#[test]
fn route_003_t01_missing_or_noauth_connection_is_a_noop() {
    for connection_id in [None, Some(""), Some("noauth")] {
        assert_eq!(
            plan_account_failure_lock(
                connection_id,
                429,
                "quota exceeded",
                Some("openai"),
                Some("gpt-5"),
                None,
                1_000,
                generic(true, 30_000, 2),
                2_000,
            ),
            AccountFailureDecision::Noop
        );
    }
}

#[test]
fn route_003_t02_github_monthly_402_uses_account_lock_and_supplied_reset() {
    let now = 1_000_000;
    let next_month_reset = 2_000_000;
    let decision = plan_account_failure_lock(
        Some("github-a"),
        402,
        "You've reached your additional usage limit for your plan. Go to GitHub settings for details.",
        Some("github"),
        Some("claude-fable-5"),
        None,
        now,
        generic(true, 120_000, 7),
        next_month_reset,
    );
    let patch = patch(decision);

    assert_eq!(patch.connection_id, "github-a");
    assert_eq!(patch.lock_scope, AccountLockScope::Account);
    assert_eq!(patch.lock_until_ms, next_month_reset);
    assert_eq!(patch.backoff_level, 0);
    assert_eq!(patch.error_code, 402);
    assert_eq!(patch.last_error_at_ms, now);
    assert!(patch.mark_unavailable);
}

#[test]
fn route_003_t03_unrelated_github_402_uses_model_scoped_generic_fallback_safely() {
    let now = u64::MAX - 60_000;
    let decision = plan_account_failure_lock(
        Some("github-a"),
        402,
        "Payment required",
        Some("github"),
        Some("claude-fable-5"),
        None,
        now,
        generic(true, 120_000, 4),
        u64::MAX,
    );
    let patch = patch(decision);

    assert_eq!(
        patch.lock_scope,
        AccountLockScope::Model("claude-fable-5".to_owned())
    );
    assert_eq!(patch.lock_until_ms, u64::MAX);
    assert_eq!(patch.backoff_level, 4);
    assert_eq!(patch.reason, "Payment required");
}

#[test]
fn route_003_t04_future_precise_reset_overrides_generic_with_cap_except_antigravity() {
    let now: u64 = 10_000;
    let precise_reset = now + MAX_RATE_LIMIT_COOLDOWN_MS + 45_000;
    let generic_outcome = generic(false, 1, 9);

    let capped = patch(plan_account_failure_lock(
        Some("codex-a"),
        429,
        "usage limit reached",
        Some("openai"),
        Some("gpt-5"),
        Some(precise_reset),
        now,
        generic_outcome,
        precise_reset,
    ));
    assert_eq!(
        capped.lock_until_ms,
        now.saturating_add(MAX_RATE_LIMIT_COOLDOWN_MS)
    );
    assert_eq!(capped.backoff_level, 0);

    let exact = patch(plan_account_failure_lock(
        Some("ag-a"),
        429,
        "quota exhausted",
        Some("antigravity"),
        Some("claude-opus-4-6-thinking"),
        Some(precise_reset),
        now,
        generic(false, 1, 9),
        precise_reset,
    ));
    assert_eq!(exact.lock_until_ms, precise_reset);
    assert_eq!(exact.backoff_level, 0);
}

#[test]
fn route_003_t05_reason_is_bounded_and_non_fallback_generic_outcome_has_no_patch() {
    let long_reason = "x".repeat(MAX_FAILURE_REASON_CHARS + 37);
    let decision = plan_account_failure_lock(
        Some("provider-a"),
        503,
        &long_reason,
        Some("provider"),
        Some("model-a"),
        None,
        50_000,
        generic(true, 30_000, 3),
        100_000,
    );
    let patch = patch(decision);

    assert_eq!(patch.reason.chars().count(), MAX_FAILURE_REASON_CHARS);
    assert_eq!(patch.reason, "x".repeat(MAX_FAILURE_REASON_CHARS));
    assert!(patch.mark_unavailable);

    assert_eq!(
        plan_account_failure_lock(
            Some("provider-a"),
            418,
            "not fallback eligible",
            Some("provider"),
            Some("model-a"),
            None,
            50_000,
            generic(false, 30_000, 8),
            100_000,
        ),
        AccountFailureDecision::Noop
    );
}
