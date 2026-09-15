use opencode_rk_providers::{
    rate_limit::MAX_FAILURE_REASON_CHARS,
    router::{
        AccountAttemptOutcome, AccountEligibilityError, AccountFallbackCoordinator,
        AccountFallbackDecision, AccountFallbackError, AccountFallbackExhaustion,
        AccountFallbackFailure, MAX_ACCOUNT_ID_BYTES, MAX_ACCOUNT_SELECTION_CANDIDATES,
    },
};

fn fallback_failure(status: u16, error: impl Into<String>) -> AccountAttemptOutcome {
    AccountAttemptOutcome::Failure {
        status,
        error: error.into(),
        should_fallback: true,
    }
}

#[test]
fn route_011_t01_fallback_failure_excludes_current_and_retains_only_bounded_last_failure() {
    let mut coordinator = AccountFallbackCoordinator::new();
    let long_error = "x".repeat(MAX_FAILURE_REASON_CHARS + 19);

    let decision = coordinator
        .record_attempt("account-a", fallback_failure(429, long_error))
        .expect("a first fallback-eligible failure should be accepted");

    assert_eq!(decision, AccountFallbackDecision::Retry);
    assert_eq!(
        coordinator.excluded_account_ids(),
        &["account-a".to_owned()]
    );
    assert_eq!(
        coordinator.last_failure(),
        Some(&AccountFallbackFailure {
            status: 429,
            error: "x".repeat(MAX_FAILURE_REASON_CHARS),
        })
    );
}

#[test]
fn route_011_t02_distinct_fallback_attempts_preserve_order_and_exhaustion_returns_last_failure() {
    let mut coordinator = AccountFallbackCoordinator::new();

    assert_eq!(
        coordinator
            .record_attempt("first", fallback_failure(503, "first failed"))
            .expect("first fallback should register"),
        AccountFallbackDecision::Retry
    );
    assert_eq!(
        coordinator
            .record_attempt("second", fallback_failure(429, "second failed"))
            .expect("second fallback should register"),
        AccountFallbackDecision::Retry
    );

    assert_eq!(
        coordinator.excluded_account_ids(),
        &["first".to_owned(), "second".to_owned()]
    );
    assert_eq!(
        coordinator.finish(AccountEligibilityError::Unavailable),
        AccountFallbackExhaustion::Exhausted {
            last_failure: AccountFallbackFailure {
                status: 429,
                error: "second failed".to_owned(),
            },
        }
    );
}

#[test]
fn route_011_t03_initial_unavailability_and_rate_limited_exhaustion_are_typed_distinctly() {
    let empty = AccountFallbackCoordinator::new();
    assert_eq!(
        empty.finish(AccountEligibilityError::Unavailable),
        AccountFallbackExhaustion::NoCredentials
    );
    assert_eq!(
        empty.finish(AccountEligibilityError::RateLimited { retry_at: 7_500 }),
        AccountFallbackExhaustion::RateLimited {
            retry_at: 7_500,
            last_failure: None,
        }
    );

    let mut after_fallback = AccountFallbackCoordinator::new();
    after_fallback
        .record_attempt("limited", fallback_failure(429, "quota"))
        .expect("fallback fixture should register");
    assert_eq!(
        after_fallback.finish(AccountEligibilityError::RateLimited { retry_at: 9_000 }),
        AccountFallbackExhaustion::RateLimited {
            retry_at: 9_000,
            last_failure: Some(AccountFallbackFailure {
                status: 429,
                error: "quota".to_owned(),
            }),
        }
    );
}

