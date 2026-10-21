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
//! Wired into `main.rs` as `crate::native_approvals`; replay window bounded by
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

/// Why a decision attempt was refused. Stale/replay refusals mutate
/// nothing; confirmation-required refusals bump only the broadcast
/// generation counter.
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
    grants: VecDeque<AlwaysGrant>,
    broadcast_seq: u64,
}

impl ApprovalBoard {
    /// Upper bound on tracked always-allow grants (oldest dropped first).
    pub const MAX_GRANTS: usize = 16;

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
            // Unknown digest (T02): pure refusal, zero mutation.
            return Outcome::Refused(Refusal::StaleOrReplayed);
        };
        if approved {
            let matches = self.pending[idx].scope == *scope.expect("approve binds scope");
            if !matches {
                // Scope mismatch: bound exactly like a stale digest, no mutation.
                // N.B. also covers digest reuse under a different scope (replay).
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

    /// Settle the pending request as an always-allow grant for `tool` under
    /// `scope`. Safe risk is single-step; destructive still requires the
    /// arm + `Confirm::Confirmed` two-step; human-only is always refused
    /// with zero mutation (human-only grants are never persistent). An
    /// empty tool name is refused like a stale digest. On success the
    /// decision settles exactly like [`Self::approve`] and an
    /// [`AlwaysGrant`] is recorded (bounded, oldest dropped).
    pub fn approve_always(
        &mut self,
        digest: &OperationDigest,
        tool: &str,
        scope: &ApprovalScope,
        confirm: Confirm,
        now: u64,
    ) -> Outcome {
        if tool.is_empty() {
            // Nothing worth granting: refuse exactly like a replayed digest.
            return Outcome::Refused(Refusal::StaleOrReplayed);
        }
        let Some(idx) = self.pending.iter().position(|p| &p.digest == digest) else {
            return Outcome::Refused(Refusal::StaleOrReplayed);
        };
        if self.pending[idx].risk == Risk::HumanOnly {
            // Human-only grants are never persistent: refuse with zero
            // mutation (no decision, no grant, pending untouched).
            return Outcome::Refused(Refusal::ConfirmationRequired);
        }
        if self.pending[idx].scope != *scope {
            // Scope mismatch: bound exactly like a stale digest.
            return Outcome::Refused(Refusal::StaleOrReplayed);
        }
        let request = self.pending.remove(idx).expect("position verified");
        if request.is_expired(now) {
            // Expired grants settle as a non-effect denial, no grant recorded.
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
        let two_step = request.needs_two_step();
        if two_step && !(request.armed && confirm == Confirm::Confirmed) {
            self.pending.insert(idx, request);
            self.bump();
            return Outcome::Refused(Refusal::ConfirmationRequired);
        }
        let decision = Decision {
            digest: request.digest,
            scope: request.scope.clone(),
            approved: true,
            confirmed_twice: two_step,
            expired: false,
        };
        self.record(decision.clone());
        if self.grants.len() >= Self::MAX_GRANTS {
            self.grants.pop_front();
        }
        self.grants.push_back(AlwaysGrant {
            tool: tool.to_string(),
            scope: request.scope,
        });
        self.bump();
        Outcome::Decided(decision)
    }

    /// True only when an exact `tool` + `ApprovalScope` grant exists and has
    /// not expired (`now < expires_at_tick`). Denied or expired operations
    /// never match.
    #[must_use]
    pub fn is_always_allowed(&self, tool: &str, scope: &ApprovalScope, now: u64) -> bool {
        self.grants
            .iter()
            .any(|g| g.tool == tool && g.scope == *scope && now < g.scope.expires_at_tick)
    }

    /// Recorded always-allow grants, oldest first, bounded. Read-only view.
    #[must_use]
    pub fn grants(&self) -> &VecDeque<AlwaysGrant> {
        &self.grants
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

/// A persistent always-allow grant: the granted tool plus the exact scope the
/// reviewer approved (never for human-only operations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlwaysGrant {
    pub tool: String,
    pub scope: ApprovalScope,
}

/// A choice offered by the approval popup. `AlwaysAllow` is never offered
/// for human-only operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    AllowOnce,
    AlwaysAllow,
    Deny,
}

impl Choice {
    /// Plain-text label used by render and announce.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Choice::AllowOnce => "Allow once",
            Choice::AlwaysAllow => "Always allow",
            Choice::Deny => "Deny",
        }
    }
}

