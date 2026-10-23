#![forbid(unsafe_code)]
//! Visible focus-owning dialog views for permission / question / confirm /
//! error overlays used by the native interactive client.
//!
//! Pure state + pure rendering: no IO, no threads, no clock, no network, no
//! execution tools, no secret/credential access. The module never fetches,
//! stores or prints raw tokens, environment values or backend error bodies;
//! every visible string is caller-supplied display text sanitized here, or a
//! [`StaticError`] headline defined in this file.
//!
//! Decision contract (mirrors the installed backend contracts, adds none):
//! * [`DialogView::answer`] / [`DialogView::cancel`] return a
//!   [`DecisionIntent`] — an intent only. The view then enters *pending*:
//!   it keeps `is_open() == true` and keeps intercepting input
//!   ([`DialogView::intercepts_input`]) so busy typing or paste can never
//!   reach the underlying composer while a decision is in flight.
//! * The view closes only in [`DialogView::ack`] on a confirmed ack whose
//!   request id **and** session id match the open request. A mismatched or
//!   rejected ack leaves the view open; there is no timeout, no retry and no
//!   local success pretence.
//! * For [`DialogKind::Permission`] an approve intent is the backend's
//!   explicit single-shot reply (`once`, see `crates/server/src/lib.rs`
//!   `reply_session_permission`); a cancel/deny intent is a real
//!   permission-broker denial (`reject`) and is *never* an automatic
//!   approval. The review verdict `component-review-v1` P1 (v1 line 471
//!   emitted [`DecisionIntent::Approve`] for *every* Permission choice,
//!   including a visible "deny") is repaired here: each Permission choice
//!   label is parsed **once, at open**, against an exact allow-list of
//!   aliases into a private [`PermissionChoice`] mapping, and `answer`
//!   returns the matching explicit intent — approve labels → `Approve`,
//!   deny labels → `Reject`. Unknown labels refuse `open_request` with
//!   [`DialogError::UnknownPermissionChoice`] and change no state; there is
//!   no implicit-approve fallback and no substring inference (a label such
//!   as "do not approve" never approves — it never opens). Display labels
//!   never authorize backend execution. Wildcard / always-allow choices
//!   (`*`, `always`, `always-allow`) are refused by [`DialogView::open_request`]: no
//!   requester capability is ever granted from display text and no choice
//!   can bypass mandatory protection. Persistent-grant storage stays in the
//!   existing broker, untouched by this proposal.
//! * Security review `dialogs-security-review-v2` repairs (V3):
//!   1. A raw Permission label containing **any control scalar**
//!      (`"approve\u{0}once"`, `"approve\nonce"`, ESC insertions) is refused
//!      *before* alias parsing — classification runs on the original label
//!      with only ASCII-space trimming, never on display-sanitized output,
//!      so sanitization can never turn an untrusted label into a
//!      grant-bearing one. Display sanitization for rendering is unchanged
//!      and stays non-authoritative.
//!   2. Every [`DecisionIntent`] carries a locally monotonic
//!      `attempt: u64` correlation value, echoed by [`DialogAck`]. The
//!      counter increments on each issued answer/cancel, never resets on
//!      close, `open_request` or rejected ACK, and fails closed on
//!      overflow. An ACK must match request id, session id **and** the
//!      pending attempt (checked in the pending phase) before any state
//!      change: a delayed ACK from an earlier attempt can neither close
//!      nor reopen nor clear focus of the next attempt. This is a purely
//!      local correlation value: no server nonce, endpoint or backend API
//!      change — the caller wraps each existing HTTP reply with the
//!      attempt captured when it dispatched that intent.
//! * [`DialogKind::Question`] has no backend route today (no question POST
//!   exists server-side). While the caller's capability flag is false the
//!   view renders "question backend unavailable" and refuses to answer;
//!   nothing here invents a question endpoint or pretends an answer was
//!   delivered.
//!
//! Layout: [`DialogView::render`] clips by Unicode scalar (the layout worker
//! performs the final terminal cell width pass), strips all control bytes,
//! and always keeps the heading on row 0 so a short terminal still shows
//! what is blocking input. Pinned upstream 95daf question/permission UI is
//! only needed if the integration owner wants affordances beyond these
//! existing native contracts.

/// Upper bound on choices in one dialog (UTF-8 bytes per choice text is
/// bounded separately by [`MAX_CHOICE_LEN`]).
pub const MAX_CHOICES: usize = 64;
/// Upper bound on the request id length in bytes.
pub const MAX_ID_LEN: usize = 128;
/// Upper bound on the heading/title length in bytes.
pub const MAX_TITLE_LEN: usize = 2048;
/// Upper bound on a single choice label length in bytes.
pub const MAX_CHOICE_LEN: usize = 128;
/// Upper bound on the supplementary message body (diff / explanation) in
/// bytes.
pub const MAX_MESSAGE_LEN: usize = 2048;
/// Hard cap on rendered message block lines (keeps a hostile multi-line
/// body from flooding the viewport).
const MAX_BLOCK_LINES: usize = 16;

/// Which overlay is shown. Each kind has its own answer/cancel rules; see
/// the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Permission,
    Question,
    Confirm,
    Error,
}

/// Static, safe error-card categories. Defined in this file so raw backend
/// error text (which may embed tokens or environment values) is never
/// rendered or stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticError {
    /// Backend unavailable: no decision reached, nothing retried.
    BackendUnavailable,
    /// Outcome unknown: side effects may exist, manual review required,
    /// never retried automatically.
    UnknownSideEffects,
}

impl StaticError {
    /// Stable machine-readable code (safe to log).
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::BackendUnavailable => "backend_unavailable",
            Self::UnknownSideEffects => "unknown_side_effects",
        }
    }

    fn headline(self) -> &'static str {
        match self {
            Self::BackendUnavailable => "backend unavailable",
            Self::UnknownSideEffects => "unknown side effects",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::BackendUnavailable => "no decision was recorded; not retried automatically",
            Self::UnknownSideEffects => "manual review required; not retried automatically",
        }
    }
}

/// A typed dialog request. Pure input: every field is bounded and
/// sanitized by [`DialogView::open_request`]. `message` is optional
/// display text (diff body, explanation); `disabled` lists choice indices
/// that render but can never be selected; `error` is mandatory for
/// [`DialogKind::Error`] so the card shows a static category instead of
/// raw backend error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogRequest {
    pub id: String,
    pub session_id: String,
    pub kind: DialogKind,
    pub title: String,
    pub choices: Vec<String>,
    pub message: Option<String>,
    pub disabled: Vec<usize>,
    pub error: Option<StaticError>,
}

/// Validation / lifecycle failure. Every variant maps to a static code;
/// no request content ever appears in the error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogError {
    IdTooLong,
    TitleTooLong,
    MessageTooLong,
    TooManyChoices,
    ChoiceTooLong,
    NoChoices,
    MissingError,
    InvalidDisabledIndex,
    ForbiddenChoice,
    /// A Permission choice label is not in the exact alias allow-list —
    /// or contains a control scalar (refused raw, before any alias
    /// parsing). Refused during `open_request`: no dialog opens, no state
    /// changes, nothing becomes answerable.
    UnknownPermissionChoice,
    /// Checked decision-attempt counter overflowed (`u64::MAX` issued):
    /// fail closed — no intent issued, no pending state, counter unchanged.
    AttemptExhausted,
    /// ACK's `attempt` does not match the pending attempt. Ignored: the
    /// pending decision and focus are retained unchanged.
    AckStaleAttempt,
    AlreadyOpen,
    NotOpen,
    NoSelection,
    DisabledChoice,
    NotAnswerable,
    QuestionUnavailable,
    QuestionCancelUnsupported,
    PendingDecision,
    NoPendingDecision,
    AckMismatch,
}

