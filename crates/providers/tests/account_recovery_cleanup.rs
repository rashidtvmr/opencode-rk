use opencode_rk_providers::rate_limit::{
    plan_account_recovery, AccountLock, AccountLockScope, AccountRecoveryError,
    AccountRecoveryState, MAX_ACCOUNT_MODEL_LOCKS,
};

fn model_lock(model: &str, expires_at: u64) -> AccountLock {
    AccountLock {
        scope: AccountLockScope::Model(model.to_owned()),
        expires_at,
    }
}

fn account_wide_lock(expires_at: u64) -> AccountLock {
    AccountLock {
        scope: AccountLockScope::AccountWide,
        expires_at,
    }
}

fn state(test_status_unavailable: bool, last_error_present: bool, backoff_level: u32) -> AccountRecoveryState {
    AccountRecoveryState {
        test_status_unavailable,
        last_error_present,
        backoff_level,
    }
}

#[test]
fn route_004_t01_missing_noauth_and_already_clean_state_are_no_ops() {
    let clean = state(false, false, 0);

    assert_eq!(
        plan_account_recovery(None, Some("model-a"), &clean, &[], 100),
        Ok(None)
    );
    assert_eq!(
        plan_account_recovery(Some("noauth"), Some("model-a"), &clean, &[], 100),
        Ok(None)
    );
    assert_eq!(
        plan_account_recovery(Some("conn-a"), Some("model-a"), &clean, &[], 100),
        Ok(None)
    );
}

#[test]
fn route_004_t02_success_clears_current_model_account_wide_and_expired_locks() {
    let locks = vec![
        model_lock("model-a", 500),
        account_wide_lock(700),
        model_lock("expired-b", 100),
        model_lock("expired-c", 99),
    ];

    let patch = plan_account_recovery(
        Some("conn-a"),
        Some("model-a"),
        &state(true, true, 3),
        &locks,
        100,
    )
    .expect("bounded cleanup planning should succeed")
    .expect("dirty state with locks should produce a patch");

    assert_eq!(
        patch.clear_locks,
        vec![
            AccountLockScope::Model("model-a".to_owned()),
            AccountLockScope::AccountWide,
            AccountLockScope::Model("expired-b".to_owned()),
            AccountLockScope::Model("expired-c".to_owned()),
        ]
    );
}

#[test]
fn route_004_t03_active_unrelated_lock_remains_and_prevents_error_reset() {
    let locks = vec![
        model_lock("model-a", 500),
        model_lock("model-b", 900),
    ];

    let patch = plan_account_recovery(
        Some("conn-a"),
        Some("model-a"),
        &state(true, true, 4),
        &locks,
        100,
    )
    .expect("bounded cleanup planning should succeed")
    .expect("the successful model lock should produce a clear patch");

    assert_eq!(
        patch.clear_locks,
        vec![AccountLockScope::Model("model-a".to_owned())]
    );
    assert!(!patch.set_test_status_active);
    assert!(!patch.clear_last_error);
    assert!(!patch.reset_backoff_level);
}

#[test]
fn route_004_t04_when_no_active_locks_remain_error_test_and_backoff_reset_is_requested() {
    let locks = vec![
        model_lock("model-a", 500),
        account_wide_lock(800),
        model_lock("expired-b", 100),
    ];

    let patch = plan_account_recovery(
        Some("conn-a"),
        Some("model-a"),
        &state(true, true, 7),
        &locks,
        100,
    )
    .expect("bounded cleanup planning should succeed")
    .expect("dirty state should produce a recovery patch");

    assert!(patch.set_test_status_active);
    assert!(patch.clear_last_error);
    assert!(patch.reset_backoff_level);
}

#[test]
fn route_004_t05_lock_bound_accepts_exact_limit_and_rejects_overflow_with_typed_error() {
    assert_eq!(MAX_ACCOUNT_MODEL_LOCKS, 16);

    let at_limit = (0..MAX_ACCOUNT_MODEL_LOCKS)
        .map(|index| model_lock(&format!("model-{index}"), 100))
        .collect::<Vec<_>>();
    let patch = plan_account_recovery(
        Some("conn-a"),
        Some("successful"),
        &state(false, false, 0),
        &at_limit,
        100,
    )
    .expect("the exact public lock bound must be accepted")
    .expect("expired locks at the bound should produce a cleanup patch");
    assert_eq!(patch.clear_locks.len(), MAX_ACCOUNT_MODEL_LOCKS);

    let mut overflow = at_limit;
    overflow.push(model_lock("overflow", 100));
    assert_eq!(
        plan_account_recovery(
            Some("conn-a"),
            Some("successful"),
            &state(false, false, 0),
            &overflow,
            100,
        ),
        Err(AccountRecoveryError::TooManyLocks {
            max: MAX_ACCOUNT_MODEL_LOCKS,
            actual: MAX_ACCOUNT_MODEL_LOCKS + 1,
        })
    );
}
