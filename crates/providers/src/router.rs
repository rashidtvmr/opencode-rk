//! Cheap-model routing for small advisory/provider chores.

use crate::rate_limit::MAX_FAILURE_REASON_CHARS;

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

pub const MAX_ACCOUNT_SELECTION_CANDIDATES: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSelectionCandidate {
    pub id: String,
    pub priority: u32,
    pub last_used_at: Option<u64>,
    pub consecutive_use_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountSelectionStrategy {
    FillFirst,
    RoundRobin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountSelectionPatch {
    pub last_used_at: u64,
    pub consecutive_use_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountSelectionDecision {
    pub selected_id: String,
    pub patch: Option<AccountSelectionPatch>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountSelectionError {
    EmptyCandidates,
    InvalidStickyLimit,
    TooManyCandidates { max: usize, actual: usize },
}

pub const MAX_ACCOUNT_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountFallbackFailure {
    pub status: u16,
    pub error: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountAttemptOutcome {
    Success,
    Cancelled,
    Failure {
        status: u16,
        error: String,
        should_fallback: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountFallbackDecision {
    Retry,
    Success,
    Cancelled,
    Failure { status: u16, error: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountFallbackExhaustion {
    NoCredentials,
    Exhausted {
        last_failure: AccountFallbackFailure,
    },
    RateLimited {
        retry_at: u64,
        last_failure: Option<AccountFallbackFailure>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccountFallbackError {
    InvalidAccountId,
    AccountIdTooLong { max: usize, actual: usize },
    DuplicateAccount { account_id: String },
    TooManyAttempts { max: usize, actual: usize },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountFallbackCoordinator {
    excluded_account_ids: Vec<String>,
    last_failure: Option<AccountFallbackFailure>,
}

impl AccountFallbackCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn excluded_account_ids(&self) -> &[String] {
        &self.excluded_account_ids
    }

    #[must_use]
    pub fn last_failure(&self) -> Option<&AccountFallbackFailure> {
        self.last_failure.as_ref()
    }

    pub fn record_attempt(
        &mut self,
        account_id: &str,
        outcome: AccountAttemptOutcome,
    ) -> Result<AccountFallbackDecision, AccountFallbackError> {
        if account_id.is_empty() {
            return Err(AccountFallbackError::InvalidAccountId);
        }
        if account_id.len() > MAX_ACCOUNT_ID_BYTES {
            return Err(AccountFallbackError::AccountIdTooLong {
                max: MAX_ACCOUNT_ID_BYTES,
                actual: account_id.len(),
            });
        }

        match outcome {
            AccountAttemptOutcome::Success => Ok(AccountFallbackDecision::Success),
            AccountAttemptOutcome::Cancelled => Ok(AccountFallbackDecision::Cancelled),
            AccountAttemptOutcome::Failure {
                status,
                error,
                should_fallback: false,
            } => Ok(AccountFallbackDecision::Failure { status, error }),
            AccountAttemptOutcome::Failure {
                status,
                error,
                should_fallback: true,
            } => {
                if self
                    .excluded_account_ids
                    .iter()
                    .any(|excluded| excluded == account_id)
                {
                    return Err(AccountFallbackError::DuplicateAccount {
                        account_id: account_id.to_owned(),
                    });
                }

                if self.excluded_account_ids.len() >= MAX_ACCOUNT_SELECTION_CANDIDATES {
                    return Err(AccountFallbackError::TooManyAttempts {
                        max: MAX_ACCOUNT_SELECTION_CANDIDATES,
                        actual: self.excluded_account_ids.len().saturating_add(1),
                    });
                }

                let failure = AccountFallbackFailure {
                    status,
                    error: error.chars().take(MAX_FAILURE_REASON_CHARS).collect(),
                };
                self.excluded_account_ids.push(account_id.to_owned());
                self.last_failure = Some(failure);
                Ok(AccountFallbackDecision::Retry)
            }
        }
    }

    #[must_use]
    pub fn finish(&self, eligibility: AccountEligibilityError) -> AccountFallbackExhaustion {
        match eligibility {
            AccountEligibilityError::RateLimited { retry_at } => {
                AccountFallbackExhaustion::RateLimited {
                    retry_at,
                    last_failure: self.last_failure.clone(),
                }
            }
            AccountEligibilityError::Unavailable => self
                .last_failure
                .clone()
                .map_or(AccountFallbackExhaustion::NoCredentials, |last_failure| {
                    AccountFallbackExhaustion::Exhausted { last_failure }
                }),
        }
    }
}

/// Select one already-eligible account using the pinned 9router strategy rules.
///
/// The caller owns persistence. Round-robin therefore returns the recency/count
/// patch that upstream persists; preferred and fill-first selection return no
/// patch. `now` is caller supplied so this decision has no wall-clock dependency.
pub fn select_account(
    candidates: &[AccountSelectionCandidate],
    preferred_id: Option<&str>,
    strategy: AccountSelectionStrategy,
    sticky_limit: u32,
    now: u64,
) -> Result<AccountSelectionDecision, AccountSelectionError> {
    if candidates.len() > MAX_ACCOUNT_SELECTION_CANDIDATES {
        return Err(AccountSelectionError::TooManyCandidates {
            max: MAX_ACCOUNT_SELECTION_CANDIDATES,
            actual: candidates.len(),
        });
    }
    if candidates.is_empty() {
        return Err(AccountSelectionError::EmptyCandidates);
    }

    if let Some(preferred_id) = preferred_id {
        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| candidate.id == preferred_id)
        {
            return Ok(AccountSelectionDecision {
                selected_id: candidate.id.clone(),
                patch: None,
            });
        }
    }

    match strategy {
        AccountSelectionStrategy::FillFirst => Ok(AccountSelectionDecision {
            selected_id: candidates[0].id.clone(),
            patch: None,
        }),
        AccountSelectionStrategy::RoundRobin => {
            if sticky_limit == 0 {
                return Err(AccountSelectionError::InvalidStickyLimit);
            }

            let mut by_recency = candidates.iter().collect::<Vec<_>>();
            by_recency.sort_by(
                |left, right| match (left.last_used_at, right.last_used_at) {
                    (None, None) => left.priority.cmp(&right.priority),
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (Some(left_used), Some(right_used)) => right_used.cmp(&left_used),
                },
            );

            let current = by_recency[0];
            if current.last_used_at.is_some() && current.consecutive_use_count < sticky_limit {
                return Ok(AccountSelectionDecision {
                    selected_id: current.id.clone(),
                    patch: Some(AccountSelectionPatch {
                        last_used_at: now,
                        consecutive_use_count: current.consecutive_use_count.saturating_add(1),
                    }),
                });
            }

            let mut by_oldest = candidates.iter().collect::<Vec<_>>();
            by_oldest.sort_by(
                |left, right| match (left.last_used_at, right.last_used_at) {
                    (None, None) => left.priority.cmp(&right.priority),
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(left_used), Some(right_used)) => left_used.cmp(&right_used),
                },
            );
            let selected = by_oldest[0];

            Ok(AccountSelectionDecision {
                selected_id: selected.id.clone(),
                patch: Some(AccountSelectionPatch {
                    last_used_at: now,
                    consecutive_use_count: 1,
                }),
            })
        }
    }
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

    retry_at.map_or(Err(AccountEligibilityError::Unavailable), |retry_at| {
        Err(AccountEligibilityError::RateLimited { retry_at })
    })
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
        .filter(|candidate| candidate.available && candidate.supported_chores.contains(&chore))
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
