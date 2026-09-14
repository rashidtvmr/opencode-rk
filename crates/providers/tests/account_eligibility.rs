use opencode_rk_providers::router::{
    eligible_accounts, AccountCandidate, AccountEligibilityError,
};

fn account(
    id: &str,
    priority: u32,
    active: bool,
    excluded: bool,
    model_lock_until: Option<u64>,
) -> AccountCandidate {
    AccountCandidate {
        id: id.to_owned(),
        priority,
        active,
        excluded,
        model_lock_until,
    }
}

#[test]
fn route_001_t01_active_unlocked_non_excluded_accounts_sort_by_priority_then_id() {
    let candidates = vec![
        account("zeta", 2, true, false, None),
        account("beta", 1, true, false, None),
        account("alpha", 1, true, false, None),
    ];

    let eligible = eligible_accounts(&candidates, 100).expect("eligible accounts should exist");

    let ids: Vec<&str> = eligible.iter().map(|candidate| candidate.id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "beta", "zeta"]);
}

#[test]
fn route_001_t02_inactive_excluded_and_currently_locked_accounts_are_filtered() {
    let candidates = vec![
        account("eligible", 4, true, false, None),
        account("inactive", 1, false, false, None),
        account("excluded", 2, true, true, None),
        account("locked", 3, true, false, Some(150)),
    ];

    let eligible = eligible_accounts(&candidates, 100).expect("one account remains eligible");

    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].id, "eligible");
}

#[test]
fn route_001_t03_expired_model_lock_is_eligible_at_caller_supplied_now() {
    let candidates = vec![account("expired", 1, true, false, Some(100))];

    let eligible = eligible_accounts(&candidates, 100).expect("expired lock must not block");

    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].id, "expired");
}

#[test]
fn route_001_t04_all_otherwise_active_accounts_locked_returns_earliest_retry() {
    let candidates = vec![
        account("later", 1, true, false, Some(180)),
        account("earlier", 2, true, false, Some(125)),
        account("inactive", 3, false, false, None),
        account("excluded", 4, true, true, None),
    ];

    let error = eligible_accounts(&candidates, 100)
        .expect_err("all otherwise-active accounts are still model-locked");

    assert_eq!(
        error,
        AccountEligibilityError::RateLimited { retry_at: 125 }
    );
}

#[test]
fn route_001_t05_no_active_candidates_for_non_lock_reasons_returns_unavailable() {
    let candidates = vec![
        account("inactive", 1, false, false, None),
        account("excluded", 2, true, true, None),
    ];

    let error = eligible_accounts(&candidates, 100)
        .expect_err("non-lock exhaustion should be ordinary unavailability");

    assert_eq!(error, AccountEligibilityError::Unavailable);
}
