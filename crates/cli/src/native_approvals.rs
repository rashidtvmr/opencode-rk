#![forbid(unsafe_code)]
//! Native approval interaction state (TUI-008: approval semantics).
//!
//! Pure state only: digest+scope-bound approval decisions, replay/stale
//! rejection, per-grant expiry and policy-version binding, two-step
//! confirmation for destructive/human-only operations, interrupt receipts
//! with a broadcast generation, safe-vs-ambiguous retry classification.
//! No rendering, no IO, no clock, no threads. Caller supplies `now: u64`
//! ticks and dispatches the effect (tool execution, client broadcast).
//!
//! Commit: base 5af7884. Evidence: card TUI-008 (T01 digest/workspace/
//! requester/expiry/policy binding, T02 stale/replay rejection without
//! side effects, T03 interrupt cancels real in-flight op and updates all
//! clients, T04 safe retry vs ambiguous side effects, T05 keyboard focus
//! cannot accidentally approve destructive or human-only actions); contract
//! `externalAuthority` (human-only grants are never auto-approved);
//! `docs/SECURITY.md` sections 1, 5, 6 (explicit grants carry expiry and
//! policy-version bounds); prior art `crates/cli/src/native_composer.rs`
//! (pure state, bounded queues, std only).
//!
//! Not wired into `main.rs` (integrator-owned); replay window bounded by
//! `MAX_HISTORY` eviction (ancient digests eventually forgotten).

use std::collections::VecDeque;

/// Upper bound on queued pending approvals.
pub const MAX_PENDING: usize = 16;
/// Upper bound on recorded decision history entries (also the replay window).
pub const MAX_HISTORY: usize = 32;
/// Upper bound on tracked started/interrupted digests for retry classification.
pub const MAX_TRACKED_OPS: usize = 32;

/// Identifies the exact operation an approval is bound to. A decision is
/// valid only for the pending request carrying this digest; anything else
/// is stale or replayed and refused without side effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationDigest(pub String);

/// What kind of operation is being approved. Destructive and human-only
/// operations need a two-step confirmation and can never be approved by a
/// single keystroke or by keyboard focus landing on the approve control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Safe,
    Destructive,
    HumanOnly,
}

/// The grant scope the reviewer saw. Approval binds to all four fields:
/// a scope mismatch refuses exactly like a stale digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalScope {
    pub workspace: String,
    pub requester: String,
    pub expires_at_tick: u64,
    pub policy_version: u64,
}

/// Explicit confirmation level. Focus activation and a plain Enter map to
/// `Unconfirmed`, which can never approve a destructive or human-only
/// operation. Only an explicit second gesture maps to `Confirmed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Unconfirmed,
    Confirmed,
}

/// The proposed operation shown in the native dialog.
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub digest: OperationDigest,
    pub summary: String,
    pub scope: ApprovalScope,
    pub risk: Risk,
    /// Set by [`ApprovalBoard::arm`]; the first explicit step of the
    /// two-step confirmation. Stored here, never a caller-supplied bool.
    pub armed: bool,
}

impl ApprovalRequest {
    /// The grant aged out; the board settles it as a non-effect denial.
    #[must_use]
    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.scope.expires_at_tick
    }

    #[must_use]
    fn needs_two_step(&self) -> bool {
        matches!(self.risk, Risk::Destructive | Risk::HumanOnly)
    }
}

/// A settled decision retained for audit. Once recorded, the same digest
/// can never be decided or re-offered again (replay rejection).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub digest: OperationDigest,
    pub scope: ApprovalScope,
    pub approved: bool,
    /// True when the user completed both confirmation steps.
    pub confirmed_twice: bool,
    pub expired: bool,
}

/// Why a decision attempt was refused. Refusals mutate nothing except the
/// broadcast generation counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No pending request carries this digest+scope (stale, replayed, or
    /// scope mismatch).
    StaleOrReplayed,
    /// Destructive/human-only operation needs arm + explicit confirmation.
    ConfirmationRequired,
}

/// Result of a decide attempt. Effects (tool execution) belong to the
/// caller; the state machine only records that a decision was reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Decided(Decision),
    Refused(Refusal),
}

/// Proof that an in-flight operation was cancelled. `broadcast_seq` lets
/// every client poll for updates; the caller performs the real cancel and
/// the fan-out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterruptReceipt {
    pub digest: OperationDigest,
    pub interrupted_at_tick: u64,
    pub broadcast_seq: u64,
}

/// Retry classification (T04): only an operation that never started may be
/// retried freely; anything started or interrupted may have completed side
/// effects, so its retry must carry a fresh digest and fresh approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryKind {
    SafeToRetry,
    AmbiguousSideEffects,
}

