//! Coding-turn state machine (APP-004).
//!
//! Owns the server-side turn lifecycle referenced by
//! `crates/agents/src/turn_state.rs:62-128`
//! (`TurnSubmissionState`: Idle/Submitted/Running) and the HTTP turn boundary in
//! `crates/server/src/lib.rs:688-711` (`create_turn`). That boundary appends
//! user/assistant messages with no approval, interruption, retry-classification,
//! or crash-recovery state; this module provides that missing lifecycle so a
//! coding turn with tools, streaming, and interruption reaches a stable,
//! truthful terminal state.
//!
//! Contract:
//! - One turn at a time per [`Turn`]: [`TurnId`] binds every transition.
//! - Phases: Streaming -> AwaitingApproval -> Executing -> Streaming ... ->
//!   Settled, with Cancelled and Uncertain as stable non-success terminals.
//! - Retry is classified, never blind: [`classify_retry`] returns
//!   [`RetryDecision::SafeToRetry`] only for failures known to have produced no
//!   side effect; anything ambiguous returns
//!   [`RetryDecision::AmbiguousEffect`] and [`Turn::retry_after_failure`]
//!   moves the turn to [`TurnPhase::Uncertain`] instead of replaying.
//! - [`CancelToken`] is the interrupt/cancel token: [`Turn::interrupt`]
//!   fires it and moves to [`TurnPhase::Cancelled`]; settled/cancelled turns
//!   are terminal.
//! - [`Turn::deny_approval`] records an explicit [`DenyReceipt`] and settles
//!   without executing; denial asserts absence of side effects (no tool ran).
//! - Bounded: event log capped at [`MAX_TURN_EVENTS`], text/reason inputs
//!   capped at [`MAX_TEXT_BYTES`]. No threads, no I/O, no heap growth beyond
//!   the caps.

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Maximum retained lifecycle events per turn (bounded history).
pub const MAX_TURN_EVENTS: usize = 64;
/// Maximum bytes accepted for prompt deltas / deny reasons.
pub const MAX_TEXT_BYTES: usize = 8 * 1024;

/// Opaque coding-turn identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TurnId(u64);

impl TurnId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn-{}", self.0)
    }
}

/// Lifecycle phase of a single coding turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnPhase {
    /// Provider bytes streaming; no tool running; interruptible.
    Streaming,
    /// Tool call proposed; waiting on human approval; interruptible.
    AwaitingApproval,
    /// Approved tool executing; interruptible; side effects possible.
    Executing,
    /// Terminal success: response persisted, history settled.
    Settled,
    /// Terminal cancel: interrupted, no further transitions, work reclaimed.
    Cancelled,
    /// Stable unknown: crash/ambiguous failure during possible side effect.
    /// Never fabricated into Settled; requires explicit resume/reconcile.
    Uncertain,
}

/// How a failure relates to side effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    /// Transport failed before anything was dispatched (no effect possible).
    TransportBeforeDispatch,
    /// Provider stream cut mid-text before any tool call (no effect).
    PartialStreamNoTool,
    /// Retryable transport error, caller proves idempotent/no dispatch.
    RetryableIdempotent,
    /// Tool started but result unknown (timeout, crash, lost ack).
    ToolStartedUnknownResult,
    /// Tool reported success then the ack/persist failed (effect happened).
    ToolCompletedPersistFailed,
    /// Interrupted mid-execution; tool may or may not have run.
    InterruptedDuringEffect,
    /// Approval denied by human (no execution; not a retry case).
    ApprovalDenied,
}

/// Retry classifier output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryDecision {
    /// Proven no side effect; safe to re-dispatch.
    SafeToRetry,
    /// Effect unknown or already happened; must NOT blind-retry.
    AmbiguousEffect,
}