/// A key dispatched to the popup. `Digit(n)` carries the 1-based digit value
/// (the caller maps key characters, e.g. `b'2' - b'0'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKey {
    Digit(u8),
    Up,
    Down,
    Tab,
    Enter,
    Escape,
}

/// What a popup key/click means for the board: submit a choice, cancel, or
/// nothing (movement / out-of-range input).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupAction {
    Submit(Choice),
    Cancel,
    Ignored,
}

/// Pure approval popup state: options per risk, keyboard/mouse selection,
/// canonical rendering (char-safe, fixed clamp width) and screen-reader text.
/// No IO, no clock; the caller dispatches the textual lines.
#[derive(Debug)]
pub struct ApprovalPopup {
    risk: Risk,
    options: Vec<Choice>,
    selected: usize,
    cancelled: bool,
}

impl ApprovalPopup {
    /// The hint row printed under the options.
    const HINT: &'static str =
        "↑/↓ or Tab select · Enter confirm · 1-3 quick select · Esc deny-free cancel";
    /// The human-only notice printed under the title.
    const HUMAN_ONLY_NOTICE: &'static str =
        "Human-only action: always-allow is not available.";
    /// Visible, non-color focus marker for the selected row.
    const MARKER: &'static str = "› ";
    /// Render widths clamp to this inclusive range.
    const MIN_WIDTH: usize = 30;
    const MAX_WIDTH: usize = 80;

    /// Build a popup for `risk`. Safe and destructive list Allow once, Always
    /// allow, Deny; human-only never lists Always allow (human-only grants
    /// are never persistent).
    #[must_use]
    pub fn new(risk: Risk) -> Self {
        let options = match risk {
            Risk::HumanOnly => vec![Choice::AllowOnce, Choice::Deny],
            Risk::Safe | Risk::Destructive => {
                vec![Choice::AllowOnce, Choice::AlwaysAllow, Choice::Deny]
            }
        };
        Self {
            risk,
            options,
            selected: 0,
            cancelled: false,
        }
    }

    /// The choices offered, in display order.
    #[must_use]
    pub fn options(&self) -> &[Choice] {
        &self.options
    }

    /// The currently selected choice.
    #[must_use]
    pub fn selected(&self) -> Choice {
        self.options[self.selected]
    }

    /// The currently selected row index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Move one row down, wrapping. True when a row exists (always).
    pub fn move_next(&mut self) -> bool {
        let len = self.options.len();
        if len == 0 {
            return false;
        }
        self.selected = (self.selected + 1) % len;
        true
    }

    /// Move one row up, wrapping. True when a row exists (always).
    pub fn move_prev(&mut self) -> bool {
        if self.options.is_empty() {
            return false;
        }
        let len = self.options.len();
        self.selected = (self.selected + len - 1) % len;
        true
    }

    /// Dispatch a key. Digits 1..=len submit that choice immediately; Up /
    /// Down / Tab only move (Ignored); Enter submits the selected row;
    /// Escape cancels, and a cancelled popup never submits again.
    pub fn handle_key(&mut self, key: PopupKey) -> PopupAction {
        if self.cancelled {
            return PopupAction::Cancel;
        }
        match key {
            PopupKey::Digit(n) => {
                let row = usize::from(n);
                if row >= 1 && row <= self.options.len() {
                    self.selected = row - 1;
                    PopupAction::Submit(self.options[self.selected])
                } else {
                    PopupAction::Ignored
                }
            }
            PopupKey::Up => {
                self.move_prev();
                PopupAction::Ignored
            }
            PopupKey::Down | PopupKey::Tab => {
                self.move_next();
                PopupAction::Ignored
            }
            PopupKey::Enter => PopupAction::Submit(self.selected()),
            PopupKey::Escape => {
                self.cancelled = true;
                PopupAction::Cancel
            }
        }
    }

    /// Mouse activation of a 0-based option row: selects it and submits.
    /// Out-of-range rows are ignored without changing selection.
    pub fn handle_click(&mut self, row: usize) -> PopupAction {
        if self.cancelled || row >= self.options.len() {
            return PopupAction::Ignored;
        }
        self.selected = row;
        PopupAction::Submit(self.options[row])
    }