/// Pending-approval queue plus settled history, in-flight tracking and a
/// broadcast generation. Every collection is bounded.
#[derive(Debug, Default)]
pub struct ApprovalBoard {
    pending: VecDeque<ApprovalRequest>,
    history: VecDeque<Decision>,
    started: VecDeque<OperationDigest>,
    interrupted: VecDeque<OperationDigest>,
    in_flight: Option<OperationDigest>,
    broadcast_seq: u64,
}

impl ApprovalBoard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Monotonic generation bumped on every state mutation. Clients poll
    /// it to learn that approvals, arming, starts or interrupts changed.
    #[must_use]
    pub fn broadcast_seq(&self) -> u64 {
        self.broadcast_seq
    }

    /// Queue a request. Rejects digests already pending or already decided
    /// (replay). Oldest pending is dropped when over budget (bounded queue).
    pub fn offer(&mut self, request: ApprovalRequest) -> Result<(), &'static str> {
        if self.pending.iter().any(|p| p.digest == request.digest) {
            return Err("duplicate pending digest");
        }
        if self.history.iter().any(|d| d.digest == request.digest) {
            return Err("replayed digest");
        }
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(request);
        self.bump();
        Ok(())
    }

    /// The request currently shown in the dialog (oldest pending).
    #[must_use]
    pub fn current(&self) -> Option<&ApprovalRequest> {
        self.pending.front()
    }

    /// Pending count for the status bar.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Settled decisions, oldest first, bounded.
    #[must_use]
    pub fn history(&self) -> &VecDeque<Decision> {
        &self.history
    }

    /// The real in-flight operation, if any.
    #[must_use]
    pub fn in_flight(&self) -> Option<&OperationDigest> {
        self.in_flight.as_ref()
    }

    /// First step of the two-step confirmation: arms the pending request
    /// so a later explicit `Confirm::Confirmed` may approve it. Focus
    /// movement alone never arms.
    pub fn arm(&mut self, digest: &OperationDigest) -> bool {
        let Some(request) = self.pending.iter_mut().find(|p| &p.digest == digest) else {
            return false;
        };
        request.armed = true;
        self.bump();
        true
    }

    /// Approve the pending request bound to `digest` + `scope`.
    /// Destructive and human-only requests need a prior [`Self::arm`] and
    /// `Confirm::Confirmed`; focus activation (`Unconfirmed`) returns
    /// `ConfirmationRequired` and leaves the request pending (T05).
    pub fn approve(
        &mut self,
        digest: &OperationDigest,
        scope: &ApprovalScope,
        confirm: Confirm,
        now: u64,
    ) -> Outcome {
        self.decide(digest, Some(scope), true, confirm, now)
    }

    /// Deny the pending request. Denial is always single-step; it never
    /// requires arming or confirmation friction.
    pub fn deny(&mut self, digest: &OperationDigest, now: u64) -> Outcome {
        self.decide(digest, None, false, Confirm::Unconfirmed, now)
    }

    fn decide(
        &mut self,
        digest: &OperationDigest,
        scope: Option<&ApprovalScope>,
        approved: bool,
        confirm: Confirm,
        now: u64,
    ) -> Outcome {
        let Some(idx) = self.pending.iter().position(|p| &p.digest == digest) else {
            return Outcome::Refused(Refusal::StaleOrReplayed);
        };
        if approved {
            let matches = self.pending[idx].scope == *scope.expect("approve binds scope");
            if !matches {
                // Scope mismatch: bound exactly like a stale digest, no mutation.
                return Outcome::Refused(Refusal::StaleOrReplayed);
            }
        }
        let request = self.pending.remove(idx).expect("position verified");
        if request.is_expired(now) {
            // Expired grants settle as a non-effect denial with `expired`.
            let decision = Decision {
                digest: request.digest,
                scope: request.scope,
                approved: false,
                confirmed_twice: false,
                expired: true,
            };
            self.record(decision.clone());
            self.bump();
            return Outcome::Decided(decision);
        }
        // Two-step guard (T05): destructive/human-only approvals need the
        // stored arm step plus an explicit `Confirmed` gesture. Focus
        // activation and plain Enter map to `Unconfirmed` and refuse,
        // leaving the request pending. Refusal bumps the generation so
        // clients observe the attempt.
        if approved && request.needs_two_step() && !(request.armed && confirm == Confirm::Confirmed)
        {
            self.pending.insert(idx, request);
            self.bump();
            return Outcome::Refused(Refusal::ConfirmationRequired);
        }
        let two_step = request.needs_two_step();
        let decision = Decision {
            digest: request.digest,
            scope: request.scope,
            approved,
            confirmed_twice: two_step && approved,
            expired: false,
        };
        self.record(decision.clone());
        self.bump();
        Outcome::Decided(decision)
    }

    /// Register that an approved operation really started executing, so a
    /// later interrupt/retry can reason about side effects. False unless
    /// the digest was approved, unexpired-settled, and nothing is in flight.
    pub fn note_started(&mut self, digest: &OperationDigest) -> bool {
        let approved = self
            .history
            .iter()
            .any(|d| &d.digest == digest && d.approved && !d.expired);
        if !approved || self.in_flight.is_some() {
            return false;
        }
        self.in_flight = Some(digest.clone());
        if self.started.len() >= MAX_TRACKED_OPS {
            self.started.pop_front();
        }
        self.started.push_back(digest.clone());
        self.bump();
        true
    }

    /// Cancel the real in-flight operation. The caller performs the actual
    /// cancel; this records it, clears the slot and returns a receipt whose
    /// `broadcast_seq` every client can poll for. `None` when idle.
    pub fn interrupt(&mut self, now: u64) -> Option<InterruptReceipt> {
        let digest = self.in_flight.take()?;
        if self.interrupted.len() >= MAX_TRACKED_OPS {
            self.interrupted.pop_front();
        }
        self.interrupted.push_back(digest.clone());
        self.bump();
        Some(InterruptReceipt {
            digest,
            interrupted_at_tick: now,
            broadcast_seq: self.broadcast_seq,
        })
    }

    /// Classify a retry attempt: anything started or interrupted may have
    /// completed side effects and is ambiguous; anything else is safe to
    /// retry (still under a fresh digest with fresh approval).
    #[must_use]
    pub fn classify_retry(&self, digest: &OperationDigest) -> RetryKind {
        if self.interrupted.contains(digest) || self.started.contains(digest) {
            RetryKind::AmbiguousSideEffects
        } else {
            RetryKind::SafeToRetry
        }
    }

    /// Record a settled decision, dropping the oldest on overflow (bounded).
    fn record(&mut self, decision: Decision) {
        if self.history.len() >= MAX_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(decision);
    }

    fn bump(&mut self) {
        self.broadcast_seq = self.broadcast_seq.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> ApprovalScope {
        ApprovalScope {
            workspace: "ws:demo".into(),
            requester: "user".into(),
            expires_at_tick: 600,
            policy_version: 7,
        }
    }

    fn req(digest: &str, risk: Risk) -> ApprovalRequest {
        ApprovalRequest {
            digest: OperationDigest(digest.to_string()),
            summary: format!("op {digest}"),
            scope: scope(),
            risk,
            armed: false,
        }
    }

    #[test]
    fn approval_binds_digest_workspace_requester_expiry_policy() {
        let mut board = ApprovalBoard::new();
        board.offer(req("abc", Risk::Safe)).unwrap();
        // Wrong workspace refuses without touching the pending request.
        let mut other = scope();
        other.workspace = "ws:evil".into();
        assert_eq!(
            board.approve(&OperationDigest("abc".into()), &other, Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        // Wrong policy version refuses likewise.
        let mut older = scope();
        older.policy_version = 6;
        assert_eq!(
            board.approve(&OperationDigest("abc".into()), &older, Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board.pending_len(), 1);
        // Exact digest + full scope approves and snapshots the scope.
        assert_eq!(
            board.approve(&OperationDigest("abc".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(Decision {
                digest: OperationDigest("abc".into()),
                scope: scope(),
                approved: true,
                confirmed_twice: false,
                expired: false,
            })
        );
        assert_eq!(board.pending_len(), 0);
    }

    #[test]
    fn stale_unknown_digest_rejected_without_side_effects() {
        let mut board = ApprovalBoard::new();
        board.offer(req("abc", Risk::Safe)).unwrap();
        let before = board.broadcast_seq();
        assert_eq!(
            board.approve(&OperationDigest("nope".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board.pending_len(), 1);
        assert_eq!(board.history().len(), 0);
        assert_eq!(board.broadcast_seq(), before, "refusal mutates nothing");
    }

    #[test]
    fn replayed_digest_rejected_after_decision() {
        let mut board = ApprovalBoard::new();
        board.offer(req("abc", Risk::Safe)).unwrap();
        assert!(matches!(
            board.approve(&OperationDigest("abc".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(_)
        ));
        // Same digest can never be decided twice.
        assert_eq!(
            board.approve(&OperationDigest("abc".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board.history().len(), 1);
        // ... nor re-offered.
        assert_eq!(board.offer(req("abc", Risk::Safe)), Err("replayed digest"));
    }

    #[test]
    fn expired_grant_settles_as_non_effect_denial() {
        let mut board = ApprovalBoard::new();
        board.offer(req("old", Risk::Safe)).unwrap();
        assert!(matches!(
            board.approve(&OperationDigest("old".into()), &scope(), Confirm::Confirmed, 600),
            Outcome::Decided(Decision { expired: true, approved: false, .. })
        ));
        assert_eq!(board.pending_len(), 0);
    }

    #[test]
    fn focus_activation_cannot_approve_destructive() {
        let mut board = ApprovalBoard::new();
        board.offer(req("rm", Risk::Destructive)).unwrap();
        // Focus/Enter maps to Unconfirmed: must not approve.
        assert_eq!(
            board.approve(&OperationDigest("rm".into()), &scope(), Confirm::Unconfirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(board.pending_len(), 1, "request stays pending after refusal");
        // Explicit confirm without the arm step is still not enough.
        assert_eq!(
            board.approve(&OperationDigest("rm".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        // Arm, then explicit confirm: approves with both steps recorded.
        assert!(board.arm(&OperationDigest("rm".into())));
        assert_eq!(
            board.approve(&OperationDigest("rm".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(Decision {
                digest: OperationDigest("rm".into()),
                scope: scope(),
                approved: true,
                confirmed_twice: true,
                expired: false,
            })
        );
    }

    #[test]
    fn human_only_never_bypasses_two_step() {
        let mut board = ApprovalBoard::new();
        board.offer(req("acct", Risk::HumanOnly)).unwrap();
        assert_eq!(
            board.approve(&OperationDigest("acct".into()), &scope(), Confirm::Unconfirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(
            board.approve(&OperationDigest("acct".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert!(board.arm(&OperationDigest("acct".into())));
        assert!(matches!(
            board.approve(&OperationDigest("acct".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(Decision { approved: true, confirmed_twice: true, .. })
        ));
    }

    #[test]
    fn denial_is_single_step_and_records_no_approval() {
        let mut board = ApprovalBoard::new();
        board.offer(req("x", Risk::Destructive)).unwrap();
        assert!(matches!(
            board.deny(&OperationDigest("x".into()), 0),
            Outcome::Decided(Decision { approved: false, .. })
        ));
    }

    #[test]
    fn interrupt_cancels_in_flight_and_bumps_broadcast() {
        let mut board = ApprovalBoard::new();
        assert_eq!(board.interrupt(9), None, "idle interrupt is None");
        board.offer(req("op1", Risk::Safe)).unwrap();
        board.approve(&OperationDigest("op1".into()), &scope(), Confirm::Confirmed, 0);
        assert!(board.note_started(&OperationDigest("op1".into())));
        assert_eq!(board.in_flight(), Some(&OperationDigest("op1".into())));
        let before = board.broadcast_seq();
        let receipt = board.interrupt(42).expect("in-flight interrupt has a receipt");
        assert_eq!(receipt.digest, OperationDigest("op1".into()));
        assert_eq!(receipt.interrupted_at_tick, 42);
        assert_eq!(receipt.broadcast_seq, before + 1);
        assert_eq!(board.in_flight(), None, "slot cleared");
        assert_eq!(board.broadcast_seq(), before + 1, "clients observe the update");
        assert_eq!(board.interrupt(43), None, "second interrupt is idle");
    }

    #[test]
    fn retry_distinguishes_safe_from_ambiguous_side_effects() {
        let mut board = ApprovalBoard::new();
        // Denied-before-start: safe to retry (fresh digest, fresh approval).
        board.offer(req("denied", Risk::Safe)).unwrap();
        board.deny(&OperationDigest("denied".into()), 0);
        assert_eq!(
            board.classify_retry(&OperationDigest("denied".into())),
            RetryKind::SafeToRetry
        );
        // Interrupted after start: ambiguous, may have completed side effects.
        board.offer(req("op1", Risk::Safe)).unwrap();
        board.approve(&OperationDigest("op1".into()), &scope(), Confirm::Confirmed, 0);
        assert!(board.note_started(&OperationDigest("op1".into())));
        board.interrupt(7);
        assert_eq!(
            board.classify_retry(&OperationDigest("op1".into())),
            RetryKind::AmbiguousSideEffects
        );
        // Retry itself runs under a fresh digest, never a silent replay.
        board.offer(req("op1-retry", Risk::Safe)).unwrap();
        assert!(matches!(
            board.approve(&OperationDigest("op1-retry".into()), &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(_)
        ));
    }

    #[test]
    fn bounded_queue_and_history_drop_oldest() {
        let mut board = ApprovalBoard::new();
        for i in 0..(MAX_PENDING + 8) {
            let digest = format!("d{i}");
            board.offer(req(&digest, Risk::Safe)).unwrap();
        }
        assert_eq!(board.pending_len(), MAX_PENDING);
        for i in 0..(MAX_HISTORY + 8) {
            let digest = format!("h{i}");
            board.offer(req(&digest, Risk::Safe)).unwrap();
            board.deny(&OperationDigest(digest), 0);
        }
        assert_eq!(board.history().len(), MAX_HISTORY);
        assert_eq!(board.history()[0].digest.0, format!("h8"));
    }
}
