//! Cheap-model routing for small advisory/provider chores.

/// Small background chores that may be routed independently from the caller's
/// main model choice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChoreKind {
    QuotaProbe,
    Topic,
    Title,
    Summarize,
}

/// Stable provider/model identity used by the pure routing decision.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

/// One caller-supplied routing candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelCandidate {
    pub model: ModelRef,
    pub cost_microunits: u64,
    pub available: bool,
    pub supported_chores: Vec<ChoreKind>,
}

/// Decision returned to the provider call site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDecision {
    pub model: ModelRef,
    pub max_output_tokens: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountCandidate {
    pub id: String,
    pub priority: u32,
    pub active: bool,
    pub excluded: bool,
    pub model_lock_until: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountEligibilityError {
    RateLimited { retry_at: u64 },
    Unavailable,
}

pub fn eligible_accounts(
    candidates: &[AccountCandidate],
    now: u64,
) -> Result<Vec<AccountCandidate>, AccountEligibilityError> {
    let mut eligible = candidates
        .iter()
        .filter(|candidate| {
            candidate.active
                && !candidate.excluded
                && candidate
                    .model_lock_until
                    .is_none_or(|lock_until| lock_until <= now)
        })
        .cloned()
        .collect::<Vec<_>>();

    if !eligible.is_empty() {
        eligible.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        return Ok(eligible);
    }

    let retry_at = candidates
        .iter()
        .filter(|candidate| candidate.active && !candidate.excluded)
        .filter_map(|candidate| candidate.model_lock_until)
        .filter(|lock_until| *lock_until > now)
        .min();

    retry_at.map_or(
        Err(AccountEligibilityError::Unavailable),
        |retry_at| Err(AccountEligibilityError::RateLimited { retry_at }),
    )
}

/// Select the cheapest available model that explicitly supports `chore`.
/// Equal-cost candidates are ordered by stable provider/model identity so the
/// result does not depend on input ordering. If no candidate qualifies, the
/// caller's main model is preserved as a fail-open fallback.
#[must_use]
pub fn route_chore(
    chore: ChoreKind,
    main_model: &ModelRef,
    candidates: &[ModelCandidate],
) -> RouteDecision {
    let model = candidates
        .iter()
        .filter(|candidate| {
            candidate.available && candidate.supported_chores.contains(&chore)
        })
        .min_by(|left, right| {
            left.cost_microunits
                .cmp(&right.cost_microunits)
                .then_with(|| left.model.cmp(&right.model))
        })
        .map_or_else(|| main_model.clone(), |candidate| candidate.model.clone());

    RouteDecision {
        model,
        max_output_tokens: (chore == ChoreKind::QuotaProbe).then_some(1),
    }
}
