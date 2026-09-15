use opencode_rk_providers::router::{
    select_account, AccountSelectionCandidate, AccountSelectionError, AccountSelectionPatch,
    AccountSelectionStrategy, MAX_ACCOUNT_SELECTION_CANDIDATES,
};

fn candidate(
    id: &str,
    priority: u32,
    last_used_at: Option<u64>,
    consecutive_use_count: u32,
) -> AccountSelectionCandidate {
    AccountSelectionCandidate {
        id: id.to_owned(),
        priority,
        last_used_at,
        consecutive_use_count,
    }
}

#[test]
fn route_002_t01_available_preferred_id_wins_and_missing_preferred_falls_through() {
    let candidates = vec![
        candidate("first", 1, None, 0),
        candidate("preferred", 2, Some(90), 2),
    ];

    let preferred = select_account(
        &candidates,
        Some("preferred"),
        AccountSelectionStrategy::FillFirst,
        3,
        100,
    )
    .expect("an available preferred account should be selected directly");
    assert_eq!(preferred.selected_id, "preferred");
    assert_eq!(preferred.patch, None);

    let fallback = select_account(
        &candidates,
        Some("missing"),
        AccountSelectionStrategy::FillFirst,
        3,
        100,
    )
    .expect("a missing preferred id should fall through to the configured strategy");
    assert_eq!(fallback.selected_id, "first");
    assert_eq!(fallback.patch, None);
}

#[test]
fn route_002_t02_fill_first_returns_first_priority_ordered_candidate_without_patch() {
    let candidates = vec![
        candidate("priority-one", 1, Some(90), 7),
        candidate("priority-two", 2, None, 0),
        candidate("priority-three", 3, Some(10), 1),
    ];

    let decision = select_account(
        &candidates,
        None,
        AccountSelectionStrategy::FillFirst,
        3,
        100,
    )
    .expect("fill-first should consume the caller's already-priority-ordered slice");

    assert_eq!(decision.selected_id, "priority-one");
    assert_eq!(decision.patch, None);
}

#[test]
fn route_002_t03_round_robin_stays_on_recent_candidate_below_sticky_limit_and_increments_patch() {
    let candidates = vec![
        candidate("older", 1, Some(50), 1),
        candidate("current", 2, Some(90), 2),
        candidate("unused", 3, None, 0),
    ];

    let decision = select_account(
        &candidates,
        None,
        AccountSelectionStrategy::RoundRobin,
        3,
        100,
    )
    .expect("round-robin should stay sticky below the configured limit");

    assert_eq!(decision.selected_id, "current");
    assert_eq!(
        decision.patch,
        Some(AccountSelectionPatch {
            last_used_at: 100,
            consecutive_use_count: 3,
        })
    );
}

#[test]
fn route_002_t04_round_robin_at_limit_prefers_unused_then_priority_and_caller_order() {
    let priority_candidates = vec![
        candidate("current", 9, Some(90), 2),
        candidate("unused-second-priority", 2, None, 0),
        candidate("unused-first-priority", 1, None, 0),
        candidate("old-used", 0, Some(10), 1),
    ];

    let by_priority = select_account(
        &priority_candidates,
        None,
        AccountSelectionStrategy::RoundRobin,
        2,
        100,
    )
    .expect("sticky exhaustion should move to the least-recent candidate");
    assert_eq!(by_priority.selected_id, "unused-first-priority");
    assert_eq!(
        by_priority.patch,
        Some(AccountSelectionPatch {
            last_used_at: 100,
            consecutive_use_count: 1,
        })
    );

    let caller_order_tie = vec![
        candidate("current", 9, Some(90), 2),
        candidate("unused-first-in-slice", 1, None, 0),
        candidate("unused-second-in-slice", 1, None, 0),
    ];
    let by_caller_order = select_account(
        &caller_order_tie,
        None,
        AccountSelectionStrategy::RoundRobin,
        2,
        100,
    )
    .expect("equal-priority unused candidates should retain deterministic caller order");
    assert_eq!(by_caller_order.selected_id, "unused-first-in-slice");
    assert_eq!(
        by_caller_order.patch,
        Some(AccountSelectionPatch {
            last_used_at: 100,
            consecutive_use_count: 1,
        })
    );
}

#[test]
fn route_002_t05_invalid_or_unbounded_inputs_return_typed_errors_and_boundary_is_allowed() {
    let empty = select_account(&[], None, AccountSelectionStrategy::FillFirst, 3, 100)
        .expect_err("an empty post-eligibility candidate slice is invalid");
    assert_eq!(empty, AccountSelectionError::EmptyCandidates);

    let one = vec![candidate("only", 1, None, 0)];
    let invalid_sticky = select_account(&one, None, AccountSelectionStrategy::RoundRobin, 0, 100)
        .expect_err("round-robin sticky limit must be positive");
    assert_eq!(invalid_sticky, AccountSelectionError::InvalidStickyLimit);

    let boundary = (0..MAX_ACCOUNT_SELECTION_CANDIDATES)
        .map(|index| candidate(&format!("account-{index:02}"), index as u32, None, 0))
        .collect::<Vec<_>>();
    let boundary_decision =
        select_account(&boundary, None, AccountSelectionStrategy::FillFirst, 1, 100)
            .expect("the documented candidate bound should be accepted");
    assert_eq!(boundary_decision.selected_id, "account-00");

    let overflow = (0..=MAX_ACCOUNT_SELECTION_CANDIDATES)
        .map(|index| candidate(&format!("account-{index:02}"), index as u32, None, 0))
        .collect::<Vec<_>>();
    let overflow_error =
        select_account(&overflow, None, AccountSelectionStrategy::FillFirst, 1, 100)
            .expect_err("candidate slices above the public bound must be rejected");
    assert_eq!(
        overflow_error,
        AccountSelectionError::TooManyCandidates {
            max: MAX_ACCOUNT_SELECTION_CANDIDATES,
            actual: MAX_ACCOUNT_SELECTION_CANDIDATES + 1,
        }
    );
}