impl DialogError {
    /// Stable machine-readable code (safe to log; contains no user text).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::IdTooLong => "dialog.id_too_long",
            Self::TitleTooLong => "dialog.title_too_long",
            Self::MessageTooLong => "dialog.message_too_long",
            Self::TooManyChoices => "dialog.too_many_choices",
            Self::ChoiceTooLong => "dialog.choice_too_long",
            Self::NoChoices => "dialog.no_choices",
            Self::MissingError => "dialog.missing_error",
            Self::InvalidDisabledIndex => "dialog.invalid_disabled_index",
            Self::ForbiddenChoice => "dialog.forbidden_choice",
            Self::UnknownPermissionChoice => "dialog.unknown_permission_choice",
            Self::AttemptExhausted => "dialog.attempt_exhausted",
            Self::AckStaleAttempt => "dialog.ack_stale_attempt",
            Self::AlreadyOpen => "dialog.already_open",
            Self::NotOpen => "dialog.not_open",
            Self::NoSelection => "dialog.no_selection",
            Self::DisabledChoice => "dialog.disabled_choice",
            Self::NotAnswerable => "dialog.not_answerable",
            Self::QuestionUnavailable => "dialog.question_backend_unavailable",
            Self::QuestionCancelUnsupported => "dialog.question_cancel_unsupported",
            Self::PendingDecision => "dialog.pending_decision",
            Self::NoPendingDecision => "dialog.no_pending_decision",
            Self::AckMismatch => "dialog.ack_mismatch",
        }
    }
}

/// A decision to hand to the existing backend routes — an *intent* only.
/// Carries the exact request/session identity it belongs to plus the
/// locally monotonic `attempt` correlation value that must be echoed by
/// the matching [`DialogAck`], so a stale or cross-session intent/ack can
/// be refused by the integration owner. Approve is always the single-shot
/// reply; denial is a real broker denial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionIntent {
    ScopedPermission {
        request_id: String,
        session_id: String,
        attempt: u64,
        allow: bool,
        persistent: bool,
    },
    /// Permission: explicit single-shot approval (`once`).
    Approve {
        request_id: String,
        session_id: String,
        attempt: u64,
    },
    /// Permission: explicit denial / cancel — a real permission-broker
    /// denial (`reject`), never an automatic approval.
    Reject {
        request_id: String,
        session_id: String,
        attempt: u64,
    },
    /// Question / confirm: the selected choice index plus its label, kept
    /// as typed metadata for the existing reply/persistence path (not
    /// rewritten here).
    Answer {
        request_id: String,
        session_id: String,
        choice_index: usize,
        choice: String,
        attempt: u64,
    },
    /// Confirm: explicit cancellation of the confirmation card.
    Cancel {
        request_id: String,
        session_id: String,
        attempt: u64,
    },
}

impl DecisionIntent {
    /// Request identity this intent is bound to.
    #[must_use]
    pub fn request_id(&self) -> &str {
        match self {
            Self::ScopedPermission { request_id, .. } => request_id,
            Self::Approve { request_id, .. }
            | Self::Reject { request_id, .. }
            | Self::Answer { request_id, .. }
            | Self::Cancel { request_id, .. } => request_id,
        }
    }

    /// Session identity this intent is bound to.
    #[must_use]
    pub fn session_id(&self) -> &str {
        match self {
            Self::ScopedPermission { session_id, .. } => session_id,
            Self::Approve { session_id, .. }
            | Self::Reject { session_id, .. }
            | Self::Answer { session_id, .. }
            | Self::Cancel { session_id, .. } => session_id,
        }
    }

    /// Local decision-attempt correlation value; the ACK must echo it.
    #[must_use]
    pub fn attempt(&self) -> u64 {
        match self {
            Self::ScopedPermission { attempt, .. } => *attempt,
            Self::Approve { attempt, .. }
            | Self::Reject { attempt, .. }
            | Self::Answer { attempt, .. }
            | Self::Cancel { attempt, .. } => *attempt,
        }
    }
}

/// Backend acknowledgement for the in-flight decision. `accepted == true`
/// with matching request id, session id **and** `attempt` (the value
/// carried by the pending intent) is the only condition that closes the
/// view. `attempt` is a purely local correlation value — not a server
/// nonce; the caller echoes the attempt it captured when dispatching the
/// intent alongside the existing HTTP reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogAck {
    pub request_id: String,
    pub session_id: String,
    pub accepted: bool,
    pub attempt: u64,
}

/// Result of a matching acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckOutcome {
    /// Backend confirmed the decision; the view closed.
    Closed,
    /// Backend refused the decision; the view stays open for a new choice.
    Rejected,
}

/// Modal lifecycle. Pending holds focus (and input interception) until a
/// matching ack arrives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Closed,
    Open,
    Pending,
}

/// Sanitized, bounds-checked dialog kept while the overlay is visible.
#[derive(Clone, Debug)]
struct OpenDialog {
    id: String,
    session_id: String,
    kind: DialogKind,
    title: String,
    choices: Vec<String>,
    disabled: Vec<bool>,
    /// Parallel Permission decision mapping (same length as `choices`);
    /// empty for non-Permission kinds.
    permission: Vec<PermissionChoice>,
    message_lines: Vec<String>,
    error: StaticError,
}

/// Focus-owning dialog overlay state. `Default` is a closed view.
#[derive(Debug, Default)]
pub struct DialogView {
    phase: Phase,
    dialog: Option<OpenDialog>,
    selection: Option<usize>,
    question_capable: bool,
    rejected: bool,
    /// Locally monotonic decision-attempt counter. Never reset on close,
    /// `open_request` or rejected ACK; fails closed on overflow.
    attempt_counter: u64,
    /// Attempt of the in-flight decision; meaningful while pending.
    pending_attempt: u64,
}

/// Replace every control character (C0/C1, including newline and ESC) with
/// a space, then trim. Single-line fields only — no line can be injected.
fn sanitize_inline(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Multi-line display body: CR removed, split on LF, each line sanitized
/// inline, bounded to [`MAX_BLOCK_LINES`].
fn sanitize_block(s: &str) -> Vec<String> {
    s.replace('\r', "")
        .split('\n')
        .map(sanitize_inline)
        .take(MAX_BLOCK_LINES)
        .collect()
}

/// Clip by Unicode scalar count. The layout worker does the final terminal
/// cell width (wcwidth) pass afterwards.
fn clip(line: &str, width: usize) -> String {
    line.chars().take(width).collect()
}

fn wildcard_choice(choice: &str) -> bool {
    let normalized = choice.trim().to_ascii_lowercase();
    normalized == "*" || normalized == "always" || normalized == "always-allow"
}

/// Explicit decision type for one Permission choice. Every Permission
/// option is mapped affirmative or negative at open time — there is no
/// "unknown ⇒ approve" fallback anywhere in this module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PermissionChoice {
    ApproveOnce,
    Reject,
    Scoped { allow: bool, persistent: bool },
}

/// Exact allow-list of Permission choice aliases, matched against the
/// **original unmodified label** after trimming ASCII spaces only — never
/// against display-sanitized output, never with case folding or
/// whitespace collapsing, so no transformation can manufacture a
/// grant-bearing label from untrusted input.
fn permission_alias(label: &str) -> Option<PermissionChoice> {
    match label.trim_matches(' ') {
        "allow" | "approve" | "once" | "allow once" | "approve once" => {
            Some(PermissionChoice::ApproveOnce)
        }
        "deny" | "reject" => Some(PermissionChoice::Reject),
        "Allow for this session" => Some(PermissionChoice::Scoped {allow:true,persistent:false}),
        "Always allow" => Some(PermissionChoice::Scoped {allow:true,persistent:true}),
        "Deny for this session" => Some(PermissionChoice::Scoped {allow:false,persistent:false}),
        "Always deny" => Some(PermissionChoice::Scoped {allow:false,persistent:true}),
        "Allow once" => Some(PermissionChoice::ApproveOnce),
        "Deny once" => Some(PermissionChoice::Reject),
        _ => None,
    }
}