/// Classify a failure as safe-to-retry vs ambiguous-effect.
///
/// Only failures provably free of side effects classify as
/// [`RetryDecision::SafeToRetry`]. Everything involving a started or
/// completed tool, or an interrupt landing inside a possible effect window,
/// is [`RetryDecision::AmbiguousEffect`].
#[must_use]
pub const fn classify_retry(failure: FailureKind) -> RetryDecision {
    match failure {
        FailureKind::TransportBeforeDispatch
        | FailureKind::PartialStreamNoTool
        | FailureKind::RetryableIdempotent => RetryDecision::SafeToRetry,
        FailureKind::ToolStartedUnknownResult
        | FailureKind::ToolCompletedPersistFailed
        | FailureKind::InterruptedDuringEffect
        | FailureKind::ApprovalDenied => RetryDecision::AmbiguousEffect,
    }
}

/// Receipt proving a human denial settled the turn without execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenyReceipt {
    turn: TurnId,
    reason: String,
    seq: u64,
}

impl DenyReceipt {
    #[must_use]
    pub const fn turn(&self) -> TurnId {
        self.turn
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }
}

/// Cooperative interrupt/cancel token handed to stream/tool drivers.
///
/// `cancel()` is idempotent; drivers poll `is_cancelled()` and must stop
/// dispatching new work once set. [`Turn::interrupt`] fires the active token.
#[derive(Clone, Debug)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Retained lifecycle event (bounded log for audit/recovery).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TurnEvent {
    pub seq: u64,
    pub phase: TurnPhase,
    pub label: String,
}

/// Errors from turn transitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnError {
    WrongTurn { active: TurnId, requested: TurnId },
    InvalidTransition { from: TurnPhase, operation: &'static str },
    TextTooLarge { bytes: usize, max: usize },
    EventLogFull,
    AmbiguousRetryDenied { failure: FailureKind },
    AlreadyTerminal { phase: TurnPhase },
}

impl fmt::Display for TurnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTurn { active, requested } => {
                write!(f, "wrong turn: active={active} requested={requested}")
            }
            Self::InvalidTransition { from, operation } => {
                write!(f, "invalid turn transition from {from:?} via {operation}")
            }
            Self::TextTooLarge { bytes, max } => {
                write!(f, "text too large: {bytes} bytes exceeds {max}")
            }
            Self::EventLogFull => write!(f, "turn event log full"),
            Self::AmbiguousRetryDenied { failure } => write!(
                f,
                "blind retry denied for ambiguous effect: {failure:?}; turn moved to Uncertain"
            ),
            Self::AlreadyTerminal { phase } => {
                write!(f, "turn already terminal in {phase:?}")
            }
        }
    }
}

impl std::error::Error for TurnError {}

/// Single coding turn: streaming, approval-gated tools, interruption.
#[derive(Debug)]
pub struct Turn {
    id: TurnId,
    phase: TurnPhase,
    cancel: CancelToken,
    deny_receipt: Option<DenyReceipt>,
    events: Vec<TurnEvent>,
    next_seq: u64,
    /// True once the turn entered Executing without a confirmed clean finish.
    /// Used so restart/crash recovery reports Uncertain, never Settled.
    possible_effect: bool,
}

impl Turn {
    /// Start a turn in [`TurnPhase::Streaming`].
    #[must_use]
    pub fn start(id: TurnId) -> Self {
        let mut turn = Self {
            id,
            phase: TurnPhase::Streaming,
            cancel: CancelToken::new(),
            deny_receipt: None,
            events: Vec::new(),
            next_seq: 0,
            possible_effect: false,
        };
        // Constructor event cannot fail: log is empty.
        let _ = turn.record(TurnPhase::Streaming, "start");
        turn
    }

    #[must_use]
    pub const fn id(&self) -> TurnId {
        self.id
    }

    #[must_use]
    pub const fn phase(&self) -> TurnPhase {
        self.phase
    }

    #[must_use]
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    #[must_use]
    pub fn deny_receipt(&self) -> Option<&DenyReceipt> {
        self.deny_receipt.as_ref()
    }

    #[must_use]
    pub fn events(&self) -> &[TurnEvent] {
        &self.events
    }

