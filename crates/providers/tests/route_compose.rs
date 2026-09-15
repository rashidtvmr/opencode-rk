use opencode_rk_providers::{
    route_compose::{
        compose_route, RouteComposeError, RouteComposeOutcome, RouteComposeRequest,
        MAX_COMPOSE_CANDIDATES,
    },
    router::AccountCandidate,
};

fn candidate(
    id: &str,
    priority: u32,
    active: bool,
    excluded: bool,
    lock: Option<u64>,
) -> AccountCandidate {
    AccountCandidate {
        id: id.to_owned(),
        priority,
        active,
        excluded,
        model_lock_until: lock,
    }
}

#[test]
fn route_010_t01_eligible_selects_priority_order() {
    let candidates = vec![
        candidate("b", 2, true, false, None),
        candidate("a", 1, true, false, None),
        candidate("c", 1, true, false, None),
    ];
    let req = RouteComposeRequest {
        candidates: &candidates,
        preferred_id: None,
        now: 100,
    };
    assert_eq!(
        compose_route(&req),
        Ok(RouteComposeOutcome::Selected {
            account_id: "a".to_owned(),
        })
    );
}

#[test]
fn route_010_t02_preferred_wins() {
    let candidates = vec![
        candidate("a", 1, true, false, None),
        candidate("b", 2, true, false, None),
    ];
    let req = RouteComposeRequest {
        candidates: &candidates,
        preferred_id: Some("b"),
        now: 100,
    };
    assert_eq!(
        compose_route(&req),
        Ok(RouteComposeOutcome::Selected {
            account_id: "b".to_owned(),
        })
    );
}

#[test]
fn route_010_t03_all_locked_maps_rate_limited() {
    let candidates = vec![
        candidate("a", 1, true, false, Some(200)),
        candidate("b", 2, true, false, Some(150)),
    ];
    let req = RouteComposeRequest {
        candidates: &candidates,
        preferred_id: None,
        now: 100,
    };
    assert_eq!(
        compose_route(&req),
        Ok(RouteComposeOutcome::RateLimited { retry_at: 150 })
    );
}

#[test]
fn route_010_t04_none_active_maps_unavailable() {
    let candidates = vec![
        candidate("a", 1, false, false, None),
        candidate("b", 1, false, false, Some(200)),
    ];
    let req = RouteComposeRequest {
        candidates: &candidates,
        preferred_id: None,
        now: 100,
    };
    assert_eq!(compose_route(&req), Ok(RouteComposeOutcome::Unavailable));
}

#[test]
fn route_010_t05_empty_and_overflow_are_typed_errors() {
    let empty: Vec<AccountCandidate> = vec![];
    let req = RouteComposeRequest {
        candidates: &empty,
        preferred_id: None,
        now: 100,
    };
    assert_eq!(compose_route(&req), Err(RouteComposeError::EmptyCandidates));

    let many: Vec<AccountCandidate> = (0..MAX_COMPOSE_CANDIDATES + 1)
        .map(|i| candidate(&format!("id-{i}"), 1, true, false, None))
        .collect();
    let req = RouteComposeRequest {
        candidates: &many,
        preferred_id: None,
        now: 100,
    };
    assert_eq!(
        compose_route(&req),
        Err(RouteComposeError::TooManyCandidates {
            max: MAX_COMPOSE_CANDIDATES,
            actual: MAX_COMPOSE_CANDIDATES + 1,
        })
    );
}
