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

/// Measure the visible, unpadded terminal cells using the exact cluster
/// segmentation used by the native painter. This deliberately does not use
/// `visit_cells`, which first pads through `fit`; action hit regions must never
/// include padding after a wide cluster is clipped.
fn visible_text(text: &str, budget: usize) -> String {
    let mut visible = String::new();
    crate::native_view_layout::visit_unpadded_cells(text, budget, |glyph, _| {
        visible.push_str(glyph);
    });
    visible
}

fn visible_cells(text: &str, budget: usize) -> usize {
    crate::native_view_layout::visit_unpadded_cells(text, budget, |_, _| {})
}

/// Which upstream layout contract owns the list viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogViewportPolicy {
    /// Generic permission/confirm/error action dialogs use their supplied
    /// content budget. These are not `DialogSelect` surfaces upstream.
    ContentBudget,
    /// Shared DialogSelect lists derive their viewport from full terminal H.
    SelectTerminal,
}

/// Geometry shared by dialog paint and pointer hit testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogLayoutMetrics {
    pub width: usize,
    pub content_height: usize,
    pub terminal_height: usize,
    pub viewport: DialogViewportPolicy,
}

impl DialogLayoutMetrics {
    #[must_use]
    pub const fn legacy(width: usize, content_height: usize) -> Self {
        Self { width, content_height, terminal_height: content_height, viewport: DialogViewportPolicy::ContentBudget }
    }

    #[must_use]
    pub const fn new(
        width: usize,
        content_height: usize,
        terminal_height: usize,
        viewport: DialogViewportPolicy,
    ) -> Self {
        Self { width, content_height, terminal_height, viewport }
    }

    #[must_use]
    pub const fn select(width: usize, content_height: usize, terminal_height: usize) -> Self {
        Self::new(width, content_height, terminal_height, DialogViewportPolicy::SelectTerminal)
    }
}