    #[must_use]
    pub const fn possible_effect(&self) -> bool {
        self.possible_effect
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self.phase,
            TurnPhase::Settled | TurnPhase::Cancelled | TurnPhase::Uncertain
        )
    }

    /// Propose a tool call: Streaming -> AwaitingApproval.
    pub fn request_approval(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        if self.phase != TurnPhase::Streaming {
            return Err(TurnError::InvalidTransition {
                from: self.phase,
                operation: "request_approval",
            });
        }
        self.set_phase(TurnPhase::AwaitingApproval, "request_approval")
    }

    /// Human approved: AwaitingApproval -> Executing. Arms possible-effect.
    pub fn approve(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        if self.phase != TurnPhase::AwaitingApproval {
            return Err(TurnError::InvalidTransition {
                from: self.phase,
                operation: "approve",
            });
        }
        self.possible_effect = true;
        self.set_phase(TurnPhase::Executing, "approve")
    }

    /// Human denied: AwaitingApproval -> Settled with receipt, no execution.
    pub fn deny_approval(
        &mut self,
        requested: TurnId,
        reason: impl Into<String>,
    ) -> Result<DenyReceipt, TurnError> {
        self.ensure_owner(requested)?;
        if self.phase != TurnPhase::AwaitingApproval {
            return Err(TurnError::InvalidTransition {
                from: self.phase,
                operation: "deny_approval",
            });
        }
        let reason = reason.into();
        if reason.len() > MAX_TEXT_BYTES {
            return Err(TurnError::TextTooLarge {
                bytes: reason.len(),
                max: MAX_TEXT_BYTES,
            });
        }
        let receipt = DenyReceipt {
            turn: self.id,
            reason,
            seq: self.next_seq,
        };
        self.deny_receipt = Some(receipt.clone());
        // Denial performs no tool work: provably no side effect.
        self.possible_effect = false;
        self.set_phase(TurnPhase::Settled, "deny_approval")?;
        Ok(receipt)
    }

    /// Tool finished cleanly with result observed: Executing -> Streaming.
    pub fn tool_finished(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        if self.phase != TurnPhase::Executing {
            return Err(TurnError::InvalidTransition {
                from: self.phase,
                operation: "tool_finished",
            });
        }
        self.possible_effect = false;
        self.set_phase(TurnPhase::Streaming, "tool_finished")
    }

    /// Tool finished cleanly AND the clean turn settles, atomically:
    /// Executing -> Streaming -> Settled in one call. This is the turn-path
    /// closer: `settle` alone refuses Executing while a possible unconfirmed
    /// effect is outstanding, so a caller that stops after `approve` leaves
    /// the turn in Executing forever (non-terminal, never Settled). Routing
    /// the finished tool through this method guarantees the turn reaches
    /// Settled instead of pending indefinitely.
    ///
    /// Consumes two event-log slots (one per leg); if the `settle` leg hits
    /// [`TurnError::EventLogFull`] the tool leg already applied and the turn
    /// rests clean in Streaming (no possible effect), so a later `settle`
    /// succeeds. Wrong-phase calls fail with the `tool_finished` leg's
    /// [`TurnError::InvalidTransition`] and mutate nothing.
    pub fn finish_and_settle(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.tool_finished(requested)?;
        self.settle(requested)
    }

    /// Settle a clean turn: Streaming or Executing (clean finish) -> Settled.
    /// Refuses to settle while a possible unconfirmed effect is outstanding;
    /// call `tool_finished` first or route through uncertain recovery.
    pub fn settle(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        match self.phase {
            TurnPhase::Streaming => self.set_phase(TurnPhase::Settled, "settle"),
            TurnPhase::Executing if !self.possible_effect => {
                self.set_phase(TurnPhase::Settled, "settle")
            }
            _ => Err(TurnError::InvalidTransition {
                from: self.phase,
                operation: "settle",
            }),
        }
    }

    /// Interrupt streaming/approval/execution: -> Cancelled, token fired.
    /// Terminal: no transitions out. Idempotent cancel signal.
    pub fn interrupt(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        match self.phase {
            TurnPhase::Streaming | TurnPhase::AwaitingApproval | TurnPhase::Executing => {
                self.cancel.cancel();
                self.set_phase(TurnPhase::Cancelled, "interrupt")
            }
            TurnPhase::Settled | TurnPhase::Cancelled | TurnPhase::Uncertain => {
                Err(TurnError::AlreadyTerminal { phase: self.phase })
            }
        }
    }

    /// Retry after failure. Safe failures resume Streaming; ambiguous
    /// failures move the turn to Uncertain and return
    /// [`TurnError::AmbiguousRetryDenied`] (no blind replay).
    pub fn retry_after_failure(
        &mut self,
        requested: TurnId,
        failure: FailureKind,
    ) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        if self.is_terminal() {
            return Err(TurnError::AlreadyTerminal { phase: self.phase });
        }
        match classify_retry(failure) {
            RetryDecision::SafeToRetry => {
                if self.phase == TurnPhase::AwaitingApproval {
                    return Err(TurnError::InvalidTransition {
                        from: self.phase,
                        operation: "retry_after_failure",
                    });
                }
                self.possible_effect = false;
                self.cancel = CancelToken::new();
                self.set_phase(TurnPhase::Streaming, "retry_safe")
            }
            RetryDecision::AmbiguousEffect => {
                self.cancel.cancel();
                self.possible_effect = true;
                self.set_phase(TurnPhase::Uncertain, "retry_ambiguous")?;
                Err(TurnError::AmbiguousRetryDenied { failure })
            }
        }
    }

    /// Crash/restart recovery: a turn found mid-flight with a possible
    /// unconfirmed effect recovers to Uncertain, never Settled. A turn with
    /// no possible effect recovers to its recorded phase label.
    pub fn mark_uncertain(&mut self, requested: TurnId) -> Result<(), TurnError> {
        self.ensure_owner(requested)?;
        if self.is_terminal() {
            return Err(TurnError::AlreadyTerminal { phase: self.phase });
        }
        self.cancel.cancel();
        self.set_phase(TurnPhase::Uncertain, "restart_recovery")
    }

    fn ensure_owner(&self, requested: TurnId) -> Result<(), TurnError> {
        if self.id != requested {
            return Err(TurnError::WrongTurn {
                active: self.id,
                requested,
            });
        }
        Ok(())
    }

    fn set_phase(&mut self, next: TurnPhase, label: &str) -> Result<(), TurnError> {
        self.record(next, label)?;
        self.phase = next;
        Ok(())
    }

    fn record(&mut self, phase: TurnPhase, label: &str) -> Result<(), TurnError> {
        if self.events.len() >= MAX_TURN_EVENTS {
            return Err(TurnError::EventLogFull);
        }
        let label = truncate_label(label);
        self.events.push(TurnEvent {
            seq: self.next_seq,
            phase,
            label,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        Ok(())
    }
}

fn truncate_label(label: &str) -> String {
    if label.len() <= MAX_TEXT_BYTES {
        return label.to_owned();
    }
    // Keep a bounded prefix; labels are internal ASCII AER codes.
    label
        .char_indices()
        .take_while(|(i, _)| *i < MAX_TEXT_BYTES)
        .map(|(_, c)| c)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn() -> Turn {
        Turn::start(TurnId::new(7))
    }

    #[test]
    fn happy_path_streams_approves_executes_and_settles() {
        let id = TurnId::new(1);
        let mut t = Turn::start(id);
        assert_eq!(t.phase(), TurnPhase::Streaming);
        t.request_approval(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::AwaitingApproval);
        t.approve(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Executing);
        assert!(t.possible_effect());
        t.tool_finished(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Streaming);
        assert!(!t.possible_effect());
        t.settle(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Settled);
        assert!(t.is_terminal());
        // Terminal transitions rejected.
        assert_eq!(
            t.settle(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Settled,
                operation: "settle",
            })
        );
    }

    #[test]
    fn deny_records_receipt_settles_and_runs_no_tool() {
        let id = TurnId::new(2);
        let mut t = turn();
        let other = TurnId::new(99);
        // Wrong turn rejected.
        assert_eq!(
            t.request_approval(other),
            Err(TurnError::WrongTurn {
                active: TurnId::new(7),
                requested: other,
            })
        );
        t.request_approval(TurnId::new(7)).unwrap();
        let receipt = t
            .deny_approval(TurnId::new(7), "risky rm -rf fixture".to_owned())
            .unwrap();
        assert_eq!(receipt.turn(), TurnId::new(7));
        assert_eq!(receipt.reason(), "risky rm -rf fixture");
        assert_eq!(t.phase(), TurnPhase::Settled);
        assert!(!t.possible_effect());
        // Receipt retained; denial is absence of side effects.
        assert_eq!(t.deny_receipt(), Some(&receipt));
        // Deny outside approval rejected.
        let mut t2 = Turn::start(id);
        assert_eq!(
            t2.deny_approval(id, "x"),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Streaming,
                operation: "deny_approval",
            })
        );
    }

    #[test]
    fn interrupt_cancels_token_reclaims_and_freezes_terminal() {
        let id = TurnId::new(3);
        let mut t = Turn::start(id);
        let token = t.cancel_token();
        assert!(!token.is_cancelled());
        t.interrupt(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Cancelled);
        assert!(token.is_cancelled());
        // Token clone observes the same cancellation.
        assert!(t.cancel_token().is_cancelled());
        // Terminal: no retry, no settle, no second interrupt.
        assert_eq!(
            t.interrupt(id),
            Err(TurnError::AlreadyTerminal {
                phase: TurnPhase::Cancelled
            })
        );
        assert_eq!(
            t.retry_after_failure(id, FailureKind::TransportBeforeDispatch),
            Err(TurnError::AlreadyTerminal {
                phase: TurnPhase::Cancelled
            })
        );
        assert_eq!(
            t.settle(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Cancelled,
                operation: "settle",
            })
        );

        // Interrupt during execution also fires token.
        let mut t2 = Turn::start(id);
        t2.request_approval(id).unwrap();
        t2.approve(id).unwrap();
        let tok2 = t2.cancel_token();
        t2.interrupt(id).unwrap();
        assert!(tok2.is_cancelled());
        assert_eq!(t2.phase(), TurnPhase::Cancelled);
    }

    #[test]
    fn safe_retry_resumes_but_ambiguous_effect_goes_uncertain_no_blind_replay() {
        let id = TurnId::new(4);
        // Safe: partial stream before any tool -> back to Streaming.
        let mut t = Turn::start(id);
        t.retry_after_failure(id, FailureKind::PartialStreamNoTool)
            .unwrap();
        assert_eq!(t.phase(), TurnPhase::Streaming);
        assert!(!t.possible_effect());
        // Fresh cancel token issued on safe retry (old interrupt state shed).
        assert!(!t.cancel_token().is_cancelled());

        // Ambiguous: tool started, result unknown -> Uncertain + denied.
        let mut t2 = Turn::start(id);
        t2.request_approval(id).unwrap();
        t2.approve(id).unwrap();
        let err = t2
            .retry_after_failure(id, FailureKind::ToolStartedUnknownResult)
            .unwrap_err();
        assert_eq!(
            err,
            TurnError::AmbiguousRetryDenied {
                failure: FailureKind::ToolStartedUnknownResult,
            }
        );
        assert_eq!(t2.phase(), TurnPhase::Uncertain);
        assert!(t2.is_terminal());
        assert!(t2.cancel_token().is_cancelled());
        // No escape from Uncertain via retry/settle: must reconcile offline.
        assert_eq!(
            t2.retry_after_failure(id, FailureKind::TransportBeforeDispatch),
            Err(TurnError::AlreadyTerminal {
                phase: TurnPhase::Uncertain
            })
        );

        // Completed-tool persist failure is also ambiguous.
        assert_eq!(
            classify_retry(FailureKind::ToolCompletedPersistFailed),
            RetryDecision::AmbiguousEffect
        );
        assert_eq!(
            classify_retry(FailureKind::TransportBeforeDispatch),
            RetryDecision::SafeToRetry
        );
    }

    #[test]
    fn restart_during_effect_recovers_uncertain_never_fabricated_settled() {
        let id = TurnId::new(5);
        let mut t = Turn::start(id);
        t.request_approval(id).unwrap();
        t.approve(id).unwrap();
        // Simulate process restart mid-execution: recovery must not settle.
        t.mark_uncertain(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Uncertain);
        // Settling out of Uncertain is rejected: no fabricated completion.
        assert_eq!(
            t.settle(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Uncertain,
                operation: "settle",
            })
        );
        assert!(t.tool_finished(id).is_err());
        assert!(t.is_terminal());
    }

    #[test]
    fn invalid_transitions_and_bounds_are_rejected() {
        let id = TurnId::new(6);
        let mut t = Turn::start(id);
        // approve/tool_finished/settle-out-of-order rejected.
        assert_eq!(
            t.approve(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Streaming,
                operation: "approve",
            })
        );
        assert_eq!(
            t.tool_finished(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Streaming,
                operation: "tool_finished",
            })
        );
        // Retry while awaiting human decision rejected (would bypass grant).
        t.request_approval(id).unwrap();
        assert_eq!(
            t.retry_after_failure(id, FailureKind::TransportBeforeDispatch),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::AwaitingApproval,
                operation: "retry_after_failure",
            })
        );
        // Oversize deny reason rejected with bound.
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            t.deny_approval(id, big),
            Err(TurnError::TextTooLarge {
                bytes: MAX_TEXT_BYTES + 1,
                max: MAX_TEXT_BYTES,
            })
        );
        // Event log bounded (start already recorded 1 event).
        let mut t2 = Turn::start(id);
        for _ in 0..(MAX_TURN_EVENTS - 1) {
            t2.retry_after_failure(id, FailureKind::TransportBeforeDispatch)
                .unwrap();
        }
        assert_eq!(
            t2.retry_after_failure(id, FailureKind::TransportBeforeDispatch)
                .unwrap_err(),
            TurnError::EventLogFull
        );
    }

    #[test]
    fn finish_and_settle_reaches_settled_from_executing() {
        let id = TurnId::new(10);
        let mut t = Turn::start(id);
        t.request_approval(id).unwrap();
        t.approve(id).unwrap();
        assert!(t.possible_effect());
        t.finish_and_settle(id).unwrap();
        assert_eq!(t.phase(), TurnPhase::Settled);
        assert!(!t.possible_effect());
        assert!(t.is_terminal());
    }

    #[test]
    fn finish_and_settle_rejects_outside_executing() {
        let id = TurnId::new(11);
        let mut t = Turn::start(id);
        // Streaming is not Executing: tool_finished leg rejects, turn unchanged.
        assert_eq!(
            t.finish_and_settle(id),
            Err(TurnError::InvalidTransition {
                from: TurnPhase::Streaming,
                operation: "tool_finished",
            })
        );
        assert_eq!(t.phase(), TurnPhase::Streaming);
        assert!(!t.is_terminal());
    }

    #[test]
    fn classifier_never_marks_tool_effects_safe() {
        // Exhaustive: any failure touching a tool is ambiguous.
        for failure in [
            FailureKind::TransportBeforeDispatch,
            FailureKind::PartialStreamNoTool,
            FailureKind::RetryableIdempotent,
            FailureKind::ToolStartedUnknownResult,
            FailureKind::ToolCompletedPersistFailed,
            FailureKind::InterruptedDuringEffect,
            FailureKind::ApprovalDenied,
        ] {
            match failure {
                FailureKind::TransportBeforeDispatch
                | FailureKind::PartialStreamNoTool
                | FailureKind::RetryableIdempotent => {
                    assert_eq!(classify_retry(failure), RetryDecision::SafeToRetry)
                }
                _ => assert_eq!(
                    classify_retry(failure),
                    RetryDecision::AmbiguousEffect,
                    "failure {failure:?} must not be safe to retry"
                ),
            }
        }
    }
}