impl DialogView {
    /// Closed view (no dialog, input passes through).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True while a dialog is visible — including the pending-decision
    /// phase. While true, the shell must route keys/paste here and never to
    /// the composer.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.phase != Phase::Closed
    }

    /// True while a decision is in flight awaiting a backend ack.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.phase == Phase::Pending
    }

    /// Whether input must be intercepted (busy input/paste cannot submit
    /// the underlying composer while a dialog is visible).
    #[must_use]
    pub fn intercepts_input(&self) -> bool {
        self.is_open()
    }

    /// Caller-reported capability: the question backend exists. Defaults to
    /// `false` because no question route is installed today; until a caller
    /// turns it on, question dialogs render unavailable and refuse answers.
    pub fn set_question_capable(&mut self, capable: bool) {
        self.question_capable = capable;
    }

    /// Current question capability flag.
    #[must_use]
    pub fn question_capable(&self) -> bool {
        self.question_capable
    }

    /// Dismiss the dialog. A pending decision holds focus: `close` is a
    /// no-op until the matching ack arrives (a denial cannot be discarded
    /// locally to make the overlay disappear).
    pub fn close(&mut self) {
        if self.phase == Phase::Open {
            self.reset();
        }
    }

    /// Validate, sanitize and open a dialog. Refuses to replace a visible
    /// dialog (including one pending) so a focus owner is never silently
    /// swapped out.
    pub fn open_request(&mut self, request: DialogRequest) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        if request.id.len() > MAX_ID_LEN {
            return Err(DialogError::IdTooLong);
        }
        if request.title.len() > MAX_TITLE_LEN {
            return Err(DialogError::TitleTooLong);
        }
        if request
            .message
            .as_ref()
            .is_some_and(|m| m.len() > MAX_MESSAGE_LEN)
        {
            return Err(DialogError::MessageTooLong);
        }
        if request.choices.len() > MAX_CHOICES {
            return Err(DialogError::TooManyChoices);
        }
        for choice in &request.choices {
            if choice.len() > MAX_CHOICE_LEN {
                return Err(DialogError::ChoiceTooLong);
            }
        }
        if request.kind != DialogKind::Error && request.choices.is_empty() {
            return Err(DialogError::NoChoices);
        }
        if request.kind == DialogKind::Error && request.error.is_none() {
            return Err(DialogError::MissingError);
        }
        if request.disabled.iter().any(|&i| i >= request.choices.len()) {
            return Err(DialogError::InvalidDisabledIndex);
        }
        // No wildcard / always-allow choice: display text never grants a
        // persistent capability and never bypasses mandatory protection.
        if request.kind == DialogKind::Permission
            && request.choices.iter().any(|c| wildcard_choice(c))
        {
            return Err(DialogError::ForbiddenChoice);
        }

        let sanitized_choices: Vec<String> =
            request.choices.iter().map(|c| sanitize_inline(c)).collect();
        // P1/V3 repair: every Permission choice must map to an explicit
        // decision at open time, classified on the ORIGINAL raw label
        // (ASCII-space trim only). Any control scalar in the raw label is
        // refused before alias parsing — display sanitization output is
        // never classified, so it can never turn "approve\0once" or
        // "approve\nonce" into a valid approval. Unknown/malicious labels
        // refuse before any state change — no dialog opens, nothing is
        // pending, nothing answerable, and no label can ever fall through
        // to an implicit approve.
        let permission: Vec<PermissionChoice> = if request.kind == DialogKind::Permission {
            let mut mapped = Vec::with_capacity(request.choices.len());
            for raw in &request.choices {
                if raw.chars().any(char::is_control) {
                    return Err(DialogError::UnknownPermissionChoice);
                }
                match permission_alias(raw) {
                    Some(choice) => mapped.push(choice),
                    None => return Err(DialogError::UnknownPermissionChoice),
                }
            }
            mapped
        } else {
            Vec::new()
        };

        let mut disabled = vec![false; request.choices.len()];
        for &i in &request.disabled {
            disabled[i] = true;
        }
        self.dialog = Some(OpenDialog {
            id: request.id,
            session_id: request.session_id,
            kind: request.kind,
            title: sanitize_inline(&request.title),
            choices: sanitized_choices,
            disabled,
            permission,
            message_lines: request
                .message
                .as_deref()
                .map(sanitize_block)
                .unwrap_or_default(),
            error: request.error.unwrap_or(StaticError::BackendUnavailable),
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        Ok(())
    }

    /// Typed integration helper (additive): open a Permission dialog with
    /// fixed, reviewed labels — index 0 "approve once" (single-shot
    /// affirmative), index 1 "deny" (explicit negative) — so callers never
    /// depend on free-form display text for the decision identity.
    pub fn open_permission(
        &mut self,
        request_id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
    ) -> Result<(), DialogError> {
        self.open_request(DialogRequest {
            id: request_id.into(),
            session_id: session_id.into(),
            kind: DialogKind::Permission,
            title: title.into(),
            choices: vec!["Allow once", "Deny once", "Allow for this session", "Always allow", "Deny for this session", "Always deny"].into_iter().map(str::to_owned).collect(),
            message: None,
            disabled: Vec::new(),
            error: None,
        })
    }

    /// Move the selection forward, wrapping and skipping disabled choices.
    /// Ignored while pending (the decision is already in flight) and when
    /// no choice is enabled (selection stays unset).
    pub fn next(&mut self) {
        self.step(true);
    }

    pub fn select(&mut self, index: usize) -> bool {
        if self.phase != Phase::Open { return false; }
        let Some(dialog) = self.dialog.as_ref() else { return false; };
        if index >= dialog.choices.len() || dialog.disabled[index] { return false; }
        self.selection = Some(index);
        true
    }

    /// Move the selection backward, wrapping and skipping disabled choices.
    pub fn prev(&mut self) {
        self.step(false);
    }

    /// Produce the answer intent for the current selection and enter the
    /// pending phase (focus held until ack). No valid selection — none, or
    /// all disabled — yields no answer.
    pub fn answer(&mut self) -> Result<DecisionIntent, DialogError> {
        self.require_interactive()?;
        let Some(dialog) = self.dialog.as_ref() else {
            return Err(DialogError::NotOpen);
        };
        match dialog.kind {
            DialogKind::Error => return Err(DialogError::NotAnswerable),
            DialogKind::Question if !self.question_capable => {
                return Err(DialogError::QuestionUnavailable)
            }
            _ => {}
        }
        let index = self.selection.ok_or(DialogError::NoSelection)?;
        if dialog.disabled.get(index).copied().unwrap_or(true) {
            return Err(DialogError::DisabledChoice);
        }
        let kind = dialog.kind;
        let request_id = dialog.id.clone();
        let session_id = dialog.session_id.clone();
        let permission = dialog.permission.get(index).copied();
        let choice = dialog.choices.get(index).cloned().unwrap_or_default();
        // Resolve the decision BEFORE consuming an attempt number: a
        // refused answer burns no attempt and never reaches pending.
        if kind == DialogKind::Permission && permission.is_none() {
            return Err(DialogError::UnknownPermissionChoice);
        }
        let attempt = self.next_attempt()?;
        let intent = match kind {
            DialogKind::Permission => match permission {
                Some(PermissionChoice::Scoped { allow, persistent }) => DecisionIntent::ScopedPermission {
                    request_id,session_id,attempt,allow,persistent,
                },
                Some(PermissionChoice::ApproveOnce) => DecisionIntent::Approve {
                    request_id,
                    session_id,
                    attempt,
                },
                Some(PermissionChoice::Reject) => DecisionIntent::Reject {
                    request_id,
                    session_id,
                    attempt,
                },
                // Defensive only: `open_request` refuses unmapped labels,
                // so this arm is unreachable — and even here the outcome
                // is an error, never an approve fallback.
                None => unreachable!("validated above"),
            },
            DialogKind::Question | DialogKind::Confirm => DecisionIntent::Answer {
                request_id,
                session_id,
                choice_index: index,
                choice,
                attempt,
            },
            DialogKind::Error => unreachable!("answered above"),
        };
        self.pending_attempt = attempt;
        self.phase = Phase::Pending;
        Ok(intent)
    }

    /// Explicit cancel/deny intent. Permission cancel is a real broker
    /// denial (`reject`), never an automatic approval. Question cancel
    /// (Esc) is unsupported today: reported explicitly as an error and the
    /// dialog stays open — safe, no fabricated answer.
    pub fn cancel(&mut self) -> Result<DecisionIntent, DialogError> {
        self.require_interactive()?;
        let Some(dialog) = self.dialog.as_ref() else {
            return Err(DialogError::NotOpen);
        };
        let kind = dialog.kind;
        match kind {
            DialogKind::Question => return Err(DialogError::QuestionCancelUnsupported),
            DialogKind::Error => return Err(DialogError::NotAnswerable),
            _ => {}
        }
        let request_id = dialog.id.clone();
        let session_id = dialog.session_id.clone();
        let attempt = self.next_attempt()?;
        let intent = match kind {
            DialogKind::Permission => DecisionIntent::Reject {
                request_id,
                session_id,
                attempt,
            },
            DialogKind::Confirm => DecisionIntent::Cancel {
                request_id,
                session_id,
                attempt,
            },
            DialogKind::Question | DialogKind::Error => unreachable!("handled above"),
        };
        self.pending_attempt = attempt;
        self.phase = Phase::Pending;
        Ok(intent)
    }

    /// Backend acknowledgement. The view closes only on `accepted == true`
    /// with request id, session id **and** `attempt` matching the pending
    /// decision, and only in the pending phase. A mismatched identity or a
    /// stale attempt is refused with no state change — a delayed ACK from
    /// an earlier attempt can never close, reopen or clear focus of the
    /// current attempt. A matching rejected ACK reopens the choice; the
    /// next intent then carries a different, higher attempt.
    pub fn ack(&mut self, ack: DialogAck) -> Result<AckOutcome, DialogError> {
        if self.phase == Phase::Closed {
            return Err(DialogError::NotOpen);
        }
        if self.phase == Phase::Open {
            return Err(DialogError::NoPendingDecision);
        }
        let Some(dialog) = self.dialog.as_ref() else {
            return Err(DialogError::NotOpen);
        };
        if ack.request_id != dialog.id || ack.session_id != dialog.session_id {
            return Err(DialogError::AckMismatch);
        }
        if ack.attempt != self.pending_attempt {
            return Err(DialogError::AckStaleAttempt);
        }
        if ack.accepted {
            self.reset();
            Ok(AckOutcome::Closed)
        } else {
            self.phase = Phase::Open;
            self.selection = None;
            self.rejected = true;
            Ok(AckOutcome::Rejected)
        }
    }

    /// Consume the next decision-attempt number. Locally monotonic: never
    /// reset on close, `open_request` or rejected ACK. Checked at
    /// `u64::MAX`: fails closed before pending — error returned, counter
    /// unchanged, no wrap, no state change.
    fn next_attempt(&mut self) -> Result<u64, DialogError> {
        let attempt = self
            .attempt_counter
            .checked_add(1)
            .ok_or(DialogError::AttemptExhausted)?;
        self.attempt_counter = attempt;
        Ok(attempt)
    }

    /// Render the modal into a `height`-line grid of at most `width`
    /// chars each. Row 0 is always the heading, so a short terminal still
    /// shows what owns focus. All control bytes are stripped; clipping is
    /// by Unicode scalar (the layout worker applies final cell width).
    #[must_use]
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        self.render_styled(width, height).into_iter().map(|line| line.text).collect()
    }

    pub fn render_styled(&self, width: usize, height: usize) -> Vec<crate::native_ui_style::Line> {
        self.layout(width, height).0
    }

    pub fn choice_at_row(&self, width: usize, height: usize, row: usize) -> Option<usize> {
        self.layout(width, height).1.get(row).copied().flatten()
    }

    /// Click-confirm affordance derived from the same layout that renders
    /// the dialog: true only for the actual rendered footer row. Never the
    /// blank padding below it, never a hidden/clipped footer (no footer
    /// line exists for errors), and never a pending decision.
    #[must_use]
    pub fn confirm_at_row(&self, width: usize, height: usize, row: usize) -> bool {
        self.layout(width, height).2 == Some(row)
    }

    #[must_use]
    pub fn choice_at_cell(&self, width: usize, height: usize, row: usize, col: usize) -> Option<usize> {
        let (lines, hits, _) = self.layout(width, height);
        let index = hits.get(row).copied().flatten()?;
        let text = lines.get(row)?.text.as_str();
        let dialog = self.dialog.as_ref()?;
        let label = dialog.choices.get(index)?;
        let start = 3 + (index + 1).to_string().chars().count() + 2;
        let end = start + label.chars().count();
        (col >= start && col < end && col < text.chars().count()).then_some(index)
    }

    #[must_use]
    pub fn confirm_at_cell(&self, width: usize, height: usize, row: usize, col: usize) -> bool {
        let (lines, _, footer) = self.layout(width, height);
        if footer != Some(row) { return false; }
        let text = lines.get(row).map(|line| line.text.as_str()).unwrap_or("");
        let Some(start) = text
            .match_indices("click here confirm")
            .next()
            .map(|(byte, _)| text[..byte].chars().count())
        else { return false };
        let end = start + "click here confirm".chars().count();
        col >= start && col < end
    }

    fn layout(&self, width: usize, height: usize) -> (Vec<crate::native_ui_style::Line>, Vec<Option<usize>>, Option<usize>) {
        use crate::native_ui_style::{Line, Role};
        if height == 0 {
            return (Vec::new(), Vec::new(), None);
        }
        let mut hits = vec![None; height];
        let mut footer_row: Option<usize> = None;
        let mut lines = Vec::new();
        let line = |text, role| Line {text,role,detail_from:None};
        if let Some(dialog) = self.dialog.as_ref() {
            // Heading first: prioritized on short terminals.
            lines.push(line(dialog.title.clone(), Role::Heading));
            let roomy = height >= dialog.choices.len() + 7;
            if roomy {
                lines.push(line(String::new(), Role::Description));
            }
            match dialog.kind {
                DialogKind::Permission => {
                    lines.push(line("permission — explicit approve or deny required".to_string(), Role::Description));
                }
                DialogKind::Question if !self.question_capable => {
                    lines.push(line("question backend unavailable".to_string(), Role::Error));
                }
                DialogKind::Question => lines.push(line("question — select an answer".to_string(), Role::Description)),
                DialogKind::Confirm => lines.push(line("confirmation required".to_string(), Role::Description)),
                DialogKind::Error => {
                    lines.push(line(dialog.error.headline().to_string(), Role::Error));
                    lines.push(line(dialog.error.detail().to_string(), Role::Description));
                }
            }
            let has_footer = !matches!(dialog.kind, DialogKind::Error)
                && !(dialog.kind == DialogKind::Question && !self.question_capable);
            let footer_rows = usize::from(has_footer && height > 1);
            let choice_rows = dialog.choices.len().min(height.saturating_sub(3)).max(1);
            let message_budget = height.saturating_sub(
                lines.len() + footer_rows + choice_rows + if roomy { 2 } else { 0 },
            );
            for message in dialog.message_lines.iter().take(message_budget) {
                lines.push(line(format!("  {message}"), Role::Item));
            }
            if roomy && !dialog.choices.is_empty() {
                lines.push(line(String::new(), Role::Description));
            }
            // Keep the active choice in the viewport; supplementary text may
            // shrink, but neither focus nor the action hint may scroll away.
            let capacity = height.saturating_sub(lines.len() + footer_rows + usize::from(roomy));
            let start = self.selection.unwrap_or(0).saturating_sub(capacity.saturating_sub(1));
            for (index, choice) in dialog.choices.iter().enumerate().skip(start).take(capacity) {
                if lines.len() < height && !dialog.disabled[index] && self.phase == Phase::Open {
                    hits[lines.len()] = Some(index);
                }
                let marker = if self.selection == Some(index) {
                    " > "
                } else {
                    "   "
                };
                let suffix = if dialog.disabled[index] {
                    " (disabled)"
                } else {
                    ""
                };
                lines.push(line(format!("{marker}{}. {}{suffix}", index + 1, choice),
                    if self.selection == Some(index) { Role::Selected } else if dialog.disabled[index] { Role::Description } else { Role::Item }));
            }
            if roomy && has_footer {
                lines.push(line(String::new(), Role::Description));
            }
            let footer = if self.phase == Phase::Pending {
                "decision sent — waiting for backend acknowledgement"
            } else if self.rejected {
                "Decision rejected — ↑/↓ choose · Enter retry · Esc deny"
            } else {
                "1–9 / ↑↓ choose · Enter / click here confirm · Esc deny"
            };
            match dialog.kind {
                DialogKind::Question if !self.question_capable => {}
                DialogKind::Error => {}
                _ if height > 1 => {
                    lines.truncate(height - 1);
                    hits[height - 1] = None;
                    lines.push(line(footer.to_owned(), if self.rejected { Role::Error } else { Role::Hint }));
                    footer_row = (self.phase == Phase::Open).then(|| lines.len() - 1);
                }
                _ => {}
            }
        }

        lines.truncate(height);
        while lines.len() < height {
            lines.push(line(String::new(), Role::Description));
        }
        (lines.into_iter().map(|mut line| { line.text = clip(&line.text, width); line }).collect(), hits, footer_row)
    }

    fn require_interactive(&self) -> Result<(), DialogError> {
        match self.phase {
            Phase::Closed => Err(DialogError::NotOpen),
            Phase::Pending => Err(DialogError::PendingDecision),
            Phase::Open => Ok(()),
        }
    }

    fn step(&mut self, forward: bool) {
        if self.phase != Phase::Open {
            return;
        }
        let Some(dialog) = self.dialog.as_ref() else {
            return;
        };
        let total = dialog.choices.len();
        if total == 0 {
            return;
        }
        let start = match self.selection {
            None if forward => 0,
            None => total - 1,
            Some(current) if forward => (current + 1) % total,
            Some(current) => (current + total - 1) % total,
        };
        for offset in 0..total {
            let index = (start + offset) % total;
            if !dialog.disabled[index] {
                self.selection = Some(index);
                return;
            }
        }
        // Every choice disabled: no valid selection exists.
        self.selection = None;
    }

    fn reset(&mut self) {
        self.phase = Phase::Closed;
        self.dialog = None;
        self.selection = None;
        self.rejected = false;
    }
}

