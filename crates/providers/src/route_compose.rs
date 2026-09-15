//! Routing composition: eligibility + selection + fallback-exhaustion mapping.
//!
//! Pure data transform over caller-supplied [`AccountCandidate`]s. No
//! execution, no I/O, no clock; `now` is caller supplied for determinism.

use thiserror::Error;

/// Bound on candidates per compose call (mirrors selection bound).
pub const MAX_COMPOSE_CANDIDATES: usize = 16;

/// Caller-supplied routing input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteComposeRequest<'a> {
    pub candidates: &'a [super::router::AccountCandidate],
    pub preferred_id: Option<&'a str>,
    pub now: u64,
}

/// Terminal routing outcome (fallback-exhaustion mapped to data).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteComposeOutcome {
    Selected { account_id: String },
    RateLimited { retry_at: u64 },
    Unavailable,
}

/// Typed input-bound errors.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RouteComposeError {
    #[error("no candidates supplied")]
    EmptyCandidates,
    #[error("too many candidates: max {max}, actual {actual}")]
    TooManyCandidates { max: usize, actual: usize },
}

/// Compose eligibility, preferred/priority selection, and exhaustion mapping.
///
/// Eligibility predicate matches [`super::router::eligible_accounts`]:
/// active, not excluded, lock expired (`None` or `<= now`).
/// Empty eligible set maps via the same rule: any active non-excluded
/// future lock yields `RateLimited{min lock}`, else `Unavailable`.
/// Preferred id wins among eligible; otherwise lowest `(priority, id)`.
#[must_use = "composition result must be handled"]
pub fn compose_route(
    req: &RouteComposeRequest<'_>,
) -> Result<RouteComposeOutcome, RouteComposeError> {
    if req.candidates.is_empty() {
        return Err(RouteComposeError::EmptyCandidates);
    }
    if req.candidates.len() > MAX_COMPOSE_CANDIDATES {
        return Err(RouteComposeError::TooManyCandidates {
            max: MAX_COMPOSE_CANDIDATES,
            actual: req.candidates.len(),
        });
    }

    let mut eligible: Vec<&super::router::AccountCandidate> = req
        .candidates
        .iter()
        .filter(|c| {
            c.active && !c.excluded && c.model_lock_until.is_none_or(|lock| lock <= req.now)
        })
        .collect();

    if eligible.is_empty() {
        let retry_at = req
            .candidates
            .iter()
            .filter(|c| c.active && !c.excluded)
            .filter_map(|c| c.model_lock_until)
            .filter(|lock| *lock > req.now)
            .min();
        return Ok(
            retry_at.map_or(RouteComposeOutcome::Unavailable, |retry_at| {
                RouteComposeOutcome::RateLimited { retry_at }
            }),
        );
    }

    if let Some(preferred) = req.preferred_id {
        if let Some(hit) = eligible.iter().find(|c| c.id == preferred) {
            return Ok(RouteComposeOutcome::Selected {
                account_id: (*hit).id.clone(),
            });
        }
    }

    eligible.sort_by(|l, r| l.priority.cmp(&r.priority).then_with(|| l.id.cmp(&r.id)));
    Ok(RouteComposeOutcome::Selected {
        account_id: eligible[0].id.clone(),
    })
}