    /// Render the dialog as lines of exactly `clamp(30..=80)` characters
    /// (char-safe padding/truncation, never panics). The selected row carries
    /// the only visible "› " marker so focus never depends on color.
    #[must_use]
    pub fn render(&self, width: usize) -> Vec<String> {
        let width = width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);
        let mut lines = vec![Self::pad("Permission required", width)];
        if self.risk == Risk::HumanOnly {
            lines.push(Self::pad(Self::HUMAN_ONLY_NOTICE, width));
        }
        for (i, choice) in self.options.iter().enumerate() {
            let row = format!("{}. {}", i + 1, choice.label());
            let text = if i == self.selected {
                format!("{}{}", Self::MARKER, row)
            } else {
                row
            };
            lines.push(Self::pad(&text, width));
        }
        lines.push(Self::pad(Self::HINT, width));
        lines
    }

    /// Screen-reader text: numbered options, labels and the current selection.
    #[must_use]
    pub fn announce(&self) -> String {
        let list: Vec<String> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{}. {}", i + 1, c.label()))
            .collect();
        format!(
            "Permission required. Options: {}. selected: {}.",
            list.join(", "),
            self.selected().label()
        )
    }

    /// Pad or char-safe truncate `text` to exactly `width` characters.
    fn pad(text: &str, width: usize) -> String {
        let mut line: String = text.chars().take(width).collect();
        let len = line.chars().count();
        if len < width {
            line.push_str(&" ".repeat(width - len));
        }
        line
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
            board.approve(
                &OperationDigest("abc".into()),
                &other,
                Confirm::Confirmed,
                0
            ),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        // Wrong policy version refuses likewise.
        let mut older = scope();
        older.policy_version = 6;
        assert_eq!(
            board.approve(
                &OperationDigest("abc".into()),
                &older,
                Confirm::Confirmed,
                0
            ),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board.pending_len(), 1);
        // Exact digest + full scope approves and snapshots the scope.
        assert_eq!(
            board.approve(
                &OperationDigest("abc".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
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
            board.approve(
                &OperationDigest("nope".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
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
            board.approve(
                &OperationDigest("abc".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
            Outcome::Decided(_)
        ));
        // Same digest can never be decided twice.
        assert_eq!(
            board.approve(
                &OperationDigest("abc".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
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
            board.approve(
                &OperationDigest("old".into()),
                &scope(),
                Confirm::Confirmed,
                600
            ),
            Outcome::Decided(Decision {
                expired: true,
                approved: false,
                ..
            })
        ));
        assert_eq!(board.pending_len(), 0);
    }

    #[test]
    fn focus_activation_cannot_approve_destructive() {
        let mut board = ApprovalBoard::new();
        board.offer(req("rm", Risk::Destructive)).unwrap();
        // Focus/Enter maps to Unconfirmed: must not approve.
        assert_eq!(
            board.approve(
                &OperationDigest("rm".into()),
                &scope(),
                Confirm::Unconfirmed,
                0
            ),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(
            board.pending_len(),
            1,
            "request stays pending after refusal"
        );
        // Explicit confirm without the arm step is still not enough.
        assert_eq!(
            board.approve(
                &OperationDigest("rm".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        // Arm, then explicit confirm: approves with both steps recorded.
        assert!(board.arm(&OperationDigest("rm".into())));
        assert_eq!(
            board.approve(
                &OperationDigest("rm".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
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
            board.approve(
                &OperationDigest("acct".into()),
                &scope(),
                Confirm::Unconfirmed,
                0
            ),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(
            board.approve(
                &OperationDigest("acct".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert!(board.arm(&OperationDigest("acct".into())));
        assert!(matches!(
            board.approve(
                &OperationDigest("acct".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
            Outcome::Decided(Decision {
                approved: true,
                confirmed_twice: true,
                ..
            })
        ));
    }

    #[test]
    fn denial_is_single_step_and_records_no_approval() {
        let mut board = ApprovalBoard::new();
        board.offer(req("x", Risk::Destructive)).unwrap();
        assert!(matches!(
            board.deny(&OperationDigest("x".into()), 0),
            Outcome::Decided(Decision {
                approved: false,
                ..
            })
        ));
    }

    #[test]
    fn interrupt_cancels_in_flight_and_bumps_broadcast() {
        let mut board = ApprovalBoard::new();
        assert_eq!(board.interrupt(9), None, "idle interrupt is None");
        board.offer(req("op1", Risk::Safe)).unwrap();
        board.approve(
            &OperationDigest("op1".into()),
            &scope(),
            Confirm::Confirmed,
            0,
        );
        assert!(board.note_started(&OperationDigest("op1".into())));
        assert_eq!(board.in_flight(), Some(&OperationDigest("op1".into())));
        let before = board.broadcast_seq();
        let receipt = board
            .interrupt(42)
            .expect("in-flight interrupt has a receipt");
        assert_eq!(receipt.digest, OperationDigest("op1".into()));
        assert_eq!(receipt.interrupted_at_tick, 42);
        assert_eq!(receipt.broadcast_seq, before + 1);
        assert_eq!(board.in_flight(), None, "slot cleared");
        assert_eq!(
            board.broadcast_seq(),
            before + 1,
            "clients observe the update"
        );
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
        board.approve(
            &OperationDigest("op1".into()),
            &scope(),
            Confirm::Confirmed,
            0,
        );
        assert!(board.note_started(&OperationDigest("op1".into())));
        board.interrupt(7);
        assert_eq!(
            board.classify_retry(&OperationDigest("op1".into())),
            RetryKind::AmbiguousSideEffects
        );
        // Retry itself runs under a fresh digest, never a silent replay.
        board.offer(req("op1-retry", Risk::Safe)).unwrap();
        assert!(matches!(
            board.approve(
                &OperationDigest("op1-retry".into()),
                &scope(),
                Confirm::Confirmed,
                0
            ),
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

#[cfg(test)]
mod popup_tests {
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

    /// (a) Option sets per risk.
    #[test]
    fn safe_lists_three_options_human_only_two() {
        let safe = ApprovalPopup::new(Risk::Safe);
        assert_eq!(
            safe.options(),
            &[Choice::AllowOnce, Choice::AlwaysAllow, Choice::Deny]
        );
        let destructive = ApprovalPopup::new(Risk::Destructive);
        assert_eq!(
            destructive.options(),
            &[Choice::AllowOnce, Choice::AlwaysAllow, Choice::Deny]
        );
        let human = ApprovalPopup::new(Risk::HumanOnly);
        assert_eq!(human.options(), &[Choice::AllowOnce, Choice::Deny]);
        assert!(
            !human.options().contains(&Choice::AlwaysAllow),
            "always-allow is never offered for human-only grants"
        );
    }

    /// (b) Digit keys: in-range submits with that choice, out of range ignores.
    #[test]
    fn digit_quick_select_submits_and_out_of_range_ignored() {
        // Safe: Digit(2) is AlwaysAllow.
        let mut safe = ApprovalPopup::new(Risk::Safe);
        assert_eq!(safe.handle_key(PopupKey::Digit(2)), PopupAction::Submit(Choice::AlwaysAllow));
        // Out of range: ignored, no selection change.
        let mut popup = ApprovalPopup::new(Risk::Safe);
        assert_eq!(popup.handle_key(PopupKey::Digit(9)), PopupAction::Ignored);
        assert_eq!(popup.selected_index(), 0, "digit out of range changes nothing");
        // Zero and one past the end are out of range too.
        assert_eq!(popup.handle_key(PopupKey::Digit(0)), PopupAction::Ignored);
        assert_eq!(popup.handle_key(PopupKey::Digit(4)), PopupAction::Ignored);
        assert_eq!(popup.selected_index(), 0);
        // Digit('1') submits AllowOnce.
        let mut one = ApprovalPopup::new(Risk::Safe);
        assert_eq!(one.handle_key(PopupKey::Digit(b'1' - b'0')), PopupAction::Submit(Choice::AllowOnce));
        // HumanOnly: Digit(2) is Deny; Digit(3) is out of range (only 2 options).
        let mut human = ApprovalPopup::new(Risk::HumanOnly);
        assert_eq!(human.handle_key(PopupKey::Digit(b'2' - b'0')), PopupAction::Submit(Choice::Deny));
        assert_eq!(
            human.handle_key(PopupKey::Digit(b'3' - b'0')),
            PopupAction::Ignored,
            "digit past HumanOnly option count is out of range"
        );
    }

    /// (c) Up/Down/Tab wrap; selection tracks; Enter submits the current row.
    #[test]
    fn movement_wraps_and_enter_submits_current() {
        let mut popup = ApprovalPopup::new(Risk::Safe);
        assert_eq!(popup.selected(), Choice::AllowOnce);
        assert!(popup.move_prev(), "wrap backwards from first to last");
        assert_eq!(popup.selected(), Choice::Deny);
        assert!(popup.move_next(), "wrap forwards from last to first");
        assert_eq!(popup.selected(), Choice::AllowOnce);
        assert!(popup.move_next());
        assert_eq!(popup.selected(), Choice::AlwaysAllow);
        assert!(popup.move_next());
        assert_eq!(popup.selected(), Choice::Deny);
        // Tab behaves like Down (next, wrapping).
        assert!(popup.move_next());
        assert_eq!(popup.selected(), Choice::AllowOnce, "next from last wraps to first");
        assert!(popup.move_prev());
        assert_eq!(popup.selected(), Choice::Deny, "prev from first wraps to last");
        // Enter submits the currently selected row.
        assert_eq!(popup.handle_key(PopupKey::Enter), PopupAction::Submit(Choice::Deny));
    }

    /// (d) Escape cancels; a subsequent Enter still returns Cancel, never Submit.
    #[test]
    fn escape_cancels_and_blocked_enter_stays_cancelled() {
        let mut popup = ApprovalPopup::new(Risk::Safe);
        popup.move_next();
        assert_eq!(popup.handle_key(PopupKey::Escape), PopupAction::Cancel);
        // Movement after cancel still cannot resurrect approval.
        assert_eq!(popup.handle_key(PopupKey::Digit(1)), PopupAction::Cancel);
        assert_eq!(popup.handle_key(PopupKey::Enter), PopupAction::Cancel);
        assert_eq!(popup.handle_key(PopupKey::Escape), PopupAction::Cancel);
    }

    /// (e) Render: fixed width, numbered labels, one focus marker, hint mentions Esc.
    #[test]
    fn render_lines_fixed_width_with_single_marker() {
        let mut popup = ApprovalPopup::new(Risk::Safe);
        popup.move_next(); // select AlwaysAllow
        // Width invariance only: every line exactly clamp(30..=80) chars.
        for width in [0usize, 30, 44, 80, usize::MAX] {
            let lines = popup.render(width);
            assert!(lines.len() >= 3, "title, options, hint");
            for line in &lines {
                assert_eq!(line.chars().count(), width.clamp(30, 80), "line width clamped");
            }
            assert_eq!(lines[0].trim_end(), "Permission required", "title first");
        }
        // Content checks at full width (short widths truncate the hint tail).
        let lines = popup.render(80);
        let joined = lines.join("\n");
        assert!(joined.contains("1. Allow once"));
        assert!(joined.contains("2. Always allow"));
        assert!(joined.contains("3. Deny"));
        assert!(joined.contains("Esc"), "hint mentions Esc deny-free cancel");
        let markers = joined.matches("› ").count();
        assert_eq!(markers, 1, "exactly one selected-row marker");
        assert!(lines[2].starts_with("› "), "selected row carries the focus marker");
        assert!(
            !lines[1].starts_with("› ") && !lines[3].starts_with("› "),
            "only the selected row is marked"
        );
        // HumanOnly renders the always-allow notice and only 2 option rows.
        let human = ApprovalPopup::new(Risk::HumanOnly);
        let lines = human.render(80);
        let joined = lines.join("\n");
        assert!(joined.contains("Human-only action: always-allow is not available."));
        assert!(joined.contains("1. Allow once"));
        assert!(joined.contains("2. Deny"));
        assert!(!joined.contains("Always allow"));
    }

    /// (f) Mouse: click selects and submits; out-of-range click is ignored.
    #[test]
    fn click_submits_row_and_out_of_range_ignored() {
        let mut popup = ApprovalPopup::new(Risk::Safe);
        assert_eq!(popup.handle_click(1), PopupAction::Submit(Choice::AlwaysAllow));
        let mut again = ApprovalPopup::new(Risk::Safe);
        assert_eq!(again.handle_click(0), PopupAction::Submit(Choice::AllowOnce));
        assert_eq!(again.handle_click(2), PopupAction::Submit(Choice::Deny));
        let mut fresh = ApprovalPopup::new(Risk::Safe);
        assert_eq!(fresh.handle_click(3), PopupAction::Ignored);
        assert_eq!(fresh.handle_click(99), PopupAction::Ignored);
        assert_eq!(fresh.selected_index(), 0, "ignored click changes nothing");
    }

    /// (g) announce() is screen-reader text: numbered options + current selection.
    #[test]
    fn announce_lists_options_and_selection() {
        let mut popup = ApprovalPopup::new(Risk::Safe);
        let text = popup.announce();
        assert!(text.contains("Allow once"));
        assert!(text.contains("Always allow"));
        assert!(text.contains("Deny"));
        assert!(text.contains("selected:"));
        assert!(text.contains("selected: Allow once"));
        popup.move_next();
        assert!(popup.announce().contains("selected: Always allow"));
        let human = ApprovalPopup::new(Risk::HumanOnly);
        let text = human.announce();
        assert!(text.contains("1. Allow once"));
        assert!(text.contains("2. Deny"));
        assert!(!text.contains("Always allow"));
    }

    /// (h) Board: safe always-allow binds scope+expiry; wrong scope never matches.
    #[test]
    fn board_always_allow_records_and_matches_scope() {
        let mut board = ApprovalBoard::new();
        board.offer(req("lsof", Risk::Safe)).unwrap();
        let digest = OperationDigest("lsof".into());
        assert_eq!(
            board.approve_always(&digest, "shell", &scope(), Confirm::Unconfirmed, 100),
            Outcome::Decided(Decision {
                digest: digest.clone(),
                scope: scope(),
                approved: true,
                confirmed_twice: false,
                expired: false,
            }),
            "safe risk is single-step"
        );
        assert_eq!(board.pending_len(), 0);
        assert_eq!(board.history().len(), 1);
        assert_eq!(
            board.grants().len(),
            1,
            "exactly one grant recorded"
        );
        assert_eq!(
            board.grants()[0],
            AlwaysGrant {
                tool: "shell".into(),
                scope: scope(),
            }
        );
        assert!(board.is_always_allowed("shell", &scope(), 100));
        assert!(board.is_always_allowed("shell", &scope(), 599));
        // Wrong workspace: no match.
        let mut other = scope();
        other.workspace = "ws:other".into();
        assert!(!board.is_always_allowed("shell", &other, 100));
        // Older policy version: no match.
        let mut older = scope();
        older.policy_version = 6;
        assert!(!board.is_always_allowed("shell", &older, 100));
        // Expired: no match.
        assert!(!board.is_always_allowed("shell", &scope(), 600));
        assert!(!board.is_always_allowed("shell", &scope(), 601));
        // Wrong tool: no match.
        assert!(!board.is_always_allowed("edit", &scope(), 100));
        // Denied operations never grant.
        let mut board2 = ApprovalBoard::new();
        board2.offer(req("un", Risk::Safe)).unwrap();
        assert!(matches!(
            board2.deny(&OperationDigest("un".into()), 0),
            Outcome::Decided(_)
        ));
        assert!(board2.grants().is_empty());
        assert!(!board2.is_always_allowed("shell", &scope(), 0));
        // Empty tool name refused with zero mutation.
        let mut board3 = ApprovalBoard::new();
        board3.offer(req("e", Risk::Safe)).unwrap();
        let seq = board3.broadcast_seq();
        assert_eq!(
            board3.approve_always(&OperationDigest("e".into()), "", &scope(), Confirm::Unconfirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board3.pending_len(), 1, "empty tool leaves pending untouched");
        assert!(board3.grants().is_empty());
        assert_eq!(board3.broadcast_seq(), seq, "empty tool refusal mutates nothing");
        // Scope mismatch on approve_always is stale.
        let mut board4 = ApprovalBoard::new();
        board4.offer(req("m", Risk::Safe)).unwrap();
        let mut evil = scope();
        evil.requester = "other".into();
        let seq4 = board4.broadcast_seq();
        assert_eq!(
            board4.approve_always(&OperationDigest("m".into()), "shell", &evil, Confirm::Unconfirmed, 0),
            Outcome::Refused(Refusal::StaleOrReplayed)
        );
        assert_eq!(board4.pending_len(), 1);
        assert!(board4.grants().is_empty());
        assert_eq!(board4.broadcast_seq(), seq4);
    }

    /// (i) HumanOnly: always-allow refused, zero mutation, pending intact.
    #[test]
    fn board_always_allow_human_only_refused_with_zero_mutation() {
        let mut board = ApprovalBoard::new();
        board.offer(req("acct", Risk::HumanOnly)).unwrap();
        let seq = board.broadcast_seq();
        assert_eq!(
            board.approve_always(
                &OperationDigest("acct".into()),
                "account-manager",
                &scope(),
                Confirm::Confirmed,
                0
            ),
            Outcome::Refused(Refusal::ConfirmationRequired),
            "human-only grants are never persistent"
        );
        assert_eq!(board.pending_len(), 1, "pending untouched");
        assert!(board.grants().is_empty());
        assert!(board.history().is_empty());
        assert_eq!(
            board.broadcast_seq(),
            seq,
            "human-only always-allow refusal mutates nothing"
        );
        assert!(!board.is_always_allowed("account-manager", &scope(), 0));
    }

    /// (j) Destructive: always-allow needs arm + Confirmed; then confirmed_twice.
    #[test]
    fn board_always_allow_destructive_needs_two_steps() {
        let mut board = ApprovalBoard::new();
        board.offer(req("rm", Risk::Destructive)).unwrap();
        // Unconfirmed refuses and leaves the request pending.
        let seq = board.broadcast_seq();
        assert_eq!(
            board.approve_always(&OperationDigest("rm".into()), "shell", &scope(), Confirm::Unconfirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(board.pending_len(), 1);
        assert!(board.grants().is_empty());
        assert!(
            board.broadcast_seq() > seq,
            "confirmed-required refusal is observable, request stays pending"
        );
        // Confirmed without arming still refuses.
        assert_eq!(
            board.approve_always(&OperationDigest("rm".into()), "shell", &scope(), Confirm::Confirmed, 0),
            Outcome::Refused(Refusal::ConfirmationRequired)
        );
        assert_eq!(board.pending_len(), 1);
        // Arm + explicit confirm: approves with both steps recorded, grant lands.
        assert!(board.arm(&OperationDigest("rm".into())));
        let digest = OperationDigest("rm".into());
        assert_eq!(
            board.approve_always(&digest, "shell", &scope(), Confirm::Confirmed, 0),
            Outcome::Decided(Decision {
                digest,
                scope: scope(),
                approved: true,
                confirmed_twice: true,
                expired: false,
            })
        );
        assert_eq!(board.pending_len(), 0);
        assert_eq!(board.grants().len(), 1);
        assert!(board.is_always_allowed("shell", &scope(), 0));
    }

    /// (k) Grants are bounded: oldest dropped at MAX_GRANTS.
    #[test]
    fn board_grants_bounded_at_max() {
        let mut board = ApprovalBoard::new();
        for i in 0..(ApprovalBoard::MAX_GRANTS + 4) {
            let digest = OperationDigest(format!("g{i}"));
            board.offer(req(&digest.0, Risk::Safe)).unwrap();
            let outcome = board.approve_always(&digest, &format!("tool{i}"), &scope(), Confirm::Unconfirmed, 0);
            assert!(
                matches!(outcome, Outcome::Decided(_)),
                "grant {i} should settle"
            );
        }
        assert_eq!(board.grants().len(), ApprovalBoard::MAX_GRANTS);
        // Oldest dropped first: the four newest survive, g0..g3 are gone.
        assert_eq!(board.grants()[0].tool, "tool4");
        assert_eq!(board.grants().back().map(|g| g.tool.as_str()), Some("tool19"));
        assert!(!board.is_always_allowed("tool0", &scope(), 0));
        assert!(board.is_always_allowed("tool19", &scope(), 0));
    }
}
