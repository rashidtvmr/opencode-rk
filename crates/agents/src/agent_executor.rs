//! Real-loop scaffold: pure state machine for the agent turn loop (PAR-004).
//!
//! Caller-supplied IO only: this file never calls a provider, tool, policy
//! engine, or settler. The caller peeks [`AgentExecutor::next_step`], performs
//! the IO itself, then reports back via [`AgentExecutor::complete_step`].
//! Cancellation is observed at explicit checkpoints; a cancelled loop reclaims
//! its live slot and emits a [`ReclaimReceipt`]. Ambiguous effects are never
//! retried ([`classify`]).
#![forbid(unsafe_code)]

use std::fmt;

/// Bound on planned steps: no unbounded queue.
pub const MAX_STEPS: usize = 64;

/// One scaffold stage of the agent loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopStep {
    ProviderCall,
    ToolDispatch,
    PolicyCheck,
    Settle,
}

/// Effect class of the last caller-performed IO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    ReadOnly,
    IdempotentWrite,
    Ambiguous,
}

/// No-blind-retry classifier output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDecision {
    RetryAllowed,
    NeverReplay,
}

/// Local no-blind-retry classifier: ambiguous effects never replay.
/// Mirrors `app_delegation::classify` without a cross-module dependency so
/// this scaffold compiles standalone under `rustc --test`.
#[must_use]
pub const fn classify(effect: EffectKind) -> RetryDecision {
    match effect {
        EffectKind::Ambiguous => RetryDecision::NeverReplay,
        EffectKind::ReadOnly | EffectKind::IdempotentWrite => RetryDecision::RetryAllowed,
    }
}

/// Receipt proving cancellation reclaimed loop resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimReceipt {
    pub steps_completed: usize,
    pub steps_dropped: usize,
    pub reclaimed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopError {
    Cancelled,
    Terminal,
    AtCapacity,
    AmbiguousReplayDenied,
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "loop cancelled"),
            Self::Terminal => write!(f, "loop already reached a terminal state"),
            Self::AtCapacity => write!(f, "plan exceeds step bound"),
            Self::AmbiguousReplayDenied => write!(f, "ambiguous effect must never replay"),
        }
    }
}

impl std::error::Error for LoopError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunReport {
    pub steps_completed: usize,
    pub settled: bool,
    pub cancelled: bool,
    pub receipt: Option<ReclaimReceipt>,
}

#[derive(Debug)]
pub struct AgentExecutor {
    steps: Vec<LoopStep>,
    cursor: usize,
    live: bool,
    cancelled: bool,
}

