//! Ordered provider failover: try candidates in order, bounded attempts.
//!
//! Pure orchestration over caller-supplied candidates. No I/O, no clock, no
//! global state. The caller owns provider handles; this runner only enforces
//! order, validation, and the attempt bound, stopping at the first success.

use thiserror::Error;

/// Bound on candidates per failover run (mirrors selection bound in
/// [`crate::router::MAX_ACCOUNT_SELECTION_CANDIDATES`]).
pub const MAX_FALLBACK_CANDIDATES: usize = 16;

/// Successful failover outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackSuccess<T> {
    /// Winning candidate, in caller order.
    pub provider_id: String,
    /// Value returned by the winning attempt.
    pub value: T,
    /// 1-based count of attempts made (position of winner in order).
    pub attempts: usize,
}

/// One failed attempt, retained in caller order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackAttempt<E> {
    pub provider_id: String,
    pub error: E,
}

/// Terminal failover failure: every candidate failed, in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FallbackExhaustion<E> {
    pub attempts: Vec<FallbackAttempt<E>>,
}

impl<E> FallbackExhaustion<E> {
    /// Borrow the last failure's error, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<&E> {
        self.attempts.last().map(|attempt| &attempt.error)
    }
}

/// Failover run errors: invalid input vs. ordered exhaustion.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FallbackError<E> {
    #[error("no fallback candidates supplied")]
    EmptyCandidates,
    #[error("too many candidates: max {max}, actual {actual}")]
    TooManyCandidates { max: usize, actual: usize },
    #[error("invalid (empty) provider id")]
    InvalidProviderId,
    #[error("duplicate provider id: {provider_id}")]
    DuplicateProvider { provider_id: String },
    #[error("all {actual} fallback candidates failed")]
    Exhausted {
        actual: usize,
        attempts: Vec<FallbackAttempt<E>>,
    },
}

impl<E> FallbackError<E> {
    /// Borrow ordered attempts when exhausted, else `None`.
    #[must_use]
    pub fn attempts(&self) -> Option<&[FallbackAttempt<E>]> {
        match self {
            Self::Exhausted { attempts, .. } => Some(attempts),
            _ => None,
        }
    }
}

/// Try each candidate in slice order, returning the first success.
///
/// Validation runs before any attempt executes, so invalid input never
/// partially advances side effects: empty/oversized/duplicate/empty-id
/// inputs error without calling `attempt`. Each distinct candidate is tried
/// at most once, bounding total attempts to `candidates.len()`.
#[must_use = "failover result must be handled"]
pub fn run_ordered_fallback<T, E>(
    candidates: &[String],
    mut attempt: impl FnMut(&str) -> Result<T, E>,
) -> Result<FallbackSuccess<T>, FallbackError<E>> {
    if candidates.is_empty() {
        return Err(FallbackError::EmptyCandidates);
    }
    if candidates.len() > MAX_FALLBACK_CANDIDATES {
        return Err(FallbackError::TooManyCandidates {
            max: MAX_FALLBACK_CANDIDATES,
            actual: candidates.len(),
        });
    }
    for id in candidates {
        if id.is_empty() {
            return Err(FallbackError::InvalidProviderId);
        }
    }
    for (index, id) in candidates.iter().enumerate() {
        if candidates[..index].iter().any(|seen| seen == id) {
            return Err(FallbackError::DuplicateProvider {
                provider_id: id.clone(),
            });
        }
    }

    let mut failures: Vec<FallbackAttempt<E>> = Vec::with_capacity(candidates.len());
    for (index, id) in candidates.iter().enumerate() {
        match attempt(id) {
            Ok(value) => {
                return Ok(FallbackSuccess {
                    provider_id: id.clone(),
                    value,
                    attempts: index.saturating_add(1),
                });
            }
            Err(error) => failures.push(FallbackAttempt {
                provider_id: id.clone(),
                error,
            }),
        }
    }
    let actual = failures.len();
    Err(FallbackError::Exhausted {
        actual,
        attempts: failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn primary_fail_secondary_ok() {
        let candidates = ids(&["primary", "secondary"]);
        let mut calls: Vec<String> = Vec::new();
        let out = run_ordered_fallback(&candidates, |id: &str| {
            calls.push(id.to_owned());
            if id == "primary" {
                return Err("boom".to_owned());
            }
            Ok(42)
        })
        .expect("secondary should win");
        assert_eq!(out.provider_id, "secondary");
        assert_eq!(out.value, 42);
        assert_eq!(out.attempts, 2);
        assert_eq!(calls, vec!["primary".to_owned(), "secondary".to_owned()]);
    }

    #[test]
    fn first_success_stops_after_one_attempt() {
        let candidates = ids(&["primary", "secondary"]);
        let mut calls = 0_usize;
        let out = run_ordered_fallback(&candidates, |_: &str| {
            calls += 1;
            Ok::<bool, String>(true)
        })
        .expect("first should win");
        assert_eq!(out.provider_id, "primary");
        assert_eq!(out.attempts, 1);
        assert_eq!(calls, 1);
    }

    #[test]
    fn all_fail_returns_ordered_exhaustion() {
        let candidates = ids(&["a", "b", "c"]);
        let err = run_ordered_fallback(&candidates, |id: &str| {
            Err::<u8, String>(format!("{id} down"))
        })
        .expect_err("all failing must exhaust");
        match &err {
            FallbackError::Exhausted { actual, attempts } => {
                assert_eq!(*actual, 3);
                let order: Vec<&str> = attempts.iter().map(|a| a.provider_id.as_str()).collect();
                assert_eq!(order, vec!["a", "b", "c"]);
                assert_eq!(attempts[2].error, "c down");
            }
            other => panic!("wrong variant: {other:?}"),
        }
        assert_eq!(err.attempts().expect("attempts").len(), 3);
    }

    #[test]
    fn empty_and_oversized_are_bounded_input_errors() {
        assert_eq!(
            run_ordered_fallback::<u8, String>(&[], |_| Ok(1)).expect_err("empty"),
            FallbackError::EmptyCandidates
        );
        let many = vec!["p".to_owned(); MAX_FALLBACK_CANDIDATES + 1];
        assert_eq!(
            run_ordered_fallback(&many, |_: &str| Ok::<u8, String>(1)).expect_err("oversized"),
            FallbackError::TooManyCandidates {
                max: MAX_FALLBACK_CANDIDATES,
                actual: MAX_FALLBACK_CANDIDATES + 1,
            }
        );
    }

    #[test]
    fn invalid_and_duplicate_reject_before_any_attempt() {
        let mut calls = 0_usize;
        assert_eq!(
            run_ordered_fallback(&ids(&["ok", ""]), |_: &str| {
                calls += 1;
                Ok::<u8, String>(1)
            })
            .expect_err("empty id"),
            FallbackError::InvalidProviderId
        );
        assert_eq!(calls, 0, "validation must precede attempts");

        let mut calls = 0_usize;
        assert_eq!(
            run_ordered_fallback(&ids(&["dup", "dup"]), |_: &str| {
                calls += 1;
                Ok::<u8, String>(1)
            })
            .expect_err("duplicate"),
            FallbackError::DuplicateProvider {
                provider_id: "dup".to_owned(),
            }
        );
        assert_eq!(calls, 0, "validation must precede attempts");
    }
}