#[cfg(test)]
mod scoped_input_tests {
    use super::*;
    #[test]
    fn six_choices_have_exact_scopes_and_no_implicit_approval() {
        for index in 0..6 {
            let mut dialog = DialogView::new();
            dialog.open_permission("request", "session", "Tool permission").unwrap();
            assert!(dialog.answer().is_err());
            assert!(dialog.select(index));
            let intent = dialog.answer().unwrap();
            match index {
                0 => assert!(matches!(intent, DecisionIntent::Approve {..})),
                1 => assert!(matches!(intent, DecisionIntent::Reject {..})),
                2..=5 => assert!(matches!(intent, DecisionIntent::ScopedPermission {allow,persistent,..}
                    if allow == (index < 4) && persistent == (index == 3 || index == 5))),
                _ => unreachable!(),
            }
            assert!(!dialog.select(0));
        }
    }
    #[test]
    fn mouse_rows_match_rendered_choices_at_compact_and_large_sizes() {
        let mut dialog = DialogView::new();
        dialog.open_permission("r","s","permission").unwrap();
        for height in [9, 12, 24] {
            let rows = dialog.render(60, height);
            let mut hits = 0;
            for row in 0..height {
                if let Some(index) = dialog.choice_at_row(60,height,row) {
                    assert!(rows[row].contains(&format!("{}. ",index + 1)));
                    hits += 1;
                }
            }
            assert_eq!(hits,6);
            assert!(dialog.choice_at_row(60,height,0).is_none());
        }
        dialog.select(5);
        dialog.answer().unwrap();
        assert!((0..24).all(|row| dialog.choice_at_row(60,24,row).is_none()));
    }
    #[test]
    fn click_footer_row_from_render_confirms_blank_padding_inert() {
        let mut dialog = DialogView::new();
        dialog.open_permission("r","s","permission").unwrap();
        let height = 24;
        let rows = dialog.render(60, height);
        let footer = rows.iter().position(|row| row.contains("click here confirm")).expect("footer row rendered");
        assert_ne!(footer + 1, height, "regression: rendered footer sits above blank padding");
        for row in 0..height {
            assert_eq!(dialog.confirm_at_row(60, height, row), rows[row].contains("click here confirm"), "row {row}");
        }
        assert!(dialog.choice_at_row(60, height, footer).is_none());
        assert!(dialog.answer().is_err(), "unselected footer confirm stays inert");
        dialog.select(5);
        assert!(dialog.confirm_at_row(60, height, footer));
        let intent = dialog.answer().unwrap();
        assert!(matches!(intent, DecisionIntent::ScopedPermission { allow: false, persistent: true, .. }));
        assert!(!dialog.confirm_at_row(60, height, footer), "pending phase footer never re-activates");
    }
    #[test]
    fn compact_heights_and_low_widths_keep_confirm_rows_safe() {
        let mut dialog = DialogView::new();
        dialog.open_permission("r","s","permission").unwrap();
        for height in 0..=8 {
            for width in [0usize, 1, 4, 60] {
                let mut confirm_rows = 0;
                for row in 0..height {
                    if dialog.confirm_at_row(width, height, row) {
                        confirm_rows += 1;
                        assert!(dialog.choice_at_row(width, height, row).is_none(), "row {row}");
                    }
                }
                assert!(confirm_rows <= 1, "width {width} height {height}");
            }
        }
    }