#[test]
fn route_011_t04_success_cancel_and_non_fallback_failure_terminate_without_new_exclusion() {
    let mut success = AccountFallbackCoordinator::new();
    success
        .record_attempt("prior", fallback_failure(503, "retry prior"))
        .expect("fallback fixture should register");
    let before_success = success.excluded_account_ids().to_vec();
    assert_eq!(
        success
            .record_attempt("next", AccountAttemptOutcome::Success)
            .expect("success should terminate"),
        AccountFallbackDecision::Success
    );
    assert_eq!(success.excluded_account_ids(), before_success.as_slice());

    let mut cancelled = AccountFallbackCoordinator::new();
    cancelled
        .record_attempt("prior", fallback_failure(503, "retry prior"))
        .expect("fallback fixture should register");
    let before_cancel = cancelled.excluded_account_ids().to_vec();
    assert_eq!(
        cancelled
            .record_attempt("next", AccountAttemptOutcome::Cancelled)
            .expect("caller cancellation should terminate"),
        AccountFallbackDecision::Cancelled
    );
    assert_eq!(cancelled.excluded_account_ids(), before_cancel.as_slice());

    let mut terminal_failure = AccountFallbackCoordinator::new();
    terminal_failure
        .record_attempt("prior", fallback_failure(503, "retry prior"))
        .expect("fallback fixture should register");
    let before_failure = terminal_failure.excluded_account_ids().to_vec();
    let direct_error = "caller-owned direct failure".repeat(32);
    assert_eq!(
        terminal_failure
            .record_attempt(
                "next",
                AccountAttemptOutcome::Failure {
                    status: 400,
                    error: direct_error.clone(),
                    should_fallback: false,
                },
            )
            .expect("non-fallback failure should terminate"),
        AccountFallbackDecision::Failure {
            status: 400,
            error: direct_error,
        }
    );
    assert_eq!(
        terminal_failure.excluded_account_ids(),
        before_failure.as_slice()
    );
    assert_eq!(
        terminal_failure.last_failure(),
        Some(&AccountFallbackFailure {
            status: 503,
            error: "retry prior".to_owned(),
        })
    );
}

#[test]
fn route_011_t05_invalid_duplicate_oversized_and_attempt_overflow_are_typed_and_non_mutating() {
    let mut coordinator = AccountFallbackCoordinator::new();

    assert_eq!(
        coordinator
            .record_attempt("", fallback_failure(503, "invalid"))
            .expect_err("empty account ids must be rejected"),
        AccountFallbackError::InvalidAccountId
    );
    assert!(coordinator.excluded_account_ids().is_empty());
    assert_eq!(coordinator.last_failure(), None);

    let oversized = "a".repeat(MAX_ACCOUNT_ID_BYTES + 1);
    assert_eq!(
        coordinator
            .record_attempt(&oversized, fallback_failure(503, "oversized"))
            .expect_err("oversized account ids must be rejected"),
        AccountFallbackError::AccountIdTooLong {
            max: MAX_ACCOUNT_ID_BYTES,
            actual: MAX_ACCOUNT_ID_BYTES + 1,
        }
    );
    assert!(coordinator.excluded_account_ids().is_empty());
    assert_eq!(coordinator.last_failure(), None);

    coordinator
        .record_attempt("duplicate", fallback_failure(503, "first"))
        .expect("first distinct account should register");
    let before_duplicate = coordinator.excluded_account_ids().to_vec();
    let before_duplicate_failure = coordinator.last_failure().cloned();
    assert_eq!(
        coordinator
            .record_attempt("duplicate", fallback_failure(429, "replacement"))
            .expect_err("duplicate fallback attempts must be rejected"),
        AccountFallbackError::DuplicateAccount {
            account_id: "duplicate".to_owned(),
        }
    );
    assert_eq!(
        coordinator.excluded_account_ids(),
        before_duplicate.as_slice()
    );
    assert_eq!(
        coordinator.last_failure(),
        before_duplicate_failure.as_ref()
    );

    let mut capped = AccountFallbackCoordinator::new();
    for index in 0..MAX_ACCOUNT_SELECTION_CANDIDATES {
        let error = if index + 1 == MAX_ACCOUNT_SELECTION_CANDIDATES {
            "z".repeat(MAX_FAILURE_REASON_CHARS + 11)
        } else {
            format!("failure-{index}")
        };
        capped
            .record_attempt(&format!("account-{index:02}"), fallback_failure(503, error))
            .expect("the public account-selection bound should also be the fallback-attempt bound");
    }
    assert_eq!(
        capped.excluded_account_ids().len(),
        MAX_ACCOUNT_SELECTION_CANDIDATES
    );
    assert_eq!(
        capped
            .last_failure()
            .expect("last fallback failure should be retained")
            .error
            .chars()
            .count(),
        MAX_FAILURE_REASON_CHARS
    );

    let before_overflow = capped.excluded_account_ids().to_vec();
    let before_overflow_failure = capped.last_failure().cloned();
    assert_eq!(
        capped
            .record_attempt("overflow", fallback_failure(503, "must not commit"))
            .expect_err("fallback attempts above the public candidate cap must be rejected"),
        AccountFallbackError::TooManyAttempts {
            max: MAX_ACCOUNT_SELECTION_CANDIDATES,
            actual: MAX_ACCOUNT_SELECTION_CANDIDATES + 1,
        }
    );
    assert_eq!(capped.excluded_account_ids(), before_overflow.as_slice());
    assert_eq!(capped.last_failure(), before_overflow_failure.as_ref());
}