impl AgentExecutor {
    pub fn new(steps: Vec<LoopStep>) -> Result<Self, LoopError> {
        if steps.len() > MAX_STEPS {
            return Err(LoopError::AtCapacity);
        }
        let live = !steps.is_empty();
        Ok(Self {
            steps,
            cursor: 0,
            live,
            cancelled: false,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn is_live(&self) -> bool {
        self.live
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        self.cursor >= self.steps.len()
    }

    #[must_use]
    pub fn next_step(&self) -> Option<LoopStep> {
        if self.cancelled || self.is_done() {
            return None;
        }
        self.steps.get(self.cursor).copied()
    }

    pub fn request_cancel(&mut self) {
        self.cancelled = true;
    }

    /// Cancellation checkpoint: if `request_cancel` was observed, reclaim the
    /// live slot (drop retained plan tail) and emit a receipt. Returns `None`
    /// when the loop is still running (no cancel pending). Idempotent: a
    /// second call after reclaim returns the same-shaped receipt with no
    /// additional mutation.
    pub fn checkpoint(&mut self) -> Option<ReclaimReceipt> {
        if !self.cancelled {
            return None;
        }
        let receipt = ReclaimReceipt {
            steps_completed: self.cursor,
            steps_dropped: self.steps.len().saturating_sub(self.cursor),
            reclaimed: true,
        };
        self.live = false;
        self.cursor = self.steps.len();
        Some(receipt)
    }

    /// Report one caller-performed IO step. Advances the cursor and returns
    /// the retry class of the reported effect; ambiguous effects report
    /// `NeverReplay` and are never auto-retried here.
    pub fn complete_step(&mut self, effect: EffectKind) -> Result<RetryDecision, LoopError> {
        if self.cancelled {
            return Err(LoopError::Cancelled);
        }
        if self.is_done() {
            return Err(LoopError::Terminal);
        }
        self.cursor += 1;
        if self.is_done() {
            self.live = false;
        }
        Ok(classify(effect))
    }

    /// Gate a caller-requested retry: ambiguous effects are denied outright
    /// (no replay); repairable classes are allowed while the loop is live.
    pub fn request_retry(&self, effect: EffectKind) -> Result<(), LoopError> {
        match classify(effect) {
            RetryDecision::NeverReplay => Err(LoopError::AmbiguousReplayDenied),
            RetryDecision::RetryAllowed => {
                if self.cancelled {
                    return Err(LoopError::Cancelled);
                }
                if !self.live {
                    return Err(LoopError::Terminal);
                }
                Ok(())
            }
        }
    }

    /// Drive the planned sequence to termination. The caller supplies all IO:
    /// each step is peeked via `next_step`, performed by `io`, then reported
    /// via `complete_step`. Stops early with a reclaim receipt if a cancel is
    /// observed at a checkpoint between steps. Bounded: at most `len()` steps.
    pub fn run_with(&mut self, mut io: impl FnMut(LoopStep) -> EffectKind) -> RunReport {
        let mut completed = 0usize;
        while let Some(step) = self.next_step() {
            let effect = io(step);
            match self.complete_step(effect) {
                Ok(_) => completed += 1,
                Err(LoopError::Cancelled) => {
                    let receipt = self.checkpoint();
                    return RunReport {
                        steps_completed: completed,
                        settled: false,
                        cancelled: true,
                        receipt,
                    };
                }
                Err(_) => break,
            }
            if self.cancelled {
                let receipt = self.checkpoint();
                return RunReport {
                    steps_completed: completed,
                    settled: false,
                    cancelled: true,
                    receipt,
                };
            }
        }
        RunReport {
            steps_completed: completed,
            settled: self.is_done() && !self.cancelled,
            cancelled: false,
            receipt: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Vec<LoopStep> {
        vec![
            LoopStep::ProviderCall,
            LoopStep::ToolDispatch,
            LoopStep::PolicyCheck,
            LoopStep::Settle,
        ]
    }

    #[test]
    fn cancel_reclaims_at_checkpoint() {
        let mut ex = AgentExecutor::new(plan()).unwrap();
        assert!(ex.is_live());
        ex.request_cancel();
        let receipt = ex.checkpoint().expect("cancel must reclaim at checkpoint");
        assert!(receipt.reclaimed);
        assert_eq!(receipt.steps_completed, 0);
        assert_eq!(receipt.steps_dropped, 4);
        assert!(!ex.is_live());
        assert_eq!(ex.next_step(), None);
    }

    #[test]
    fn ambiguous_never_retried() {
        assert_eq!(classify(EffectKind::Ambiguous), RetryDecision::NeverReplay);
        assert_eq!(classify(EffectKind::ReadOnly), RetryDecision::RetryAllowed);
        assert_eq!(
            classify(EffectKind::IdempotentWrite),
            RetryDecision::RetryAllowed
        );
        let ex = AgentExecutor::new(plan()).unwrap();
        assert_eq!(
            ex.request_retry(EffectKind::Ambiguous),
            Err(LoopError::AmbiguousReplayDenied)
        );
        assert_eq!(ex.request_retry(EffectKind::ReadOnly), Ok(()));
        let mut ex2 = AgentExecutor::new(vec![LoopStep::ToolDispatch]).unwrap();
        assert_eq!(
            ex2.complete_step(EffectKind::Ambiguous),
            Ok(RetryDecision::NeverReplay)
        );
    }

    #[test]
    fn sequence_terminates() {
        let mut ex = AgentExecutor::new(plan()).unwrap();
        let report = ex.run_with(|_| EffectKind::ReadOnly);
        assert!(report.settled);
        assert!(!report.cancelled);
        assert_eq!(report.steps_completed, 4);
        assert!(ex.is_done());
        assert!(!ex.is_live());
        assert_eq!(ex.next_step(), None);
    }

    #[test]
    fn over_bound_plan_rejected() {
        let big = vec![LoopStep::PolicyCheck; MAX_STEPS + 1];
        assert_eq!(AgentExecutor::new(big).unwrap_err(), LoopError::AtCapacity);
    }

    #[test]
    fn checkpoint_none_when_running() {
        let mut ex = AgentExecutor::new(plan()).unwrap();
        assert_eq!(ex.checkpoint(), None);
        assert!(ex.is_live());
    }
}