    #[test]
    fn clipped_choice_and_footer_columns_are_inert() {
        let mut dialog = DialogView::new();
        dialog.open_permission("r", "s", "permission").unwrap();
        let height = 24;
        let rows = dialog.render(60, height);
        let choice_row = (0..height)
            .find(|&row| dialog.choice_at_row(60, height, row) == Some(0))
            .unwrap();
        assert_eq!(dialog.choice_at_cell(60, height, choice_row, 0), None);
        assert_eq!(dialog.choice_at_cell(60, height, choice_row, 6), Some(0));
        let footer = rows.iter().position(|row| row.contains("click here confirm")).unwrap();
        assert!(!dialog.confirm_at_cell(60, height, footer, 0));
        let confirm_column = rows[footer]
            .match_indices("click here confirm")
            .next()
            .map(|(byte, _)| rows[footer][..byte].chars().count())
            .unwrap();
        assert!(dialog.confirm_at_cell(60, height, footer, confirm_column));
        assert!(!dialog.confirm_at_cell(4, height, footer, 3));
        assert!(dialog.choice_at_cell(0, height, choice_row, 0).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_dialog_keeps_focused_choice_and_keyboard_footer_visible() {
        let mut view = open_permission();
        view.next();
        view.next();
        let rows = view.render(56, 5);
        assert!(rows.iter().any(|row| row.contains("> 2. deny")));
        assert!(rows.last().unwrap().contains("Enter"), "{rows:?}");
    }

    #[test]
    fn roomy_dialog_separates_heading_choices_and_footer() {
        let view = open_permission();
        let rows = view.render(56, 12);
        assert_eq!(rows[1], "");
        let choice = rows.iter().position(|row| row.contains("1. approve once")).unwrap();
        assert_eq!(rows[choice - 1], "");
        let footer = rows.iter().position(|row| row.contains("Enter")).unwrap();
        assert_eq!(rows[footer - 1], "");
    }

    fn permission_request() -> DialogRequest {
        DialogRequest {
            id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Permission,
            title: "Run shell command".to_string(),
            choices: vec!["approve once".to_string(), "deny".to_string()],
            message: Some("diff body".to_string()),
            disabled: Vec::new(),
            error: None,
        }
    }

    fn open_permission() -> DialogView {
        let mut view = DialogView::new();
        view.open_request(permission_request())
            .expect("valid request");
        view
    }

    #[test]
    fn mismatched_ack_keeps_pending_and_matching_ack_closes() {
        let mut view = open_permission();
        assert!(view.is_open());
        view.next();
        let intent = view.answer().expect("selection valid");
        assert_eq!(intent.request_id(), "req-1");
        assert_eq!(intent.session_id(), "ses-1");
        // First issued attempt of a fresh view; fixtures capture it from
        // the intent instead of assuming a value.
        let attempt = intent.attempt();
        assert_eq!(attempt, 1);
        assert!(view.pending());
        assert!(view.is_open());
        assert!(view.intercepts_input());
        // No second answer while pending.
        assert_eq!(view.answer(), Err(DialogError::PendingDecision));
        // Wrong request id: refused, still pending, focus retained.
        let wrong_id = DialogAck {
            request_id: "req-2".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt,
        };
        assert_eq!(view.ack(wrong_id), Err(DialogError::AckMismatch));
        assert!(view.pending());
        // Wrong session id: refused likewise.
        let wrong_session = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-2".to_string(),
            accepted: true,
            attempt,
        };
        assert_eq!(view.ack(wrong_session), Err(DialogError::AckMismatch));
        assert!(view.is_open());
        // Rejected ack (matching attempt) reopens the choice.
        let rejected = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: false,
            attempt,
        };
        assert_eq!(view.ack(rejected), Ok(AckOutcome::Rejected));
        assert!(view.is_open());
        assert!(!view.pending());
        // Only a confirmed matching ack closes — retry gets a NEW attempt.
        view.next();
        let retry = view.answer().expect("retry allowed after rejection");
        let retry_attempt = retry.attempt();
        assert!(retry_attempt > attempt, "attempt must advance");
        // The previous attempt's accepted ack must not close this one.
        let stale = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt,
        };
        assert_eq!(view.ack(stale), Err(DialogError::AckStaleAttempt));
        assert!(view.pending());
        let accepted = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt: retry_attempt,
        };
        assert_eq!(view.ack(accepted), Ok(AckOutcome::Closed));
        assert!(!view.is_open());
        assert!(!view.intercepts_input());
        assert_eq!(
            view.ack(DialogAck {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                accepted: true,
                attempt: retry_attempt,
            }),
            Err(DialogError::NotOpen)
        );
    }

    #[test]
    fn cancel_is_broker_denial_and_question_cancel_is_unsupported() {
        let mut view = open_permission();
        let intent = view.cancel().expect("permission cancel emits intent");
        assert!(matches!(intent, DecisionIntent::Reject { .. }));
        assert_eq!(intent.attempt(), 1);
        assert_ne!(
            intent,
            DecisionIntent::Approve {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                attempt: 1,
            }
        );
        assert_eq!(intent.request_id(), "req-1");
        // Denial is also pending: it needs the real backend decision ack.
        assert!(view.pending());
        assert_eq!(view.cancel(), Err(DialogError::PendingDecision));

        // Question cancel (Esc) is unsupported and safe: explicit error,
        // no answer, dialog stays open.
        let mut question = DialogView::new();
        question
            .open_request(DialogRequest {
                id: "q-1".to_string(),
                session_id: "ses-1".to_string(),
                kind: DialogKind::Question,
                title: "Pick a region".to_string(),
                choices: vec!["eu".to_string(), "us".to_string()],
                message: None,
                disabled: Vec::new(),
                error: None,
            })
            .expect("valid question");
        assert_eq!(
            question.cancel(),
            Err(DialogError::QuestionCancelUnsupported)
        );
        assert!(question.is_open());
        assert!(!question.pending());
    }

    #[test]
    fn disabled_choice_never_answers() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "c-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Confirm,
            title: "Format diff".to_string(),
            choices: vec!["apply".to_string(), "skip".to_string(), "abort".to_string()],
            message: Some("-a\n+b".to_string()),
            disabled: vec![1],
            error: None,
        })
        .expect("valid confirm");
        // Fresh dialog: no selection, so no answer.
        assert_eq!(view.answer(), Err(DialogError::NoSelection));
        let shows = |view: &DialogView, line: &str| view.render(80, 40).iter().any(|l| l == line);
        // Selection skips the disabled choice in both directions.
        view.next();
        assert!(shows(&view, " > 1. apply"));
        view.next(); // skips disabled 2 straight to 3
        assert!(shows(&view, " > 3. abort"));
        assert!(shows(&view, "   2. skip (disabled)"));
        assert!(!shows(&view, " > 2. skip (disabled)"));
        view.next(); // wraps to 1
        assert!(shows(&view, " > 1. apply"));
        view.prev(); // back over disabled 2 to 3
        assert!(shows(&view, " > 3. abort"));
        // All choices disabled: still no valid selection, no answer.
        let mut all_disabled = DialogView::new();
        all_disabled
            .open_request(DialogRequest {
                disabled: vec![0, 1, 2],
                ..permission_request_for_confirm()
            })
            .expect("valid");
        all_disabled.next();
        assert_eq!(all_disabled.answer(), Err(DialogError::NoSelection));
        // Out-of-range disabled index is a validation error.
        let mut bad = DialogView::new();
        assert_eq!(
            bad.open_request(DialogRequest {
                disabled: vec![9],
                ..permission_request()
            }),
            Err(DialogError::InvalidDisabledIndex)
        );
    }

    fn permission_request_for_confirm() -> DialogRequest {
        DialogRequest {
            kind: DialogKind::Confirm,
            choices: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            disabled: Vec::new(),
            ..permission_request()
        }
    }

    #[test]
    fn secret_control_bytes_and_line_injection_never_render() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "s-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Confirm,
            title: "TOK\u{1b}[31m=secret\nINJECTED".to_string(),
            choices: vec!["ok".to_string()],
            message: Some("line1\r\u{7}bell\u{0}nul\nline2\u{1b}[2J".to_string()),
            disabled: Vec::new(),
            error: None,
        })
        .expect("valid");
        let lines = view.render(120, 40);
        assert_eq!(lines.len(), 40);
        for line in &lines {
            assert!(
                !line.chars().any(char::is_control),
                "control byte leaked: {line:?}"
            );
        }
        // The newline in the title cannot create a second heading line:
        // row 0 holds the whole (space-substituted) heading.
        assert!(lines[0].contains("INJECTED"));
        assert!(!lines[0].contains('\u{1b}'));
        // Message block keeps intended line separation only.
        assert!(lines.iter().any(|l| l.contains("line1 bell nul")));
        assert!(lines.iter().any(|l| l.contains("line2 [2J")));
    }

    #[test]
    fn tiny_viewport_keeps_heading_on_row_zero_and_bounds_lines() {
        let mut view = open_permission();
        view.next();
        view.answer().expect("valid");
        let lines = view.render(10, 1);
        assert_eq!(lines.len(), 1);
        // Heading wins row 0 on a one-line terminal, clipped to width.
        assert!(lines[0].starts_with("Run shell"));
        assert!(lines[0].chars().count() <= 10);
        // Width clipping is char-based; the layout worker does cell width.
        let wide = view.render(4, 3);
        assert!(wide.iter().all(|l| l.chars().count() <= 4));
        // Zero-sized viewport renders nothing.
        assert!(view.render(0, 0).is_empty());
        // Pending marker shows on taller viewports.
        let tall = view.render(80, 40);
        assert!(tall
            .iter()
            .any(|l| l.contains("waiting for backend acknowledgement")));
    }

    #[test]
    fn question_renders_unavailable_and_never_answers_without_capability() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "q-9".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Question,
            title: "Choose".to_string(),
            choices: vec!["one".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
        })
        .expect("valid question");
        let lines = view.render(80, 40);
        assert!(lines
            .iter()
            .any(|l| l.contains("question backend unavailable")));
        assert!(!lines.iter().any(|l| l.contains("answered")));
        view.next();
        assert_eq!(view.answer(), Err(DialogError::QuestionUnavailable));
        assert!(!view.pending());
        // Caller capability flips the contract on.
        view.set_question_capable(true);
        assert!(view.question_capable());
        view.next();
        let intent = view.answer().expect("capable caller may answer");
        assert!(matches!(
            intent,
            DecisionIntent::Answer {
                choice_index: 0,
                ..
            }
        ));
        assert!(view.pending());
    }

    #[test]
    fn error_card_uses_static_categories_and_validation_bounds() {
        let mut view = DialogView::new();
        assert_eq!(
            view.open_request(DialogRequest {
                id: "e-1".to_string(),
                session_id: "ses-1".to_string(),
                kind: DialogKind::Error,
                title: "Boom".to_string(),
                choices: Vec::new(),
                message: None,
                disabled: Vec::new(),
                error: None,
            }),
            Err(DialogError::MissingError)
        );
        view.open_request(DialogRequest {
            id: "e-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Error,
            title: "Boom".to_string(),
            choices: Vec::new(),
            message: None,
            disabled: Vec::new(),
            error: Some(StaticError::UnknownSideEffects),
        })
        .expect("valid error card");
        let lines = view.render(80, 40);
        assert!(lines.iter().any(|l| l.contains("unknown side effects")));
        assert!(lines
            .iter()
            .any(|l| l.contains("not retried automatically")));
        // Error cards are not answerable and not cancelable.
        assert_eq!(view.answer(), Err(DialogError::NotAnswerable));
        assert_eq!(view.cancel(), Err(DialogError::NotAnswerable));
        assert_eq!(
            StaticError::BackendUnavailable.code(),
            "backend_unavailable"
        );

        // Bounds.
        let mut bounded = DialogView::new();
        let too_many = DialogRequest {
            choices: vec!["x".to_string(); MAX_CHOICES + 1],
            ..permission_request()
        };
        assert_eq!(
            bounded.open_request(too_many),
            Err(DialogError::TooManyChoices)
        );
        let long_id = DialogRequest {
            id: "i".repeat(MAX_ID_LEN + 1),
            ..permission_request()
        };
        assert_eq!(bounded.open_request(long_id), Err(DialogError::IdTooLong));
        let long_title = DialogRequest {
            title: "t".repeat(MAX_TITLE_LEN + 1),
            ..permission_request()
        };
        assert_eq!(
            bounded.open_request(long_title),
            Err(DialogError::TitleTooLong)
        );
        let long_choice = DialogRequest {
            choices: vec!["c".repeat(MAX_CHOICE_LEN + 1)],
            ..permission_request()
        };
        assert_eq!(
            bounded.open_request(long_choice),
            Err(DialogError::ChoiceTooLong)
        );
        // Wildcard / always-allow choices are refused for permissions.
        for forbidden in ["*", "Always", " always-allow "] {
            let wildcard = DialogRequest {
                choices: vec!["once".to_string(), forbidden.to_string()],
                ..permission_request()
            };
            assert_eq!(
                bounded.open_request(wildcard),
                Err(DialogError::ForbiddenChoice),
                "expected {forbidden:?} to be refused"
            );
        }
        // A visible dialog is never silently replaced.
        let mut busy = open_permission();
        assert_eq!(
            busy.open_request(permission_request()),
            Err(DialogError::AlreadyOpen)
        );
        busy.close();
        assert!(!busy.is_open());
    }

    #[test]
    fn error_codes_are_static_and_free_of_request_content() {
        let err = DialogError::AckMismatch;
        assert_eq!(err.code(), "dialog.ack_mismatch");
        // Codes contain no request text (safe to log).
        for code in [
            DialogError::IdTooLong,
            DialogError::ForbiddenChoice,
            DialogError::QuestionUnavailable,
            DialogError::PendingDecision,
        ] {
            assert!(code.code().starts_with("dialog."));
        }
        assert_eq!(
            DialogError::UnknownPermissionChoice.code(),
            "dialog.unknown_permission_choice"
        );
    }

    // --- P1 regression: Permission choice identity is authoritative ---

    #[test]
    fn deny_selected_returns_reject_with_identity_and_pending() {
        let mut view = open_permission();
        view.next(); // 0: approve once
        view.next(); // 1: deny
        let intent = view.answer().expect("visible deny choice answers");
        // Explicit negative decision, bound to request/session identity
        // and to the first local attempt.
        assert_eq!(
            intent,
            DecisionIntent::Reject {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                attempt: 1,
            }
        );
        assert_ne!(
            intent,
            DecisionIntent::Approve {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                attempt: 1,
            }
        );
        assert!(view.pending());
        assert!(view.intercepts_input());
        // Mismatched ACK retains pending (focus held, no close, no retry);
        // fixture captures the attempt carried by the intent.
        let mismatched = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-WRONG".to_string(),
            accepted: true,
            attempt: intent.attempt(),
        };
        assert_eq!(view.ack(mismatched), Err(DialogError::AckMismatch));
        assert!(view.pending());
        assert!(view.is_open());
        assert_eq!(view.answer(), Err(DialogError::PendingDecision));
    }

    #[test]
    fn allow_once_selected_returns_approve_and_typed_helper_uses_fixed_labels() {
        let mut view = DialogView::new();
        view.open_permission("p1", "s1", "Run shell command")
            .expect("typed helper opens");
        // No selection: still no implicit approve.
        assert_eq!(view.answer(), Err(DialogError::NoSelection));
        view.next(); // fixed label 0: "approve once"
        let intent = view.answer().expect("allow-once answers");
        assert_eq!(
            intent,
            DecisionIntent::Approve {
                request_id: "p1".to_string(),
                session_id: "s1".to_string(),
                attempt: 1,
            }
        );
        assert!(view.pending());
        // Pending holds focus: close() is a no-op until the matching ack.
        view.close();
        assert!(view.is_open());
        assert!(view.pending());

        let mut deny_view = DialogView::new();
        deny_view
            .open_permission("p1", "s1", "Run shell command")
            .expect("typed helper opens");
        deny_view.next();
        deny_view.next(); // fixed label 1: "deny"
        assert!(matches!(
            deny_view.answer().expect("deny answers"),
            DecisionIntent::Reject { .. }
        ));

        // Generic permission labels also map both directions explicitly.
        let mut generic = DialogView::new();
        generic
            .open_request(DialogRequest {
                choices: vec!["allow".to_string(), "reject".to_string()],
                ..permission_request()
            })
            .expect("valid aliases");
        generic.next();
        assert!(matches!(
            generic.answer().unwrap(),
            DecisionIntent::Approve { .. }
        ));
    }

    #[test]
    fn unknown_permission_label_refused_side_effect_free() {
        let mut view = DialogView::new();
        // Substring "approve" inside a negative phrase must not authorize.
        for unknown in ["do not approve", "not deny", "approve?"] {
            let request = DialogRequest {
                choices: vec!["approve once".to_string(), unknown.to_string()],
                ..permission_request()
            };
            assert_eq!(
                view.open_request(request),
                Err(DialogError::UnknownPermissionChoice),
                "expected {unknown:?} to be refused"
            );
            // No state change: nothing opened, nothing pending, no intent.
            assert!(!view.is_open());
            assert!(!view.pending());
            assert_eq!(view.answer(), Err(DialogError::NotOpen));
            assert_eq!(view.cancel(), Err(DialogError::NotOpen));
        }
        // Refusal is side-effect free: a valid request still opens after.
        view.open_request(permission_request())
            .expect("valid after refusals");
        assert!(view.is_open());
    }

    #[test]
    fn disabled_deny_choice_is_inert() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            disabled: vec![1],
            ..permission_request()
        })
        .expect("valid");
        view.next();
        let grid = view.render(80, 40);
        assert!(grid.iter().any(|l| l == " > 1. approve once"));
        assert!(grid.iter().any(|l| l == "   2. deny (disabled)"));
        assert!(!grid.iter().any(|l| l == " > 2. deny (disabled)"));
        view.next(); // deny disabled: selection cannot land on it
        assert!(view
            .render(80, 40)
            .iter()
            .any(|l| l == " > 1. approve once"));
        assert!(matches!(
            view.answer().unwrap(),
            DecisionIntent::Approve { .. }
        ));

        // Approve disabled: only the explicit deny slot is reachable.
        let mut deny_only = DialogView::new();
        deny_only
            .open_request(DialogRequest {
                disabled: vec![0],
                ..permission_request()
            })
            .expect("valid");
        deny_only.next();
        assert!(matches!(
            deny_only.answer().unwrap(),
            DecisionIntent::Reject { .. }
        ));

        // Everything disabled: no valid selection, no answer at all.
        let mut none_enabled = DialogView::new();
        none_enabled
            .open_request(DialogRequest {
                disabled: vec![0, 1],
                ..permission_request()
            })
            .expect("valid");
        none_enabled.next();
        assert_eq!(none_enabled.answer(), Err(DialogError::NoSelection));
    }

    #[test]
    fn esc_on_permission_dialog_emits_reject_never_approve() {
        let mut view = open_permission();
        let intent = view.cancel().expect("Esc emits explicit intent");
        assert_eq!(
            intent,
            DecisionIntent::Reject {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                attempt: 1,
            }
        );
        assert!(view.pending());
        assert_eq!(view.cancel(), Err(DialogError::PendingDecision));
    }

    // --- V3 regressions: raw-label boundary + attempt correlation ---

    #[test]
    fn raw_control_bytes_in_permission_aliases_refused_side_effect_free() {
        let mut view = DialogView::new();
        // Each of these would display-sanitize into a valid approval or
        // denial alias; classification runs on the raw label instead, so
        // none can ever become grant-bearing.
        for malicious in [
            "approve\u{0}once",
            "approve\nonce",
            "approve\u{1b}once",
            "deny\u{0}",
            "\u{0}deny",
            "approve\r once",
        ] {
            let request = DialogRequest {
                choices: vec!["approve once".to_string(), malicious.to_string()],
                ..permission_request()
            };
            assert_eq!(
                view.open_request(request),
                Err(DialogError::UnknownPermissionChoice),
                "expected {malicious:?} to be refused"
            );
            // Refused before any state change.
            assert!(!view.is_open());
            assert!(!view.pending());
            assert_eq!(view.answer(), Err(DialogError::NotOpen));
            assert_eq!(view.cancel(), Err(DialogError::NotOpen));
            assert_eq!(
                view.ack(DialogAck {
                    request_id: "req-1".to_string(),
                    session_id: "ses-1".to_string(),
                    accepted: true,
                    attempt: 1,
                }),
                Err(DialogError::NotOpen)
            );
        }
        // Still usable afterwards: a clean request opens normally.
        view.open_request(permission_request())
            .expect("valid after refusals");
        assert!(view.is_open());
        assert!(!view.pending());
    }

    #[test]
    fn delayed_ack_from_rejected_attempt_cannot_close_next_attempt() {
        let mut view = open_permission();
        view.next();
        let first = view.answer().expect("attempt 1");
        let first_attempt = first.attempt();
        assert_eq!(first_attempt, 1);
        // Attempt 1 rejected: view reopens; counter stays advanced.
        let reject_first = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: false,
            attempt: first_attempt,
        };
        assert_eq!(view.ack(reject_first.clone()), Ok(AckOutcome::Rejected));
        // Replay of that old rejected ACK while still open: ignored.
        assert_eq!(
            view.ack(reject_first.clone()),
            Err(DialogError::NoPendingDecision)
        );
        assert!(view.is_open());
        // Attempt 2 goes pending.
        view.next();
        let second = view.answer().expect("attempt 2");
        let second_attempt = second.attempt();
        assert_eq!(second_attempt, 2);
        assert!(view.pending());
        // Delayed ACCEPTED ACK for attempt 1 (same request/session):
        // must not close the next attempt.
        let stale_accepted = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt: first_attempt,
        };
        assert_eq!(view.ack(stale_accepted), Err(DialogError::AckStaleAttempt));
        assert!(view.pending());
        assert!(view.is_open());
        // Delayed REJECTED ACK for attempt 1: likewise ignored.
        let stale_rejected = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: false,
            attempt: first_attempt,
        };
        assert_eq!(view.ack(stale_rejected), Err(DialogError::AckStaleAttempt));
        assert!(view.pending());
        // Only the exact matching attempt closes.
        let matching = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt: second_attempt,
        };
        assert_eq!(view.ack(matching), Ok(AckOutcome::Closed));
        assert!(!view.is_open());
    }

    #[test]
    fn attempt_counter_never_resets_across_close_and_new_open() {
        let mut view = open_permission();
        view.next();
        let first = view.answer().expect("attempt 1");
        assert_eq!(first.attempt(), 1);
        let ack = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: true,
            attempt: first.attempt(),
        };
        assert_eq!(view.ack(ack), Ok(AckOutcome::Closed));
        // Accepted close must not reset the counter.
        view.open_request(permission_request()).expect("reopen");
        view.next();
        let second = view.answer().expect("attempt 2");
        assert_eq!(second.attempt(), 2);
        let rejected = DialogAck {
            request_id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            accepted: false,
            attempt: second.attempt(),
        };
        assert_eq!(view.ack(rejected), Ok(AckOutcome::Rejected));
        // Rejected ACK + explicit close must not reset the counter.
        view.close();
        assert!(!view.is_open());
        view.open_request(permission_request()).expect("reopen");
        view.next();
        let third = view.answer().expect("attempt 3");
        assert_eq!(third.attempt(), 3);
    }

    #[test]
    fn attempt_overflow_fails_closed_before_pending() {
        let mut view = open_permission();
        view.next();
        view.attempt_counter = u64::MAX;
        let err = view.answer().expect_err("overflow fails closed");
        assert_eq!(err, DialogError::AttemptExhausted);
        assert_eq!(err.code(), "dialog.attempt_exhausted");
        // Failed BEFORE pending: still open for the user, not pending,
        // no intent issued, counter unchanged (no wrap).
        assert!(view.is_open());
        assert!(!view.pending());
        assert_eq!(view.attempt_counter, u64::MAX);
        // cancel() fails closed on overflow too.
        assert_eq!(view.cancel(), Err(DialogError::AttemptExhausted));
        assert!(!view.pending());
        assert_eq!(view.attempt_counter, u64::MAX);
        // Stale-code sanity for the new error classes.
        assert_eq!(
            DialogError::AckStaleAttempt.code(),
            "dialog.ack_stale_attempt"
        );
    }
}