impl DialogView {
    /// Current native dialogs are action overlays, not upstream DialogSelect
    /// lists. The production caller must not opt them into the select viewport
    /// merely because a full terminal height is available.
    #[must_use]
    pub fn production_viewport_policy(&self) -> DialogViewportPolicy {
        match self.dialog.as_ref().map(|dialog| dialog.kind) {
            Some(DialogKind::Permission)
            | Some(DialogKind::Question)
            | Some(DialogKind::Confirm)
            | Some(DialogKind::Error)
            | None => DialogViewportPolicy::ContentBudget,
        }
    }

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
        // The compatibility helper is retained for callers that do not have
        // request metadata.  The real permission poll uses the explicit
        // save-aware helper below; keeping this default enabled preserves the
        // long-standing typed-dialog contract for local/test callers.
        self.open_permission_with_persistent(request_id, session_id, title, true)
    }

    /// Open a permission prompt with the persistent-grant choice controlled
    /// by the backend request's validated `save` capability.  `false` must
    /// never remove either scoped deny choice or the session-scoped allow.
    pub fn open_permission_with_persistent(
        &mut self,
        request_id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        persistent_allowed: bool,
    ) -> Result<(), DialogError> {
        let mut choices = vec![
            "Allow once".to_owned(),
            "Deny once".to_owned(),
            "Allow for this session".to_owned(),
        ];
        if persistent_allowed {
            choices.push("Always allow".to_owned());
        }
        choices.extend([
            "Deny for this session".to_owned(),
            "Always deny".to_owned(),
        ]);
        self.open_request(DialogRequest {
            id: request_id.into(),
            session_id: session_id.into(),
            kind: DialogKind::Permission,
            title: title.into(),
            choices,
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

    /// Render with one geometry object shared by paint and hit testing.
    #[must_use]
    pub fn render_with_metrics(&self, metrics: DialogLayoutMetrics) -> Vec<String> {
        self.render_styled_with_metrics(metrics).into_iter().map(|line| line.text).collect()
    }

    /// Compatibility API: preserves the historical content-budget contract.
    #[must_use]
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        self.render_with_metrics(DialogLayoutMetrics::legacy(width, height))
    }

    #[must_use]
    pub fn render_styled_with_metrics(&self, metrics: DialogLayoutMetrics) -> Vec<crate::native_ui_style::Line> {
        self.layout(metrics).0
    }

    pub fn render_styled(&self, width: usize, height: usize) -> Vec<crate::native_ui_style::Line> {
        self.render_styled_with_metrics(DialogLayoutMetrics::legacy(width, height))
    }

    #[must_use]
    pub fn choice_at_row_with_metrics(&self, metrics: DialogLayoutMetrics, row: usize) -> Option<usize> {
        self.layout(metrics).1.get(row).copied().flatten()
    }

    pub fn choice_at_row(&self, width: usize, height: usize, row: usize) -> Option<usize> {
        self.choice_at_row_with_metrics(DialogLayoutMetrics::legacy(width, height), row)
    }

    /// Click-confirm affordance derived from the same layout that renders
    /// the dialog: true only for the actual rendered footer row. Never the
    /// blank padding below it, never a hidden/clipped footer (no footer
    /// line exists for errors), and never a pending decision.
    #[must_use]
    pub fn confirm_at_row_with_metrics(&self, metrics: DialogLayoutMetrics, row: usize) -> bool {
        self.layout(metrics).2 == Some(row)
    }

    #[must_use]
    pub fn confirm_at_row(&self, width: usize, height: usize, row: usize) -> bool {
        self.confirm_at_row_with_metrics(DialogLayoutMetrics::legacy(width, height), row)
    }

    /// Return the rendered terminal-cell span for a permission action on `row`.
    /// Both the prefix and label are measured by the native cell iterator used
    /// by the painter, so wide/combining clusters and cell clipping cannot move
    /// the pointer target away from the painted action.
    #[must_use]
    pub fn choice_region_at_row_with_metrics(&self, metrics: DialogLayoutMetrics, row: usize) -> Option<(usize, usize, usize)> {
        let (lines, hits, _) = self.layout(metrics);
        let index = hits.get(row).copied().flatten()?;
        let dialog = self.dialog.as_ref()?;
        let label = dialog.choices.get(index)?;
        lines.get(row)?;
        let marker = if self.selection == Some(index) { " > " } else { "   " };
        let prefix = format!("{marker}{}. ", index + 1);
        // Measure the same unpadded cluster stream as `fit`, with the same
        // terminal-cell budget. A wide cluster that does not fit is dropped,
        // leaving only actual painted source cells clickable.
        let start = visible_cells(&prefix, metrics.width);
        let label_width = visible_cells(label, metrics.width.saturating_sub(start));
        let end = start.saturating_add(label_width);
        (end > start).then_some((index, start, end - start))
    }

    #[must_use]
    pub fn choice_at_cell_with_metrics(&self, metrics: DialogLayoutMetrics, row: usize, col: usize) -> Option<usize> {
        let (lines, hits, _) = self.layout(metrics);
        let index = hits.get(row).copied().flatten()?;
        let (_, start, span) = self.choice_region_at_row_with_metrics(metrics, row)?;
        (col >= start && col < start.saturating_add(span)).then_some(index)
    }

    pub fn choice_region_at_row(&self, width: usize, height: usize, row: usize) -> Option<(usize, usize, usize)> {
        self.choice_region_at_row_with_metrics(DialogLayoutMetrics::legacy(width, height), row)
    }

    pub fn choice_at_cell(&self, width: usize, height: usize, row: usize, col: usize) -> Option<usize> {
        self.choice_at_cell_with_metrics(DialogLayoutMetrics::legacy(width, height), row, col)
    }

    #[must_use]
    pub fn confirm_at_cell_with_metrics(&self, metrics: DialogLayoutMetrics, row: usize, col: usize) -> bool {
        let (lines, _, footer) = self.layout(metrics);
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

    pub fn confirm_at_cell(&self, width: usize, height: usize, row: usize, col: usize) -> bool {
        self.confirm_at_cell_with_metrics(DialogLayoutMetrics::legacy(width, height), row, col)
    }

    fn layout(&self, metrics: DialogLayoutMetrics) -> (Vec<crate::native_ui_style::Line>, Vec<Option<usize>>, Option<usize>) {
        use crate::native_ui_style::{Line, Role};
        let width = metrics.width;
        let height = metrics.content_height;
        if height == 0 {
            return (Vec::new(), Vec::new(), None);
        }
        let mut hits = vec![None; height];
        let mut footer_row: Option<usize> = None;
        let mut lines = Vec::new();
        let line = |text, role| Line {text,role,detail_from:None};
        if let Some(dialog) = self.dialog.as_ref() {
            // Heading first: prioritized on short terminals.
            lines.push(line(dialog.title.clone(), if dialog.kind == DialogKind::Permission { Role::Warning } else { Role::Heading }));
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
            // Only DialogSelect owns the terminal-derived viewport. Generic
            // permission/confirm/error action dialogs retain their content
            // budget, including on tiny terminals.
            let viewport_cap = match metrics.viewport {
                DialogViewportPolicy::ContentBudget => usize::MAX,
                DialogViewportPolicy::SelectTerminal => {
                    metrics.terminal_height.saturating_div(2).saturating_sub(6)
                }
            };
            let capacity = height
                .saturating_sub(lines.len() + footer_rows + usize::from(roomy))
                .min(viewport_cap);
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
                    if self.selection == Some(index) { Role::Selected } else if dialog.disabled[index] { Role::Description } else if dialog.kind == DialogKind::Permission { Role::Action } else { Role::Item }));
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
        (lines.into_iter().map(|mut line| {
            line.text = visible_text(&line.text, width);
            line
        }).collect(), hits, footer_row)
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

/// Upstream question/form overlay port (P17, FORM-003..FORM-008).
///
/// Pure state + pure row rendering: no IO, no clock, no threads. Caller
/// supplies the form definition collected from the daemon, routes keys into
/// [`form_overlay::FormPrompt::handle_key`] while [`form_overlay::FormPrompt::active`]
/// is true, and dispatches the resolved answer map.
///
/// Upstream evidence, pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
/// * conditional `when` gating and the scoped answer map —
///   `packages/tui/src/routes/session/form.tsx:97-110`;
/// * single-field fast path, tab count, tab fit test at `width - 8`, 24-cell
///   label truncation and `+3` per tab — `form.tsx:111-122,795-847`;
/// * rows/selection/display value/toggle and multiselect custom helpers —
///   `packages/tui/src/util/form.ts:15-131`;
/// * validation messages and timing — `util/form.ts:72-110`,
///   `form.tsx:381-448,513-534`;
/// * default values and custom defaults — `util/form.ts:19-34`;
/// * key bindings (left/h, right/l, tab, 1-9, up/k, down/j, return, space
///   multiselect only, escape) — `form.tsx:642-765`;
/// * paste routing, burst typing interceptor, up leaving the editor only from
///   the first visual line — `form.tsx:243-255,345-379,607-617`;
/// * external acknowledgement requires a confirmed action, not open/copy —
///   `form.tsx:477-511`; review acknowledgement/progress and the
///   `max(3, height-14)` review viewport — `form.tsx:140,1063-1149`;
/// * draft stash/restore across remounts — `form.tsx:44-57,72-73,211-228`;
/// * keyboard scope: the form pushes its own mode and only consumes keys
///   while active — `form.tsx:66,536-553`.
pub mod form_overlay {
    use std::collections::BTreeMap;

    /// Form field kind (`FormField` union in `util/form.ts:11-61`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FieldKind {
        Text,
        Number,
        Integer,
        Boolean,
        Single,
        Multi,
        External,
    }

    /// String format constraints (`util/form.ts:89-92`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TextFormat {
        Email,
        Uri,
        Date,
        DateTime,
    }

    /// A submitted/edited value. `List` is only used by multiselect fields.
    #[derive(Clone, Debug, PartialEq)]
    pub enum FormValue {
        Text(String),
        Number(f64),
        Bool(bool),
        List(Vec<String>),
    }

    /// One selectable option (`FormRow` upstream).
    #[derive(Clone, Debug, PartialEq)]
    pub struct FormOption {
        pub value: String,
        pub label: String,
        pub description: Option<String>,
    }

    impl FormOption {
        #[must_use]
        pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
            Self { value: value.into(), label: label.into(), description: None }
        }
        #[must_use]
        pub fn described(value: impl Into<String>, label: impl Into<String>, description: impl Into<String>) -> Self {
            Self { value: value.into(), label: label.into(), description: Some(description.into()) }
        }
    }

    /// One `when` condition (`form.tsx:101-105`). `equal == false` is the
    /// upstream `ne` operator.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Condition {
        pub key: String,
        pub value: FormValue,
        pub equal: bool,
    }

    impl Condition {
        #[must_use]
        pub fn eq(key: impl Into<String>, value: FormValue) -> Self {
            Self { key: key.into(), value, equal: true }
        }
        #[must_use]
        pub fn ne(key: impl Into<String>, value: FormValue) -> Self {
            Self { key: key.into(), value, equal: false }
        }
    }

    /// One form field. `options` is empty for free text/number/boolean
    /// fields; `custom` enables the "type your own answer" row.
    #[derive(Clone, Debug, PartialEq)]
    pub struct FormField {
        pub key: String,
        pub title: Option<String>,
        pub kind: FieldKind,
        pub description: Option<String>,
        pub placeholder: Option<String>,
        pub required: bool,
        pub default: Option<FormValue>,
        pub options: Vec<FormOption>,
        pub custom: bool,
        pub format: Option<TextFormat>,
        pub min_length: Option<usize>,
        pub max_length: Option<usize>,
        pub minimum: Option<f64>,
        pub maximum: Option<f64>,
        pub min_items: Option<usize>,
        pub max_items: Option<usize>,
        pub when: Vec<Condition>,
        pub url: Option<String>,
    }

    impl FormField {
        #[must_use]
        pub fn new(key: impl Into<String>, kind: FieldKind) -> Self {
            Self {
                key: key.into(),
                title: None,
                kind,
                description: None,
                placeholder: None,
                required: false,
                default: None,
                options: Vec::new(),
                custom: false,
                format: None,
                min_length: None,
                max_length: None,
                minimum: None,
                maximum: None,
                min_items: None,
                max_items: None,
                when: Vec::new(),
                url: None,
            }
        }

        #[must_use]
        pub fn titled(mut self, title: impl Into<String>) -> Self {
            self.title = Some(title.into());
            self
        }
        #[must_use]
        pub fn required(mut self) -> Self {
            self.required = true;
            self
        }
        #[must_use]
        pub fn defaulted(mut self, value: FormValue) -> Self {
            self.default = Some(value);
            self
        }
        #[must_use]
        pub fn with_options(mut self, options: Vec<FormOption>) -> Self {
            self.options = options;
            self
        }
        #[must_use]
        pub fn custom(mut self) -> Self {
            self.custom = true;
            self
        }
        #[must_use]
        pub fn described(mut self, description: impl Into<String>) -> Self {
            self.description = Some(description.into());
            self
        }

        /// Visible label: `title ?? (external ? url : key)`
        /// (`util/form.ts:15-17`).
        #[must_use]
        pub fn label(&self) -> String {
            if let Some(title) = &self.title {
                return title.clone();
            }
            if self.kind == FieldKind::External {
                return self.url.clone().unwrap_or_default();
            }
            self.key.clone()
        }

        #[must_use]
        pub fn is_answer_field(&self) -> bool {
            self.kind != FieldKind::External
        }

        /// `formTextual`: number/integer, or string without options
        /// (`util/form.ts:36-39`).
        #[must_use]
        pub fn is_textual(&self) -> bool {
            matches!(self.kind, FieldKind::Number | FieldKind::Integer)
                || (self.kind == FieldKind::Text && self.options.is_empty())
        }

        /// `formCustom`: option-backed string/multiselect with `custom`
        /// (`util/form.ts:41-45`).
        #[must_use]
        pub fn is_custom(&self) -> bool {
            self.custom
                && (self.kind == FieldKind::Multi
                    || (self.kind == FieldKind::Text && !self.options.is_empty())
                    || self.kind == FieldKind::Single)
        }
    }

    /// Rendered row for a field (`FormRow`, `util/form.ts:5-9`).
    #[derive(Clone, Debug, PartialEq)]
    pub struct FormRow {
        pub value: FormValue,
        pub label: String,
        pub description: Option<String>,
    }

    /// `formRows` (`util/form.ts:47-61`): booleans are Yes/No, option fields
    /// map their options, everything else has no rows.
    #[must_use]
    pub fn form_rows(field: &FormField) -> Vec<FormRow> {
        match field.kind {
            FieldKind::Boolean => vec![
                FormRow { value: FormValue::Bool(true), label: "Yes".into(), description: None },
                FormRow { value: FormValue::Bool(false), label: "No".into(), description: None },
            ],
            FieldKind::Multi | FieldKind::Single | FieldKind::Text if !field.options.is_empty() => field
                .options
                .iter()
                .map(|option| FormRow {
                    value: FormValue::Text(option.value.clone()),
                    label: option.label.clone(),
                    description: option.description.clone(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// `formSelected` (`util/form.ts:63-70`): matching row index, the custom
    /// row for a non-option value of a custom field, else 0.
    #[must_use]
    pub fn form_selected(field: &FormField, value: Option<&FormValue>) -> usize {
        let Some(value) = value else {
            return 0;
        };
        if matches!(value, FormValue::List(_)) {
            return 0;
        }
        let rows = form_rows(field);
        if let Some(index) = rows.iter().position(|row| &row.value == value) {
            return index;
        }
        if matches!(value, FormValue::Text(_)) && field.is_custom() {
            return rows.len();
        }
        0
    }

    /// `formDisplayValue` (`util/form.ts:112-118`).
    #[must_use]
    pub fn form_display_value(field: &FormField, value: Option<&FormValue>, empty_multiselect: &str) -> String {
        let Some(value) = value else {
            return String::new();
        };
        let rows = form_rows(field);
        let label = |item: &FormValue| {
            rows.iter()
                .find(|row| &row.value == item)
                .map(|row| row.label.clone())
                .unwrap_or_else(|| match item {
                    FormValue::Text(text) => text.clone(),
                    FormValue::Number(number) => num(*number),
                    FormValue::Bool(flag) => flag.to_string(),
                    FormValue::List(items) => items.join(", "),
                })
        };
        match value {
            FormValue::List(items) if items.is_empty() => empty_multiselect.to_string(),
            FormValue::List(items) => items.iter().map(|item| label(&FormValue::Text(item.clone()))).collect::<Vec<_>>().join(", "),
            other => label(other),
        }
    }

    /// `formToggleMultiselect` (`util/form.ts:120-124`).
    #[must_use]
    pub fn toggle_multiselect(value: Option<&FormValue>, item: &str) -> FormValue {
        let mut values = match value {
            Some(FormValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        if let Some(index) = values.iter().position(|value| value == item) {
            values.remove(index);
        } else {
            values.push(item.to_string());
        }
        FormValue::List(values)
    }

    /// `formSetMultiselectCustom` (`util/form.ts:126-131`).
    #[must_use]
    pub fn set_multiselect_custom(value: Option<&FormValue>, previous: Option<&str>, next: &str) -> FormValue {
        let mut current = match value {
            Some(FormValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        if let Some(previous) = previous {
            if let Some(index) = current.iter().position(|item| item == previous) {
                current.remove(index);
            }
        }
        if !next.is_empty() && !current.iter().any(|item| item == next) {
            current.push(next.to_string());
        }
        FormValue::List(current)
    }

    fn num(value: f64) -> String {
        if value.fract() == 0.0 && value.abs() < 1e15 {
            format!("{}", value as i64)
        } else {
            value.to_string()
        }
    }

    fn valid_email(value: &str) -> bool {
        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 || value.chars().any(char::is_whitespace) {
            return false;
        }
        let (local, domain) = (parts[0], parts[1]);
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && domain.split('.').all(|label| !label.is_empty())
    }

    fn valid_url(value: &str) -> bool {
        let Some((scheme, rest)) = value.split_once(':') else {
            return false;
        };
        let mut chars = scheme.chars();
        matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && !rest.is_empty()
    }

    fn leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    fn days_in_month(year: i64, month: i64) -> i64 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap_year(year) => 29,
            2 => 28,
            _ => 0,
        }
    }

    fn parse_date(value: &str) -> Option<(i64, i64, i64)> {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let year = value[0..4].parse::<i64>().ok()?;
        let month = value[5..7].parse::<i64>().ok()?;
        let day = value[8..10].parse::<i64>().ok()?;
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some((year, month, day))
    }

    fn valid_date(value: &str) -> bool {
        parse_date(value).is_some()
    }

    fn valid_date_time(value: &str) -> bool {
        let Some((date, time)) = value.split_once(['T', 't']) else {
            return false;
        };
        if parse_date(date).is_none() {
            return false;
        }
        // HH:MM[:SS][.fff][Z|±HH:MM] — the subset every `Date` parse accepts.
        let time = time.trim_end_matches(['Z', 'z']);
        let time = match time.split_once('+').or_else(|| time.rsplit_once('-')) {
            Some((clock, offset)) if !offset.is_empty() => {
                let (hours, minutes) = offset.split_once(':').unwrap_or((offset, "0"));
                if hours.parse::<u32>().map(|h| h > 23).unwrap_or(true)
                    || minutes.parse::<u32>().map(|m| m > 59).unwrap_or(true)
                {
                    return false;
                }
                clock
            }
            _ => time,
        };
        let time = time.split_once('.').map(|(clock, _)| clock).unwrap_or(time);
        let clock: Vec<&str> = time.split(':').collect();
        if clock.len() < 2 || clock.len() > 3 {
            return false;
        }
        let hours = clock[0].parse::<u32>().ok();
        let minutes = clock[1].parse::<u32>().ok();
        let seconds = if clock.len() == 3 { clock[2].parse::<u32>().ok() } else { Some(0) };
        matches!((hours, minutes, seconds), (Some(h), Some(m), Some(s)) if h <= 23 && m <= 59 && s <= 59)
    }

    /// `formValidateValue` (`util/form.ts:72-110`). Returns the exact
    /// upstream message, or `None` when the value is acceptable. The regex
    /// `pattern` branch is not ported (no regex engine here) and is reported
    /// as unimplemented in the handoff — no pattern is silently accepted.
    #[must_use]
    pub fn validate(field: &FormField, value: Option<&FormValue>) -> Option<String> {
        let Some(value) = value else {
            return field.required.then(|| "Answer required".to_string());
        };
        if field.required {
            let empty = match value {
                FormValue::Text(text) => text.is_empty(),
                FormValue::List(items) => items.is_empty(),
                _ => false,
            };
            if empty {
                return Some(if field.kind == FieldKind::Multi {
                    "Select at least one option"
                } else {
                    "Answer required"
                }
                .to_string());
            }
        }
        match field.kind {
            FieldKind::Text | FieldKind::Single => {
                let FormValue::Text(text) = value else {
                    return Some("Expected text".to_string());
                };
                if let Some(min) = field.min_length {
                    if text.chars().count() < min {
                        return Some(format!("Must be at least {min} characters"));
                    }
                }
                if let Some(max) = field.max_length {
                    if text.chars().count() > max {
                        return Some(format!("Must be at most {max} characters"));
                    }
                }
                match field.format {
                    Some(TextFormat::Email) if !valid_email(text) => {
                        return Some("Expected an email address".to_string())
                    }
                    Some(TextFormat::Uri) if !valid_url(text) => return Some("Expected a URL".to_string()),
                    Some(TextFormat::Date) if !valid_date(text) => {
                        return Some("Expected a date (YYYY-MM-DD)".to_string())
                    }
                    Some(TextFormat::DateTime) if !valid_date_time(text) => {
                        return Some("Expected a date and time".to_string())
                    }
                    _ => {}
                }
                if !field.options.is_empty()
                    && !field.custom
                    && !field.options.iter().any(|option| option.value == *text)
                {
                    return Some("Select an available option".to_string());
                }
                None
            }
            FieldKind::Number | FieldKind::Integer => {
                let FormValue::Number(number) = value else {
                    return Some("Expected a number".to_string());
                };
                if !number.is_finite() {
                    return Some("Expected a number".to_string());
                }
                if field.kind == FieldKind::Integer && number.fract() != 0.0 {
                    return Some("Expected an integer".to_string());
                }
                if let Some(minimum) = field.minimum {
                    if *number < minimum {
                        return Some(format!("Must be at least {}", num(minimum)));
                    }
                }
                if let Some(maximum) = field.maximum {
                    if *number > maximum {
                        return Some(format!("Must be at most {}", num(maximum)));
                    }
                }
                None
            }
            FieldKind::Boolean => {
                if matches!(value, FormValue::Bool(_)) { None } else { Some("Expected yes or no".to_string()) }
            }
            FieldKind::Multi => {
                let FormValue::List(items) = value else {
                    return Some("Expected selections".to_string());
                };
                if let Some(min) = field.min_items {
                    if items.len() < min {
                        return Some(format!("Select at least {min}"));
                    }
                }
                if let Some(max) = field.max_items {
                    if items.len() > max {
                        return Some(format!("Select at most {max}"));
                    }
                }
                if !field.custom
                    && items.iter().any(|item| !field.options.iter().any(|option| option.value == *item))
                {
                    return Some("Select only available options".to_string());
                }
                None
            }
            FieldKind::External => None,
        }
    }

    /// `formInitialValues` (`util/form.ts:19-34`).
    #[must_use]
    pub fn initial_values(fields: &[FormField]) -> (BTreeMap<String, FormValue>, BTreeMap<String, String>) {
        let mut answers = BTreeMap::new();
        for field in fields {
            if field.is_answer_field() {
                if let Some(default) = &field.default {
                    answers.insert(field.key.clone(), default.clone());
                }
            }
        }
        let mut custom = BTreeMap::new();
        for field in fields {
            if field.kind != FieldKind::Text || field.options.is_empty() || !field.custom {
                continue;
            }
            let Some(FormValue::Text(default)) = field.default.as_ref() else {
                continue;
            };
            if field.options.iter().any(|option| option.value == *default) {
                continue;
            }
            custom.insert(field.key.clone(), default.clone());
        }
        (answers, custom)
    }

    /// Where keyboard focus must go when the overlay leaves.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FocusTarget {
        Overlay,
        Composer,
    }

    /// Why the form stopped being shown.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DismissReason {
        /// Session/terminal ended while the form was still pending.
        Terminal,
        /// The form was dropped upstream (stale location).
        Stale,
        /// The daemon rejected revalidation.
        RevalidationFailed,
        /// The user dismissed it (Esc).
        Cancelled,
        /// The user answered it.
        Resolved,
    }

    /// Consumed-on-take draft, mirroring `drafts` in `form.tsx:44-57,72-73`.
    #[derive(Clone, Debug, PartialEq)]
    pub struct FormDraft {
        pub tab: usize,
        pub answers: BTreeMap<String, FormValue>,
        pub custom: BTreeMap<String, String>,
        pub external_ready: BTreeMap<String, bool>,
        pub selected: usize,
        pub editing: bool,
        pub error: String,
    }

    /// Result of leaving the overlay.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Dismissal {
        pub focus: FocusTarget,
        pub draft: Option<FormDraft>,
    }

    /// Paste routing outcome (`form.tsx:354-379`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum PasteOutcome {
        InsertedInEditor,
        CustomRowOpened,
        Ignored,
    }

    /// Review row accent (`form.tsx:1075-1120`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ReviewState {
        Acknowledged,
        AcknowledgeRequired,
        Answered,
        Missing,
        Invalid(String),
        NotAnswered,
    }

    /// One review row.
    #[derive(Clone, Debug, PartialEq)]
    pub struct ReviewRow {
        pub label: String,
        pub display: String,
        pub state: ReviewState,
    }

    const REVIEW_LABEL_MAX: usize = 40;
    const TAB_LABEL_MAX: usize = 24;
    const EDITOR_MAX_HEIGHT: usize = 6;

    fn truncate(label: &str, max: usize) -> String {
        if label.chars().count() > max {
            let mut text: String = label.chars().take(max.saturating_sub(1)).collect();
            text.truncate(text.trim_end().len());
            text.push('…');
            text
        } else {
            label.to_string()
        }
    }

    fn condition_hit(value: &FormValue, target: &FormValue) -> bool {
        match value {
            FormValue::List(items) => {
                matches!(target, FormValue::Text(text) if items.iter().any(|item| item == text))
            }
            other => other == target,
        }
    }

    /// Question/form prompt state (`FormPrompt`, `form.tsx:59-1175`).
    #[derive(Clone, Debug)]
    pub struct FormPrompt {
        id: String,
        title: String,
        message: Option<String>,
        fields: Vec<FormField>,
        answers: BTreeMap<String, FormValue>,
        custom: BTreeMap<String, String>,
        external_ready: BTreeMap<String, bool>,
        tab: usize,
        selected: usize,
        editing: bool,
        error: String,
        cursor: usize,
        active: bool,
        resolution: Option<BTreeMap<String, FormValue>>,
    }

    impl FormPrompt {
        #[must_use]
        pub fn new(
            id: impl Into<String>,
            title: impl Into<String>,
            message: Option<String>,
            fields: Vec<FormField>,
        ) -> Self {
            let (answers, custom) = initial_values(&fields);
            let selected = fields
                .iter()
                .find(|field| field.is_answer_field())
                .map(|field| form_selected(field, answers.get(&field.key)))
                .unwrap_or(0);
            let mut prompt = Self {
                id: id.into(),
                title: title.into(),
                message,
                fields,
                answers,
                custom,
                external_ready: BTreeMap::new(),
                tab: 0,
                selected,
                editing: false,
                error: String::new(),
                cursor: 0,
                active: true,
                resolution: None,
            };
            // The editor mounts with the cursor at the end (`form.tsx:897-900`).
            prompt.cursor = prompt.editor_text().chars().count();
            prompt
        }

        /// Restore a consumed draft (`form.tsx:77-87`).
        #[must_use]
        pub fn restore(
            id: impl Into<String>,
            title: impl Into<String>,
            message: Option<String>,
            fields: Vec<FormField>,
            draft: FormDraft,
        ) -> Self {
            let mut prompt = Self {
                id: id.into(),
                title: title.into(),
                message,
                fields,
                answers: draft.answers,
                custom: draft.custom,
                external_ready: draft.external_ready,
                tab: draft.tab,
                selected: draft.selected,
                editing: draft.editing,
                error: draft.error,
                cursor: 0,
                active: true,
                resolution: None,
            };
            // Restored editor text (draft custom) mounts with the cursor at
            // the end (`form.tsx:897-900`).
            prompt.cursor = prompt.editor_text().chars().count();
            prompt
        }

        #[must_use]
        pub fn id(&self) -> &str {
            &self.id
        }

        /// `active = interactivity && keymap mode == form` (`form.tsx:66`).
        pub fn set_active(&mut self, active: bool) {
            self.active = active;
        }

        #[must_use]
        pub fn active(&self) -> bool {
            self.active
        }

        /// Focus owner while the prompt is mounted.
        #[must_use]
        pub fn focus_target(&self) -> FocusTarget {
            if self.active { FocusTarget::Overlay } else { FocusTarget::Composer }
        }

        #[must_use]
        pub fn tab(&self) -> usize {
            self.tab
        }

        #[must_use]
        pub fn selected(&self) -> usize {
            self.selected
        }

        #[must_use]
        pub fn editing(&self) -> bool {
            self.editing
        }

        #[must_use]
        pub fn error(&self) -> &str {
            &self.error
        }

        #[must_use]
        pub fn resolution(&self) -> Option<&BTreeMap<String, FormValue>> {
            self.resolution.as_ref()
        }

        #[must_use]
        pub fn answer(&self, key: &str) -> Option<&FormValue> {
            self.answers.get(key)
        }

        /// Fields passing their `when` conditions, in declaration order
        /// (`form.tsx:97-110`). External fields are always shown.
        #[must_use]
        pub fn visible_indices(&self) -> Vec<usize> {
            let mut scope: BTreeMap<&str, &FormValue> = BTreeMap::new();
            let mut visible = Vec::new();
            for (index, field) in self.fields.iter().enumerate() {
                if field.kind == FieldKind::External {
                    visible.push(index);
                    continue;
                }
                let active = field.when.iter().all(|condition| {
                    match scope.get(condition.key.as_str()) {
                        Some(value) => condition.equal == condition_hit(value, &condition.value),
                        None => false,
                    }
                });
                if active {
                    if let Some(value) = self.answers.get(&field.key) {
                        scope.insert(field.key.as_str(), value);
                    }
                    visible.push(index);
                }
            }
            visible
        }

        #[must_use]
        pub fn visible_fields(&self) -> Vec<&FormField> {
            self.visible_indices().into_iter().map(|index| &self.fields[index]).collect()
        }

        fn current_index(&self) -> Option<usize> {
            self.visible_indices().get(self.tab).copied()
        }

        #[must_use]
        pub fn answer_field(&self) -> Option<&FormField> {
            let field = &self.fields[self.current_index()?];
            field.is_answer_field().then_some(field)
        }

        #[must_use]
        pub fn external_field(&self) -> Option<&FormField> {
            let field = &self.fields[self.current_index()?];
            (field.kind == FieldKind::External).then_some(field)
        }

        /// One visible answerable field takes the single-field fast path
        /// (`form.tsx:111-117`).
        #[must_use]
        pub fn single(&self) -> bool {
            let visible = self.visible_indices();
            if visible.len() != 1 {
                return false;
            }
            let field = &self.fields[visible[0]];
            match field.kind {
                FieldKind::External => false,
                FieldKind::Boolean => true,
                FieldKind::Text | FieldKind::Single => !field.options.is_empty(),
                _ => false,
            }
        }

        /// `tabs = single ? 1 : fields + 1` (`form.tsx:118`).
        #[must_use]
        pub fn tabs(&self) -> usize {
            if self.single() { 1 } else { self.visible_indices().len() + 1 }
        }

        /// Tabs fit only when `sum(truncate(label, 24) + 3) <= width - 8`
        /// (`form.tsx:119-122`).
        #[must_use]
        pub fn tabbed(&self, width: usize) -> bool {
            let needed = self
                .visible_fields()
                .iter()
                .map(|field| truncate(&field.label(), TAB_LABEL_MAX).chars().count() + 3)
                .sum::<usize>()
                + "Submit".chars().count()
                + 3;
            needed <= width.saturating_sub(8)
        }

        /// The confirmation/review tab is active (`form.tsx:139`).
        #[must_use]
        pub fn confirm(&self) -> bool {
            !self.single() && self.tab >= self.visible_indices().len()
        }

        /// Configured rows plus any extra custom multiselect picks
        /// (`form.tsx:141-158`).
        #[must_use]
        pub fn rows(&self) -> Vec<FormRow> {
            let Some(field) = self.answer_field() else {
                return Vec::new();
            };
            let mut rows = form_rows(field);
            if field.kind != FieldKind::Multi {
                return rows;
            }
            let Some(FormValue::List(values)) = self.answers.get(&field.key) else {
                return rows;
            };
            let known: Vec<FormValue> = rows.iter().map(|row| row.value.clone()).collect();
            let custom_text = self.custom.get(&field.key).map(String::as_str).unwrap_or("");
            for value in values {
                let present = known.contains(&FormValue::Text(value.clone()));
                if !present && value != custom_text {
                    rows.push(FormRow {
                        value: FormValue::Text(value.clone()),
                        label: value.clone(),
                        description: None,
                    });
                }
            }
            rows
        }

        fn multi(&self) -> bool {
            self.answer_field().map(|field| field.kind == FieldKind::Multi).unwrap_or(false)
        }

        fn textual(&self) -> bool {
            if self.confirm() {
                return false;
            }
            self.answer_field().map(FormField::is_textual).unwrap_or(false)
        }

        fn custom_field(&self) -> bool {
            self.answer_field().map(FormField::is_custom).unwrap_or(false)
        }

        /// `other()` — the custom row is selected (`form.tsx:185`).
        #[must_use]
        pub fn other(&self) -> bool {
            self.custom_field() && self.selected == self.rows().len()
        }

        /// Current custom row text (`form.tsx:186`).
        #[must_use]
        pub fn input(&self) -> String {
            let key = self.answer_field().map(|field| field.key.as_str()).unwrap_or("");
            self.custom.get(key).cloned().unwrap_or_default()
        }

        fn custom_picked(&self) -> bool {
            let value = self.input();
            if value.is_empty() {
                return false;
            }
            let key = self.answer_field().map(|field| field.key.as_str()).unwrap_or("");
            match self.answers.get(key) {
                Some(FormValue::List(items)) => items.iter().any(|item| *item == value),
                Some(FormValue::Text(text)) => *text == value,
                _ => false,
            }
        }

        fn custom_checked(&self) -> bool {
            self.custom_picked() || (self.multi() && self.other() && self.editing)
        }

        /// Footer/action label (`form.tsx:195-209`).
        #[must_use]
        pub fn action_label(&self) -> &'static str {
            if self.confirm() {
                return "submit";
            }
            if let Some(external) = self.external_field() {
                if self.answers.get(&external.key) == Some(&FormValue::Bool(true)) {
                    return "continue";
                }
                return if self.external_ready.get(&external.key).copied().unwrap_or(false) {
                    "I finished"
                } else {
                    "open link"
                };
            }
            if self.multi() {
                if self.other() && self.editing {
                    return "done";
                }
                if self.other() && self.input().is_empty() {
                    return "edit";
                }
                return "toggle";
            }
            if self.single() { "submit" } else { "confirm" }
        }

        /// `completed(item)` (`form.tsx:123-128`).
        #[must_use]
        pub fn completed(&self, field: &FormField) -> bool {
            let Some(value) = self.answers.get(&field.key) else {
                return false;
            };
            if field.kind == FieldKind::External {
                return *value == FormValue::Bool(true);
            }
            validate(field, Some(value)).is_none()
        }

        /// `answered()` (`form.tsx:129`).
        #[must_use]
        pub fn answered_count(&self) -> usize {
            self.visible_fields().iter().filter(|field| self.completed(field)).count()
        }

        /// Store an answer, normalizing an empty optional multiselect away
        /// (`form.tsx:257-267`).
        fn store_answer(&mut self, key: &str, value: Option<FormValue>) {
            let optional_multi = self
                .fields
                .iter()
                .find(|field| field.key == key)
                .map(|field| field.kind == FieldKind::Multi && !field.required)
                .unwrap_or(false);
            let normalized = match value {
                Some(FormValue::List(items)) if items.is_empty() && optional_multi => None,
                Some(FormValue::List(items)) if items.is_empty() => {
                    // Still omit: an empty multiselect is no answer.
                    let _ = items;
                    None
                }
                other => other,
            };
            match normalized {
                Some(value) => {
                    self.answers.insert(key.to_string(), value);
                }
                None => {
                    self.answers.remove(key);
                }
            }
            self.error.clear();
        }

        /// Public answer write used by paste/typing tests.
        pub fn set_answer(&mut self, key: &str, value: Option<FormValue>) {
            self.store_answer(key, value);
        }

        /// `selectTab` (`form.tsx:314-320`).
        pub fn select_tab(&mut self, index: usize) {
            let visible = self.visible_indices();
            let index = if visible.is_empty() { 0 } else { index % (visible.len() + 1) };
            self.tab = index;
            self.selected = match visible.get(index) {
                Some(field_index) => {
                    let field = &self.fields[*field_index];
                    if field.is_answer_field() { form_selected(field, self.answers.get(&field.key)) } else { 0 }
                }
                None => 0,
            };
            self.editing = false;
            self.error.clear();
        }

        fn step_tab(&mut self, delta: isize) {
            let total = self.tabs();
            let next = (self.tab as isize + delta).rem_euclid(total as isize) as usize;
            self.select_tab(next);
        }

        fn step_selection(&mut self, delta: isize) {
            let total = self.rows().len() + usize::from(self.custom_field());
            if total == 0 {
                return;
            }
            self.selected = (self.selected as isize + delta).rem_euclid(total as isize) as usize;
        }

        /// Move to the custom row (`form.tsx:347-351,975-1012`).
        pub fn select_custom_row(&mut self) {
            if self.custom_field() {
                self.selected = self.rows().len();
            }
        }

        fn update_custom(&mut self, value: &str) {
            let Some(field) = self.answer_field().cloned() else {
                return;
            };
            let previous = self.custom.get(&field.key).cloned().unwrap_or_default();
            if previous == value {
                return;
            }
            self.custom.insert(field.key.clone(), value.to_string());
            let answer = if field.kind == FieldKind::Multi {
                Some(set_multiselect_custom(
                    self.answers.get(&field.key),
                    if previous.is_empty() { None } else { Some(previous.as_str()) },
                    value,
                ))
            } else if value.is_empty() {
                None
            } else {
                Some(FormValue::Text(value.to_string()))
            };
            self.store_answer(&field.key, answer);
        }

        fn toggle(&mut self, value: &str) {
            let Some(field) = self.answer_field().cloned() else {
                return;
            };
            let next = toggle_multiselect(self.answers.get(&field.key), value);
            self.store_answer(&field.key, Some(next));
        }

        /// Validate a picked option and advance (`form.tsx:279-294`).
        pub fn pick(&mut self, value: FormValue, custom_value: Option<String>) -> Option<BTreeMap<String, FormValue>> {
            let field = self.answer_field()?.clone();
            if let Some(invalid) = validate(&field, Some(&value)) {
                self.error = invalid;
                return None;
            }
            self.store_answer(&field.key, Some(value.clone()));
            if let Some(custom) = custom_value {
                self.custom.insert(field.key.clone(), custom);
            }
            if self.single() {
                let reply = BTreeMap::from([(field.key, value)]);
                self.resolution = Some(reply.clone());
                return Some(reply);
            }
            self.select_tab(self.tab + 1);
            None
        }

        /// Row/custom activation (`form.tsx:322-343`).
        pub fn select_option(&mut self) {
            if self.other() {
                if !self.multi() {
                    self.editing = true;
                    return;
                }
                let value = self.input();
                if !value.is_empty() && self.custom_picked() {
                    self.toggle(&value);
                    return;
                }
                self.editing = true;
                return;
            }
            let Some(row) = self.rows().get(self.selected).cloned() else {
                return;
            };
            if self.multi() {
                if let FormValue::Text(value) = &row.value {
                    self.toggle(value);
                }
                return;
            }
            let _ = self.pick(row.value, None);
        }

        /// Space toggles a multiselect row only (`form.tsx:753-755`); single
        /// choice and textual fields never consume it. Returns whether the
        /// key was consumed.
        pub fn space(&mut self) -> bool {
            if self.confirm() || self.editing || self.textual() || !self.multi() {
                return false;
            }
            self.select_option();
            true
        }

        /// Character interceptor for the custom row (`form.tsx:243-255`).
        /// Control/whitespace characters are ignored, and a selected custom
        /// row starts editing on the first accepted character.
        pub fn type_char(&mut self, character: char) -> bool {
            if character.is_control() || character.is_whitespace() || !self.other() || self.textual() || self.editing {
                return false;
            }
            let mut next = self.input();
            next.push(character);
            self.update_custom(&next);
            self.editing = true;
            self.cursor = next.chars().count();
            true
        }

        /// Insert text at the editor cursor (`form.tsx:356-358,366-377`).
        /// Textual answers live in the editor until committed and are only
        /// captured into `custom` (`form.tsx:211-228`), so typing must not
        /// write the answer map; custom rows update their answer live
        /// (`form.tsx:302-310`).
        pub fn editor_insert(&mut self, text: &str) {
            let mut chars: Vec<char> = self.editor_text().chars().collect();
            let cursor = self.cursor.min(chars.len());
            for (offset, character) in text.chars().enumerate() {
                chars.insert(cursor + offset, character);
            }
            self.cursor = cursor + text.chars().count();
            let joined: String = chars.into_iter().collect();
            if self.textual() {
                if let Some(field) = self.answer_field() {
                    self.custom.insert(field.key.clone(), joined);
                }
            } else {
                self.update_custom(&joined);
                self.editing = true;
            }
        }

        #[must_use]
        pub fn cursor(&self) -> usize {
            self.cursor
        }

        /// Position the editor cursor (char index, clamped on insert).
        pub fn set_cursor(&mut self, cursor: usize) {
            self.cursor = cursor;
        }

        /// Paste routing (`form.tsx:345-364`): an active editor inserts at
        /// the cursor; otherwise a custom row opens for editing; never a
        /// submit.
        pub fn paste(&mut self, text: &str) -> PasteOutcome {
            if !self.active {
                return PasteOutcome::Ignored;
            }
            if self.editing || self.textual() {
                self.editor_insert(text);
                return PasteOutcome::InsertedInEditor;
            }
            if !self.custom_field() || self.confirm() {
                return PasteOutcome::Ignored;
            }
            self.select_custom_row();
            let mut next = self.input();
            next.push_str(text);
            self.update_custom(&next);
            self.editing = true;
            self.cursor = next.chars().count();
            PasteOutcome::CustomRowOpened
        }

        /// Up leaves the editor only from the first visual line and with a
        /// row above it (`form.tsx:607-617`). `visual_row` is the editor's
        /// `scrollY + visualCursor.visualRow`.
        pub fn up_in_editor(&mut self, visual_row: usize) -> bool {
            if self.textual() || self.selected == 0 || visual_row > 0 {
                return false;
            }
            self.editing = false;
            self.selected -= 1;
            true
        }

        /// `commitInput` (`form.tsx:381-440`). Returns `false` (with the
        /// error set) when validation fails.
        pub fn commit_input(&mut self, text: &str) -> bool {
            let Some(field) = self.answer_field().cloned() else {
                return false;
            };
            let is_textual = field.is_textual();
            let is_multi = field.kind == FieldKind::Multi;
            if text.is_empty() {
                let value = if !is_textual && is_multi {
                    let previous = self.custom.get(&field.key).cloned().unwrap_or_default();
                    Some(set_multiselect_custom(
                        self.answers.get(&field.key),
                        if previous.is_empty() { None } else { Some(previous.as_str()) },
                        "",
                    ))
                } else {
                    None
                };
                if is_textual || !is_multi {
                    if let Some(invalid) = validate(&field, value.as_ref()) {
                        self.error = invalid;
                        return false;
                    }
                }
                self.store_answer(&field.key, value);
                self.custom.insert(field.key.clone(), String::new());
                self.editing = false;
                return true;
            }
            if is_textual && matches!(field.kind, FieldKind::Number | FieldKind::Integer) {
                let value = text.trim().parse::<f64>().unwrap_or(f64::NAN);
                if let Some(invalid) = validate(&field, Some(&FormValue::Number(value))) {
                    self.error = invalid;
                    return false;
                }
                self.store_answer(&field.key, Some(FormValue::Number(value)));
            }
            if is_textual && field.kind == FieldKind::Text {
                if let Some(invalid) = validate(&field, Some(&FormValue::Text(text.to_string()))) {
                    self.error = invalid;
                    return false;
                }
                self.store_answer(&field.key, Some(FormValue::Text(text.to_string())));
            }
            if !is_textual && is_multi {
                let previous = self.custom.get(&field.key).cloned().unwrap_or_default();
                let next = set_multiselect_custom(
                    self.answers.get(&field.key),
                    if previous.is_empty() { None } else { Some(previous.as_str()) },
                    text,
                );
                self.store_answer(&field.key, Some(next));
            }
            if !is_textual && !is_multi {
                if let Some(invalid) = validate(&field, Some(&FormValue::Text(text.to_string()))) {
                    self.error = invalid;
                    return false;
                }
                self.store_answer(&field.key, Some(FormValue::Text(text.to_string())));
            }
            let configured = field.options.iter().any(|option| option.value == text);
            self.custom.insert(field.key.clone(), if configured { String::new() } else { text.to_string() });
            self.editing = false;
            true
        }

        /// Tab/shift-tab while editing (`form.tsx:442-448`).
        pub fn submit_input(&mut self, text: &str, direction: isize) {
            if !self.commit_input(text) {
                if direction == -1 {
                    self.step_tab(-1);
                }
                return;
            }
            if !self.single() {
                self.step_tab(direction);
            }
        }

        /// Open/copy only mark the external action ready; they never
        /// acknowledge it (`form.tsx:477-496`).
        pub fn mark_external_opened(&mut self) {
            let Some(field) = self.external_field() else {
                return;
            };
            let key = field.key.clone();
            self.error.clear();
            self.external_ready.insert(key, true);
        }

        pub fn mark_external_copied(&mut self) {
            self.mark_external_opened();
        }

        #[must_use]
        pub fn external_ready(&self, key: &str) -> bool {
            self.external_ready.get(key).copied().unwrap_or(false)
        }

        /// Enter on an external field (`form.tsx:498-511`): the first
        /// activation opens the link, the second marks the action ready, and
        /// only the third records the acknowledgement.
        pub fn acknowledge_external(&mut self) {
            let Some(field) = self.external_field() else {
                return;
            };
            let key = field.key.clone();
            if self.answers.get(&key) == Some(&FormValue::Bool(true)) {
                self.select_tab(self.tab + 1);
                return;
            }
            if !self.external_ready(key.as_str()) {
                self.mark_external_opened();
                return;
            }
            self.store_answer(&key, Some(FormValue::Bool(true)));
            self.select_tab(self.tab + 1);
        }

        /// Final submission (`form.tsx:513-534`): every external action must
        /// be acknowledged and every visible answer must validate. Returns
        /// the answer map, omitting undefined answers.
        pub fn submit(&mut self) -> Option<BTreeMap<String, FormValue>> {
            let visible = self.visible_indices();
            let unacknowledged = visible.iter().find_map(|index| {
                let field = &self.fields[*index];
                (field.kind == FieldKind::External
                    && self.answers.get(&field.key) != Some(&FormValue::Bool(true)))
                .then(|| field.label())
            });
            if let Some(label) = unacknowledged {
                self.error = format!("External action must be acknowledged: {label}");
                return None;
            }
            let invalid = visible.iter().find_map(|index| {
                let field = &self.fields[*index];
                if field.is_answer_field() {
                    validate(field, self.answers.get(&field.key))
                } else {
                    None
                }
            });
            if let Some(invalid) = invalid {
                self.error = invalid;
                return None;
            }
            let mut answer = BTreeMap::new();
            for index in visible {
                let field = &self.fields[index];
                if let Some(value) = self.answers.get(&field.key) {
                    answer.insert(field.key.clone(), value.clone());
                }
            }
            self.resolution = Some(answer.clone());
            Some(answer)
        }

        /// Review rows (`form.tsx:1063-1123`).
        #[must_use]
        pub fn review_rows(&self) -> Vec<ReviewRow> {
            self.visible_fields()
                .into_iter()
                .map(|field| {
                    let label = truncate(&field.label(), REVIEW_LABEL_MAX);
                    if field.kind == FieldKind::External {
                        let acknowledged = self.answers.get(&field.key) == Some(&FormValue::Bool(true));
                        return ReviewRow {
                            label,
                            display: if acknowledged { "Acknowledged" } else { "(acknowledgement required)" }.to_string(),
                            state: if acknowledged { ReviewState::Acknowledged } else { ReviewState::AcknowledgeRequired },
                        };
                    }
                    let value = self.answers.get(&field.key);
                    let answered = value.is_some();
                    let missing = !answered && field.required;
                    if let Some(invalid) = validate(field, value) {
                        return ReviewRow { label, display: invalid.clone(), state: ReviewState::Invalid(invalid) };
                    }
                    let (display, state) = if answered {
                        (form_display_value(field, value, "(none)"), ReviewState::Answered)
                    } else if missing {
                        ("(required)".to_string(), ReviewState::Missing)
                    } else {
                        ("(not answered)".to_string(), ReviewState::NotAnswered)
                    };
                    ReviewRow { label, display, state }
                })
                .collect()
        }

        /// Review viewport cap: `max(3, height - 14)` (`form.tsx:140`).
        #[must_use]
        pub fn review_max_height(&self, terminal_height: usize) -> usize {
            terminal_height.saturating_sub(14).max(3)
        }

        /// Scroll hints appear only when the review actually overflows
        /// (`form.tsx:1145-1149`).
        #[must_use]
        pub fn review_scroll_available(&self, content_height: usize, terminal_height: usize) -> bool {
            content_height > self.review_max_height(terminal_height)
        }

        /// Number of editor rows: `minHeight 1`, `maxHeight 6`
        /// (`form.tsx:887-908,1026-1052`).
        #[must_use]
        pub fn editor_height(&self) -> usize {
            let text = self.editor_text();
            let lines = text.split('\n').count().max(1);
            lines.min(EDITOR_MAX_HEIGHT)
        }

        #[must_use]
        pub fn editor_text(&self) -> String {
            if self.textual() {
                let typed = self.input();
                if !typed.is_empty() {
                    return typed;
                }
                if let Some(field) = self.answer_field() {
                    return form_display_value(field, self.answers.get(&field.key), "");
                }
                return String::new();
            }
            self.input()
        }

        /// Leave the overlay. Upstream stashes a draft whenever the form is
        /// still listed after unmount (terminal/stale/revalidation failure)
        /// and restores composer focus (`form.tsx:211-228`).
        pub fn dismiss(&mut self, reason: DismissReason) -> Dismissal {
            self.active = false;
            let stash = matches!(
                reason,
                DismissReason::Terminal | DismissReason::Stale | DismissReason::RevalidationFailed
            );
            Dismissal {
                focus: FocusTarget::Composer,
                draft: stash.then(|| FormDraft {
                    tab: self.tab,
                    answers: self.answers.clone(),
                    custom: self.custom.clone(),
                    external_ready: self.external_ready.clone(),
                    selected: self.selected,
                    editing: self.editing,
                    error: self.error.clone(),
                }),
            }
        }

        /// Keyboard routing. Returns whether the key was consumed. Inactive
        /// prompts consume nothing, so transcript keys cannot leak into (or
        /// out of) the overlay (`form.tsx:66,243-255,536-553`).
        pub fn handle_key(&mut self, key: FormKey) -> bool {
            if !self.active {
                return false;
            }
            match key {
                FormKey::Char(character) => {
                    if self.editing || self.textual() {
                        if character.is_control() {
                            return true;
                        }
                        self.editor_insert(&character.to_string());
                        true
                    } else {
                        self.type_char(character)
                    }
                }
                FormKey::Space => self.space(),
                FormKey::Up { visual_row } => {
                    if self.editing || self.textual() {
                        self.up_in_editor(visual_row);
                        true
                    } else if self.confirm() {
                        true
                    } else {
                        self.step_selection(-1);
                        true
                    }
                }
                FormKey::Down => {
                    if !self.editing && !self.textual() && !self.confirm() {
                        self.step_selection(1);
                    }
                    true
                }
                FormKey::Left | FormKey::ShiftTab => {
                    if self.editing || self.textual() {
                        let text = self.editor_text();
                        self.submit_input(text.trim(), -1);
                    } else if !self.confirm() {
                        self.step_tab(-1);
                    }
                    true
                }
                FormKey::Right | FormKey::Tab => {
                    if self.editing || self.textual() {
                        let text = self.editor_text();
                        self.submit_input(text.trim(), 1);
                    } else if !self.confirm() {
                        self.step_tab(1);
                    }
                    true
                }
                FormKey::Enter => {
                    if self.confirm() {
                        let _ = self.submit();
                        return true;
                    }
                    if let Some(external) = self.external_field() {
                        let key = external.key.clone();
                        let _ = key;
                        self.acknowledge_external();
                        return true;
                    }
                    if self.editing || self.textual() {
                        let text = self.editor_text();
                        let trimmed = text.trim().to_string();
                        let multi = self.multi();
                        if !self.commit_input(&trimmed) {
                            return true;
                        }
                        if multi || trimmed.is_empty() {
                            return true;
                        }
                        if let Some(field) = self.answer_field().cloned() {
                            if self.single() {
                                let reply = BTreeMap::from([(field.key, FormValue::Text(trimmed))]);
                                self.resolution = Some(reply);
                            } else {
                                self.select_tab(self.tab + 1);
                            }
                        }
                        return true;
                    }
                    self.select_option();
                    true
                }
                FormKey::Escape => {
                    if self.confirm() {
                        let _ = self.dismiss(DismissReason::Cancelled);
                        return true;
                    }
                    if self.editing {
                        if self.textual() {
                            let _ = self.dismiss(DismissReason::Cancelled);
                        } else {
                            self.editing = false;
                        }
                        return true;
                    }
                    let _ = self.dismiss(DismissReason::Cancelled);
                    true
                }
                FormKey::CtrlC => {
                    if !self.textual() && !self.editing {
                        let _ = self.dismiss(DismissReason::Cancelled);
                        return true;
                    }
                    let text = self.editor_text();
                    if !text.is_empty() {
                        self.update_custom("");
                        self.cursor = 0;
                    } else if self.textual() {
                        let _ = self.dismiss(DismissReason::Cancelled);
                    } else {
                        self.editing = false;
                    }
                    true
                }
                FormKey::Digit(digit) => {
                    if digit == 0 || self.editing || self.textual() || self.confirm() {
                        return false;
                    }
                    let total = self.rows().len() + usize::from(self.custom_field());
                    let max = total.min(9);
                    let index = usize::from(digit) - 1;
                    if index >= max {
                        return false;
                    }
                    self.selected = index;
                    self.select_option();
                    true
                }
            }
        }

        /// Render the prompt rows (`form.tsx:767-1174`). Widths are counted
        /// in chars; the terminal painter performs the final cell pass.
        #[must_use]
        pub fn render(&self, width: usize, terminal_height: usize) -> Vec<String> {
            let clip = |text: &str| -> String { text.chars().take(width).collect() };
            let mut rows = Vec::new();
            rows.push(clip(&self.title));
            if let Some(message) = &self.message {
                rows.push(clip(message));
            }
            if !self.single() {
                if self.tabbed(width) {
                    let mut bar = String::new();
                    for field in self.visible_fields() {
                        bar.push_str(&truncate(&field.label(), TAB_LABEL_MAX));
                        bar.push_str("  ");
                    }
                    bar.push_str("Submit");
                    rows.push(clip(&bar));
                } else {
                    let visible = self.visible_indices().len();
                    let counter = if self.confirm() {
                        "Review".to_string()
                    } else {
                        format!("Field {} of {visible}", self.tab.min(visible.saturating_sub(1)) + 1)
                    };
                    let progress = if visible > 0 {
                        format!(" · {}/{visible} completed", self.answered_count())
                    } else {
                        String::new()
                    };
                    rows.push(clip(&format!("{counter}{progress}")));
                }
            }
            if !self.confirm() {
                if let Some(external) = self.external_field() {
                    if let Some(title) = &external.title {
                        rows.push(clip(title));
                    }
                    if let Some(description) = &external.description {
                        rows.push(clip(description));
                    }
                    if let Some(url) = &external.url {
                        rows.push(clip(url));
                    }
                    rows.push(clip(if self.answers.get(&external.key) == Some(&FormValue::Bool(true)) {
                        "✓ Acknowledged"
                    } else if self.external_ready(&external.key) {
                        "Complete the external action, then press enter to confirm."
                    } else {
                        "Open or copy the URL, complete the external action, then confirm."
                    }));
                }
                if let Some(field) = self.answer_field() {
                    rows.push(clip(field.description.as_deref().unwrap_or(&field.label())));
                    if self.textual() {
                        let text = self.editor_text();
                        let lines: Vec<&str> = text.split('\n').collect();
                        if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
                            rows.push(String::new());
                        } else {
                            for line in lines.iter().take(EDITOR_MAX_HEIGHT) {
                                rows.push(clip(line));
                            }
                        }
                    } else {
                        let multi = self.multi();
                        for (index, row) in self.rows().iter().enumerate() {
                            let picked = match self.answers.get(&field.key) {
                                Some(FormValue::List(items)) => match &row.value {
                                    FormValue::Text(value) => items.iter().any(|item| item == value),
                                    other => items.iter().any(|item| FormValue::Text(item.clone()) == *other),
                                },
                                Some(value) => *value == row.value,
                                None => false,
                            };
                            let prefix = format!("{}. ", index + 1);
                            if multi {
                                rows.push(clip(&format!("{prefix}[{}] {}", if picked { "✓" } else { " " }, row.label)));
                            } else {
                                rows.push(clip(&format!("{prefix}{}{}", row.label, if picked { " ✓" } else { "" })));
                            }
                            if let Some(description) = &row.description {
                                rows.push(clip(&format!("{}{}", " ".repeat(if multi { 7 } else { 3 }), description)));
                            }
                        }
                        if self.custom_field() {
                            let prefix = format!("{}. ", self.rows().len() + 1);
                            if multi {
                                let checked = self.custom_checked();
                                let body = if self.editing || !self.input().is_empty() {
                                    self.input()
                                } else {
                                    "Type your own answer".to_string()
                                };
                                rows.push(clip(&format!("{prefix}[{}] {body}", if checked { "✓" } else { " " })));
                            } else {
                                let body = if self.editing || !self.input().is_empty() {
                                    self.input()
                                } else {
                                    "Type your own answer".to_string()
                                };
                                rows.push(clip(&format!("{prefix}{body}{}", if self.custom_picked() { "✓" } else { "" })));
                            }
                        }
                    }
                }
            }
            if self.confirm() {
                for row in self.review_rows().into_iter().take(self.review_max_height(terminal_height)) {
                    rows.push(clip(&format!("{}: {}", row.label, row.display)));
                }
            }
            let mut footer: Vec<String> = Vec::new();
            if !self.single() {
                footer.push("⇆ tab".to_string());
            }
            if !self.confirm() && !self.textual() && self.external_field().is_none() && !self.editing {
                footer.push("↑↓ select".to_string());
            }
            footer.push(format!("enter {}", self.action_label()));
            if self.external_field().is_some() {
                footer.push("c copy".to_string());
            }
            footer.push(if self.editing && !self.textual() { "esc close" } else { "esc dismiss" }.to_string());
            rows.push(clip(&footer.join(" ")));
            rows
        }
    }

    /// Keys routed into [`FormPrompt::handle_key`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FormKey {
        Char(char),
        Space,
        Up { visual_row: usize },
        Down,
        Left,
        Right,
        Tab,
        ShiftTab,
        Enter,
        Escape,
        CtrlC,
        Digit(u8),
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
    fn save_ineligible_prompt_keeps_only_scoped_and_deny_choices() {
        let mut dialog = DialogView::new();
        dialog.open_permission_with_persistent("request", "session", "permission", false).unwrap();
        let rows = dialog.render(80, 24);
        assert!(rows.iter().any(|row| row.contains("Allow once")));
        assert!(rows.iter().any(|row| row.contains("Allow for this session")));
        assert!(rows.iter().any(|row| row.contains("Deny for this session")));
        assert!(rows.iter().any(|row| row.contains("Always deny")));
        assert!(!rows.iter().any(|row| row.contains("Always allow")));
        assert!(dialog.select(3));
        assert!(matches!(dialog.answer().unwrap(), DecisionIntent::ScopedPermission {
            allow: false, persistent: false, ..
        }));
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
    fn generic_unicode_action_regions_use_terminal_cells_and_pending_is_inert() {
        let mut dialog = DialogView::new();
        dialog.open_request(DialogRequest {
            id: "r".into(), session_id: "s".into(), kind: DialogKind::Confirm,
            title: "Confirm".into(),
            choices: vec!["界 e\u{301}".into(), "last".into()], message: None,
            disabled: Vec::new(), error: None,
        }).unwrap();
        let width = 22;
        let height = 12;
        let row = (0..height).find(|&r| dialog.choice_at_row(width, height, r) == Some(0)).unwrap();
        let (_, start, span) = dialog.choice_region_at_row(width, height, row).unwrap();
        assert_eq!(dialog.choice_at_cell(width, height, row, start), Some(0));
        assert_eq!(dialog.choice_at_cell(width, height, row, start + span), None);
        assert_eq!(dialog.choice_at_cell(width, height, row, start.saturating_sub(1)), None);
        assert!(span >= 3, "wide + combining label must occupy terminal cells");
        assert!(dialog.select(0));
        let intent = dialog.answer().unwrap();
        assert!(matches!(intent, DecisionIntent::Answer { choice_index: 0, .. }));
        assert!((0..width).all(|col| dialog.choice_at_cell(width, height, row, col).is_none()));
        assert!((0..height).all(|r| dialog.choice_region_at_row(width, height, r).is_none()));
    }

    #[test]
    fn arbitrary_unicode_permission_label_is_explicitly_rejected() {
        let mut dialog = DialogView::new();
        let result = dialog.open_request(DialogRequest {
            id: "r".into(), session_id: "s".into(), kind: DialogKind::Permission,
            title: "Permission".into(),
            choices: vec!["界 e\u{301}".into(), "Deny once".into()], message: None,
            disabled: Vec::new(), error: None,
        });
        assert_eq!(result, Err(DialogError::UnknownPermissionChoice));
        assert!(!dialog.is_open());
        assert_eq!(dialog.answer(), Err(DialogError::NotOpen));
    }


    #[test]
    fn permission_action_first_and_last_labels_clip_without_outside_hits() {
        let mut dialog = DialogView::new();
        dialog.open_permission("r", "s", "Permission required").unwrap();
        for width in [1, 5, 12, 80] {
            for height in [2, 8, 24] {
                for row in 0..height {
                    if let Some((index, start, span)) = dialog.choice_region_at_row(width, height, row) {
                        assert!(start < width && span > 0);
                        assert!(dialog.choice_at_cell(width, height, row, start).is_some());
                        assert_eq!(dialog.choice_at_cell(width, height, row, start + span), None);
                        assert!(index < 6);
                    }
                }
            }
        }
        let mut generic = DialogView::new();
        generic.open_request(DialogRequest {
            id: "wide".into(), session_id: "s".into(), kind: DialogKind::Confirm,
            title: "Confirm".into(), choices: vec!["界".into()], message: None,
            disabled: Vec::new(), error: None,
        }).unwrap();
        // Prefix consumes three cells; the two-cell glyph cannot fit in the
        // one-cell remainder at width four, so no padding cell is clickable.
        let row = (0..8).find(|&r| generic.choice_at_row(4, 8, r) == Some(0)).unwrap();
        assert!(generic.choice_region_at_row(4, 8, row).is_none());
        assert!((0..4).all(|col| generic.choice_at_cell(4, 8, row, col).is_none()));
    }

    #[test]
    fn select_policy_uses_full_terminal_height_only_for_select_surface() {
        let mut dialog = DialogView::new();
        dialog.open_request(DialogRequest {
            id: "r".into(), session_id: "s".into(), kind: DialogKind::Confirm,
            title: "Choose".into(), choices: (0..20).map(|i| format!("choice-{i}")).collect(),
            message: None, disabled: Vec::new(), error: None,
        }).unwrap();
        assert_eq!(dialog.production_viewport_policy(), DialogViewportPolicy::ContentBudget);
        let metrics = DialogLayoutMetrics::select(60, 13, 24);
        assert!(dialog.select(19));
        let rows = dialog.render_with_metrics(metrics);
        let visible = rows.iter().filter(|row| row.contains("choice-")).count();
        assert_eq!(visible, 6);
        assert!(rows.iter().any(|row| row.contains("choice-19")));
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

#[cfg(test)]
mod form_tests {
    use super::form_overlay::*;

    fn option(value: &str, label: &str) -> FormOption {
        FormOption::new(value, label)
    }

    fn text_default(field: FormField, text: &str) -> FormField {
        field.defaulted(FormValue::Text(text.to_string()))
    }

    fn kinds_form() -> Vec<FormField> {
        vec![
            text_default(
                FormField { min_length: Some(3), required: true, ..FormField::new("name", FieldKind::Text).titled("Name") },
                "Ada",
            ),
            FormField { minimum: Some(18.0), maximum: Some(99.0), required: true, ..FormField::new("age", FieldKind::Number).titled("Age") },
            FormField::new("count", FieldKind::Integer).titled("Count"),
            FormField::new("agree", FieldKind::Boolean).titled("Agree"),
            FormField::new("region", FieldKind::Single)
                .titled("Region")
                .required()
                .with_options(vec![option("eu", "Europe"), option("us", "United States")]),
            FormField::new("topics", FieldKind::Multi)
                .titled("Topics")
                .with_options(vec![option("a", "Alpha"), option("b", "Beta")])
                .custom(),
            FormField { url: Some("https://example.com/approve".into()), description: Some("Approve in the browser".into()), ..FormField::new("site", FieldKind::External).titled("Site") },
            FormField { when: vec![Condition::eq("agree", FormValue::Bool(true))], ..FormField::new("why", FieldKind::Text).titled("Why") },
        ]
    }

    // FORM-003 — every field kind, validation, review and draft restoration.
    // Upstream: util/form.ts:19-110 (initial values, rows, validation),
    // form.tsx:97-110 (conditional gating), form.tsx:1063-1123 (review),
    // form.tsx:44-57,72-73,211-228 (draft stash/restore).
    #[test]
    fn form_003_field_kinds_validation_review_and_draft_restoration() {
        let mut form = FormPrompt::new("f-3", "Details", None, kinds_form());
        // Defaults seed answers; conditional field is hidden until `agree`.
        assert_eq!(form.answer("name"), Some(&FormValue::Text("Ada".into())));
        assert!(form.visible_fields().iter().all(|field| field.key != "why"));
        form.set_answer("agree", Some(FormValue::Bool(true)));
        assert!(form.visible_fields().iter().any(|field| field.key == "why"));
        assert_eq!(form.visible_fields().last().unwrap().key, "why");

        // Validation messages are the exact upstream strings.
        let fields = kinds_form();
        let number = fields.iter().find(|field| field.key == "age").unwrap();
        assert_eq!(validate(number, Some(&FormValue::Number(17.0))).as_deref(), Some("Must be at least 18"));
        assert_eq!(validate(number, Some(&FormValue::Number(100.0))).as_deref(), Some("Must be at most 99"));
        assert_eq!(validate(number, None).as_deref(), Some("Answer required"));
        let integer = fields.iter().find(|field| field.key == "count").unwrap();
        assert_eq!(validate(integer, Some(&FormValue::Number(1.5))).as_deref(), Some("Expected an integer"));
        assert_eq!(validate(integer, Some(&FormValue::Text("1".into()))).as_deref(), Some("Expected a number"));
        let boolean = fields.iter().find(|field| field.key == "agree").unwrap();
        assert_eq!(validate(boolean, Some(&FormValue::Text("yes".into()))).as_deref(), Some("Expected yes or no"));
        assert_eq!(validate(boolean, Some(&FormValue::Bool(true))), None);
        let single = fields.iter().find(|field| field.key == "region").unwrap();
        assert_eq!(validate(single, Some(&FormValue::Text("mars".into()))).as_deref(), Some("Select an available option"));
        assert_eq!(validate(single, Some(&FormValue::Text("eu".into()))), None);
        let multi = fields.iter().find(|field| field.key == "topics").unwrap();
        assert_eq!(validate(multi, Some(&FormValue::List(vec![]))).as_deref(), None);
        let strict_multi = FormField::new("strict", FieldKind::Multi).required().with_options(vec![option("a", "A")]);
        assert_eq!(validate(&strict_multi, Some(&FormValue::List(vec![]))).as_deref(), Some("Select at least one option"));
        assert_eq!(
            validate(&strict_multi, Some(&FormValue::List(vec!["zzz".into()]))).as_deref(),
            Some("Select only available options")
        );
        let long = FormField { min_length: Some(3), ..FormField::new("long", FieldKind::Text) };
        assert_eq!(validate(&long, Some(&FormValue::Text("ab".into()))).as_deref(), Some("Must be at least 3 characters"));
        let email = FormField { format: Some(TextFormat::Email), ..FormField::new("mail", FieldKind::Text) };
        assert_eq!(validate(&email, Some(&FormValue::Text("nope".into()))).as_deref(), Some("Expected an email address"));
        assert_eq!(validate(&email, Some(&FormValue::Text("a@b.co".into()))), None);
        let date = FormField { format: Some(TextFormat::Date), ..FormField::new("date", FieldKind::Text) };
        assert_eq!(validate(&date, Some(&FormValue::Text("2023-02-30".into()))).as_deref(), Some("Expected a date (YYYY-MM-DD)"));
        assert_eq!(validate(&date, Some(&FormValue::Text("2024-02-29".into()))), None);

        // Review rows report answered/missing/required states.
        form.set_answer("age", Some(FormValue::Number(30.0)));
        form.set_answer("region", Some(FormValue::Text("eu".into())));
        let review = form.review_rows();
        let name = review.iter().find(|row| row.label == "Name").unwrap();
        assert_eq!(name.display, "Ada");
        assert_eq!(name.state, ReviewState::Answered);
        let region = review.iter().find(|row| row.label == "Region").unwrap();
        assert_eq!(region.display, "Europe");
        let site = review.iter().find(|row| row.label == "Site").unwrap();
        assert_eq!(site.display, "(acknowledgement required)");
        assert_eq!(site.state, ReviewState::AcknowledgeRequired);
        let why = review.iter().find(|row| row.label == "Why").unwrap();
        assert_eq!(why.display, "(not answered)");
        assert_eq!(why.state, ReviewState::NotAnswered);

        // Draft stash/restore keeps the tab, answers, custom text and error.
        form.select_tab(2);
        form.commit_input("7");
        assert_eq!(form.answer("count"), Some(&FormValue::Number(7.0)));
        form.set_answer("topics", Some(FormValue::List(vec!["custom".into()])));
        let dismissal = form.dismiss(DismissReason::Stale);
        assert_eq!(dismissal.focus, FocusTarget::Composer);
        let draft = dismissal.draft.expect("stale form keeps a draft");
        assert_eq!(draft.tab, 2);
        let restored = FormPrompt::restore("f-3", "Details", None, kinds_form(), draft);
        assert_eq!(restored.tab(), 2);
        assert_eq!(restored.answer("count"), Some(&FormValue::Number(7.0)));
        assert_eq!(restored.answer("topics"), Some(&FormValue::List(vec!["custom".into()])));
        assert!(restored.visible_fields().iter().any(|field| field.key == "why"));
    }

    // FORM-004 — narrow fallback at the exact fit boundary, 24-cell labels,
    // checkbox width 4, description insets 3/7, textarea height 1..6.
    // Upstream: form.tsx:119-122 (fit), form.tsx:40-42,817-820 (24-cell
    // labels), form.tsx:943-957 (checkbox width 4), form.tsx:966-970
    // (insets), form.tsx:902-904,1042-1044 (1..6 editor height).
    #[test]
    fn form_004_narrow_tab_fallback_checkbox_insets_and_editor_height() {
        let long = FormField::new("a", FieldKind::Text).titled("A".repeat(30));
        let long_b = FormField::new("b", FieldKind::Text).titled("B".repeat(30));
        let form = FormPrompt::new("f-4", "Form", None, vec![long, long_b]);
        // 24-cell truncation: 23 chars + ellipsis.
        let expected = format!("{}…", "A".repeat(23));
        assert_eq!(expected.chars().count(), 24);
        // Needed = (24 + 3) * 2 + (6 + 3) = 63 <= width - 8.
        assert!(form.tabbed(71));
        assert!(!form.tabbed(70));
        let wide = form.render(71, 30);
        assert!(wide[1].starts_with(&expected), "{:?}", wide[1]);
        assert!(wide[1].contains("Submit"));
        let narrow = form.render(70, 30);
        assert!(narrow[1].starts_with("Field 1 of 2"), "{:?}", narrow[1]);
        assert!(!narrow[1].contains("Submit"));

        // Checkbox width 4 and label column 7 for multiselect; single has no
        // checkbox so its label starts at column 3.
        let multi = FormPrompt::new(
            "f-4m",
            "Form",
            None,
            vec![FormField::new("topics", FieldKind::Multi)
                .titled("Topics")
                .with_options(vec![FormOption::described("a", "Alpha", "first")])
                .custom()],
        );
        let rows = multi.render(80, 30);
        let alpha = rows.iter().find(|row| row.starts_with("1. ")).expect("option row");
        assert!(alpha.starts_with("1. [ ] Alpha"), "{alpha:?}");
        assert_eq!(alpha.find("Alpha"), Some(7));
        let description = rows.iter().find(|row| row.contains("first")).expect("description row");
        assert!(description.starts_with("       first"), "{description:?}");
        let single = FormPrompt::new(
            "f-4s",
            "Form",
            None,
            vec![FormField::new("region", FieldKind::Single)
                .titled("Region")
                .with_options(vec![FormOption::described("eu", "Europe", "old world")])],
        );
        let rows = single.render(80, 30);
        let europe = rows.iter().find(|row| row.starts_with("1. ")).expect("option row");
        assert!(europe.starts_with("1. Europe"), "{europe:?}");
        assert_eq!(europe.find("Europe"), Some(3));
        let description = rows.iter().find(|row| row.contains("old world")).expect("description row");
        assert!(description.starts_with("   old world"), "{description:?}");

        // Editor height is clamped to 1..=6.
        let empty = FormPrompt::new("f-4e", "Form", None, vec![FormField::new("t", FieldKind::Text)]);
        assert_eq!(empty.editor_height(), 1);
        let three = FormPrompt::new(
            "f-4t",
            "Form",
            None,
            vec![text_default(FormField::new("t", FieldKind::Text), "1\n2\n3")],
        );
        assert_eq!(three.editor_height(), 3);
        let ten = FormPrompt::new(
            "f-4h",
            "Form",
            None,
            vec![text_default(FormField::new("t", FieldKind::Text), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10")],
        );
        assert_eq!(ten.editor_height(), 6);
    }

    // FORM-005 — paste into the custom row opens editing without submit,
    // paste into the editor inserts at the cursor, burst typing is kept, and
    // up leaves the editor only from the first visual line.
    // Upstream: form.tsx:345-379 (paste), form.tsx:243-255 (burst typing),
    // form.tsx:607-617 (up), form.tsx:322-343 (custom row activation).
    #[test]
    fn form_005_paste_custom_row_cursor_burst_typing_and_up_scope() {
        let field = FormField::new("region", FieldKind::Single)
            .titled("Region")
            .with_options(vec![option("eu", "Europe")])
            .custom();
        let mut form = FormPrompt::new("f-5", "Form", None, vec![field]);
        form.select_custom_row();
        assert_eq!(form.selected(), 1, "custom row follows the configured row");
        assert_eq!(form.paste("berlin"), PasteOutcome::CustomRowOpened);
        assert!(form.editing());
        assert_eq!(form.input(), "berlin");
        assert_eq!(form.answer("region"), Some(&FormValue::Text("berlin".into())));
        assert_eq!(form.resolution(), None, "paste never submits");
        assert_eq!(form.error(), "");

        // Paste into the active editor inserts at the cursor, not at the end.
        form.set_cursor(3);
        assert_eq!(form.paste("XY"), PasteOutcome::InsertedInEditor);
        assert_eq!(form.input(), "berXYlin");
        assert_eq!(form.cursor(), 5);

        // Burst typing is routed character by character without loss. Textual
        // answers stay in the editor until committed (`form.tsx:211-228`).
        let mut typed = FormPrompt::new("f-5b", "Form", None, vec![FormField::new("q", FieldKind::Text)]);
        for character in "burst".chars() {
            assert!(typed.handle_key(FormKey::Char(character)));
        }
        assert_eq!(typed.editor_text(), "burst");
        assert_eq!(typed.answer("q"), None, "typing does not commit the answer");
        assert!(typed.handle_key(FormKey::Tab));
        assert_eq!(typed.answer("q"), Some(&FormValue::Text("burst".into())));

        // Up only leaves the editor from the first visual line.
        assert!(form.editing());
        assert!(!form.up_in_editor(2), "wrapped line keeps the editor");
        assert!(form.editing());
        assert!(!form.up_in_editor(1));
        assert!(form.editing());
        assert!(form.up_in_editor(0));
        assert!(!form.editing());
        assert_eq!(form.selected(), 0);
    }

    // FORM-006 — space toggles multiselect only, an optional empty
    // multiselect omits its answer, and validation waits for the submit
    // point.
    // Upstream: form.tsx:753-755 (space binding), form.tsx:257-267 (empty
    // optional multiselect), util/form.ts:72-75, form.tsx:381-448 (commit
    // validation), form.tsx:513-525 (submit validation).
    #[test]
    fn form_006_space_toggle_empty_multiselect_and_validation_timing() {
        let multi_field = FormField::new("topics", FieldKind::Multi)
            .titled("Topics")
            .with_options(vec![option("a", "Alpha"), option("b", "Beta")]);
        let mut form = FormPrompt::new("f-6", "Form", None, vec![multi_field]);
        assert!(form.handle_key(FormKey::Space));
        assert_eq!(form.answer("topics"), Some(&FormValue::List(vec!["a".into()])));
        assert!(form.handle_key(FormKey::Space));
        assert_eq!(form.answer("topics"), None, "optional empty multiselect is omitted");
        // Toggling by digit also works and keeps both values.
        assert!(form.handle_key(FormKey::Digit(1)));
        assert!(form.handle_key(FormKey::Down));
        assert!(form.handle_key(FormKey::Space));
        assert_eq!(form.answer("topics"), Some(&FormValue::List(vec!["a".into(), "b".into()])));

        // Space never toggles (or answers) a single choice.
        let mut single = FormPrompt::new(
            "f-6s",
            "Form",
            None,
            vec![FormField::new("region", FieldKind::Single)
                .titled("Region")
                .with_options(vec![option("eu", "Europe")])],
        );
        assert!(!single.handle_key(FormKey::Space));
        assert_eq!(single.answer("region"), None);
        assert!(!single.editing());

        // Validation is deferred until the matching submit point.
        let mut textual = FormPrompt::new(
            "f-6t",
            "Form",
            None,
            vec![FormField { min_length: Some(3), required: true, ..FormField::new("name", FieldKind::Text) }],
        );
        assert!(textual.handle_key(FormKey::Char('a')));
        assert!(textual.handle_key(FormKey::Char('b')));
        assert_eq!(textual.error(), "", "typing is not validated early");
        assert_eq!(textual.editor_text(), "ab");
        assert_eq!(textual.answer("name"), None, "typing does not commit");
        assert!(textual.handle_key(FormKey::Tab));
        assert_eq!(textual.error(), "Must be at least 3 characters", "tab commit validates");
        assert_eq!(textual.answer("name"), None);
        // Enter on the review tab re-validates and refuses to submit.
        let mut empty_required = FormPrompt::new(
            "f-6r",
            "Form",
            None,
            vec![FormField::new("name", FieldKind::Text).required(), FormField::new("other", FieldKind::Text)],
        );
        empty_required.select_tab(2);
        assert!(empty_required.confirm());
        assert_eq!(empty_required.submit(), None);
        assert_eq!(empty_required.error(), "Answer required");
        assert_eq!(empty_required.resolution(), None);
    }

    // FORM-007 — opening/copying a URL does not acknowledge it, review
    // errors/progress count external acknowledgement, and the review only
    // offers scrolling when it overflows.
    // Upstream: form.tsx:477-511 (external actions), form.tsx:513-517
    // (submit gate), form.tsx:1063-1123 (review), form.tsx:140,1145-1149
    // (review viewport/scroll hint).
    #[test]
    fn form_007_external_acknowledgement_and_review_scroll() {
        let external = FormField {
            url: Some("https://example.com/approve".into()),
            ..FormField::new("site", FieldKind::External).titled("Site")
        };
        let fields = vec![external, text_default(FormField::new("note", FieldKind::Text).titled("Note"), "ok")];
        let mut form = FormPrompt::new("f-7", "Form", None, fields);
        assert_eq!(form.action_label(), "open link");
        // Submission is refused while the external action is unacknowledged.
        assert_eq!(form.submit(), None);
        assert_eq!(form.error(), "External action must be acknowledged: Site");
        // Opening and copying only mark the action ready.
        form.mark_external_opened();
        assert!(form.external_ready("site"));
        assert_eq!(form.answer("site"), None);
        assert_eq!(form.action_label(), "I finished");
        assert_eq!(form.submit(), None, "opened is not acknowledged");
        form.mark_external_copied();
        assert_eq!(form.answer("site"), None);
        // Review shows the acknowledgement requirement as an error state, and
        // progress only counts acknowledged external fields
        // (`form.tsx:123-129`).
        let row = form.review_rows().into_iter().find(|row| row.label == "Site").unwrap();
        assert_eq!(row.state, ReviewState::AcknowledgeRequired);
        assert_eq!(row.display, "(acknowledgement required)");
        assert_eq!(form.answered_count(), 1, "external field is not complete yet");
        // Enter acknowledges once ready, and only then can the form submit.
        form.acknowledge_external();
        assert_eq!(form.answer("site"), Some(&FormValue::Bool(true)));
        assert_eq!(form.answered_count(), 2, "acknowledged external counts");
        let row = form.review_rows().into_iter().find(|row| row.label == "Site").unwrap();
        assert_eq!(row.state, ReviewState::Acknowledged);
        let answer = form.submit().expect("acknowledged form submits");
        assert_eq!(answer.get("site"), Some(&FormValue::Bool(true)));
        assert_eq!(answer.get("note"), Some(&FormValue::Text("ok".into())));
        // Scroll is offered only when the review overflows the viewport.
        assert_eq!(form.review_max_height(30), 16);
        assert_eq!(form.review_max_height(10), 3);
        assert!(!form.review_scroll_available(16, 30));
        assert!(form.review_scroll_available(17, 30));
        assert!(form.review_scroll_available(4, 10));
    }

    // FORM-008 — terminal/stale/revalidation-failed forms restore composer
    // focus and drafts, and keyboard scope never leaks into the transcript.
    // Upstream: form.tsx:211-228 (draft on unmount), form.tsx:66,536-553
    // (mode scope), form.tsx:607-617,642-765 (scoped key layers).
    #[test]
    fn form_008_dismissal_focus_drafts_and_keyboard_scope() {
        let fields = || vec![text_default(FormField::new("name", FieldKind::Text).titled("Name"), "Ada")];
        for reason in [DismissReason::Terminal, DismissReason::Stale, DismissReason::RevalidationFailed] {
            let mut form = FormPrompt::new("f-8", "Form", None, fields());
            form.select_tab(0);
            form.set_cursor(3);
            form.editor_insert("!");
            let dismissal = form.dismiss(reason);
            assert_eq!(dismissal.focus, FocusTarget::Composer, "{reason:?} restores composer focus");
            let draft = dismissal.draft.unwrap_or_else(|| panic!("{reason:?} keeps a draft"));
            let restored = FormPrompt::restore("f-8", "Form", None, fields(), draft);
            assert_eq!(restored.focus_target(), FocusTarget::Overlay);
            // Uncommitted editor text is captured into the draft
            // (`form.tsx:211-228`), so the restored editor shows it while the
            // committed answer map still holds the last committed value.
            assert_eq!(restored.editor_text(), "Ada!");
            assert_eq!(restored.input(), "Ada!");
            assert_eq!(restored.answer("name"), Some(&FormValue::Text("Ada".into())));
        }
        // Cancelled/resolved forms leave no draft behind.
        let mut cancelled = FormPrompt::new("f-8c", "Form", None, fields());
        cancelled.handle_key(FormKey::Escape);
        let dismissal = cancelled.dismiss(DismissReason::Cancelled);
        assert_eq!(dismissal.focus, FocusTarget::Composer);
        assert_eq!(dismissal.draft, None);

        // An inactive overlay consumes nothing: no tab move, no dismissal, no
        // action reaching the transcript underneath.
        let mut inactive = FormPrompt::new(
            "f-8i",
            "Form",
            None,
            vec![FormField::new("a", FieldKind::Text), FormField::new("b", FieldKind::Text)],
        );
        inactive.set_active(false);
        assert!(!inactive.handle_key(FormKey::Tab));
        assert_eq!(inactive.tab(), 0);
        assert!(!inactive.handle_key(FormKey::Escape));
        assert_eq!(inactive.resolution(), None);
        assert!(!inactive.handle_key(FormKey::Char('x')));
        assert_eq!(inactive.answer("a"), None);
        // Reactivated, the same keys are consumed by the overlay.
        inactive.set_active(true);
        assert!(inactive.handle_key(FormKey::Tab));
        assert_eq!(inactive.tab(), 1);
    }
}
