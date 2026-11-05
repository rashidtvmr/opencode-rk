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
//!
//! Modal ownership (DIALOG-19/20). `native_modal_layer` is not a parallel
//! model: [`DialogView`] *owns* a [`crate::native_modal_layer::ModalLayer`] and
//! derives from it everything the shell previously had to track twice.
//!
//! * Opening is `dialog.replace()` (`packages/tui/src/ui/dialog.tsx:396-419`):
//!   the first open saves and blurs the shell focus, every previous entry is
//!   closed, `replace()` resets `size` to medium and `centered` to false, and
//!   exactly one entry is installed. `open_request` therefore calls
//!   [`crate::native_modal_layer::ModalLayer::replace`] last, after every
//!   validation check passed, so a refused request leaves the stack (and the
//!   focus guard) untouched.
//! * Closing is `dialog.clear()` (`:389-398`): all entries close, `size` and
//!   `centered` reset, and a 1 ms refocus is requested. [`DialogView::close`]
//!   and an accepted ACK both route through it; the pending decision keeps the
//!   entry (and therefore focus) exactly as before.
//! * Focus trapping is the `modal` keymap layer, active exactly while the
//!   stack is non-empty (`:320-324,345-347`), i.e.
//!   [`crate::native_modal_layer::ModalLayer::shell_keys_blocked`].
//! * Dismissal ordering comes from the activation order, so a file menu that
//!   pushed itself on top of the dialog (`feature-plugins/system/
//!   diff-viewer-file-menu.tsx:23-30`) is dismissed first and the dialog only
//!   on the next Escape (`ui/dialog.tsx:349-363`).
//! * The painter reads [`DialogView::compose`]: the top entry, its panel rect
//!   (`dialogWidth` `:30-34`, `maxWidth = width - 2` `:80-81`,
//!   `alignItems="center"` `:63`, `paddingTop = centered ? 0 : height / 4`
//!   `:67`, panel `paddingTop = 1` `:83`) and the content rows inside that
//!   rect, all from one derivation.
//!
//! The stack and the painted entry are the same object; a dialog whose content
//! is gone cannot stay painted, and a painted dialog always has a stack entry.

use crate::native_modal_layer::{
    BackdropRelease, Clock, DialogEntry, DialogPaint, DialogRect, DialogSize, ModalFocus, ModalKey,
    ModalLayer, ModalRoute, PanelRelease,
};
use crate::native_modal_layer::ChromeLayer;


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

/// TOOL-011: hard cap on execute dialog body rows (`dialog-execute.tsx` sizes
/// its scroll box to `maxHeight`; a hostile code/output payload cannot grow
/// the paint unbounded).
const MAX_EXECUTE_BODY_ROWS: usize = 400;

/// TOOL-011: the execute dialog's body rows (`dialog-execute.tsx:222-282`):
/// gutter-numbered code rows and JSON-highlighted output as [`Role::Item`],
/// the `Warnings:`/`Logs:` tail as [`Role::Description`]. Bounded by
/// [`MAX_EXECUTE_BODY_ROWS`].
fn execute_body_rows(
    width: usize,
    execute: &crate::tui_entry::native_turn::native_tool_rows::ExecuteDialog,
) -> Vec<(String, crate::native_ui_style::Role)> {
    use crate::native_ui_style::Role;
    let mut rows: Vec<(String, Role)> = crate::tui_entry::native_turn::native_tool_rows::execute_dialog_rows(
        &execute.code,
        execute.output_json.as_deref(),
        None,
        width,
        execute.digits,
        MAX_EXECUTE_BODY_ROWS,
    )
    .into_iter()
    .map(|row| (row, Role::Item))
    .collect();
    if let Some(rest) = execute.output_rest.as_deref() {
        for row in crate::tui_entry::native_turn::native_tool_rows::execute_dialog_rows(
            "",
            None,
            Some(rest),
            width,
            1,
            MAX_EXECUTE_BODY_ROWS.saturating_sub(rows.len()),
        ) {
            rows.push((row, Role::Description));
        }
    }
    rows
}

/// Which overlay is shown. Each kind has its own answer/cancel rules; see
/// the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Permission,
    Question,
    Confirm,
    Error,
    /// DialogSelect list (`ui/dialog-select.tsx`); the model lives in
    /// `native_palette::select::SelectDialog`. Content is the choice list.
    Select,
    /// DialogPrompt text row (`ui/dialog-prompt.tsx`); the model lives in
    /// `native_palette::dialog::PromptState`.
    Prompt,
    /// DialogExecute content viewer (`routes/session/dialog-execute.tsx:28`),
    /// opened from an execute tool row (`index.tsx:3400-3427`). It is a
    /// content-only surface: code block + output block, no choices and no
    /// backend decision. The row text model lives in
    /// `native_tool_rows::ExecuteDialog`.
    Execute,
    /// Root panic overlay, rendered independently of the active theme.
    Crash,
    /// SIDE-08: DialogMcp server list (`component/dialog-mcp.tsx`), opened
    /// from a sidebar MCP row click (`feature-plugins/sidebar/mcp.tsx:56-60`
    /// `onMouseUp` -> `dialog.show(<DialogMcp initialServer details />)`). A
    /// local select list of every MCP server: the clicked server is focused
    /// (`initialServer`), each row carries the exact upstream status footer
    /// word, and a failed server's error is shown as the details message.
    /// Enter surfaces the focused server's toggle identity to the caller and
    /// Esc closes locally; it never pends a permission ack.
    Mcp,
    /// Dedicated patch viewer, matching `feature-plugins/system/diff-viewer.tsx`.
    DiffViewer,
    /// DIALOG-13: the live session picker (`dialog-session-list.tsx`), opened
    /// from the live key loop (Ctrl+S). Its model is the tested
    /// `native_palette::pickers::SessionsState`: a debounced filter with a
    /// generation fence and title filtering. Enter opens the focused session;
    /// Esc closes locally. Directory scoping and deletion are unavailable;
    /// it never pends a permission ack.
    Sessions,
    /// DIALOG-14: the live Worktrees dialog (`dialog-workspaces.tsx`),
    /// opened from the footer working-directory menu `Workspaces` action.
    /// Its model is the tested `native_palette::pickers::WorkspacesState`:
    /// filter + drill-down/back restore, two-press armed delete and a
    /// `LateLoadGuard` that fences late re-enumeration against deletion.
    /// Roots come from local git enumeration (no server route exists);
    /// Enter deletes after arming and Esc closes locally; it never pends a
    /// permission ack.
    Workspaces,
    /// DIALOG-17: the explicit prompt-stash dialog (`dialog-stash.tsx`),
    /// opened from the `prompt.stash.list` palette command. Its rows come
    /// from the tested `native_palette::pickers::stash_rows` over the live
    /// `PromptStash` entries: most recent first, `~N lines` footers and a
    /// two-press `ctrl+d` armed delete. Enter restores the focused entry
    /// into the composer (removing it from the stash); Esc closes locally.
    /// Purely client-local: no server route exists.
    Stash,
    /// DIALOG-15: the focused-server integration sign-in surface
    /// (`dialog-integration.tsx`), opened from `DialogKind::Mcp` Enter when
    /// `mcp_enter` yields `McpEnter::SignIn`. Its model is the tested
    /// `native_palette::dialog::IntegrationState` left in
    /// `IntegrationPhase::entry` initial state; the server-side sign-in
    /// producer is absent, so advancing beyond entry stays unwired.
    Integration,
    /// FORM-003..FORM-008: the upstream question/form overlay
    /// (`routes/session/form.tsx`). The model is the tested
    /// [`form_overlay::FormPrompt`]; the live session-scoped form poll opens
    /// it, routes keys into it, and dispatches the resolved answer map to
    /// `POST /api/session/{s}/form/{id}/reply`.
    Form,
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
    /// DIALOG-02: the caller's optional description line under the dialog
    /// title (`ui/dialog.tsx:102-133`, header props `description?`). Shown
    /// only under the new-designs experiment, like upstream's
    /// `<Show when={newDesigns() && props.description}>`. Bounded with the
    /// same cap as `message` and sanitized to a single line at open.
    pub description: Option<String>,
    pub disabled: Vec<usize>,
    pub error: Option<StaticError>,
}

/// F002-R2: the child-session rejection stage
/// (`permission.tsx:144-155,243-254`). While set, the open
/// [`DialogKind::Prompt`] is the rejection-reason editor for this permission
/// request: confirming emits a `reject` reply carrying the typed reason,
/// cancelling restores the permission options verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectStage {
    /// The original permission card, restored verbatim on cancel.
    request: DialogRequest,
    /// The permission selection to restore (the `reject` row).
    selection: Option<usize>,
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
    LockedSelection,
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
            Self::LockedSelection => "dialog.locked_selection",
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
    /// denial (`reject`), never an automatic approval. `message` carries the
    /// optional rejection reason for a child session's nested stage
    /// (`permission.tsx:148-153`); `None` for an immediate root reject.
    Reject {
        request_id: String,
        session_id: String,
        attempt: u64,
        message: Option<String>,
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
/// DIALOG-17: live stash-dialog state (`dialog-stash.tsx:29-85`): the
/// client-local entries, the armed delete and the clock sample that anchors
/// the relative-time descriptions. `ArmedDelete` is the shared two-press
/// model keyed by the original entry index.
#[derive(Clone, Debug)]
pub struct StashDialogState {
    pub entries: Vec<crate::native_paste::StashEntry>,
    pub armed: crate::native_palette::pickers::ArmedDelete,
    /// `Date.now()` sampled when the dialog opened; the upstream dialog
    /// re-reads it every render, the native dialog freezes it at open.
    pub now_ms: i64,
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
    /// Parallel per-choice status footer (same length as `choices`); only
    /// populated for [`DialogKind::Mcp`] (the exact upstream status word,
    /// `dialog-mcp.tsx:24-37`), empty otherwise.
    mcp_status: Vec<String>,
    message_lines: Vec<String>,
    /// DIALOG-02: the caller's optional description line under the title
    /// (header props `description?`, `ui/dialog.tsx:102-133`). Painted only
    /// under the new-designs experiment, like upstream's `Show when=`.
    description: Option<String>,
    error: StaticError,
    /// DIALOG-10: live `DialogPrompt` text state (`ui/dialog-prompt.tsx`),
    /// present only for [`DialogKind::Prompt`]. The model is the tested
    /// `native_palette::dialog::PromptState`; the live session-rename flow
    /// drives it.
    prompt: Option<crate::native_palette::select::PromptState>,
    /// DIALOG-02: the caller's `footerGap` prop (`ui/dialog.tsx:152,159`).
    /// `None` means the upstream default of 1; the live `dialog-prompt`
    /// caller passes 2 (`dialog-prompt.tsx:133`). Under the new-designs
    /// experiment a footer with gap `n` adds `n - 1` body bottom padding
    /// rows, so this is the live input to
    /// [`crate::native_palette::dialog::body_bottom_padding`].
    footer_gap: Option<u16>,
    /// DIALOG-06..09: live select state; option IDs are original choice indexes.
    select: Option<crate::native_palette::select::SelectDialog>,
    /// F002-R2: whether the permission request's session is a child
    /// (`session.parentID`). Gates the nested rejection prompt: a child
    /// session opens the reason editor, a root session replies immediately
    /// (`permission.tsx:243-254`). Meaningful only for
    /// [`DialogKind::Permission`].
    session_has_parent: bool,
    /// TOOL-011: the tested execute dialog model
    /// (`native_tool_rows::ExecuteDialog`, `dialog-execute.tsx`), present only
    /// for [`DialogKind::Execute`]; built by [`DialogView::open_execute`]
    /// from the clicked execute tool part.
    execute: Option<crate::tui_entry::native_turn::native_tool_rows::ExecuteDialog>,
    /// TOOL-011: execute body scroll offset (rows). The layout clamps the
    /// painted window; [`DialogView::execute_scroll_state`] clamps the key
    /// handler's next value the way upstream `scroll?.scrollBy` does.
    execute_scroll: usize,
    /// DIFF-001: the resolved unified/split mode of the open diff viewer,
    /// fixed once at open time from the same surface/width resolution the
    /// row was measured against (`diff-viewer.tsx:45,349`); `None` for every
    /// other kind.
    diff_mode: Option<crate::native_diff_view::DiffViewMode>,
    /// DIALOG-13: the live session-newest list state
    /// (`dialog-session-list.tsx`), present only for
    /// [`DialogKind::Sessions`]. The model is the tested
    /// `native_palette::pickers::SessionsState`; the live key gate drives it.
    sessions: Option<crate::native_palette::pickers::SessionsState>,
    /// DIALOG-14: the live worktrees dialog state (`dialog-workspaces.tsx`),
    /// present only for [`DialogKind::Workspaces`]. The model is the tested
    /// `native_palette::pickers::WorkspacesState`; the live key gate drives
    /// it exactly like [`DialogKind::Sessions`].
    workspaces: Option<crate::native_palette::pickers::WorkspacesState>,
    /// DIALOG-17: the live prompt-stash dialog state (`dialog-stash.tsx`),
    /// present only for [`DialogKind::Stash`]. Rows paint through the tested
    /// `native_palette::pickers::stash_rows`; the live key gate drives it.
    stash: Option<StashDialogState>,
    /// DIALOG-15: the integration sign-in surface (`dialog-integration.tsx`),
    /// present only for [`DialogKind::Integration`]. The model is the tested
    /// `native_palette::dialog::IntegrationState`; the live MCP dialog's
    /// `McpEnter::SignIn` outcome opens it.
    integration: Option<crate::native_palette::pickers::IntegrationState>,
    /// FORM-003..FORM-008: the live question/form prompt
    /// (`routes/session/form.tsx`), present only for [`DialogKind::Form`]. The
    /// model is the tested [`form_overlay::FormPrompt`]; the live form poll
    /// opens it, the key gate routes [`form_overlay::FormKey`] into it, and the
    /// resolved answer map is POSTed to the form reply route.
    form: Option<form_overlay::FormPrompt>,
}

/// Focus-owning dialog overlay state. `Default` is a closed view.
///
/// The visible dialog is a [`ModalLayer`] entry, not a second flag: opening,
/// closing, focus trapping, dismissal ordering and the painted rect/rows all
/// derive from [`DialogView::modal`]. `ui/dialog.tsx:306-439` exposes the same
/// split — a stack plus `size`/`centered` store that the provider renders from
/// (`:461-479`).
#[derive(Debug)]
pub struct DialogView {
    phase: Phase,
    dialog: Option<OpenDialog>,
    selection: Option<usize>,
    question_capable: bool,
    /// `useNewDesigns()` (`ui/dialog.tsx:105,109`): the live new-designs
    /// experiment fact. Defaults to `false` (legacy shell); the composition
    /// root seeds it from the same preference read that selects the theme.
    new_designs: bool,
    crash_screen: Option<crash::CrashScreen>,
    rejected: bool,
    /// Locally monotonic decision-attempt counter. Never reset on close,
    /// `open_request` or rejected ACK; fails closed on overflow.
    attempt_counter: u64,
    /// Attempt of the in-flight decision; meaningful while pending.
    pending_attempt: u64,
    /// F002-R2: the child-session rejection stage. While set, the open
    /// `DialogKind::Prompt` is the rejection-reason editor for this permission
    /// request (`permission.tsx:144-155,243-254`); confirming it emits a
    /// `reject` reply carrying the typed reason.
    reject_stage: Option<RejectStage>,
    /// Dialog stack, focus guard, dismissal order and paint geometry
    /// (`ui/dialog.tsx:306-439`). The stack is the single "is a dialog
    /// visible" authority; `phase` only carries the decision lifecycle.
    modal: ModalLayer,
    /// Monotonic stack-entry identity. Upstream's stack entry carries the
    /// caller's optional `key` (`ui/dialog.tsx:311`); the numeric entry id
    /// belongs to the presentation, so it is minted here.
    open_sequence: u64,
    /// Shell focus target saved by the first open and restored after the last
    /// close (`ui/dialog.tsx:326-343,396-404`).
    focus: ModalFocus,
}

impl Default for DialogView {
    fn default() -> Self {
        Self::new()
    }
}


/// Replace every control character (C0/C1, including newline and ESC) with
/// a space, then trim. Single-line fields only — no line can be injected.
fn bounded_crash_text(text: &str, max_bytes: usize) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        let ch = if ch.is_control() && ch != '\n' && ch != '\t' { ' ' } else { ch };
        if result.len().saturating_add(ch.len_utf8()) > max_bytes {
            break;
        }
        result.push(ch);
    }
    result
}

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
        "deny" | "reject" | "Reject" => Some(PermissionChoice::Reject),
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

/// One frame of the painted dialog, derived from the modal stack: the entry
/// that owns the frame, its geometry and the rows inside it.
///
/// `rows`/`hits`/`footer_row` are exactly what
/// [`DialogView::render_styled_with_metrics`] returns for the same width and
/// budget, so a painter can use this struct alone (which dialog, where, what
/// text) instead of reading the stack and the content separately.
///
/// No `Clone`/`Debug`/`PartialEq`: [`crate::native_ui_style::Line`] carries
/// none of them, and the frame is a per-frame paint value, not stored state.
pub struct DialogFrame {
    /// Top stack entry (`ui/dialog.tsx:472-479`).
    pub entry: DialogEntry,
    /// `dialogWidth(size)` class currently in use (`ui/dialog.tsx:30-34,432`).
    pub size: DialogSize,
    /// Backdrop vertical centering (`ui/dialog.tsx:64,67`).
    pub centered: bool,
    /// The panel rect the rows belong to.
    pub panel: DialogRect,
    /// Content rows, already clipped to the panel's inner width.
    pub rows: Vec<crate::native_ui_style::Line>,
    /// Choice index addressable at each row (`None` for non-choice rows).
    pub hits: Vec<Option<usize>>,
    /// Rendered confirm footer row, when the footer is live.
    pub footer_row: Option<usize>,
}


impl DialogView {
    /// Current native dialogs are action overlays, not upstream DialogSelect
    /// lists. The production caller must not opt them into the select viewport
    /// merely because a full terminal height is available.
    #[must_use]
    pub fn production_viewport_policy(&self) -> DialogViewportPolicy {
        match self.dialog.as_ref().map(|dialog| dialog.kind) {
            Some(DialogKind::Select) => DialogViewportPolicy::SelectTerminal,
            Some(DialogKind::Mcp) => DialogViewportPolicy::SelectTerminal,
            Some(DialogKind::Sessions) => DialogViewportPolicy::SelectTerminal,
            Some(DialogKind::Workspaces) => DialogViewportPolicy::SelectTerminal,
            Some(DialogKind::Integration) => DialogViewportPolicy::SelectTerminal,
            // DIALOG-17: the stash list uses the same DialogSelect viewport.
            Some(DialogKind::Stash) => DialogViewportPolicy::SelectTerminal,
            Some(DialogKind::Permission)
            | Some(DialogKind::Question)
            | Some(DialogKind::Confirm)
            | Some(DialogKind::Error)
            | Some(DialogKind::Prompt)
            | Some(DialogKind::Execute)
            | Some(DialogKind::Crash)
            | Some(DialogKind::DiffViewer)
            | Some(DialogKind::Form)
            | None => DialogViewportPolicy::ContentBudget,
        }
    }

    /// Closed view (no dialog, input passes through).
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: Phase::Closed,
            dialog: None,
            selection: None,
            question_capable: false,
            new_designs: false,
            crash_screen: None,
            rejected: false,
            attempt_counter: 0,
            pending_attempt: 0,
            reject_stage: None,
            modal: ModalLayer::new(),
            open_sequence: 0,
            focus: ModalFocus::new(0),
        }
    }

    /// True while a dialog is visible — including the pending-decision
    /// phase. While true, the shell must route keys/paste here and never to
    /// the composer.
    ///
    /// Derived from the modal stack, so the painted dialog and the
    /// input-owning dialog can never disagree.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.modal.is_open()
    }

    /// True while a decision is in flight awaiting a backend ack.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.phase == Phase::Pending && self.is_open()
    }

    /// Whether input must be intercepted (busy input/paste cannot submit
    /// the underlying composer while a dialog is visible).
    pub fn intercepts_input(&self) -> bool {
        self.shell_keys_blocked()
    }

    /// The `modal` keymap layer is enabled exactly while the stack is
    /// non-empty (`ui/dialog.tsx:320-324,345-347`): every shell key, not only
    /// Escape, is trapped.
    #[must_use]
    pub fn shell_keys_blocked(&self) -> bool {
        self.modal.shell_keys_blocked()
    }

    /// The modal stack this view owns. Read-only: the mutation paths are the
    /// view's own open/close/dismiss methods, so the stack can never disagree
    /// with the dialog content it paints.
    #[must_use]
    pub fn modal(&self) -> &ModalLayer {
        &self.modal
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

    /// Seed the live `new-designs` experiment fact (`useNewDesigns()`,
    /// `ui/dialog.tsx:105`). The composition root reads it from the same
    /// preference source that selects the theme (`tui_entry.rs`
    /// `new_designs_enabled`) and applies it before the first paint. Purely
    /// presentational: it changes the header weight/color, the description
    /// line and the footer gap, never any decision or focus semantics.
    pub fn set_new_designs(&mut self, enabled: bool) {
        self.new_designs = enabled;
        if let Some(select) = self.dialog.as_mut().and_then(|dialog| dialog.select.as_mut()) {
            select.set_experiment(if enabled {
                crate::native_palette::select::SelectExperiment::new_designs()
            } else {
                crate::native_palette::select::SelectExperiment::legacy()
            });
        }
    }

    /// Whether the new-designs shell is active for this view.
    #[must_use]
    pub fn new_designs(&self) -> bool {
        self.new_designs
    }

    /// The visible dialog's kind when it gates session-tab attention: a
    /// permission or question dialog. Confirm/error dialogs and a closed stack
    /// contribute no attention (`component/session-tabs.tsx:163-174`, where the
    /// tab's attention comes from `permission.list()`/`form.list()`).
    #[must_use]
    pub fn attention_dialog_kind(&self) -> Option<DialogKind> {
        match self.dialog.as_ref().map(|dialog| dialog.kind) {
            Some(DialogKind::Permission) => Some(DialogKind::Permission),
            Some(DialogKind::Question) => Some(DialogKind::Question),
            // The form overlay is the question surface upstream:
            // `form.list(session).length > 0` drives the "?" tab attention
            // (`context/session-tabs.tsx:163-174`), so an open form reports the
            // same Question kind as a question dialog.
            Some(DialogKind::Form) => Some(DialogKind::Question),
            _ => None,
        }
    }

    // ---- presentation, focus and dismissal, all owned by the modal stack ----

    /// Dialog size class (`dialogWidth(size)`, `ui/dialog.tsx:30-34`). Medium
    /// until a caller changes it, like the upstream store default (`:350`).
    #[must_use]
    pub fn dialog_size(&self) -> DialogSize {
        self.modal.size()
    }

    /// `dialog.setSize()` (`ui/dialog.tsx:432-434`).
    ///
    /// Upstream callers apply it *after* opening, from an effect
    /// (`plugin/api.tsx:338-339`, `ui/dialog-prompt.tsx:106`), because
    /// `replace()` resets the store to medium (`:406-408`). A size set while
    /// closed is therefore overwritten by the next [`DialogView::open_request`];
    /// use [`DialogView::open_request_sized`] to open and size in one step.
    pub fn set_dialog_size(&mut self, size: DialogSize) {
        self.modal.set_size(size);
    }

    /// Whether the panel is vertically centered (`ui/dialog.tsx:64,67`).
    #[must_use]
    pub fn centered(&self) -> bool {
        self.modal.centered()
    }

    /// `dialog.setCentered()` (`ui/dialog.tsx:435-437`).
    pub fn set_centered(&mut self, centered: bool) {
        self.modal.set_centered(centered);
    }

    /// Shell focus target saved by the first open
    /// (`ui/dialog.tsx:396-400`). Only consulted by the *next* open: upstream
    /// reads `renderer.currentFocusedRenderable` at that moment.
    pub fn set_focus_target(&mut self, focus: ModalFocus) {
        self.focus = focus;
    }

    /// The saved shell focus while a dialog owns focus
    /// (`ui/dialog.tsx:396-404`).
    #[must_use]
    pub fn saved_focus(&self) -> Option<ModalFocus> {
        self.modal.saved_focus()
    }

    /// True while the saved focus target is blurred (`ui/dialog.tsx:402-403`).
    #[must_use]
    pub fn focused_blurred(&self) -> bool {
        self.modal.focused_blurred()
    }

    /// The 1 ms refocus timer (`ui/dialog.tsx:326-343`). Returns the focus to
    /// restore once the stack is empty, the target is alive and it is still
    /// attached; `None` before the deadline or when nothing is restorable.
    pub fn take_refocus(&mut self, now_ms: u64) -> Option<ModalFocus> {
        self.modal.take_refocus(now_ms)
    }

    /// [`DialogView::take_refocus`] reading the deadline from an injected
    /// clock.
    pub fn take_refocus_at(&mut self, clock: &impl Clock) -> Option<ModalFocus> {
        self.modal.take_refocus_at(clock)
    }

    /// Entries on the dialog stack (upstream exposes the same slice,
    /// `ui/dialog.tsx:420-422`). At most one entry exists in production —
    /// `replace()` is the only open path — but the stack is the authority.
    #[must_use]
    pub fn stack(&self) -> &[DialogEntry] {
        self.modal.stack()
    }

    /// Entry painted this frame; `None` when closed
    /// (`ui/dialog.tsx:472-479` paints only `stack.at(-1)`).
    #[must_use]
    pub fn top_dialog(&self) -> Option<DialogEntry> {
        self.modal.top_entry()
    }

    /// Dialogs painted this frame (0 or 1, see
    /// [`crate::native_modal_layer::ModalLayer::painted_dialogs`]).
    #[must_use]
    pub fn painted_dialogs(&self) -> u8 {
        self.modal.painted_dialogs()
    }

    /// Record that a chrome layer became interactive, so dismissal order
    /// follows activation rather than paint order
    /// (`diff-viewer-file-menu.tsx:23-30`, `ui/dialog.tsx:320-324`).
    pub fn activate_chrome(&mut self, layer: ChromeLayer) {
        self.modal.activate(layer);
    }

    /// Record that a chrome layer stopped being interactive.
    pub fn deactivate_chrome(&mut self, layer: ChromeLayer) {
        self.modal.deactivate(layer);
    }

    /// Interactive layers in activation order (oldest first).
    #[must_use]
    pub fn activations(&self) -> &[ChromeLayer] {
        self.modal.activations()
    }

    /// Layer that owns Escape/Ctrl+C right now.
    #[must_use]
    pub fn top_interactive(&self) -> Option<ChromeLayer> {
        self.modal.top_interactive()
    }

    /// Escape routed through the activation order: an open file menu is
    /// dismissed before the dialog (`diff-viewer-file-menu.tsx:27,51-55`,
    /// `ui/dialog.tsx:349-363`), and a selection is cleared instead of
    /// closing anything.
    ///
    /// Unlike upstream, a pending decision is never abandoned by Escape: the
    /// key is reported as [`ModalKey::Ignored`] while the ACK is in flight,
    /// because this module's contract is that a dispatched deny/approve
    /// cannot be discarded locally (see the module docs).
    pub fn escape_route(&mut self, has_selection: bool) -> ModalRoute {
        if self.pending() {
            return ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: ModalKey::Ignored,
            };
        }
        let route = self.modal.escape_route(has_selection);
        self.reconcile_closed_route(route)
    }

    /// Ctrl+C routed through the activation order, with the same precedence
    /// and the same pending guard (`ui/dialog.tsx:364-384`).
    pub fn ctrl_c_route(&mut self, has_selection: bool, editor_has_text: bool) -> ModalRoute {
        if self.pending() {
            return ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: ModalKey::Ignored,
            };
        }
        let route = self.modal.ctrl_c_route(has_selection, editor_has_text);
        self.reconcile_closed_route(route)
    }

    /// Backdrop mouse-down: a drag that ends with a selection must not close
    /// the dialog (`ui/dialog.tsx:51-53`).
    pub fn backdrop_mouse_down(&mut self, has_selection: bool) {
        self.modal.backdrop_mouse_down(has_selection);
    }

    /// Backdrop mouse-up (`ui/dialog.tsx:54-60`). Closes the stack like
    /// `clear()`. A pending decision keeps the dialog: the release is
    /// swallowed exactly once, the same reading as an armed selection drag.
    pub fn backdrop_mouse_up(&mut self) -> BackdropRelease {
        if self.pending() {
            return BackdropRelease::Suppressed;
        }
        let release = self.modal.backdrop_mouse_up();
        if release == BackdropRelease::Closed {
            self.drop_content();
        }
        release
    }

    /// Panel mouse-up (`ui/dialog.tsx:73-79`): a selection release bubbles to
    /// copy-on-select, anything else stops at the panel.
    pub fn panel_mouse_up(&mut self, has_selected_text: bool) -> PanelRelease {
        self.modal.panel_mouse_up(has_selected_text)
    }

    /// Panel rect and paint order of the painted dialog, derived from the
    /// stack. `content_height` is the body budget the caller computed; `None`
    /// when no dialog is open (`ui/dialog.tsx:48-90,472-479`).
    #[must_use]
    pub fn paint(
        &self,
        terminal_width: u16,
        terminal_height: u16,
        content_height: usize,
    ) -> Option<DialogPaint> {
        let content_height = u16::try_from(content_height).unwrap_or(u16::MAX);
        self.modal.paint(terminal_width, terminal_height, content_height)
    }

    /// Everything the painter needs for the top dialog from one derivation:
    /// which entry is on top, its panel rect and the content rows inside that
    /// rect. `None` when nothing is open.
    ///
    /// This is the accessor a composition root should prefer over calling
    /// [`DialogView::render_with_metrics`] and the geometry helpers
    /// separately: the entry, the rect and the rows cannot come from
    /// different dialogs.
    #[must_use]
    pub fn compose(
        &self,
        terminal_width: u16,
        terminal_height: u16,
        content_height: usize,
    ) -> Option<DialogFrame> {
        let paint = self.paint(terminal_width, terminal_height, content_height)?;
        Some(self.frame_over(paint.panel, content_height))
    }

    /// [`DialogView::compose`] with a caller-supplied panel rect, for
    /// composition roots that own a capture-backed origin instead of the
    /// source-derived one (`native_view.rs` keeps the pinned capture origin).
    /// Presentation (`size`, `centered`) still comes from the stack.
    #[must_use]
    pub fn compose_at(&self, panel: DialogRect, content_height: usize) -> Option<DialogFrame> {
        self.modal.top_entry()?;
        Some(self.frame_over(panel, content_height))
    }

    fn frame_over(&self, panel: DialogRect, content_height: usize) -> DialogFrame {
        // `DialogLayout` re-applies the shared side padding inside the panel
        // (`ui/dialog.tsx:163-166`), so the body width is the panel minus
        // `DIALOG_INSET` on each side.
        let width = usize::from(
            panel
                .width
                .saturating_sub(2 * crate::native_modal_layer::DIALOG_INSET),
        );
        let metrics = DialogLayoutMetrics::new(
            width,
            content_height,
            usize::from(panel.height),
            self.production_viewport_policy(),
        );
        let (rows, hits, footer_row) = self.layout(metrics);
        DialogFrame {
            entry: self.modal.top_entry().expect("frame builder requires an entry"),
            size: self.modal.size(),
            centered: self.modal.centered(),
            panel,
            rows,
            hits,
            footer_row,
        }
    }

    /// Dismiss the dialog. A pending decision holds focus: `close` is a
    /// no-op until the matching ack arrives (a denial cannot be discarded
    /// locally to make the overlay disappear).
    pub fn close(&mut self) {
        if self.phase == Phase::Open {
            self.reset();
        }
    }

    /// Open the root panic overlay with bounded, sanitized panic details.
    pub fn open_crash(
        &mut self,
        message: Option<&str>,
        stack: Option<&str>,
        version: &str,
        mode: crash::CrashMode,
        terminal: &str,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            self.close();
        }
        let message = message.map(|text| bounded_crash_text(text, 2048));
        let stack = stack.map(|text| bounded_crash_text(text, 16 * 1024));
        self.dialog = Some(OpenDialog {
            id: "crash".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Crash,
            title: crash::HEADLINE.to_owned(),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.crash_screen = Some(crash::CrashScreen::new(
            mode,
            version,
            message.as_deref(),
            stack.as_deref(),
            std::env::consts::OS,
            terminal,
        ));
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        self.modal.replace(self.open_sequence, DialogSize::Medium, true, None, self.focus);
        Ok(())
    }

    /// Crash model, only while the crash overlay is open.
    pub fn crash_screen(&self) -> Option<&crash::CrashScreen> {
        self.crash_screen.as_ref()
    }

    /// Mutable crash model, for the live copy acknowledgment.
    pub fn crash_screen_mut(&mut self) -> Option<&mut crash::CrashScreen> {
        self.crash_screen.as_mut()
    }

    /// Route a crash key to the upstream crash-screen model.
    pub fn handle_crash_key(&mut self, key: crash::CrashKey, rows: u16) -> Option<crash::CrashOutcome> {
        self.crash_screen.as_mut().map(|screen| screen.handle(key, rows))
    }

    /// Dismiss the live crash surface and clear its model (`r` reset,
    /// `error-component.tsx:60`), so the next captured panic can reopen it.
    pub fn close_crash(&mut self) {
        self.crash_screen = None;
        self.close();
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
        // DIALOG-02: the description carries the same bound as `message`
        // (it is the same shape of free caller text, painted as one faint
        // line); reuse the variant so the error surface stays closed.
        if request
            .description
            .as_ref()
            .is_some_and(|d| d.len() > MAX_MESSAGE_LEN)
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
        // `routes/session/dialog-execute.tsx:28` (and the static error card)
        // is a content-only viewer: no choice list, so empty `choices` is
        // valid for `Execute` exactly as for `Error`.
        if request.kind != DialogKind::Error && request.kind != DialogKind::Execute && request.kind != DialogKind::DiffViewer && request.choices.is_empty() {
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
        // DIALOG-06..09: a live select keeps its own filtered model. Every
        // option id is the ORIGINAL choice index, so a filtered confirm still
        // answers with the choice the user actually picked and the paint path
        // can map a visible row back to `dialog.choices`.
        let select = (request.kind == DialogKind::Select).then(|| {
            let options = sanitized_choices
                .iter()
                .enumerate()
                .map(|(index, choice)| crate::native_palette::select::SelectOption {
                    disabled: disabled[index],
                    ..crate::native_palette::select::SelectOption::new(
                        index.to_string(),
                        choice.clone(),
                    )
                })
                .collect();
            crate::native_palette::select::SelectDialog::new(options).with_experiment(
                if self.new_designs {
                    crate::native_palette::select::SelectExperiment::new_designs()
                } else {
                    crate::native_palette::select::SelectExperiment::legacy()
                },
            )
        });
        self.dialog = Some(OpenDialog {
            id: request.id,
            session_id: request.session_id,
            kind: request.kind,
            title: sanitize_inline(&request.title),
            choices: sanitized_choices,
            disabled,
            permission,
            select,
            mcp_status: Vec::new(),
            message_lines: request
                .message
                .as_deref()
                .map(sanitize_block)
                .unwrap_or_default(),
            // DIALOG-02: single-line faint text under the title; inline
            // sanitization (no block/control leak, `ui/dialog.tsx:129`).
            description: request.description.as_deref().map(sanitize_inline),
            error: request.error.unwrap_or(StaticError::BackendUnavailable),
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
        });
        // `DialogSelect` seeds `selected: 0` (`ui/dialog-select.tsx:152`), so
        // the first enabled row is selected on open and the first Down moves
        // to the second row. Permission/Question/Confirm/Error start
        // unselected (the user must make an explicit choice).
        self.selection = self
            .dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Select)
            .and_then(|dialog| dialog.disabled.iter().position(|disabled| !disabled));
        self.rejected = false;
        self.phase = Phase::Open;
        // `dialog.replace()` (`ui/dialog.tsx:396-419`) runs last, once every
        // check above passed: it saves and blurs the shell focus on the first
        // open, closes any previous entry, resets `size` to medium and
        // `centered` to false, and installs exactly one entry. A refused
        // request therefore never touches the stack or the focus guard.
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        Ok(())
    }

    /// SIDE-08: open the `DialogMcp` server list (`component/dialog-mcp.tsx`).
    ///
    /// `servers` is the full `(name, status_word)` list in the order the
    /// caller wants painted (upstream sorts by name, `dialog-mcp.tsx:53-58`);
    /// `focused` is the index of the clicked server (`initialServer`,
    /// `sidebar/mcp.tsx:57`); `details` is the failed server's error text,
    /// shown as the dialog body when the click carried `details` (a failed
    /// server, `dialog-mcp.tsx:59-60`).
    ///
    /// This is a local select list, not a backend decision: it never enters
    /// the pending phase and never issues a [`DecisionIntent`]. Enter is
    /// handled by the caller reading [`DialogView::focused_choice`]; Esc
    /// closes through [`DialogView::close`].
    pub fn open_mcp(
        &mut self,
        servers: &[(String, String)],
        focused: Option<usize>,
        details: Option<String>,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        if servers.is_empty() {
            return Err(DialogError::NoChoices);
        }
        if servers.len() > MAX_CHOICES {
            return Err(DialogError::TooManyChoices);
        }
        let mut choices = Vec::with_capacity(servers.len());
        let mut mcp_status = Vec::with_capacity(servers.len());
        for (name, status) in servers {
            if name.len() > MAX_CHOICE_LEN {
                return Err(DialogError::ChoiceTooLong);
            }
            choices.push(sanitize_inline(name));
            mcp_status.push(sanitize_inline(status));
        }
        let message_lines = details
            .as_deref()
            .map(sanitize_block)
            .unwrap_or_default();
        self.dialog = Some(OpenDialog {
            id: "mcp".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Mcp,
            title: "MCP servers".to_owned(),
            choices,
            disabled: vec![false; servers.len()],
            permission: Vec::new(),
            select: None,
            mcp_status,
            message_lines,
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        // `DialogSelect` seeds `selected` to the focused row
        // (`dialog-select.tsx:152`, `dialog-mcp.tsx:61` `preserveSelection`).
        self.selection = focused.filter(|index| *index < servers.len()).or(Some(0));
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        Ok(())
    }

    /// DIALOG-13: open the live session picker (`dialog-session-list.tsx`).
    /// Content-only like [`DialogView::open_mcp`]: it never pends a permission
    /// ack. The tested [`crate::native_palette::pickers::SessionsState`] is the
    /// single source of truth for the rows and filter; the
    /// caller drives it through the `sessions_*` forwarders below and reads the
    /// painted rows from [`DialogView::render`].
    ///
    /// `size` mirrors `dialog.setSize("large")` (`dialog-session-list.tsx:196`);
    /// it is passed rather than hard-coded so the caller's composition root
    /// keeps one size authority inside this module.
    pub fn open_sessions(
        &mut self,
        state: crate::native_palette::pickers::SessionsState,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        let rows = state.rows.len();
        let titles: Vec<String> = state
            .row_titles("ctrl+d")
            .into_iter()
            .take(MAX_CHOICES)
            .collect();
        self.dialog = Some(OpenDialog {
            id: "sessions".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Sessions,
            title: "Sessions".to_owned(),
            choices: titles,
            disabled: vec![false; rows.min(MAX_CHOICES)],
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: Some(state),
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.selection = Some(0);
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        // `onMount(() => dialog.setSize("large"))`
        // (`dialog-session-list.tsx:196`).
        self.modal
            .replace(entry_id, DialogSize::Large, false, None, self.focus);
        Ok(())
    }

    /// DIALOG-13: the live session list state, or `None` when the picker is
    /// closed. The live key gate mutates it through the `sessions_*`
    /// forwarders; tests read it to assert the real dispatch landed.
    #[must_use]
    pub fn sessions_state(&self) -> Option<&crate::native_palette::pickers::SessionsState> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Sessions)
            .and_then(|dialog| dialog.sessions.as_ref())
    }

    /// Rebuild the picker's painted `choices` from its live model after any
    /// mutation (filter, move, scope, arm, rename), so the rows the user sees
    /// always match the model that will answer. The `choices`/`disabled` vecs
    /// are the only row channel `render` reads for a non-select kind.
    fn sync_sessions_choices(&mut self) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        if dialog.kind != DialogKind::Sessions {
            return;
        }
        let Some(state) = dialog.sessions.as_ref() else { return };
        let titles: Vec<String> = state.row_titles("ctrl+d").into_iter().take(MAX_CHOICES).collect();
        dialog.choices = titles;
        dialog.disabled = vec![false; dialog.choices.len()];
        self.selection = Some(state.selected.min(dialog.choices.len().saturating_sub(1)));
    }

    /// DIALOG-13: the live filter keystroke. Returns `true` when visible state
    /// changed (the debounce timer must be (re)started by the caller).
    pub fn sessions_push_filter(&mut self, ch: char) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
            .is_some_and(|state| state.push_filter(ch));
        if changed {
            self.sync_sessions_choices();
        }
        changed
    }

    /// DIALOG-13: Backspace removes one filter code point.
    pub fn sessions_backspace(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
            .is_some_and(|state| state.pop_filter());
        if changed {
            self.sync_sessions_choices();
        }
        changed
    }

    /// Toggle project scope and immediately re-filter rows already held by
    /// the picker. No network request is implied by this local action.
    pub fn sessions_toggle_scope(&mut self) -> bool {
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
            .is_some_and(|state| state.toggle_scope());
        if changed { self.sync_sessions_choices(); }
        changed
    }

    /// DIALOG-13: move the picker highlight.
    pub fn sessions_move(&mut self, delta: isize) -> bool {
        let Some(state) = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
        else {
            return false;
        };
        state.move_selection(delta);
        self.sync_sessions_choices();
        true
    }

    /// DIALOG-13: the debounce tick fired. Returns the `(generation, query,
    /// all_projects)` fetch request the caller must issue, or `None` when the
    /// picker is closed. A no-op debounce still bumps the generation so a
    /// stale in-flight result is fenced out.
    #[must_use]
    pub fn sessions_debounce_fired(&mut self) -> Option<(u64, String, bool)> {
        let state = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())?;
        Some(state.debounce_fired())
    }

    /// DIALOG-13: apply a completed fetch. A stale generation is dropped
    /// (`dialog-session-list.tsx:102-110`) and returns `false`; the visible
    /// rows never go backwards.
    pub fn sessions_apply_fetch(
        &mut self,
        generation: u64,
        server: Vec<crate::native_palette::pickers::SessionRow>,
    ) -> bool {
        let applied = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
            .is_some_and(|state| state.apply_fetch(generation, server));
        if applied {
            self.sync_sessions_choices();
        }
        applied
    }

    /// DIALOG-13: seed the local fallback rows before the first fetch returns.
    pub fn sessions_seed_local(&mut self, local: Vec<crate::native_palette::pickers::SessionRow>) {
        if let Some(state) = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
        {
            state.seed_local(local);
            self.sync_sessions_choices();
        }
    }

    /// DIALOG-13: focused session identity for live actions.
    #[must_use]
    pub fn sessions_focused_id(&self) -> Option<String> {
        self.sessions_state().and_then(|state| state.focused_id().map(str::to_owned))
    }

    /// DIALOG-13: the focused row when it can be opened (not armed), or `None`.
    #[must_use]
    pub fn sessions_open_focused(&self) -> Option<crate::native_palette::pickers::SessionRow> {
        self.sessions_state().and_then(|state| state.open_focused().cloned())
    }

    /// DIALOG-13: apply `session.rename` to the live list; the row title (and
    /// the local fallback copy) is replaced. Returns `true` when the id was
    /// present.
    pub fn sessions_rename(&mut self, id: &str, title: &str) -> bool {
        let renamed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.sessions.as_mut())
            .is_some_and(|state| state.rename_row(id, title));
        if renamed {
            self.sync_sessions_choices();
        }
        renamed
    }

    // ---- DIALOG-14: the live worktrees dialog ----

    /// DIALOG-14: open the live `DialogWorkspaces` (`dialog-workspaces.tsx`,
    /// `onMount(() => dialog.setSize("xlarge"))` at `:55`, title at `:328`).
    /// The tested [`crate::native_palette::pickers::WorkspacesState`] is the
    /// single source of truth; the caller seeds `state.roots` from local git
    /// enumeration before calling, then drives it through the
    /// `workspaces_*` forwarders below while [`DialogView::render`] paints
    /// from [`Self::sync_workspaces_choices`].
    pub fn open_workspaces(
        &mut self,
        state: crate::native_palette::pickers::WorkspacesState,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        self.dialog = Some(OpenDialog {
            id: "workspaces".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Workspaces,
            title: "Worktrees".to_owned(),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: Some(state),
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.selection = Some(0);
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::XLarge, false, None, self.focus);
        self.sync_workspaces_choices();
        Ok(())
    }

    /// DIALOG-14: the live workspaces state, or `None` when the dialog is
    /// closed. Tests read it to assert the real dispatch landed.
    #[must_use]
    pub fn workspaces_state(&self) -> Option<&crate::native_palette::pickers::WorkspacesState> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Workspaces)
            .and_then(|dialog| dialog.workspaces.as_ref())
    }

    /// Rebuild the worktrees painted `choices` from the live model.
    fn sync_workspaces_choices(&mut self) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        if dialog.kind != DialogKind::Workspaces {
            return;
        }
        let Some(state) = dialog.workspaces.as_ref() else { return };
        let current = state.current.clone();
        let titles: Vec<String> = state
            .rows(current.as_deref())
            .into_iter()
            .map(|row| row.title)
            .take(MAX_CHOICES)
            .collect();
        dialog.choices = titles;
        dialog.disabled = vec![false; dialog.choices.len()];
        self.selection = Some(state.selected.min(dialog.choices.len().saturating_sub(1)));
    }

    /// DIALOG-14: live filter keystroke.
    pub fn workspaces_push_filter(&mut self, ch: char) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .is_some_and(|state| {
                state.push_filter(ch);
                true
            });
        if changed {
            self.sync_workspaces_choices();
        }
        changed
    }

    /// DIALOG-14: Backspace removes one filter code point.
    pub fn workspaces_backspace(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .is_some_and(|state| {
                state.pop_filter();
                true
            });
        if changed {
            self.sync_workspaces_choices();
        }
        changed
    }

    /// DIALOG-14: move the highlight.
    pub fn workspaces_move(&mut self, delta: isize) -> bool {
        let Some(state) = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
        else {
            return false;
        };
        let visible = state.rows(state.current.as_deref()).len();
        state.move_selection(delta, visible);
        self.sync_workspaces_choices();
        true
    }

    /// DIALOG-14: drill into the focused root, snapshotting filter and
    /// selection (`dialog-open.tsx:103-122`).
    pub fn workspaces_drill_forward(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .is_some_and(|state| {
                let current = state.current.clone();
                state.drill_forward(current.as_deref());
                true
            });
        if changed {
            self.sync_workspaces_choices();
        }
        changed
    }

    /// DIALOG-14: back restores the snapshot; a removed identity reconciles
    /// to the nearest live row instead of resurrecting
    /// (`dialog-open.tsx:118-128`).
    pub fn workspaces_drill_back(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .is_some_and(|state| {
                let current = state.current.clone();
                state.drill_back(current.as_deref())
            });
        if changed {
            self.sync_workspaces_choices();
        }
        changed
    }

    /// DIALOG-14: Enter arms, second Enter deletes. Returns the removed
    /// directory when the caller must drop it from re-enumeration.
    pub fn workspaces_enter(&mut self) -> Option<String> {
        if self.phase != Phase::Open {
            return None;
        }
        let removed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .and_then(|state| {
                let current = state.current.clone();
                state.press_enter(current.as_deref())
            });
        if removed.is_some() {
            self.sync_workspaces_choices();
        }
        removed
    }

    /// DIALOG-14: apply a late re-enumeration. A removed root stays removed
    /// and a stale generation is dropped (`LateLoadGuard.apply`).
    pub fn workspaces_apply_load(&mut self, generation: u64, directories: Vec<String>) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let applied = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.workspaces.as_mut())
            .is_some_and(|state| {
                let current = state.current.clone();
                state.apply_load(generation, directories, current.as_deref())
            });
        if applied {
            self.sync_workspaces_choices();
        }
        applied
    }

    // ---- DIALOG-17: the prompt-stash dialog ----

    /// DIALOG-17: open the stash dialog from the live `PromptStash` list
    /// (the `prompt.stash.list` run replacing the stack with `DialogStash`,
    /// `prompt/index.tsx:918-940`). `now_ms` anchors the relative-time
    /// descriptions the way `Date.now()` does upstream.
    pub fn open_stash(
        &mut self,
        entries: Vec<crate::native_paste::StashEntry>,
        now_ms: i64,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        self.dialog = Some(OpenDialog {
            id: "stash".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Stash,
            title: "Stash".to_owned(),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            select: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: Some(StashDialogState {
                entries,
                armed: crate::native_palette::pickers::ArmedDelete::default(),
                now_ms,
            }),
            integration: None,
            form: None,
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        self.sync_stash_choices();
        Ok(())
    }

    /// DIALOG-17: the live stash state, or `None` when the dialog is closed.
    pub fn stash_state(&self) -> Option<&StashDialogState> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Stash)
            .and_then(|dialog| dialog.stash.as_ref())
    }

    /// DIALOG-17: rebuild the painted stash rows from the tested
    /// `native_palette::pickers::stash_rows` model (`dialog-stash.tsx:37-56`):
    /// the armed `Press ctrl+d again to confirm` title swap, the
    /// `~N lines` footers and the upstream `.toReversed()` ordering with the
    /// original entry index kept as both option id and dialog selection.
    fn sync_stash_choices(&mut self) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        if dialog.kind != DialogKind::Stash {
            return;
        }
        let Some(state) = dialog.stash.as_ref() else { return };
        let len = state.entries.len();
        let to_delete = state
            .armed
            .armed()
            .and_then(|armed| armed.parse::<usize>().ok());
        // Bridge the client-local store entries into the picker row model
        // (`From<native_paste::StashEntry>`), keeping the two types separate.
        let bridged: Vec<crate::native_palette::pickers::StashEntry> =
            state.entries.iter().cloned().map(Into::into).collect();
        // ponytail: upstream falls back to `Locale.datetime(timestamp)` for
        // entries older than 7 days; the native dialog has no datetime
        // renderer yet, so the description is empty there (upgrade to a
        // native clock format when one lands).
        let rows = crate::native_palette::pickers::stash_rows(
            &bridged,
            to_delete,
            state.now_ms,
            "",
            "ctrl+d",
        );
        dialog.choices = rows.iter().map(|row| row.title.clone()).collect();
        dialog.disabled = vec![false; dialog.choices.len()];
        dialog.select = Some(
            crate::native_palette::select::SelectDialog::new(
                rows.iter()
                    .map(|row| {
                        let mut option = crate::native_palette::select::SelectOption::new(
                            row.value.to_string(),
                            row.title.clone(),
                        );
                        if !row.description.is_empty() {
                            option = option.with_description(row.description.clone());
                        }
                        if let Some(footer) = &row.footer {
                            option = option.with_footer(footer.clone());
                        }
                        option
                    })
                    .collect(),
            ),
        );
        let selection = match self.selection {
            Some(index) if index < len => index,
            _ => len.saturating_sub(1),
        };
        self.selection = Some(selection);
        // Stash rows are displayed in reverse order, but their option ids are
        // original entry indexes. Keep the select model's painted highlight
        // on that same original-index identity.
        if let Some(select) = dialog.select.as_mut() {
            let _ = select.select_id(&selection.to_string());
        }
    }

    /// DIALOG-17: move the highlight; the upstream `onMove` disarms a
    /// pending delete (`dialog-stash.tsx:57-60`).
    pub fn stash_move(&mut self, delta: isize) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let Some(state) = self
            .dialog
            .as_mut()
            .filter(|dialog| dialog.kind == DialogKind::Stash)
            .and_then(|dialog| dialog.stash.as_mut())
        else {
            return false;
        };
        let len = state.entries.len();
        if len == 0 {
            return false;
        }
        state.armed.clear();
        // The display order is the reversed entry list; selection stays the
        // original index so painted rows, clicks and restore agree.
        let selected = match self.selection {
            Some(index) if index < len => len - 1 - index,
            _ => 0,
        };
        let next = crate::native_palette::select::move_selection(
            selected,
            len,
            delta,
            crate::native_palette::select::MovePolicy::Wrap,
        );
        self.selection = Some(len - 1 - next);
        if let Some(dialog) = self.dialog.as_mut() {
            if let Some(select) = dialog.select.as_mut() {
                let _ = select.select_id(&self.selection.unwrap().to_string());
            }
        }
        true
    }

    /// DIALOG-17: `stash.delete` two-press (`dialog-stash.tsx:66-78`): the
    /// first press arms the focused entry, the second removes it. Removing
    /// the last entry closes the dialog like an emptied upstream list.
    pub fn stash_delete(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let Some(index) = self.selection else { return false };
        let Some(state) = self
            .dialog
            .as_mut()
            .filter(|dialog| dialog.kind == DialogKind::Stash)
            .and_then(|dialog| dialog.stash.as_mut())
        else {
            return false;
        };
        if index >= state.entries.len() {
            return false;
        }
        if state.armed.press(&index.to_string()) {
            let old_len = state.entries.len();
            let display = old_len.saturating_sub(1).saturating_sub(index);
            state.entries.remove(index);
            state.armed.clear();
            let new_len = state.entries.len();
            if new_len == 0 {
                self.close();
            } else {
                // Keep the same visual row focused after the shift.
                self.selection = Some(new_len.saturating_sub(1).saturating_sub(display));
                self.sync_stash_choices();
            }
        } else {
            self.sync_stash_choices();
        }
        true
    }

    /// DIALOG-17: Enter restore (`dialog-stash.tsx:62-65`): the focused
    /// entry is removed from the stash and returned so the loop loads it
    /// into the composer; upstream then closes the dialog (`dialog.clear()`).
    pub fn stash_enter(&mut self) -> Option<crate::native_paste::StashEntry> {
        if self.phase != Phase::Open {
            return None;
        }
        let index = self.selection?;
        let state = self
            .dialog
            .as_mut()
            .filter(|dialog| dialog.kind == DialogKind::Stash)
            .and_then(|dialog| dialog.stash.as_mut())?;
        if index >= state.entries.len() {
            return None;
        }
        let entry = state.entries.remove(index);
        self.close();
        Some(entry)
    }

    // ---- DIALOG-15: the integration sign-in surface ----

    /// DIALOG-15: open the integration surface for the focused server with
    /// `IntegrationPhase::entry` initial state (`dialog-integration.tsx`
    /// replace at `dialog-mcp.tsx:104`). `methods`/`accounts` are the counts
    /// the (missing) sign-in producer would supply; zero methods meaningfully
    /// yields `IntegrationPhase::EnvironmentOnly`, matching upstream's
    /// disabled connect.
    pub fn open_integration(
        &mut self,
        server: &str,
        methods: usize,
        accounts: usize,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        let state = crate::native_palette::pickers::IntegrationState::new(server, methods, accounts);
        self.dialog = Some(OpenDialog {
            id: "integration".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Integration,
            title: sanitize_inline(&format!("Sign in: {server}")),
            choices: vec![],
            disabled: vec![],
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: vec![sanitize_inline(
                state.phase.phase_label().unwrap_or("connect"),
            )],
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: Some(state),
            form: None,
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        Ok(())
    }

    /// DIALOG-15: the live integration state, or `None` when closed.
    #[must_use]
    pub fn integration_state(
        &self,
    ) -> Option<&crate::native_palette::pickers::IntegrationState> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Integration)
            .and_then(|dialog| dialog.integration.as_ref())
    }

    // ---- FORM-003..FORM-008: the live question/form overlay ----

    /// FORM-003: open the question/form overlay for a session-scoped pending
    /// form (`routes/session/form.tsx:59-1175`). The tested
    /// [`form_overlay::FormPrompt`] is the single source of truth; the live key
    /// gate routes [`form_overlay::FormKey`] into it and the resolved answer map
    /// is dispatched to `POST /api/session/{s}/form/{id}/reply`. Content-only in
    /// the modal sense like [`DialogView::open_sessions`]: it never pends a
    /// permission ack.
    pub fn open_form(
        &mut self,
        id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        message: Option<String>,
        fields: Vec<form_overlay::FormField>,
    ) -> Result<(), DialogError> {
        self.open_form_prompt(
            session_id.into(),
            form_overlay::FormPrompt::new(id, title, message, fields),
        )
    }

    /// FORM-008: reopen the overlay from a stashed draft keyed by form id
    /// (`form.tsx:44-57,77-87`). The draft is consumed by the caller once the
    /// prompt is mounted.
    pub fn open_form_restored(
        &mut self,
        id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        message: Option<String>,
        fields: Vec<form_overlay::FormField>,
        draft: form_overlay::FormDraft,
    ) -> Result<(), DialogError> {
        self.open_form_prompt(
            session_id.into(),
            form_overlay::FormPrompt::restore(id, title, message, fields, draft),
        )
    }

    fn open_form_prompt(
        &mut self,
        session_id: String,
        form: form_overlay::FormPrompt,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        let id = form.id().to_owned();
        if id.len() > MAX_ID_LEN {
            return Err(DialogError::IdTooLong);
        }
        let title = form.title().to_owned();
        if title.len() > MAX_TITLE_LEN {
            return Err(DialogError::TitleTooLong);
        }
        self.dialog = Some(OpenDialog {
            id,
            session_id,
            kind: DialogKind::Form,
            title: sanitize_inline(&title),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: Some(form),
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        Ok(())
    }

    /// FORM-003..FORM-008: the live form model, or `None` when no form overlay
    /// is open.
    #[must_use]
    pub fn form(&self) -> Option<&form_overlay::FormPrompt> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Form)
            .and_then(|dialog| dialog.form.as_ref())
    }

    fn form_mut(&mut self) -> Option<&mut form_overlay::FormPrompt> {
        self.dialog
            .as_mut()
            .filter(|dialog| dialog.kind == DialogKind::Form)
            .and_then(|dialog| dialog.form.as_mut())
    }

    /// The open form's session identity, so the caller can refuse a reply to a
    /// form that belongs to another session (fail closed).
    #[must_use]
    pub fn form_session_id(&self) -> Option<&str> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Form)
            .map(|dialog| dialog.session_id.as_str())
    }

    /// FORM-003..FORM-008: route one key into the live form model
    /// (`routes/session/form.tsx:642-765`). Returns whether the key was
    /// consumed. An inactive/closed prompt consumes nothing, so transcript keys
    /// never leak through the overlay (`form.tsx:66,536-553`).
    pub fn form_handle_key(&mut self, key: form_overlay::FormKey) -> bool {
        self.form_mut().is_some_and(|form| form.handle_key(key))
    }

    /// FORM-005: route a paste into the live form (`form.tsx:354-379`). The
    /// model decides whether the text lands in the custom choice row or the
    /// editor; while a form is open the overlay owns the paste either way
    /// (returns whether a live form consumed it).
    pub fn form_paste(&mut self, text: &str) -> bool {
        self.form_mut()
            .map(|form| {
                let _ = form.paste(text);
                true
            })
            .unwrap_or(false)
    }

    /// FORM-008: whether the live form still owns focus. When false the caller
    /// restores composer focus (`FocusTarget`, `form.tsx:211-228`).
    #[must_use]
    pub fn form_active(&self) -> bool {
        self.form().is_some_and(form_overlay::FormPrompt::active)
    }

    /// FORM-003..FORM-007: resolve the form (`form.tsx:513-534`). `Some` on
    /// success; the error line is set on the model and stays open otherwise.
    pub fn form_submit(&mut self) -> Option<std::collections::BTreeMap<String, form_overlay::FormValue>> {
        self.form_mut().and_then(form_overlay::FormPrompt::submit)
    }

    /// FORM-008: dismiss the form, returning composer focus plus a draft when
    /// the reason is terminal/stale/revalidation-failed
    /// (`routes/session/form.tsx:44-57,211-228`). The overlay closes through the
    /// modal stack so paint and content never disagree.
    pub fn form_dismiss(
        &mut self,
        reason: form_overlay::DismissReason,
    ) -> Option<form_overlay::Dismissal> {
        let dismissal = self.form_mut().map(|form| form.dismiss(reason))?;
        self.reset();
        Some(dismissal)
    }

    /// TOOL-011: open the live execute content viewer
    /// (`dialog-execute.tsx:28`), opened from an execute tool row click
    /// (`index.tsx:3400-3414` `openDetails`). Content-only like
    /// [`DialogView::open_mcp`]: no choices, no backend decision, no footer.
    /// The caller closes it with [`DialogView::close`] and scrolls the body
    /// with [`DialogView::set_execute_scroll`].
    pub fn open_diff_viewer(&mut self, patch: String, patch_pane_width: usize) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        let preference = crate::native_diff_view::DiffViewPreference::Auto;
        // DIFF-001: keep the resolved mode on the dialog so the live render
        // arm branches on the stored fact (`diff-viewer.tsx:45,349`: viewer
        // split rule — never split below 100 columns, split at 100+).
        let mode = crate::native_diff_view::resolve_view_mode(
            crate::native_diff_view::DiffSurface::Viewer,
            preference,
            patch_pane_width,
        );
        self.dialog = Some(OpenDialog {
            id: "diff-viewer".to_owned(),
            session_id: String::new(),
            kind: DialogKind::DiffViewer,
            title: "Diff viewer".to_owned(),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: patch.lines().map(str::to_owned).collect(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: Some(mode),
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        self.modal.replace(self.open_sequence, DialogSize::XLarge, true, None, self.focus);
        Ok(())
    }

    pub fn open_execute(
        &mut self,
        model: crate::tui_entry::native_turn::native_tool_rows::ExecuteDialog,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        self.dialog = Some(OpenDialog {
            id: "execute".to_owned(),
            session_id: String::new(),
            kind: DialogKind::Execute,
            title: "Execute".to_owned(),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: Vec::new(),
            error: StaticError::BackendUnavailable,
            prompt: None,
            footer_gap: None,
            session_has_parent: false,
            execute: Some(model),
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        // `dialog.setSize("xlarge")` + `setCentered(true)`
        // (`dialog-execute.tsx:43-45`).
        self.modal
            .replace(entry_id, crate::native_modal_layer::DialogSize::XLarge, true, None, self.focus);
        Ok(())
    }

    /// TOOL-011: (current scroll offset, maximum scroll offset) of the open
    /// execute body under `width` x `height` layout cells. `(0, 0)` when no
    /// execute dialog is open. The key handler clamps its next value against
    /// `max` exactly like upstream `scroll?.scrollBy`
    /// (`dialog-execute.tsx:76-89` keymap).
    #[must_use]
    pub fn execute_scroll_state(&self, width: usize, height: usize) -> (usize, usize) {
        let Some(dialog) = self.dialog.as_ref() else { return (0, 0); };
        if dialog.kind == DialogKind::DiffViewer {
            // DIFF-003: the virtualized diff body scrolls in rendered rows;
            // the chunk window (`visible_chunk_range`) derives from this row
            // offset, exactly like upstream ScrollBox scrollTop
            // (`patch-diff.tsx:164-181`). Non-virtualized bodies do not
            // scroll.
            let patch = dialog.message_lines.join("\n");
            let Some(virtual_patch) = crate::native_diff_view::virtual_added_patch(&patch, true) else { return (0, 0); };
            let viewport = height.saturating_sub(2).max(1);
            // DIFF-001 review fix: clamp against the real painted row count,
            // not `chunks * VIRTUAL_CHUNK_LINES` — the final chunk is partial,
            // so the chunk-product over-counts by up to
            // `VIRTUAL_CHUNK_LINES - 1` rows and End would scroll into blank
            // space.
            let max = virtual_patch.total_rows.saturating_sub(viewport);
            return (dialog.execute_scroll.min(max), max);
        }
        if dialog.kind != DialogKind::Execute {
            return (0, 0);
        }
        let Some(execute) = dialog.execute.as_ref() else { return (0, 0); };
        let total = execute_body_rows(width, execute).len();
        // Viewport = body rows after the title and status lines.
        let viewport = height.saturating_sub(2).max(1);
        let max = total.saturating_sub(viewport);
        (dialog.execute_scroll.min(max.saturating_add(1)).min(max), max)
    }

    /// TOOL-011: move the execute body window the way upstream
    /// `scroll?.scrollBy` does; clamped by the caller against
    /// [`DialogView::execute_scroll_state`].
    pub fn set_execute_scroll(&mut self, offset: usize) {
        if let Some(dialog) = self.dialog.as_mut() {
            if dialog.kind == DialogKind::Execute {
                dialog.execute_scroll = offset.min(MAX_EXECUTE_BODY_ROWS.saturating_sub(1));
            } else if dialog.kind == DialogKind::DiffViewer {
                // DIFF-003: scroll-aware virtualization — the render arm
                // derives the chunk window from this row offset
                // (`patch-diff.tsx:164-181`); the state reader clamps to the
                // virtualized row count, so no extra cap here.
                dialog.execute_scroll = offset;
            }
        }
    }

    /// TOOL-011: the text a copy action writes — `code` selects the code block
    /// (`c`) and `output` the output block (`o`), mirroring
    /// `dialog-execute.tsx:96-104`. `None` when no execute dialog is open or
    /// the requested block is empty (upstream `copy` early-returns on an empty
    /// value, `:96-99`).
    #[must_use]
    pub fn execute_copy_text(&self, code: bool) -> Option<String> {
        let dialog = self.dialog.as_ref()?;
        if dialog.kind != DialogKind::Execute {
            return None;
        }
        let execute = dialog.execute.as_ref()?;
        let text = if code {
            execute.code.as_str()
        } else {
            execute.output.as_deref().unwrap_or("")
        };
        (!text.is_empty()).then(|| text.to_owned())
    }

    /// The visible dialog's kind, or `None` while closed. Used by the live
    /// loop to route Enter/Esc for a local (non-decision) dialog such as the
    /// MCP server list before calling [`DialogView::answer`].
    #[must_use]
    pub fn open_kind(&self) -> Option<DialogKind> {
        self.dialog.as_ref().map(|dialog| dialog.kind)
    }

    /// DIFF-001 review fix: whether the open dialog is a read-only content
    /// viewer that a backdrop mouse-up closes locally (`ui/dialog.tsx:54-60`)
    /// instead of emitting a decision intent. The diff viewer joins the MCP
    /// list and select surfaces here; its `cancel()` is deliberately inert, so
    /// the backdrop must close it through [`DialogView::close`] like Esc does.
    #[must_use]
    pub fn backdrop_closes_locally(&self) -> bool {
        matches!(
            self.open_kind(),
            Some(DialogKind::Mcp) | Some(DialogKind::Select) | Some(DialogKind::DiffViewer)
        )
    }

    /// DIALOG-10: open a live `DialogPrompt` text row (`ui/dialog-prompt.tsx`)
    /// with the tested `native_palette::dialog::PromptState` model. Content-only
    /// like [`DialogView::open_mcp`]: it bypasses the non-empty-choices check
    /// (a prompt has no choice list) and never pends a permission ack. The
    /// caller drives the text through the `prompt_*` forwarders and closes the
    /// dialog on submit/cancel.
    pub fn open_prompt(
        &mut self,
        id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        initial_value: &str,
        message: Option<String>,
    ) -> Result<(), DialogError> {
        if self.is_open() {
            return Err(DialogError::AlreadyOpen);
        }
        let id = id.into();
        let title = title.into();
        if id.len() > MAX_ID_LEN {
            return Err(DialogError::IdTooLong);
        }
        if title.len() > MAX_TITLE_LEN {
            return Err(DialogError::TitleTooLong);
        }
        if message.as_ref().is_some_and(|m| m.len() > MAX_MESSAGE_LEN) {
            return Err(DialogError::MessageTooLong);
        }
        self.dialog = Some(OpenDialog {
            id,
            session_id: session_id.into(),
            kind: DialogKind::Prompt,
            title: sanitize_inline(&title),
            choices: Vec::new(),
            disabled: Vec::new(),
            permission: Vec::new(),
            select: None,
            mcp_status: Vec::new(),
            message_lines: message
                .as_deref()
                .map(sanitize_block)
                .unwrap_or_default(),
            error: StaticError::BackendUnavailable,
            prompt: Some(crate::native_palette::select::PromptState::open(
                initial_value,
            )),
            // `dialog-prompt.tsx:133` passes `footerGap={2}`.
            footer_gap: Some(2),
            session_has_parent: false,
            execute: None,
            execute_scroll: 0,
            diff_mode: None,
            sessions: None,
            workspaces: None,
            stash: None,
            integration: None,
            form: None,
            description: None,
        });
        self.selection = None;
        self.rejected = false;
        self.phase = Phase::Open;
        self.open_sequence = self.open_sequence.saturating_add(1);
        let entry_id = self.open_sequence;
        self.modal
            .replace(entry_id, DialogSize::Medium, false, None, self.focus);
        Ok(())
    }

    /// DIALOG-10: the live prompt's current value, or `None` when no prompt is
    /// open.
    #[must_use]
    pub fn prompt_value(&self) -> Option<&str> {
        self.dialog
            .as_ref()
            .and_then(|dialog| dialog.prompt.as_ref())
            .map(crate::native_palette::select::PromptState::value)
    }

    /// DIALOG-10: append one character to the live prompt. Returns whether the
    /// prompt accepted it (busy/blurred/control input is refused).
    pub fn prompt_push(&mut self, ch: char) -> bool {
        self.dialog
            .as_mut()
            .and_then(|dialog| dialog.prompt.as_mut())
            .is_some_and(|prompt| prompt.push(ch))
    }

    /// DIALOG-10: delete the last character of the live prompt.
    pub fn prompt_backspace(&mut self) -> bool {
        self.dialog
            .as_mut()
            .and_then(|dialog| dialog.prompt.as_mut())
            .is_some_and(|prompt| prompt.backspace())
    }

    /// DIALOG-10: the value Enter would submit, or `None` while busy / closed.
    #[must_use]
    pub fn prompt_submit(&self) -> Option<String> {
        self.dialog
            .as_ref()
            .and_then(|dialog| dialog.prompt.as_ref())
            .and_then(crate::native_palette::select::PromptState::submit)
            .map(str::to_owned)
    }

    /// DIALOG-10: whether Escape may cancel the live prompt (suppressed while
    /// busy, `dialog-prompt.tsx:93-100`).
    #[must_use]
    pub fn prompt_escape(&self) -> bool {
        self.dialog
            .as_ref()
            .and_then(|dialog| dialog.prompt.as_ref())
            .is_some_and(crate::native_palette::select::PromptState::escape)
    }

    /// DIALOG-10: mark the live prompt busy (suspends editing, shows the busy
    /// line) while a submit is in flight.
    pub fn prompt_set_busy(&mut self, busy: bool) {
        if let Some(prompt) = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.prompt.as_mut())
        {
            prompt.set_busy(busy);
        }
    }

    /// DIALOG-10: replace the live prompt's supplementary message (the inline
    /// error line under the input). `None` clears it.
    pub fn prompt_set_message(&mut self, message: Option<String>) {
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.message_lines = message
                .as_deref()
                .map(sanitize_block)
                .unwrap_or_default();
        }
    }

    /// The focused choice's display text, or `None` when closed/unselected.
    /// For [`DialogKind::Mcp`] this is the focused server name
    /// (`dialog-mcp.tsx:81-84` `option.value`).
    ///
    /// A live `DialogSelect` owns a filtered model whose highlight can move
    /// without the caller (`set_filter`, `pointer_move`, a suppressed move
    /// while locked). The model is therefore the single source of truth for a
    /// select: this reads it directly instead of the mirrored
    /// [`DialogView::selection`], so the row that answers can never differ
    /// from the row the model (and the painted marker) shows.
    #[must_use]
    pub fn focused_choice(&self) -> Option<&str> {
        let dialog = self.dialog.as_ref()?;
        let index = match dialog.select.as_ref() {
            Some(_) => Self::select_index(dialog)?,
            None => self.selection?,
        };
        dialog.choices.get(index).map(String::as_str)
    }

    /// The highlighted original choice index reported by the live select
    /// model, or `None` for a dialog without one. Option ids are the original
    /// choice indexes (`open_request`), so a filtered highlight still maps to
    /// the choice the user actually picked.
    fn select_index(dialog: &OpenDialog) -> Option<usize> {
        dialog
            .select
            .as_ref()
            .and_then(|select| select.selected_option())
            .and_then(|option| option.id.parse::<usize>().ok())
    }

    /// `dialog.show()` followed by `dialog.set()` (`plugin/api.tsx:334-341`):
    /// open through [`DialogView::open_request`], then apply the requested
    /// presentation. This is the form to use when the caller already knows the
    /// size, because `replace()` resets the store (`ui/dialog.tsx:406-408`).
    pub fn open_request_sized(
        &mut self,
        request: DialogRequest,
        size: DialogSize,
        centered: bool,
    ) -> Result<(), DialogError> {
        self.open_request(request)?;
        self.set_dialog_size(size);
        self.set_centered(centered);
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
        // Compatibility helper for local/test callers that predate the
        // save-aware helper: it exposes the full six-label mechanics model
        // (`permission_alias` recognizes all six scoped labels). The live
        // permission poll uses `open_permission_with_persistent`, which now
        // emits the exact three-option upstream set (`permission.tsx:232-240`).
        self.open_request(DialogRequest {
            id: request_id.into(),
            session_id: session_id.into(),
            kind: DialogKind::Permission,
            title: title.into(),
            choices: vec![
                "Allow once".to_owned(),
                "Deny once".to_owned(),
                "Allow for this session".to_owned(),
                "Always allow".to_owned(),
                "Deny for this session".to_owned(),
                "Always deny".to_owned(),
            ],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
    }

    /// Open a permission prompt with the persistent-grant choice controlled
    /// by the backend request's validated `save` capability. Exact upstream
    /// option set and order (`permission.tsx:232-240`): `once`, `always`
    /// (only when the request carries `save` rules), `reject`
    /// (`permissionOptionLabel`, `util/permission.ts:162-168`). The save gate
    /// (`props.request.save?.length`) drops **only** the `always` option, so a
    /// save-ineligible request keeps `once` + `reject`.
    pub fn open_permission_with_persistent(
        &mut self,
        request_id: impl Into<String>,
        session_id: impl Into<String>,
        title: impl Into<String>,
        persistent_allowed: bool,
    ) -> Result<(), DialogError> {
        let mut choices = vec!["Allow once".to_owned()];
        if persistent_allowed {
            choices.push("Always allow".to_owned());
        }
        choices.push("Reject".to_owned());
        self.open_request(DialogRequest {
            id: request_id.into(),
            session_id: session_id.into(),
            kind: DialogKind::Permission,
            title: title.into(),
            choices,
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
    }

    /// F002-R3: the number of options on an OPEN permission prompt, or `None`
    /// when the open dialog is not a permission request.
    ///
    /// Upstream registers `left,h` / `right,l` only when `keys.length > 1`
    /// (`routes/session/permission.tsx:465-485`), so the live dispatch needs
    /// the option count to gate the cycle.
    #[must_use]
    pub fn permission_option_count(&self) -> Option<usize> {
        self.dialog
            .as_ref()
            .filter(|dialog| dialog.kind == DialogKind::Permission)
            .map(|dialog| dialog.choices.len())
    }

    /// F002-R3: cycle the permission options left/right, exactly the upstream
    /// guard. A permission prompt with a single option does NOT move
    /// (`keys.length > 1`, `permission.tsx:465-485`); a non-permission dialog
    /// never cycles here. Returns whether a repaint is needed.
    pub fn permission_cycle(&mut self, forward: bool) -> bool {
        match self.permission_option_count() {
            Some(count) if count > 1 => {
                self.step(forward);
                true
            }
            _ => false,
        }
    }

    /// F002-R2: mark the open permission request as belonging to a child
    /// session (`session.parentID`). Only a child session opens the nested
    /// rejection prompt (`permission.tsx:243-254`); a root session replies
    /// immediately.
    pub fn set_permission_session_has_parent(&mut self, has_parent: bool) {
        if let Some(dialog) = self.dialog.as_mut() {
            if dialog.kind == DialogKind::Permission {
                dialog.session_has_parent = has_parent;
            }
        }
    }

    /// F002-R2: true while the nested rejection-reason editor is open and owns
    /// input. False once the decision is pending (the stage is retained only so
    /// a rejected ACK can restore the permission card).
    #[must_use]
    pub fn reject_stage(&self) -> bool {
        self.reject_stage.is_some() && self.phase == Phase::Open
    }

    /// F002-R2: true while the rejection stage is retained, whether the editor
    /// is open or its decision is pending. The live input loop uses this to
    /// keep the stage's keys from reaching the permission byte dispatch.
    #[must_use]
    pub fn reject_stage_retained(&self) -> bool {
        self.reject_stage.is_some()
    }

    /// F002-R2: shared entry into the nested rejection stage. Requires an open
    /// child-session Permission request; preserves the current selection so a
    /// cancel restores the card verbatim. Both the `reject`-row path
    /// ([`Self::begin_reject_stage`]) and the Escape path
    /// ([`Self::escape_reject_stage`]) route here, matching upstream's single
    /// `onSelect("reject")` (`permission.tsx:241,437`).
    fn open_reject_stage(&mut self) -> bool {
        if self.reject_stage.is_some() || self.phase != Phase::Open {
            return false;
        }
        let Some(dialog) = self.dialog.as_ref() else {
            return false;
        };
        if dialog.kind != DialogKind::Permission || !dialog.session_has_parent {
            return false;
        }
        // Reconstruct the original card so cancel can restore it verbatim.
        let request = DialogRequest {
            id: dialog.id.clone(),
            session_id: dialog.session_id.clone(),
            kind: DialogKind::Permission,
            title: dialog.title.clone(),
            choices: dialog.choices.clone(),
            message: (!dialog.message_lines.is_empty())
                .then(|| dialog.message_lines.join("\n")),
            disabled: dialog
                .disabled
                .iter()
                .enumerate()
                .filter_map(|(index, disabled)| disabled.then_some(index))
                .collect(),
            error: None,
            description: None,
        };
        let selection = self.selection;
        self.reset();
        // The reason editor reuses the tested `DialogPrompt` text model; its
        // dialog id is the permission request id so the ACK correlation still
        // matches (`permission.tsx:284-307`).
        let opened = self.open_prompt(
            request.id.clone(),
            request.session_id.clone(),
            "Reject permission",
            "",
            Some("Tell OpenCode what to do differently".to_owned()),
        );
        if opened.is_err() {
            return false;
        }
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.session_has_parent = true;
        }
        self.reject_stage = Some(RejectStage { request, selection });
        true
    }

    /// F002-R2: enter the nested rejection stage when the selected permission
    /// option is `reject` and the request belongs to a child session
    /// (`permission.tsx:144-155,243-254`). Returns true when the stage was
    /// entered; the caller must NOT dispatch a reply in that case. A root
    /// session (or a non-reject selection) returns false so the caller replies
    /// immediately.
    pub fn begin_reject_stage(&mut self) -> bool {
        let Some(dialog) = self.dialog.as_ref() else {
            return false;
        };
        if dialog.kind != DialogKind::Permission || !dialog.session_has_parent {
            return false;
        }
        let Some(index) = self.selection else {
            return false;
        };
        if dialog.permission.get(index).copied() != Some(PermissionChoice::Reject) {
            return false;
        }
        self.open_reject_stage()
    }

    /// F002-R2: the Escape path. Upstream `escapeKey="reject"` routes Escape
    /// through the same `onSelect("reject")` as the reject row
    /// (`permission.tsx:241,437`), so for a child session Escape opens the
    /// rejection-reason editor regardless of which row is highlighted. Returns
    /// true when the stage was opened; the caller must NOT dispatch a reply in
    /// that case (a root session returns false so the caller rejects
    /// immediately).
    pub fn escape_reject_stage(&mut self) -> bool {
        self.open_reject_stage()
    }

    /// F002-R2: confirm the nested rejection. Emits a `reject` intent carrying
    /// the typed reason (omitted when empty) and holds the decision pending
    /// (`permission.tsx:148-153,300-306`).
    pub fn confirm_reject_stage(&mut self) -> Result<DecisionIntent, DialogError> {
        // Keep the stage set through the pending decision: a rejected ACK
        // restores the permission card from it (`ack`). The editor is only
        // "active" while the dialog is Open, so `reject_stage()` stays false
        // once the decision is in flight.
        let Some((request_id, session_id)) = self
            .reject_stage
            .as_ref()
            .map(|stage| (stage.request.id.clone(), stage.request.session_id.clone()))
        else {
            return Err(DialogError::NotOpen);
        };
        let message = self.prompt_submit().filter(|reason| !reason.is_empty());
        let attempt = self.next_attempt()?;
        self.pending_attempt = attempt;
        self.phase = Phase::Pending;
        Ok(DecisionIntent::Reject {
            request_id,
            session_id,
            attempt,
            message,
        })
    }

    /// F002-R2: cancel the nested rejection stage, restoring the permission
    /// card and its selection (`permission.tsx:299-302`). Returns whether a
    /// stage was actually cancelled.
    pub fn cancel_reject_stage(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let Some(stage) = self.reject_stage.take() else {
            return false;
        };
        self.reset();
        if self.open_request(stage.request).is_ok() {
            if let Some(dialog) = self.dialog.as_mut() {
                dialog.session_has_parent = true;
            }
            self.selection = stage.selection;
            true
        } else {
            false
        }
    }

    /// F002-R2: the rejection reason currently typed, or `""`.
    #[must_use]
    pub fn reject_reason(&self) -> &str {
        self.prompt_value().unwrap_or("")
    }

    /// F002-R2: clear a non-empty rejection reason (Ctrl-C,
    /// `permission.tsx:288-298`). Returns whether anything was cleared.
    pub fn reject_clear_reason(&mut self) -> bool {
        if self.reject_stage.is_none() {
            return false;
        }
        if self.prompt_value().is_some_and(|reason| reason.is_empty()) {
            return false;
        }
        self.prompt_clear()
    }

    /// F002-R2: reset the live prompt's text (Ctrl-C clears a non-empty
    /// rejection reason, `permission.tsx:288-298`).
    pub fn prompt_clear(&mut self) -> bool {
        self.dialog
            .as_mut()
            .and_then(|dialog| dialog.prompt.as_mut())
            .is_some_and(crate::native_palette::select::PromptState::clear)
    }

    /// Move the selection forward, wrapping and skipping disabled choices.
    /// Ignored while pending (the decision is already in flight) and when
    /// no choice is enabled (selection stays unset).
    pub fn next(&mut self) {
        self.step(true);
    }

    pub fn select(&mut self, index: usize) -> bool {
        if self.phase != Phase::Open { return false; }
        let Some(dialog) = self.dialog.as_mut() else { return false; };
        if index >= dialog.choices.len() || dialog.disabled[index] { return false; }
        // A live select owns its own filtered highlight. A pointer click (or a
        // digit shortcut) must move the model too, otherwise the next Up/Down
        // steps from the stale model row and lands on an unrelated option.
        // Because option ids are the original choice indexes, the identity is
        // recovered exactly, and a row that is not currently visible refuses.
        let synced = match dialog.select.as_mut() {
            Some(select) => select.select_id(&index.to_string()),
            None => true,
        };
        if !synced { return false; }
        self.selection = Some(index);
        true
    }

    /// Move the selection backward, wrapping and skipping disabled choices.
    pub fn prev(&mut self) {
        self.step(false);
    }

    /// DIALOG-06: printable input extends the live select filter. The dialog
    /// selection follows the filtered highlight, so a confirm still answers
    /// with the ORIGINAL choice index (`answer` reads `self.selection`).
    pub fn select_push_filter(&mut self, ch: char) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.select.as_mut())
            .is_some_and(|select| select.push_filter(ch));
        if changed {
            self.sync_select_selection();
        }
        changed
    }

    /// DIALOG-06: Backspace removes one filter code point, never a partial
    /// multi-byte scalar.
    pub fn select_backspace(&mut self) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let changed = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.select.as_mut())
            .is_some_and(|select| select.pop_filter());
        if changed {
            self.sync_select_selection();
        }
        changed
    }

    /// DIALOG-08: Page movement is ten options (`PAGE_STEP`,
    /// `dialog-select.tsx:1590-1602`).
    pub fn select_page(&mut self, forward: bool) -> bool {
        if self.phase != Phase::Open {
            return false;
        }
        let moved = self
            .dialog
            .as_mut()
            .and_then(|dialog| dialog.select.as_mut())
            .is_some_and(|select| {
                if forward {
                    select.page_down()
                } else {
                    select.page_up()
                }
            });
        if moved {
            self.sync_select_selection();
        }
        moved
    }

    /// Mirror the filtered select's highlighted option back to the dialog
    /// selection. Option ids are original choice indexes.
    fn sync_select_selection(&mut self) {
        let index = self
            .dialog
            .as_ref()
            .and_then(|dialog| dialog.select.as_ref())
            .and_then(|select| select.selected_option())
            .and_then(|option| option.id.parse::<usize>().ok());
        // Assign unconditionally: a filter that matches nothing must clear the
        // selection. Keeping the previous row would leave `answer()` and
        // `focused_choice()` pointing at a row the user can no longer see, so
        // Enter would confirm a hidden choice.
        self.selection = index;
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
            DialogKind::Execute | DialogKind::Crash => return Err(DialogError::NotAnswerable),
            DialogKind::Mcp => return Err(DialogError::NotAnswerable),
            // DIFF-001 review fix: the diff viewer is a read-only content
            // surface with no answer intent; refuse before `next_attempt()`
            // so the `unreachable!` arm below is genuinely unreachable.
            DialogKind::DiffViewer => return Err(DialogError::NotAnswerable),
            DialogKind::Sessions => return Err(DialogError::NotAnswerable),
            // A form is answered through [`DialogView::form_submit`] (the
            // resolved answer map), never the generic choice/answer path.
            DialogKind::Form => return Err(DialogError::NotAnswerable),
            // DIALOG-14/15/17: worktrees, sign-in and stash are driven by
            // their own forwarders (`workspaces_*`/`integration_*`/
            // `stash_*`), never this path.
            DialogKind::Workspaces | DialogKind::Integration | DialogKind::Stash => {
                return Err(DialogError::NotAnswerable)
            }
            DialogKind::Question if !self.question_capable => {
                return Err(DialogError::QuestionUnavailable)
            }
            _ => {}
        }
        // Confirm against the model, not a mirror of it: a live select's
        // highlight can move without going through `DialogView`, and the
        // confirmed row must always be the painted one.
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.kind == DialogKind::Select && dialog.select.is_some())
        {
            self.sync_select_selection();
            if self
                .dialog
                .as_ref()
                .and_then(|dialog| dialog.select.as_ref())
                .is_some_and(|select| select.is_locked())
            {
                return Err(DialogError::LockedSelection);
            }
        }
        let Some(dialog) = self.dialog.as_ref() else {
            return Err(DialogError::NotOpen);
        };
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
                    message: None,
                },
                // Defensive only: `open_request` refuses unmapped labels,
                // so this arm is unreachable — and even here the outcome
                // is an error, never an approve fallback.
                None => unreachable!("validated above"),
            },
            DialogKind::Question | DialogKind::Confirm | DialogKind::Select => DecisionIntent::Answer {
                request_id,
                session_id,
                choice_index: index,
                choice,
                attempt,
            },
            DialogKind::Prompt => return Err(DialogError::NotAnswerable),
            DialogKind::Mcp
            | DialogKind::Sessions
            | DialogKind::Form
            | DialogKind::Workspaces
            | DialogKind::Integration
            | DialogKind::Stash => {
                unreachable!("not answerable")
            }
            // `Crash` is refused as `NotAnswerable` by the guard above, so it
            // can never reach the intent construction.
            DialogKind::Execute | DialogKind::Error | DialogKind::Crash | DialogKind::DiffViewer => {
                unreachable!("answered above")
            }
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
            DialogKind::Execute | DialogKind::Crash => return Err(DialogError::NotAnswerable),
            DialogKind::Mcp => return Err(DialogError::NotAnswerable),
            // DIFF-001 review fix: cancel (backdrop mouse-up / Esc) on the
            // read-only diff viewer must be inert, not fall through to the
            // `unreachable!` arm. Refuse before `next_attempt()`.
            DialogKind::DiffViewer => return Err(DialogError::NotAnswerable),
            DialogKind::Sessions => return Err(DialogError::NotAnswerable),
            // A form cancel is the real form cancel route, dispatched by
            // [`DialogView::form_cancel`], never this generic path.
            DialogKind::Form => return Err(DialogError::NotAnswerable),
            // DIALOG-14/15/17: worktrees/sign-in/stash cancel through their
            // own forwarders, never this generic path.
            DialogKind::Workspaces | DialogKind::Integration | DialogKind::Stash => {
                return Err(DialogError::NotAnswerable)
            }
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
                message: None,
            },
            DialogKind::Confirm | DialogKind::Select => DecisionIntent::Cancel {
                request_id,
                session_id,
                attempt,
            },
            DialogKind::Prompt => return Err(DialogError::NotAnswerable),
            DialogKind::Question
            | DialogKind::Error
            | DialogKind::Execute
            | DialogKind::Crash
            | DialogKind::Mcp
            | DialogKind::DiffViewer
            | DialogKind::Sessions
            | DialogKind::Form
            | DialogKind::Workspaces
            | DialogKind::Integration
            | DialogKind::Stash => unreachable!("handled above"),
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
        if !self.is_open() {
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
            // F002-R2: a rejected ack on the nested rejection editor restores
            // the permission options so the retry path re-sends the decision
            // (the reason editor is not itself a decision surface). Every
            // other kind keeps the historical reopen-in-place behaviour.
            match self.reject_stage.take() {
                Some(stage) => {
                    self.reset();
                    let _ = self.open_request(stage.request);
                    self.selection = stage.selection;
                }
                None => self.selection = None,
            }
            self.phase = Phase::Open;
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
        // Visibility is decided by the modal stack, not by leftover content:
        // the provider paints only the top entry of a non-empty stack
        // (`packages/tui/src/ui/dialog.tsx:472-480`), so a dialog whose entry
        // was removed must paint nothing even before its content is dropped.
        if !self.modal.is_open() {
            return (Vec::new(), Vec::new(), None);
        }
        let mut hits = vec![None; height];
        let mut footer_row: Option<usize> = None;
        let mut lines = Vec::new();
        let line = |text, role| Line {text,role,detail_from:None};
        if let Some(dialog) = self.dialog.as_ref() {
            // Heading first: prioritized on short terminals. The new-designs
            // shell drops the title's bold weight and paints it in the base
            // text color (`ui/dialog.tsx:109`: `attributes={newDesigns() ?
            // undefined : TextAttributes.BOLD}`). The permission heading keeps
            // its Warning role because that is a security signal, not a
            // decoration the experiment toggles.
            lines.push(line(
                dialog.title.clone(),
                if dialog.kind == DialogKind::Permission {
                    Role::Warning
                } else if self.new_designs {
                    Role::Item
                } else {
                    Role::Heading
                },
            ));
            let roomy = height >= dialog.choices.len() + 7;
            if roomy {
                // DIALOG-02 (`ui/dialog.tsx:105-119`): the new-designs shell
                // paints the dialog's `description` as a faint line directly
                // under the title (`<Show when={newDesigns() && props.description}>`,
                // `fg={theme.text.faint}`); the legacy shell never shows one.
                // The row occupies the same slot as the legacy spacer, so the
                // footer position is unchanged whether or not text is present.
                if self.new_designs && dialog.description.is_some() {
                    lines.push(line(
                        dialog.description.clone().unwrap_or_default(),
                        Role::Description,
                    ));
                } else {
                    lines.push(line(String::new(), Role::Description));
                }
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
                DialogKind::Select => {
                    // DIALOG-06: the live filter is visible above the rows.
                    let select = dialog.select.as_ref();
                    let filter = select.map(|select| select.filter()).unwrap_or("");
                    let search_in_title = select.is_some_and(|select| {
                        crate::native_palette::select::search_in_title(
                            select.experiment(),
                            true,
                        )
                    });
                    if !search_in_title {
                        if filter.is_empty() {
                            lines.push(line(
                                "select: use up/down then Enter".to_string(),
                                Role::Description,
                            ));
                        } else {
                            lines.push(line(format!("select: {filter}"), Role::Item));
                        }
                    } else if !filter.is_empty() {
                        lines[0] = line(format!("select: {filter}"), Role::Item);
                    }
                }
                DialogKind::Prompt => {
                    // `dialog-prompt.tsx`: the input row shows the current value;
                    // while busy it is replaced by the spinner line
                    // (`dialog-prompt.tsx:100-140,168-170`).
                    match dialog.prompt.as_ref() {
                        Some(prompt) => match prompt.busy_line() {
                            Some(busy) => lines.push(line(busy.to_owned(), Role::Description)),
                            None if prompt.value().is_empty() => {
                                lines.push(line("  …".to_string(), Role::Description));
                            }
                            None => lines.push(line(format!("  {}", prompt.value()), Role::Item)),
                        },
                        None => lines.push(line("prompt".to_string(), Role::Description)),
                    }
                }
                DialogKind::Execute => {
                    // TOOL-011: the live execute body (`dialog-execute.tsx`):
                    // the status line, then the gutter-numbered code block,
                    // then the JSON-highlighted output, then the
                    // `Warnings:`/`Logs:` tail. Rows are windowed on
                    // `execute_scroll` inside the remaining body budget
                    // (upstream scrolls a `ScrollBox`, `dialog-execute.tsx:46`).
                    match dialog.execute.as_ref() {
                        Some(execute) => {
                            lines.push(line(
                                execute.status.clone(),
                                if execute.failed { Role::Error } else { Role::Description },
                            ));
                            let viewport = height.saturating_sub(lines.len()).max(1);
                            let body = execute_body_rows(width, execute);
                            let max = body.len().saturating_sub(viewport);
                            let start = dialog.execute_scroll.min(max);
                            let end = start.saturating_add(viewport);
                            for (row, role) in body.into_iter().take(end).skip(start) {
                                lines.push(line(row, role));
                            }
                        }
                        None => lines.push(line("execute — code and output".to_string(), Role::Description)),
                    }
                }
                DialogKind::Mcp => lines.push(line("select a server · Enter toggle".to_string(), Role::Description)),
                DialogKind::DiffViewer => {
                    lines.push(line("diff viewer · Esc to close".to_string(), Role::Description));
                    let patch = dialog.message_lines.join("\n");
                    // DIFF-001: branch on the stored open-time mode
                    // (`diff-viewer.tsx:45,349`): split renders the paired
                    // side-by-side columns, unified the single gutter column.
                    match dialog.diff_mode {
                        Some(crate::native_diff_view::DiffViewMode::Split) => {
                            let rendered = crate::native_diff_view::paint_rows_split(
                                &patch,
                                crate::native_diff_view::MAX_DIFF_LINES,
                                width,
                            );
                            for row in rendered.rows.iter() {
                                match row {
                                    crate::native_diff_view::SplitRow::Full(full) => {
                                        lines.push(line(full.content.clone(), Role::Item));
                                    }
                                    crate::native_diff_view::SplitRow::Pair { left, right } => {
                                        let mut text = String::new();
                                        for cell in left.iter().chain(right.iter()) {
                                            if !text.is_empty() {
                                                text.push(' ');
                                            }
                                            text.push(cell.sign);
                                            text.push(' ');
                                            text.push_str(&cell.text);
                                        }
                                        lines.push(line(text, Role::Item));
                                    }
                                }
                            }
                        }
                        _ => {
                        if let Some(virtual_patch) = crate::native_diff_view::virtual_added_patch(&patch, true) {
                            let (first, last) = crate::native_diff_view::visible_chunk_range(
                                virtual_patch.chunks.len(), dialog.execute_scroll, height.saturating_sub(lines.len()).max(1));
                            for (index, chunk) in virtual_patch.chunks.iter().enumerate() {
                                if index < first || index > last {
                                    lines.push(line(format!("… {} lines …", crate::native_diff_view::placeholder_height(chunk)), Role::Description));
                                } else {
                                    lines.extend(chunk.lines.iter().cloned().map(|row| line(row, Role::Item)));
                                }
                            }
                        } else {
                            lines.extend(dialog.message_lines.iter().cloned().map(|row| line(row, Role::Item)));
                        }
                        }
                    }
                }
                DialogKind::Sessions => {
                    // DIALOG-13: the live session picker (`dialog-session-list.tsx`).
                    // The filter line replaces the hint while typing
                    // (`searchInTitle`, `:206-209`); fetched rows support title
                    // search and directory scope. Delete remains unsupported.
                    match dialog.sessions.as_ref() {
                        Some(state) => {
                            let filter = state.picker.filter();
                            if filter.is_empty() {
                                lines.push(line(
                                    "search sessions".to_string(),
                                    Role::Description,
                                ));
                            } else {
                                lines.push(line(format!("search: {filter}"), Role::Item));
                            }
                            let scope = if state.picker.all_projects() { "all projects" } else { "current project" };
                            lines.push(line(format!("title filter · Ctrl+A: {scope}"), Role::Description));
                        }
                        None => lines.push(line(
                            "sessions · Enter open".to_string(),
                            Role::Description,
                        )),
                    }
                }
                DialogKind::Workspaces => {
                    // DIALOG-14: the live worktrees body
                    // (`dialog-workspaces.tsx`). Rows come from the tested
                    // `WorkspacesState`; the filter line replaces the hint
                    // while typing.
                    match dialog.workspaces.as_ref() {
                        Some(state) => {
                            if state.filter.is_empty() {
                                lines.push(line(
                                    "worktrees · Enter open · Del remove".to_string(),
                                    Role::Description,
                                ));
                            } else {
                                lines.push(line(format!("filter: {}", state.filter), Role::Item));
                            }
                        }
                        None => lines.push(line("worktrees".to_string(), Role::Description)),
                    }
                }
                DialogKind::Stash => {
                    // DIALOG-17: the live prompt-stash body
                    // (`dialog-stash.tsx`). Rows paint through the shared
                    // select loop below (reverse order, right-aligned
                    // `~N lines` footers); the hint documents the actions.
                    lines.push(line(
                        "stash · Enter restore · ctrl+d delete".to_string(),
                        Role::Description,
                    ));
                }
                DialogKind::Integration => {
                    // DIALOG-15: the live sign-in body
                    // (`dialog-integration.tsx`): server name plus the
                    // current phase's status line.
                    match dialog.integration.as_ref() {
                        Some(state) => {
                            lines.push(line(format!("sign in: {}", state.server), Role::Item));
                            lines.push(line(
                                state.phase.phase_label().unwrap_or("connect").to_string(),
                                Role::Description,
                            ));
                        }
                        None => lines.push(line("sign in".to_string(), Role::Description)),
                    }
                }
                DialogKind::Error => {
                    lines.push(line(dialog.error.headline().to_string(), Role::Error));
                    lines.push(line(dialog.error.detail().to_string(), Role::Description));
                }
                DialogKind::Form => {
                    // FORM-003..FORM-008: the live question/form body
                    // (`routes/session/form.tsx:767-1174`). `FormPrompt::render`
                    // already produces the exact upstream rows (title, message,
                    // tab bar/progress, external block, field rows incl. the
                    // custom row, review rows and footer); the dialog window
                    // shows them into the remaining body budget.
                    match dialog.form.as_ref() {
                        Some(form) => {
                            for row in form.render(width, height) {
                                if lines.len() >= height {
                                    break;
                                }
                                lines.push(line(row, Role::Item));
                            }
                        }
                        None => lines.push(line("form".to_string(), Role::Description)),
                    }
                }
                DialogKind::Crash => {
                    // DIALOG-20: the root panic overlay paints the full
                    // upstream layout (`error-component.tsx:117-221`) from
                    // the live model: subtext on tall terminals, the
                    // ` Error ` panel, the actions row with shortcut keys,
                    // the stack-trace panel and the footer hint + version.
                    // The shell's heading row above already carries the
                    // headline (`open_crash` stores `HEADLINE` as
                    // the dialog title).
                    if let Some(screen) = self.crash_screen.as_ref() {
                        // Subtext threshold (`error-component.tsx:122-127`).
                        if crash::CrashScreen::show_subtext(height as u16) {
                            lines.push(line(crash::SUBTEXT.to_owned(), Role::Description));
                        }
                        // Error message panel (`error-component.tsx:129-141`).
                        // The message row stays verbatim so capture tests can
                        // assert the exact payload row.
                        lines.push(line(" Error ".to_string(), Role::Error));
                        lines.push(line(screen.message().to_owned(), Role::Error));
                        // Actions row (`error-component.tsx:143-179`): labels
                        // with the selected action bracketed, each action's
                        // shortcut key centered beneath it.
                        let selected = screen.selected_action();
                        let mut labels = String::new();
                        let mut keys = String::new();
                        for action in crash::CrashAction::all() {
                            let label = action.label(screen.copy_state());
                            let label_width = label.chars().count();
                            let field = if action == selected {
                                format!("[ {label} ]")
                            } else {
                                format!("  {label}  ")
                            };
                            if !labels.is_empty() {
                                labels.push_str("  ");
                                keys.push_str("  ");
                            }
                            keys.push_str(&format!(
                                "{:^field_width$}",
                                action.key(),
                                field_width = field.chars().count()
                            ));
                            labels.push_str(&field);
                        }
                        lines.push(line(labels, Role::Action));
                        lines.push(line(keys, Role::Description));
                        // Stack trace panel (`error-component.tsx:183-202`):
                        // `stack_window` derives its own viewport from the
                        // total row height (`stack_viewport`, three-row
                        // minimum), while the shell budget clips to the
                        // rows left above the footer reservation.
                        let footer_rows =
                            usize::from(crash::CrashScreen::show_footer(height as u16)) * 2;
                        let stack_budget =
                            height.saturating_sub(lines.len() + 1 + footer_rows);
                        lines.push(line(" Stack trace ".to_string(), Role::Description));
                        for row in screen.stack_window(height as u16).into_iter().take(stack_budget)
                        {
                            lines.push(line(row.to_owned(), Role::Description));
                        }
                        // Footer (`error-component.tsx:204-216`): copy hint
                        // then version, only on tall terminals.
                        if footer_rows == 2 {
                            lines.push(line(screen.footer_hint().to_owned(), Role::Description));
                            lines.push(line(screen.version_line(), Role::Description));
                        }
                    }
                }
            }
            let has_footer = !matches!(dialog.kind, DialogKind::Error | DialogKind::Crash)
                && !matches!(dialog.kind, DialogKind::Execute)
                && !matches!(dialog.kind, DialogKind::Form)
                && !(dialog.kind == DialogKind::Question && !self.question_capable);
            let footer_rows = usize::from(has_footer && height > 1);
            let choice_rows = dialog.choices.len().min(height.saturating_sub(3)).max(1);
            let message_budget = height.saturating_sub(
                lines.len() + footer_rows + choice_rows + if roomy { 2 } else { 0 },
            );
            // DIFF-001 review fix: the diff viewer's `message_lines` holds the
            // patch, already painted by its own render arm above; the generic
            // two-space-prefixed block would paint the whole patch a second
            // time. Every other kind still uses this block.
            if dialog.kind != DialogKind::DiffViewer {
                for message in dialog.message_lines.iter().take(message_budget) {
                    lines.push(line(format!("  {message}"), Role::Item));
                }
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
            // DIALOG-06..09: a live select paints only its filtered rows; every
            // other dialog keeps the original choice order. Option ids are the
            // original choice indexes, so paint, click hits and confirm agree.
            let flattened = dialog.select.as_ref().is_some_and(|select| select.flatten());
            let visible: Vec<(usize, &str, bool, Option<&str>)> = match dialog.select.as_ref() {
                Some(select) => select
                    .filtered()
                    .into_iter()
                    .filter_map(|option| {
                        option.id.parse::<usize>().ok().map(|index| {
                            (index, option.title.as_str(), option.disabled, option.row_footer(flattened))
                        })
                    })
                    .collect(),
                None => dialog
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(index, choice)| (index, choice.as_str(), dialog.disabled[index], None))
                    .collect(),
            };
            let selected_pos = visible
                .iter()
                .position(|(index, _, _, _)| Some(*index) == self.selection)
                .unwrap_or(0);
            let start = selected_pos.saturating_sub(capacity.saturating_sub(1));
            for &(index, choice, disabled, row_footer) in visible.iter().skip(start).take(capacity) {
                if lines.len() < height && !disabled && self.phase == Phase::Open {
                    hits[lines.len()] = Some(index);
                }
                // DIALOG-02: the row's left pad is the tested
                // `dialog::row_padding` value (`dialog-select.tsx:915-920`):
                // a marked row reserves one cell, an unmarked row three.
                let marked = self.selection == Some(index);
                let (row_left, _) = crate::native_palette::dialog::row_padding(marked, self.new_designs);
                let marker = if let Some(select) = dialog.select.as_ref() {
                    let experiment = select.experiment();
                    let gutter = crate::native_palette::select::active_gutter(experiment, marked);
                    let marker = if marked {
                        crate::native_palette::select::current_marker(experiment)
                    } else {
                        " "
                    };
                    if experiment.new_markers {
                        format!("{gutter}{marker} ")
                    } else if marked {
                        format!("{}> ", " ".repeat(row_left as usize))
                    } else {
                        " ".repeat(row_left as usize)
                    }
                } else if marked {
                    format!("{}> ", " ".repeat(row_left as usize))
                } else {
                    " ".repeat(row_left as usize)
                };
                let suffix = if disabled {
                    " (disabled)"
                } else {
                    ""
                };
                // SIDE-08: the MCP list prints the server name then the exact
                // upstream status word (`dialog-mcp.tsx:79-85`), with no
                // numeric index (upstream `DialogSelect` rows carry no number).
                let text = if dialog.kind == DialogKind::Mcp {
                    let status = dialog.mcp_status.get(index).map(String::as_str).unwrap_or("");
                    format!("{marker}{choice}  {status}")
                } else {
                    let left = format!("{marker}{}. {choice}{suffix}", index + 1);
                    if let Some(footer) = row_footer {
                        // Preserve the complete footer and spend any collision on the title.
                        let footer_width = footer.chars().count();
                        let gap = if width > footer_width { 1 } else { 0 };
                        let title_budget = width.saturating_sub(footer_width + gap);
                        let left = left.chars().take(title_budget).collect::<String>();
                        let pad = width.saturating_sub(left.chars().count() + footer_width);
                        format!("{left}{}{footer}", " ".repeat(pad))
                    } else { left }
                };
                lines.push(line(text,
                    // DIALOG-07: the row's own role is unchanged by the footer
                    // (the selection highlight must still win). Upstream paints
                    // the footer in `secondary()` while the title keeps `text()`;
                    // `Line` carries one role, so the footer inherits the row
                    // role — a recorded residual (no dialog span channel yet).
                    if self.selection == Some(index) { Role::Selected } else if disabled { Role::Description } else if dialog.kind == DialogKind::Permission { Role::Action } else { Role::Item }));
            }
            // DIALOG-06/09: the select's own empty/no-match view, bounded to
            // the remaining body rows.
            if visible.is_empty() {
                let message = match dialog.select.as_ref().map(|select| select.empty_state()) {
                    Some(crate::native_palette::select::EmptyState::NoMatch) => {
                        Some("No results found")
                    }
                    Some(crate::native_palette::select::EmptyState::Empty) => {
                        Some("No items available")
                    }
                    _ => None,
                };
                if let Some(message) = message.filter(|_| lines.len() < height) {
                    lines.push(line(message.to_string(), Role::Description));
                }
            }
            // DIALOG-02: the body box's own bottom padding
            // (`dialog.tsx:159`): `newDesigns() && footer ? (footerGap ?? 1) - 1
            // : 0`. The live `dialog-prompt` caller passes `footerGap={2}`, so
            // under the experiment a footer adds one body padding row.
            for _ in 0..crate::native_palette::dialog::body_bottom_padding(
                self.new_designs,
                has_footer,
                dialog.footer_gap,
            ) {
                lines.push(line(String::new(), Role::Description));
            }
            // `dialog.tsx:157`: the outer layout's `gap={1}` puts one blank row
            // between the body and the footer.
            if roomy && has_footer {
                lines.push(line(String::new(), Role::Description));
            }
            let footer = if self.phase == Phase::Pending {
                "decision sent — waiting for backend acknowledgement"
            } else if self.rejected {
                "Decision rejected — ↑/↓ choose · Enter retry · Esc deny"
            } else if dialog.kind == DialogKind::Mcp {
                "↑↓ choose · Enter toggle · Esc close"
            } else if dialog.kind == DialogKind::Sessions {
                "↑↓ choose · Enter open · Esc close"
            } else if dialog.kind == DialogKind::Stash {
                "↑↓ choose · Enter restore · ctrl+d delete · Esc close"
            } else if dialog.kind == DialogKind::Prompt {
                "Enter: save · Esc: cancel"
            } else {
                "1–9 / ↑↓ choose · Enter / click here confirm · Esc deny"
            };
            match dialog.kind {
                DialogKind::Question if !self.question_capable => {}
                DialogKind::Error => {}
                DialogKind::Execute => {}
                // The form model paints its own footer (`form.tsx:767-1174`).
                DialogKind::Form => {}
                // The crash overlay owns its own footer (`error-component.tsx`),
                // so the generic dialog footer is suppressed.
                DialogKind::Crash => {}
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
        if !self.is_open() {
            return Err(DialogError::NotOpen);
        }
        match self.phase {
            Phase::Pending => Err(DialogError::PendingDecision),
            _ => Ok(()),
        }
    }

    fn step(&mut self, forward: bool) {
        if self.phase != Phase::Open {
            return;
        }
        // DIALOG-06..08: a select navigates its own FILTERED rows, so Down on
        // a filtered list skips hidden choices instead of walking them, and
        // the dialog selection always holds an original choice index.
        //
        // The dialog selection is RE-DERIVED from the model after the move
        // rather than recomputed from the move's own result. `next`/`previous`
        // report `false` without moving when the model is locked
        // (`dialog-select.tsx:1590-1602`), and the highlight must then stay
        // exactly where the painted row shows it; reading the model's current
        // option is the single source of truth, so a suppressed move can never
        // leave `self.selection` pointing at a different row than the marker.
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.kind == DialogKind::Select && dialog.select.is_some())
        {
            if let Some(select) = self.dialog.as_mut().and_then(|dialog| dialog.select.as_mut()) {
                if forward {
                    select.next();
                } else {
                    select.previous();
                }
            }
            // Unconditional: an empty filtered list has no selection.
            self.sync_select_selection();
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

    /// Close through the modal layer: `clear()` closes every entry, resets
    /// `size`/`centered` and requests the 1 ms refocus
    /// (`ui/dialog.tsx:389-398`).
    fn reset(&mut self) {
        self.modal.clear();
        self.drop_content();
    }

    /// Drop the painted content after the stack already closed an entry (an
    /// Escape/Ctrl+C route or a backdrop release popped it). The modal layer
    /// owns the stack mutation; this only mirrors it on the content side so
    /// the two cannot disagree.
    fn drop_content(&mut self) {
        self.phase = Phase::Closed;
        self.dialog = None;
        self.selection = None;
        self.rejected = false;
        self.pending_attempt = 0;
        self.reject_stage = None;
    }

    fn reconcile_closed_route(&mut self, route: ModalRoute) -> ModalRoute {
        if route.layer == Some(ChromeLayer::Dialog) && route.key == ModalKey::Closed {
            // `pop_top()` already closed the entry (and refocused when it
            // emptied the stack); only the content remains to drop.
            self.drop_content();
        }
        route
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
    use regex_automata::meta::Regex;
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

    /// One form field. `options: None` means no options were supplied;
    /// `Some(vec![])` remains option-backed, matching JavaScript [] truthiness.
    /// `custom` enables the "type your own answer" row.
    #[derive(Clone, Debug, PartialEq)]
    pub struct FormField {
        pub key: String,
        pub title: Option<String>,
        pub kind: FieldKind,
        pub description: Option<String>,
        pub placeholder: Option<String>,
        pub required: bool,
        pub default: Option<FormValue>,
        pub options: Option<Vec<FormOption>>,
        pub custom: bool,
        pub format: Option<TextFormat>,
        pub pattern: Option<String>,
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
                options: None,
                custom: false,
                format: None,
                pattern: None,
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
            self.options = Some(options);
            self
        }

        #[must_use]
        pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
            self.pattern = Some(pattern.into());
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
                || (self.kind == FieldKind::Text && self.options.is_none())
        }

        /// `formCustom`: option-backed string/multiselect with `custom`
        /// (`util/form.ts:41-45`).
        #[must_use]
        pub fn is_custom(&self) -> bool {
            self.custom
                && (self.kind == FieldKind::Multi
                    || (matches!(self.kind, FieldKind::Text | FieldKind::Single) && self.options.is_some()))
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
            FieldKind::Multi | FieldKind::Single | FieldKind::Text => field
                .options
                .as_deref()
                .unwrap_or_default()
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
    /// upstream message, or `None` when the value is acceptable.
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
                // FORM-003 regex subset: upstream evaluates raw JS
                // `new RegExp(field.pattern).test(value)` (`util/form.ts:82-86`),
                // which also accepts look-around and backreferences. The native
                // engine is `regex_automata::meta::Regex` (regex-syntax 0.8),
                // which rejects those constructs. Decision: FAIL CLOSED. A
                // pattern the engine cannot compile is reported with upstream's
                // exact `Invalid pattern: {p}` message (rejecting the answer)
                // rather than silently accepting it. No JS-only pattern can
                // therefore pass unpoliced.
                if let Some(pattern) = &field.pattern {
                    match Regex::new(pattern) {
                        Ok(regex) if !regex.is_match(text) => {
                            return Some(format!("Must match pattern: {pattern}"));
                        }
                        Ok(_) => {}
                        Err(_) => return Some(format!("Invalid pattern: {pattern}")),
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
                if field.options.as_ref().is_some_and(|options| {
                    !field.custom && !options.iter().any(|option| option.value == *text)
                }) {
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
                    && items.iter().any(|item| {
                        !field.options.as_deref().unwrap_or_default().iter().any(|option| option.value == *item)
                    })
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
            if field.kind != FieldKind::Text || field.options.is_none() || !field.custom {
                continue;
            }
            let Some(FormValue::Text(default)) = field.default.as_ref() else {
                continue;
            };
            if field.options.as_deref().unwrap_or_default().iter().any(|option| option.value == *default) {
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

        /// Form title, shown as the overlay heading (`form.tsx:767-770`).
        #[must_use]
        pub fn title(&self) -> &str {
            &self.title
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
                FieldKind::Text | FieldKind::Single => field.options.is_some(),
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
            let configured = field.options.as_deref().unwrap_or_default().iter().any(|option| option.value == text);
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

/// Diff-viewer dialog chrome (DIALOG-18): the full/compact layout boundary,
/// the split/unified boundary, sticky card headers, reviewed-file collapse
/// and the 19-column file menu.
///
/// Upstream evidence, pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
/// * `packages/tui/src/feature-plugins/system/diff-viewer.tsx:45-52` —
///   `MIN_SPLIT_WIDTH = 100`, `FILE_TREE_MIN_WIDTH = 30`,
///   `FILE_TREE_MAX_WIDTH = 40`, `FULL_INSET = 2`,
///   `FULL_LAYOUT_MIN_WIDTH = 30 + 2*2 + floor(108/2) = 88`,
///   `FILE_HEADER_HEIGHT = 2`; `packages/tui/src/ui/layout.ts:5-9`
///   supplies `SESSION_PANEL_SPLIT_MIN_WIDTH = 64 + 44 = 108`.
/// * `:330-352` — `compact = docked || width < 88`; `inset = compact ? 0 : 2`;
///   `fileTreeWidth = max(30, min(40, floor(width / 4)))`;
///   `patchPaneWidth = width - tree - inset*2`;
///   `splitAvailable = patchPaneWidth >= 100`; the effective view is
///   `split` only when split is available, otherwise `unified` while the
///   stored preference survives.
/// * `:1098` — added and deleted files always render unified
///   (`status === "modified" ? view() : "unified"`).
/// * `:1010-1016` — a lifecycle pass sets
///   `translateY = max(0, min(scrollTop - (parent.y - content.y),
///   parent.height - header.height))`, i.e. the header sticks to the top but
///   never leaves its own card. The header is `FILE_HEADER_HEIGHT` rows with
///   `paddingLeft = compact ? 2 : 1`, `paddingRight = 1`,
///   `paddingBottom = 1` (`:1064-1066`), and a reviewed card paints a `▀`
///   gap over the header's bottom padding row (`:1052-1056`).
/// * `:990-992,1058` — reviewed files collapse their body entirely
///   (`<Show when={!reviewed()}>`), the card surface becomes
///   `theme.background.raised.high`, and the review counter is
///   `reviewed/total` (`:898-901`).
/// * `:929,1068-1073` — a file without a patch renders
///   "No patch available for this file."; an empty viewer renders
///   "No changes to show".
/// * `:1131-1175` — the compact footer: an optional `▀` transition row when
///   docked, one row with `paddingLeft = 2`, `paddingRight = 1` carrying the
///   close shortcut plus " close" (full) or " focus prompt" (docked) and the
///   help shortcut plus "see all" at the right edge, then a blank row when
///   docked.
/// * `:1177-1190` and `diff-viewer-file-menu.tsx:7-87` — the file menu:
///   width `min(19, cols)` (`:18`), `left = clamp(x, 0, cols - width)`,
///   `top = clamp(y + 1, 0, rows - 1)` (`:60-62`), one row, `zIndex = 2600`
///   (`:40,50`), label "Mark complete" / "Mark incomplete" (`:17`),
///   `escape,ctrl+c` closes (`:27`), `return` runs it, an outside press
///   closes it (`:51-55`), and running closes *before* toggling
///   (`:19-22`).
/// * `:547-573,693` — toggling from the menu of another file must not
///   navigate; in single-patch mode marking a file complete advances to the
///   next patch file, and with no next file that file is marked exhausted so
///   `diff.mark_reviewed` refuses it.
pub mod diff_dialog {
    use std::collections::BTreeSet;

    /// `MIN_SPLIT_WIDTH` (`diff-viewer.tsx:45`).
    pub const MIN_SPLIT_WIDTH: u16 = 100;
    /// `FILE_TREE_MIN_WIDTH` (`diff-viewer.tsx:46`).
    pub const FILE_TREE_MIN_WIDTH: u16 = 30;
    /// `FILE_TREE_MAX_WIDTH` (`diff-viewer.tsx:47`).
    pub const FILE_TREE_MAX_WIDTH: u16 = 40;
    /// `FULL_INSET` (`diff-viewer.tsx:48`).
    pub const FULL_INSET: u16 = 2;
    /// `SESSION_CONTENT_PREFERRED_WIDTH` (`ui/layout.ts:6`).
    pub const SESSION_CONTENT_PREFERRED_WIDTH: u16 = 64;
    /// `SESSION_CONTENT_MIN_WIDTH` (`ui/layout.ts:5`).
    pub const SESSION_CONTENT_MIN_WIDTH: u16 = 44;
    /// `SESSION_PANEL_SPLIT_MIN_WIDTH` (`ui/layout.ts:9`).
    pub const SESSION_PANEL_SPLIT_MIN_WIDTH: u16 =
        SESSION_CONTENT_PREFERRED_WIDTH + SESSION_CONTENT_MIN_WIDTH;
    /// Full-layout boundary (`diff-viewer.tsx:50`): 30 + 4 + 54 = 88.
    pub const FULL_LAYOUT_MIN_WIDTH: u16 =
        FILE_TREE_MIN_WIDTH + FULL_INSET * 2 + SESSION_PANEL_SPLIT_MIN_WIDTH / 2;
    /// `FILE_HEADER_HEIGHT` (`diff-viewer.tsx:52`).
    pub const FILE_HEADER_HEIGHT: u16 = 2;
    /// File menu width (`diff-viewer-file-menu.tsx:18`).
    pub const FILE_MENU_WIDTH: u16 = 19;
    /// File menu portal z-index (`diff-viewer-file-menu.tsx:40,50`).
    pub const FILE_MENU_Z_INDEX: u16 = 2600;
    /// Half-row glyph between cards (`diff-viewer.tsx:1000`).
    pub const CARD_GAP_TOP: char = '▄';
    /// Half-row glyph closing a reviewed header (`diff-viewer.tsx:1054`).
    pub const CARD_GAP_BOTTOM: char = '▀';
    /// Footer transition glyph (`diff-viewer.tsx:1141`, `EmptyBorder`).
    pub const FOOTER_TRANSITION: char = '▀';
    /// `diff-viewer.tsx:1073`.
    pub const NO_PATCH: &str = "No patch available for this file.";
    /// `diff-viewer.tsx:929`.
    pub const NO_CHANGES: &str = "No changes to show";

    /// File status from the diff payload (`diff-viewer.tsx:64`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DiffStatus {
        Added,
        Deleted,
        Modified,
    }

    impl DiffStatus {
        /// Single status cell used by the file tree (`diff-viewer-file-tree.tsx`).
        #[must_use]
        pub const fn code(self) -> char {
            match self {
                DiffStatus::Added => 'A',
                DiffStatus::Deleted => 'D',
                DiffStatus::Modified => 'M',
            }
        }

        /// Only modified files honour the split view
        /// (`diff-viewer.tsx:1098`).
        #[must_use]
        pub const fn supports_split(self) -> bool {
            matches!(self, DiffStatus::Modified)
        }
    }

    /// Patch presentation (`diff-viewer.tsx:55`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum DiffView {
        Split,
        #[default]
        Unified,
    }

    /// Diff source picker entries (`diff-viewer.tsx:80-85`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DiffSource {
        Branch,
        Committed,
        Working,
        Turn,
    }

    impl DiffSource {
        /// `DIFF_SOURCES[...].label`.
        #[must_use]
        pub const fn label(self) -> &'static str {
            match self {
                DiffSource::Branch => "All",
                DiffSource::Committed => "Committed",
                DiffSource::Working => "Uncommitted",
                DiffSource::Turn => "Last turn",
            }
        }

        /// `DIFF_SOURCES[...].description`.
        #[must_use]
        pub const fn description(self) -> &'static str {
            match self {
                DiffSource::Branch => "Branch + local changes",
                DiffSource::Committed => "Branch commits only",
                DiffSource::Working => "Local changes only",
                DiffSource::Turn => "Latest session turn",
            }
        }

        /// `needsBase` (`diff-viewer.tsx:89`).
        #[must_use]
        pub const fn needs_base(self) -> bool {
            matches!(self, DiffSource::Branch | DiffSource::Committed)
        }

        /// The picker only offers "Last turn" for a session
        /// (`diff-viewer.tsx:104`).
        #[must_use]
        pub const fn sources(session: bool) -> &'static [DiffSource] {
            if session {
                &[
                    DiffSource::Branch,
                    DiffSource::Committed,
                    DiffSource::Working,
                    DiffSource::Turn,
                ]
            } else {
                &[DiffSource::Branch, DiffSource::Committed, DiffSource::Working]
            }
        }
    }

    /// One file in the diff.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DiffFile {
        pub path: String,
        pub additions: u32,
        pub deletions: u32,
        pub status: DiffStatus,
        pub patch: Option<String>,
    }

    impl DiffFile {
        #[must_use]
        pub fn new(
            path: impl Into<String>,
            additions: u32,
            deletions: u32,
            status: DiffStatus,
        ) -> Self {
            Self {
                path: path.into(),
                additions,
                deletions,
                status,
                patch: None,
            }
        }

        #[must_use]
        pub fn with_patch(mut self, patch: impl Into<String>) -> Self {
            self.patch = Some(patch.into());
            self
        }

        fn patch_lines(&self) -> Vec<String> {
            self.patch
                .as_deref()
                .map(|patch| {
                    patch
                        .replace('\r', "")
                        .split('\n')
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    /// Resolved geometry of one diff viewer frame.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DiffLayout {
        /// `docked() || width < 88`.
        pub compact: bool,
        /// Docked pane rather than a full screen.
        pub docked: bool,
        /// Card inset from the pane edges (0 when compact).
        pub inset: u16,
        /// Tree width when the tree is shown.
        pub tree_width: Option<u16>,
        /// Width of the patch column.
        pub patch_width: u16,
        /// Whether split patches fit.
        pub split_available: bool,
        /// Effective view for modified files.
        pub view: DiffView,
    }

    /// `fileTreeWidth` (`diff-viewer.tsx:343-345`).
    #[must_use]
    pub const fn file_tree_width(width: u16) -> u16 {
        let quarter = width / 4;
        if quarter < FILE_TREE_MIN_WIDTH {
            FILE_TREE_MIN_WIDTH
        } else if quarter > FILE_TREE_MAX_WIDTH {
            FILE_TREE_MAX_WIDTH
        } else {
            quarter
        }
    }

    /// `showDiffViewerFileTree(tree && files.length > 0)` combined with
    /// `!compact` (`diff-viewer.tsx:341`).
    #[must_use]
    pub fn file_tree_visible(compact: bool, enabled: bool, files: usize) -> bool {
        !compact && enabled && files > 0
    }

    /// `patchPaneWidth` (`diff-viewer.tsx:346`).
    #[must_use]
    pub fn patch_pane_width(width: u16, tree_width: Option<u16>, inset: u16) -> u16 {
        width
            .saturating_sub(tree_width.unwrap_or(0))
            .saturating_sub(inset.saturating_mul(2))
    }

    /// Sticky header translation (`diff-viewer.tsx:1010-1016`):
    /// `max(0, min(scrollTop - (parent.y - content.y), parent.height - header.height))`.
    ///
    /// `card_y` is the card's offset inside the scroll content and
    /// `card_height` is the whole card (header plus body). The header never
    /// leaves its own card.
    #[must_use]
    pub fn sticky_header_offset(scroll_top: u16, card_y: u16, card_height: u16) -> u16 {
        let travel = scroll_top.saturating_sub(card_y);
        travel.min(card_height.saturating_sub(FILE_HEADER_HEIGHT))
    }

    /// Background role of one rendered row.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DiffSurface {
        /// `theme.diff.background.context` — unreviewed card.
        Context,
        /// `theme.background.raised.high` — reviewed card.
        Reviewed,
        /// The pane surface behind the insets (`theme.background.base`, or
        /// raised-base while docked).
        Base,
    }

    /// One rendered row of the patch column.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DiffRow {
        pub text: String,
        pub surface: DiffSurface,
        /// Leading/trailing cells painted with the pane surface before the
        /// card inset.
        pub inset: u16,
    }

    impl DiffRow {
        fn new(text: impl Into<String>, surface: DiffSurface, inset: u16) -> Self {
            Self {
                text: text.into(),
                surface,
                inset,
            }
        }
    }

    /// File-menu paint rectangle.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct MenuRect {
        pub x: u16,
        pub y: u16,
        pub width: u16,
        pub height: u16,
    }

    /// Open file menu anchored at the click that opened it
    /// (`diff-viewer.tsx:575-585`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct FileMenu {
        pub file_index: usize,
        pub x: u16,
        pub y: u16,
    }

    impl FileMenu {
        /// Clamped one-row menu rectangle
        /// (`diff-viewer-file-menu.tsx:18,60-62`).
        #[must_use]
        pub fn rect(&self, cols: u16, rows: u16) -> MenuRect {
            let width = FILE_MENU_WIDTH.min(cols);
            let x = self.x.min(cols.saturating_sub(width));
            let y = self.y.saturating_add(1).min(rows.saturating_sub(1));
            MenuRect {
                x,
                y,
                width,
                height: if rows == 0 { 0 } else { 1 },
            }
        }
    }

    /// Keyboard labels used by the footer; the caller supplies its resolved
    /// keymap shortcuts (`props.context.keymap.shortcuts(...)`), so no key
    /// name is invented here.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DiffKeys {
        pub close: String,
        pub help: String,
    }

    impl Default for DiffKeys {
        fn default() -> Self {
            Self {
                close: "esc".to_owned(),
                help: "?".to_owned(),
            }
        }
    }

    /// Outcome of toggling a file's reviewed flag.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ReviewEffect {
        /// File whose flag changed.
        pub file_index: usize,
        /// It is reviewed after the toggle.
        pub reviewed: bool,
        /// Single-patch mode advanced to this file.
        pub advanced: Option<usize>,
        /// Single-patch mode found no next file; the file is exhausted and
        /// `diff.mark_reviewed` refuses it until the selection moves.
        pub exhausted: bool,
    }

    /// Diff viewer state needed for layout, review state and the file menu.
    #[derive(Clone, Debug)]
    pub struct DiffDialog {
        files: Vec<DiffFile>,
        reviewed: BTreeSet<String>,
        selected: Option<usize>,
        single_patch: bool,
        tree_enabled: bool,
        view_override: Option<DiffView>,
        source: DiffSource,
        menu: Option<FileMenu>,
        exhausted: Option<usize>,
        docked: bool,
    }

    impl DiffDialog {
        /// Fresh viewer with the upstream defaults (`tree = true`,
        /// `single = false`, view follows the split boundary).
        #[must_use]
        pub fn new(files: Vec<DiffFile>) -> Self {
            Self {
                files,
                reviewed: BTreeSet::new(),
                selected: None,
                single_patch: false,
                tree_enabled: true,
                view_override: None,
                source: DiffSource::Branch,
                menu: None,
                exhausted: None,
                docked: false,
            }
        }

        #[must_use]
        pub fn files(&self) -> &[DiffFile] {
            &self.files
        }

        /// Replace the payload; resets selection, review state, the exhausted
        /// marker and the menu (`diff-viewer.tsx:356-358,493-500`).
        pub fn set_files(&mut self, files: Vec<DiffFile>) {
            self.files = files;
            self.reviewed.clear();
            self.selected = None;
            self.exhausted = None;
            self.menu = None;
        }

        pub fn set_docked(&mut self, docked: bool) {
            self.docked = docked;
        }

        #[must_use]
        pub const fn docked(&self) -> bool {
            self.docked
        }

        pub fn set_tree_enabled(&mut self, enabled: bool) {
            self.tree_enabled = enabled;
        }

        #[must_use]
        pub const fn tree_enabled(&self) -> bool {
            self.tree_enabled
        }

        pub fn set_single_patch(&mut self, single: bool) {
            self.single_patch = single;
            self.exhausted = None;
        }

        #[must_use]
        pub const fn single_patch(&self) -> bool {
            self.single_patch
        }

        pub fn set_source(&mut self, source: DiffSource) {
            self.source = source;
            self.menu = None;
        }

        #[must_use]
        pub const fn source(&self) -> DiffSource {
            self.source
        }

        /// Resolve the frame geometry (`diff-viewer.tsx:330-352`).
        #[must_use]
        pub fn layout(&self, width: u16) -> DiffLayout {
            let compact = self.docked || width < FULL_LAYOUT_MIN_WIDTH;
            let inset = if compact { 0 } else { FULL_INSET };
            let tree_width = if file_tree_visible(compact, self.tree_enabled, self.files.len()) {
                Some(file_tree_width(width))
            } else {
                None
            };
            let patch_width = patch_pane_width(width, tree_width, inset);
            let split_available = patch_width >= MIN_SPLIT_WIDTH;
            let view = if split_available {
                self.view_override.unwrap_or(DiffView::Split)
            } else {
                DiffView::Unified
            };
            DiffLayout {
                compact,
                docked: self.docked,
                inset,
                tree_width,
                patch_width,
                split_available,
                view,
            }
        }

        /// Effective view for one file: added/deleted files stay unified
        /// (`diff-viewer.tsx:1098`).
        #[must_use]
        pub fn view_for(&self, layout: &DiffLayout, index: usize) -> DiffView {
            match self.files.get(index) {
                Some(file) if file.status.supports_split() => layout.view,
                _ => DiffView::Unified,
            }
        }

        /// `diff.toggle_view` (`diff-viewer.tsx:740-748`): refused when split
        /// is unavailable, and the stored preference is untouched.
        pub fn toggle_view(&mut self, width: u16) -> bool {
            let layout = self.layout(width);
            if !layout.split_available {
                return false;
            }
            let current = self.view_override.unwrap_or(DiffView::Split);
            self.view_override = Some(match current {
                DiffView::Split => DiffView::Unified,
                DiffView::Unified => DiffView::Split,
            });
            true
        }

        #[must_use]
        pub const fn view_override(&self) -> Option<DiffView> {
            self.view_override
        }

        #[must_use]
        pub fn is_reviewed(&self, index: usize) -> bool {
            self.files
                .get(index)
                .is_some_and(|file| self.reviewed.contains(&file.path))
        }

        /// `reviewed/total` counter text (`diff-viewer.tsx:898-901`).
        #[must_use]
        pub fn review_count(&self) -> String {
            let done = self
                .files
                .iter()
                .filter(|file| self.reviewed.contains(&file.path))
                .count();
            format!("{done}/{}", self.files.len())
        }

        #[must_use]
        pub fn selected(&self) -> Option<usize> {
            self.selected
        }

        /// Select a file. Selecting clears the exhausted marker
        /// (`diff-viewer.tsx:374,409`).
        pub fn select(&mut self, index: usize) -> bool {
            if index >= self.files.len() {
                return false;
            }
            self.selected = Some(index);
            self.exhausted = None;
            true
        }

        /// File the viewer is anchored to: the explicit selection, else the
        /// first file (`currentPatchFileIndex` fallback chain).
        #[must_use]
        pub fn current(&self) -> Option<usize> {
            self.selected.or(if self.files.is_empty() {
                None
            } else {
                Some(0)
            })
        }

        /// File indexes painted as patches. Single-patch mode paints only the
        /// selected file (`diff-viewer.tsx:493-500`).
        #[must_use]
        pub fn visible_files(&self) -> Vec<usize> {
            if self.single_patch {
                self.current().into_iter().collect()
            } else {
                (0..self.files.len()).collect()
            }
        }

        /// `diff.mark_reviewed` refuses an exhausted file until the selection
        /// moves (`diff-viewer.tsx:688-695`): the command uses
        /// `selectedFileIndex() ?? currentPatchFileIndex()`, so the check is
        /// against the resolved current file, not the explicit selection.
        #[must_use]
        pub fn mark_reviewed_blocked(&self) -> bool {
            self.exhausted.is_some() && self.current() == self.exhausted
        }

        /// Toggle the reviewed flag (`diff-viewer.tsx:547-573`).
        ///
        /// In single-patch mode, marking the current file complete moves to
        /// the next patch file; when none exists the file becomes exhausted
        /// and stays selected.
        pub fn toggle_reviewed(&mut self, index: usize) -> ReviewEffect {
            let current = self.current();
            let Some(file) = self.files.get(index) else {
                return ReviewEffect {
                    file_index: index,
                    reviewed: false,
                    advanced: None,
                    exhausted: false,
                };
            };
            let path = file.path.clone();
            let was_reviewed = self.reviewed.contains(&path);
            if was_reviewed {
                self.reviewed.remove(&path);
            } else {
                self.reviewed.insert(path);
            }
            let mut effect = ReviewEffect {
                file_index: index,
                reviewed: !was_reviewed,
                advanced: None,
                exhausted: false,
            };
            // Completing another file from its menu must not navigate away.
            if current != Some(index) {
                return effect;
            }
            if self.single_patch && !was_reviewed {
                match self.next_file_index(index) {
                    Some(next) if next != index => {
                        self.select(next);
                        effect.advanced = Some(next);
                    }
                    _ => {
                        self.exhausted = Some(index);
                        effect.exhausted = true;
                    }
                }
            }
            effect
        }

        fn next_file_index(&self, index: usize) -> Option<usize> {
            if self.files.is_empty() {
                return None;
            }
            Some((index + 1) % self.files.len())
        }

        /// Open the menu from a right-click on a header
        /// (`diff-viewer.tsx:575-585`). Only valid file indexes open it.
        pub fn open_menu(&mut self, file_index: usize, x: u16, y: u16) -> bool {
            if file_index >= self.files.len() {
                return false;
            }
            self.menu = Some(FileMenu { file_index, x, y });
            true
        }

        #[must_use]
        pub const fn menu(&self) -> Option<FileMenu> {
            self.menu
        }

        /// Menu label (`diff-viewer-file-menu.tsx:17`).
        #[must_use]
        pub fn menu_label(&self) -> Option<&'static str> {
            let menu = self.menu?;
            Some(if self.is_reviewed(menu.file_index) {
                "Mark incomplete"
            } else {
                "Mark complete"
            })
        }

        /// Escape / Ctrl+C / outside press (`diff-viewer-file-menu.tsx:27,51-55`).
        pub fn close_menu(&mut self) {
            self.menu = None;
        }

        /// `return` in the menu: close first, then toggle
        /// (`diff-viewer-file-menu.tsx:19-22`).
        pub fn run_menu(&mut self) -> Option<ReviewEffect> {
            let menu = self.menu.take()?;
            Some(self.toggle_reviewed(menu.file_index))
        }

        /// Background of the fixed top edge: raised-high when the card at the
        /// top of the viewport is reviewed, else the diff context surface
        /// (`diff-viewer.tsx:954-972`).
        #[must_use]
        pub fn top_edge_surface(&self, scroll_top: u16) -> DiffSurface {
            let mut reviewed = false;
            for index in self.visible_files() {
                let (header_y, _card_height) = self.card_geometry(index);
                if header_y <= scroll_top {
                    reviewed = self.is_reviewed(index);
                }
            }
            if reviewed {
                DiffSurface::Reviewed
            } else {
                DiffSurface::Context
            }
        }

        /// Header text: path, tick, additions and deletions
        /// (`diff-viewer.tsx:1017-1048`). `max_width` is the patch column.
        #[must_use]
        pub fn header_text(&self, index: usize, max_width: u16) -> String {
            let Some(file) = self.files.get(index) else {
                return String::new();
            };
            let reviewed = self.is_reviewed(index);
            let counts = format!(
                " +{} -{}{}",
                file.additions,
                file.deletions,
                if reviewed { " ✓" } else { "" }
            );
            let budget = usize::from(max_width)
                .saturating_sub(visible_cells(&counts, usize::from(max_width)))
                .saturating_sub(1);
            let path = clip_cells(&file.path, budget);
            format!("{path}{counts}")
        }

        fn card_geometry(&self, index: usize) -> (u16, u16) {
            // Content offsets: one separator row before every card but the
            // first, two header rows, then the body.
            let visible = self.visible_files();
            let mut y = 0u16;
            for (position, file_index) in visible.iter().enumerate() {
                let file = &self.files[*file_index];
                if position > 0 {
                    y = y.saturating_add(1);
                }
                let header_y = y;
                let body = self.body_height(*file_index, file);
                y = y
                    .saturating_add(FILE_HEADER_HEIGHT)
                    .saturating_add(body);
                if *file_index == index {
                    return (header_y, FILE_HEADER_HEIGHT.saturating_add(body));
                }
            }
            (0, 0)
        }

        fn body_height(&self, index: usize, file: &DiffFile) -> u16 {
            if self.is_reviewed(index) {
                return 0;
            }
            let lines = file.patch_lines().len();
            if lines == 0 {
                1
            } else {
                u16::try_from(lines).unwrap_or(u16::MAX)
            }
        }

        /// Render the patch column at `width x height`, scrolled by
        /// `scroll_top` content rows.
        ///
        /// The returned rows are the patch column only; the file tree, when
        /// present, occupies `layout.tree_width` cells to the left and is
        /// painted by its owner. Wide-cell clipping is left to the painter,
        /// exactly like [`DialogView::render`].
        #[must_use]
        pub fn render(
            &self,
            width: u16,
            height: u16,
            scroll_top: u16,
            keys: &DiffKeys,
        ) -> Vec<DiffRow> {
            let layout = self.layout(width);
            let mut rows = Vec::new();
            if height == 0 {
                return rows;
            }
            if self.files.is_empty() {
                rows.push(DiffRow::new(
                    clip_cells(NO_CHANGES, usize::from(layout.patch_width)),
                    DiffSurface::Base,
                    layout.inset,
                ));
                rows.resize(
                    usize::from(height),
                    DiffRow::new("", DiffSurface::Base, layout.inset),
                );
                return rows;
            }
            let footer = self.footer_rows(&layout, keys);
            let viewport = usize::from(height.saturating_sub(
                u16::try_from(footer.len()).unwrap_or(u16::MAX),
            ));
            let mut content: Vec<DiffRow> = Vec::new();
            for (position, index) in self.visible_files().into_iter().enumerate() {
                let file = &self.files[index];
                let surface = if self.is_reviewed(index) {
                    DiffSurface::Reviewed
                } else {
                    DiffSurface::Context
                };
                if position > 0 {
                    content.push(DiffRow::new(
                        CARD_GAP_TOP
                            .to_string()
                            .repeat(usize::from(layout.patch_width)),
                        surface,
                        layout.inset,
                    ));
                }
                let header = self.header_text(index, layout.patch_width);
                content.push(DiffRow::new(header, surface, layout.inset));
                content.push(DiffRow::new(
                    if self.is_reviewed(index) {
                        CARD_GAP_BOTTOM
                            .to_string()
                            .repeat(usize::from(layout.patch_width))
                    } else {
                        String::new()
                    },
                    surface,
                    layout.inset,
                ));
                if self.is_reviewed(index) {
                    continue;
                }
                let lines = file.patch_lines();
                if lines.is_empty() {
                    let budget = usize::from(layout.patch_width).saturating_sub(if layout.compact {
                        3
                    } else {
                        2
                    });
                    content.push(DiffRow::new(
                        format!("{}{}", body_padding(layout.compact), clip_cells(NO_PATCH, budget)),
                        surface,
                        layout.inset,
                    ));
                    continue;
                }
                for line in lines {
                    let budget = usize::from(layout.patch_width).saturating_sub(if layout.compact {
                        3
                    } else {
                        2
                    });
                    content.push(DiffRow::new(
                        format!("{}{}", body_padding(layout.compact), clip_cells(&line, budget)),
                        surface,
                        layout.inset,
                    ));
                }
            }
            let start = usize::from(scroll_top).min(content.len());
            let mut visible: Vec<DiffRow> = content[start..]
                .iter()
                .take(viewport)
                .cloned()
                .collect();
            // Sticky header: the card whose header scrolled past the top keeps
            // its two header rows pinned over the first viewport rows until
            // its own card ends.
            if scroll_top > 0 {
                if let Some(index) = self.pinned_file(scroll_top) {
                    let surface = if self.is_reviewed(index) {
                        DiffSurface::Reviewed
                    } else {
                        DiffSurface::Context
                    };
                    let header = self.header_text(index, layout.patch_width);
                    if visible.len() > 0 {
                        visible[0] = DiffRow::new(header, surface, layout.inset);
                    }
                    if visible.len() > 1 {
                        visible[1] = DiffRow::new(
                            if self.is_reviewed(index) {
                                CARD_GAP_BOTTOM
                                    .to_string()
                                    .repeat(usize::from(layout.patch_width))
                            } else {
                                String::new()
                            },
                            surface,
                            layout.inset,
                        );
                    }
                }
            }
            rows.extend(visible);
            rows.resize(
                viewport.max(rows.len()),
                DiffRow::new("", DiffSurface::Base, layout.inset),
            );
            rows.extend(footer);
            rows.truncate(usize::from(height));
            rows
        }

        /// File whose header is currently pinned by the sticky rule.
        #[must_use]
        pub fn pinned_file(&self, scroll_top: u16) -> Option<usize> {
            if scroll_top == 0 {
                return None;
            }
            let mut pinned = None;
            for index in self.visible_files() {
                let (header_y, card_height) = self.card_geometry(index);
                if header_y >= scroll_top {
                    break;
                }
                let offset = sticky_header_offset(scroll_top, header_y, card_height);
                let max = card_height.saturating_sub(FILE_HEADER_HEIGHT);
                if offset < max {
                    pinned = Some(index);
                } else {
                    pinned = None;
                }
            }
            pinned
        }

        /// Compact footer (`diff-viewer.tsx:1131-1175`). Empty on the full
        /// layout, where the help widget is painted at the top-right instead.
        #[must_use]
        pub fn footer_rows(&self, layout: &DiffLayout, keys: &DiffKeys) -> Vec<DiffRow> {
            if !layout.compact {
                return Vec::new();
            }
            let width = usize::from(layout.patch_width);
            let mut rows = Vec::new();
            if layout.docked {
                rows.push(DiffRow::new(
                    FOOTER_TRANSITION.to_string().repeat(width),
                    DiffSurface::Base,
                    0,
                ));
            }
            let left = format!(
                "{}{}",
                keys.close,
                if layout.docked { " focus prompt" } else { " close" }
            );
            let right = format!("{} see all", keys.help);
            let row = footer_row(&left, &right, width);
            rows.push(DiffRow::new(row, DiffSurface::Base, 0));
            if layout.docked {
                rows.push(DiffRow::new("", DiffSurface::Base, 0));
            }
            rows
        }

        /// Top-right help widget when the tree is hidden on the full layout
        /// (`diff-viewer.tsx:1173-1175`).
        #[must_use]
        pub fn help_widget(&self, layout: &DiffLayout) -> Option<MenuRect> {
            if layout.compact || layout.tree_width.is_some() {
                return None;
            }
            Some(MenuRect {
                x: layout.patch_width.saturating_sub(1),
                y: 0,
                width: 1,
                height: 1,
            })
        }
    }

    fn body_padding(compact: bool) -> &'static str {
        // `paddingLeft = compact ? 2 : 1` (`diff-viewer.tsx:1064`).
        if compact {
            "  "
        } else {
            " "
        }
    }

    fn footer_row(left: &str, right: &str, width: usize) -> String {
        // `paddingLeft = 2`, `paddingRight = 1` (`diff-viewer.tsx:1144`).
        let available = width.saturating_sub(3);
        let right = clip_cells(right, available);
        let right_width = visible_cells(&right, available);
        let left_budget = available.saturating_sub(right_width + 1);
        let left = clip_cells(left, left_budget);
        let gap = available
            .saturating_sub(visible_cells(&left, available) + right_width)
            .max(1);
        let row = clip_cells(&format!("  {left}{}{right}", " ".repeat(gap)), width);
        let cells = visible_cells(&row, width);
        if cells < width {
            format!("{row}{}", " ".repeat(width - cells))
        } else {
            row
        }
    }

    fn clip_cells(text: &str, budget: usize) -> String {
        let mut out = String::new();
        crate::native_view_layout::visit_unpadded_cells(text, budget, |glyph, _| {
            out.push_str(glyph);
        });
        out
    }

    fn visible_cells(text: &str, budget: usize) -> usize {
        let mut cells = 0usize;
        crate::native_view_layout::visit_unpadded_cells(text, budget, |_, _| cells += 1);
        cells
    }
}

/// Error-details dialog (DIALOG-20): wrapped, hidden-scrollbar body with a
/// 20-row cap, 1/20-row scrolling, and the copy/investigate/back controls.
///
/// Upstream evidence, pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
/// * `:114-124` — the body is a hidden-scrollbar scrollbox with
///   `maxHeight={20}`.
/// * `:71-78` — keys: `up`/`down` scroll by 1, `pageup`/`pagedown` by 20,
///   `home`/`end` jump to the bounds.
/// * `:43-62` — the `modal` keymap layer binds `escape` → back, `c` → copy,
///   `i` → investigate (prefill a new prompt and clear the dialog).
/// * `:64-69` — footer actions in order: "investigate" (`i`), then
///   "copy details" (`c`), replaced by "✓ copied" after a successful copy.
/// * `packages/tui/src/util/error-details.ts:2-27` — the copied text and the
///   investigation prompt bodies.
pub mod error_details {
    /// `maxHeight={20}` (`dialog-error-details.tsx:114-124`).
    pub const MAX_BODY_HEIGHT: usize = 20;
    /// `scrollBy(-20)` / `scrollBy(20)` (`:74-75`).
    pub const PAGE_ROWS: usize = 20;
    /// Prompt prefill heading (`util/error-details.ts:11`).
    pub const INVESTIGATE_HEADING: &str =
        "Investigate why this OpenCode component failed in the current project.";
    /// Final investigation instruction (`util/error-details.ts:18`).
    pub const INVESTIGATE_INSTRUCTION: &str = "Inspect the relevant project and global OpenCode configuration, startup or loading behavior, required environment variables or credentials, dependencies, and logs. Identify the root cause and recommend a fix. Do not expose credentials.";

    /// Key routed into the dialog.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ErrorKey {
        Up,
        Down,
        PageUp,
        PageDown,
        Home,
        End,
        /// `c` — copy the details.
        Copy,
        /// `i` — investigate.
        Investigate,
        /// `escape` — back to the caller.
        Back,
    }

    /// What the dialog did with a key.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ErrorOutcome {
        /// Scrolled and consumed the key.
        Scrolled,
        Copy,
        Investigate,
        Back,
        /// The key is not part of this dialog.
        Ignored,
    }

    /// One footer action (`dialog-error-details.tsx:64-69`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ErrorAction {
        pub title: String,
        pub key: Option<char>,
    }

    /// State of the error-details dialog.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ErrorDetails {
        title: String,
        source: Option<String>,
        error: String,
        context: Option<String>,
        diagnostic_ref: Option<String>,
        offset: usize,
        copied: bool,
    }

    impl ErrorDetails {
        #[must_use]
        pub fn new(
            title: impl Into<String>,
            error: impl Into<String>,
            context: Option<String>,
            diagnostic_ref: Option<String>,
        ) -> Self {
            Self {
                title: title.into(),
                source: None,
                error: error.into(),
                context,
                diagnostic_ref,
                offset: 0,
                copied: false,
            }
        }

        #[must_use]
        pub fn with_source(mut self, source: impl Into<String>) -> Self {
            self.source = Some(source.into());
            self
        }

        #[must_use]
        pub fn title(&self) -> &str {
            &self.title
        }

        /// `FilePath` row under the header (`dialog-error-details.tsx:100-108`).
        #[must_use]
        pub fn source(&self) -> Option<&str> {
            self.source.as_deref()
        }

        /// Copied text (`util/error-details.ts:2-9`).
        #[must_use]
        pub fn text(&self) -> String {
            let mut lines = vec![self.title.clone()];
            if let Some(context) = &self.context {
                lines.push(context.clone());
            }
            lines.push(format!("Error: {}", self.error));
            if let Some(reference) = &self.diagnostic_ref {
                lines.push(format!("Reference: {reference}"));
            }
            lines.join("\n")
        }

        /// Prompt prefilled by `i` (`util/error-details.ts:10-19`).
        #[must_use]
        pub fn prompt(&self) -> String {
            let mut parts = vec![
                INVESTIGATE_HEADING.to_owned(),
                self.text(),
            ];
            if let Some(reference) = &self.diagnostic_ref {
                parts.push(format!(
                    "Find the server log entry matching reference {reference} and inspect its original cause. If the server logs are not accessible from this session, say so and ask for the matching log entry; do not infer the cause from the reference alone."
                ));
            }
            parts.push(INVESTIGATE_INSTRUCTION.to_owned());
            parts.join("\n\n")
        }

        /// Diagnostic reference row (`dialog-error-details.tsx:125-127`).
        #[must_use]
        pub fn diagnostic_ref(&self) -> Option<&str> {
            self.diagnostic_ref.as_deref()
        }

        #[must_use]
        pub const fn copied(&self) -> bool {
            self.copied
        }

        pub fn mark_copied(&mut self) {
            self.copied = true;
        }

        #[must_use]
        pub const fn offset(&self) -> usize {
            self.offset
        }

        /// Wrapped body rows at `width` cells (`wrapMode="word"`).
        #[must_use]
        pub fn body_rows(&self, width: usize) -> Vec<String> {
            let mut rows = Vec::new();
            for line in self.error.replace('\r', "").split('\n') {
                rows.extend(wrap_words(line, width.max(1)));
            }
            if rows.is_empty() {
                rows.push(String::new());
            }
            rows
        }

        /// Visible body window: at most [`MAX_BODY_HEIGHT`] rows starting at
        /// the current offset.
        #[must_use]
        pub fn visible_rows(&self, width: usize) -> Vec<String> {
            let rows = self.body_rows(width);
            let window = rows.len().min(MAX_BODY_HEIGHT);
            let max_offset = rows.len().saturating_sub(window);
            let start = self.offset.min(max_offset);
            rows[start..start + window].to_vec()
        }

        /// Largest valid offset (`scrollTo(scrollHeight)`).
        #[must_use]
        pub fn max_offset(&self, width: usize) -> usize {
            let rows = self.body_rows(width).len();
            let window = rows.min(MAX_BODY_HEIGHT);
            rows.saturating_sub(window)
        }

        fn scroll_by(&mut self, delta: isize, width: usize) -> bool {
            let max = self.max_offset(width);
            let next = if delta.is_negative() {
                self.offset.saturating_sub(delta.unsigned_abs())
            } else {
                self.offset.saturating_add(delta.unsigned_abs()).min(max)
            };
            let moved = next != self.offset;
            self.offset = next;
            moved
        }

        /// Route a key. `width` is the dialog body width used to resolve the
        /// bottom bound.
        pub fn handle(&mut self, key: ErrorKey, width: usize) -> ErrorOutcome {
            match key {
                ErrorKey::Up => self.scroll_outcome(-1, width),
                ErrorKey::Down => self.scroll_outcome(1, width),
                ErrorKey::PageUp => self.scroll_outcome(-(PAGE_ROWS as isize), width),
                ErrorKey::PageDown => self.scroll_outcome(PAGE_ROWS as isize, width),
                ErrorKey::Home => {
                    self.offset = 0;
                    ErrorOutcome::Scrolled
                }
                ErrorKey::End => {
                    self.offset = self.max_offset(width);
                    ErrorOutcome::Scrolled
                }
                ErrorKey::Copy => ErrorOutcome::Copy,
                ErrorKey::Investigate => ErrorOutcome::Investigate,
                ErrorKey::Back => ErrorOutcome::Back,
            }
        }

        fn scroll_outcome(&mut self, delta: isize, width: usize) -> ErrorOutcome {
            if self.scroll_by(delta, width) {
                ErrorOutcome::Scrolled
            } else {
                ErrorOutcome::Ignored
            }
        }

        /// Footer actions in upstream order (`dialog-error-details.tsx:64-69`).
        #[must_use]
        pub fn actions(&self) -> Vec<ErrorAction> {
            vec![
                ErrorAction {
                    title: "investigate".to_owned(),
                    key: Some('i'),
                },
                if self.copied {
                    ErrorAction {
                        title: "✓ copied".to_owned(),
                        key: None,
                    }
                } else {
                    ErrorAction {
                        title: "copy details".to_owned(),
                        key: Some('c'),
                    }
                },
            ]
        }
    }

    fn wrap_words(text: &str, width: usize) -> Vec<String> {
        let mut rows = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }
            if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                rows.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        if !current.is_empty() || rows.is_empty() {
            rows.push(current);
        }
        rows
    }
}

/// Root crash screen (DIALOG-20): the independent safe palette, the
/// horizontally selectable Copy/Restart/Quit actions, stack scrolling and the
/// responsive subtext/footer thresholds.
///
/// Upstream evidence, pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`,
/// `packages/tui/src/component/error-component.tsx`:
/// * `:19-42` — the palette is defined locally per mode because the theme
///   context may be the thing that crashed.
/// * `:69-100` — keys: Ctrl+C exits, Return runs the selected action,
///   Left/Right and Tab/Shift+Tab move the selection, Up/Down scroll by 1,
///   PageUp/PageDown by the viewport height, Home/End jump, and the literal
///   `q`/`c`/`r` shortcuts run Quit/Copy/Restart.
/// * `:104-106` — `contentWidth = min(84, max(24, width - 4))`, subtext at
///   `height >= 18`, footer at `height >= 20`.
/// * `:55-62,169,172` — Copy/Restart/Quit order, labels
///   "Copy report"/"✓ Copied"/"Copy failed", keys `c`/`r`/`q`.
/// * `:183` — the stack panel has `minHeight={3}`.
/// * `:221-257` — the issue URL carries title/version/os/terminal/reproduce
///   and truncates the stack so the encoded URL stays under 6000 characters,
///   appending `\n… (truncated)`.
pub mod crash {
    // DIALOG-20: the live panic hook feeds this slot. Installed exactly once
    // at native-loop boot (`app.tsx:336-339` wraps the root renderer in a
    // global panic/error boundary); the loop takes the capture and opens the
    // crash surface through [`crate::native_view_dialogs::DialogView::open_crash`].
    // The hook must never panic itself: poisoned locks recover their data,
    // payload extraction never unwraps, and the stored text is bounded.
    static PANIC_CAPTURE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    static PANIC_HOOK: std::sync::Once = std::sync::Once::new();

    /// Install the process-wide panic capture hook exactly once. The
    /// previously installed hook still runs so non-TUI panic logging is
    /// preserved; a `&str`/`String` payload plus the panic location
    /// (`panic.rs` `Location`) is captured in the bounded model format.
    pub fn install_panic_hook() {
        PANIC_HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let payload = info.payload();
                let mut message = payload
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| UNKNOWN_MESSAGE.to_owned());
                if let Some(location) = info.location() {
                    message.push_str(&format!(
                        " (at {}:{}:{})",
                        location.file(),
                        location.line(),
                        location.column()
                    ));
                }
                let mut slot = PANIC_CAPTURE
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                // First unobserved panic wins; later panics during the same
                // fatal unwind must not overwrite the surface the operator
                // is about to see.
                if slot.is_none() {
                    *slot = Some(super::bounded_crash_text(&message, 2048));
                }
                drop(slot);
                previous(info);
            }));
        });
    }

    /// Take the most recent captured panic text (message + location), if any.
    pub fn take_panic_capture() -> Option<String> {
        let mut slot = PANIC_CAPTURE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slot.take()
    }

    /// `min(84, max(24, width - 4))` (`error-component.tsx:104`).
    pub const CONTENT_MAX_WIDTH: u16 = 84;
    /// Lower clamp of the content column (`error-component.tsx:104`).
    pub const CONTENT_MIN_WIDTH: u16 = 24;
    /// Subtext threshold (`error-component.tsx:105`).
    pub const SUBTEXT_MIN_HEIGHT: u16 = 18;
    /// Footer threshold (`error-component.tsx:106`).
    pub const FOOTER_MIN_HEIGHT: u16 = 20;
    /// `minHeight={3}` on the stack panel (`error-component.tsx:183`).
    pub const STACK_MIN_HEIGHT: u16 = 3;
    /// Headline (`error-component.tsx:119-121`).
    pub const HEADLINE: &str = "OpenCode crashed";
    /// Subtext (`error-component.tsx:123`).
    pub const SUBTEXT: &str = "An unexpected error stopped the session.";
    /// Fallback message (`error-component.tsx:44`).
    pub const UNKNOWN_MESSAGE: &str = "An unknown error occurred.";
    /// Fallback stack (`error-component.tsx:45`).
    pub const NO_STACK: &str = "No stack trace available.";
    /// Issue form (`error-component.tsx:225`).
    pub const ISSUE_URL: &str =
        "https://github.com/anomalyco/opencode/issues/new?template=bug-report.yml";
    /// Practical URL budget (`error-component.tsx:239`).
    pub const MAX_URL_LENGTH: usize = 6000;
    /// Truncation marker (`error-component.tsx:240`).
    pub const TRUNCATION_MARKER: &str = "\n… (truncated)";

    /// Safe palette mode (`error-component.tsx:19-42`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrashMode {
        Dark,
        Light,
    }

    /// The independent crash palette. RGBA, alpha 255.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CrashPalette {
        pub bg: [u8; 4],
        pub element: [u8; 4],
        pub border_subtle: [u8; 4],
        pub text: [u8; 4],
        pub muted: [u8; 4],
        pub primary: [u8; 4],
        pub on_primary: [u8; 4],
        pub error: [u8; 4],
        pub success: [u8; 4],
    }

    const DARK: CrashPalette = CrashPalette {
        bg: [0x0a, 0x0a, 0x0a, 0xff],
        element: [0x1e, 0x1e, 0x1e, 0xff],
        border_subtle: [0x3c, 0x3c, 0x3c, 0xff],
        text: [0xee, 0xee, 0xee, 0xff],
        muted: [0x80, 0x80, 0x80, 0xff],
        primary: [0xfa, 0xb2, 0x83, 0xff],
        on_primary: [0x0a, 0x0a, 0x0a, 0xff],
        error: [0xe0, 0x6c, 0x75, 0xff],
        success: [0x7f, 0xd8, 0x8f, 0xff],
    };

    const LIGHT: CrashPalette = CrashPalette {
        bg: [0xff, 0xff, 0xff, 0xff],
        element: [0xf5, 0xf5, 0xf5, 0xff],
        border_subtle: [0xd4, 0xd4, 0xd4, 0xff],
        text: [0x1a, 0x1a, 0x1a, 0xff],
        muted: [0x8a, 0x8a, 0x8a, 0xff],
        primary: [0x3b, 0x7d, 0xd8, 0xff],
        on_primary: [0xff, 0xff, 0xff, 0xff],
        error: [0xd1, 0x38, 0x3d, 0xff],
        success: [0x3d, 0x9a, 0x57, 0xff],
    };

    impl CrashMode {
        /// The palette for this mode.
        #[must_use]
        pub const fn palette(self) -> CrashPalette {
            match self {
                CrashMode::Dark => DARK,
                CrashMode::Light => LIGHT,
            }
        }
    }

    /// Copy affordance state (`error-component.tsx:15,47-53`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CopyState {
        #[default]
        Idle,
        Copied,
        Failed,
    }

    /// One horizontal action (`error-component.tsx:55-62`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrashAction {
        Copy,
        Restart,
        Quit,
    }

    impl CrashAction {
        /// Actions in render order (`error-component.tsx:56-62`).
        #[must_use]
        pub const fn all() -> [CrashAction; 3] {
            [CrashAction::Copy, CrashAction::Restart, CrashAction::Quit]
        }

        /// Shortcut char kept under the button (`error-component.tsx:172`).
        #[must_use]
        pub const fn key(self) -> char {
            match self {
                CrashAction::Copy => 'c',
                CrashAction::Restart => 'r',
                CrashAction::Quit => 'q',
            }
        }

        /// Label for the current copy state (`error-component.tsx:58`).
        #[must_use]
        pub fn label(self, copy: CopyState) -> &'static str {
            match self {
                CrashAction::Copy => match copy {
                    CopyState::Idle => "Copy report",
                    CopyState::Copied => "✓ Copied",
                    CopyState::Failed => "Copy failed",
                },
                CrashAction::Restart => "Restart",
                CrashAction::Quit => "Quit",
            }
        }
    }

    /// Key routed into the crash screen.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrashKey {
        Left,
        Right,
        Tab,
        ShiftTab,
        Enter,
        Up,
        Down,
        PageUp,
        PageDown,
        Home,
        End,
        Char(char),
        CtrlC,
    }

    /// Crash-screen reaction.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CrashOutcome {
        /// Run this action.
        Run(CrashAction),
        /// Ctrl+C exits immediately.
        Exit,
        /// The selection or scroll position moved.
        Moved,
        /// Nothing to scroll; consumed without effect.
        Ignored,
    }

    /// Largest index `<= index` that is a UTF-8 char boundary (never splits a
    /// multi-byte scalar). `index` is clamped to `s.len()`.
    fn floor_char_boundary(s: &str, index: usize) -> usize {
        let mut i = index.min(s.len());
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    /// Crash screen state.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CrashScreen {
        mode: CrashMode,
        version: String,
        message: String,
        stack: String,
        os: String,
        terminal: String,
        selected: usize,
        copy: CopyState,
        offset: usize,
    }

    impl CrashScreen {
        /// Build from a panic payload. `None` message/stack fall back to the
        /// upstream placeholders.
        #[must_use]
        pub fn new(
            mode: CrashMode,
            version: impl Into<String>,
            message: Option<&str>,
            stack: Option<&str>,
            os: impl Into<String>,
            terminal: impl Into<String>,
        ) -> Self {
            Self {
                mode,
                version: version.into(),
                message: message
                    .filter(|message| !message.is_empty())
                    .unwrap_or(UNKNOWN_MESSAGE)
                    .to_owned(),
                stack: stack
                    .filter(|stack| !stack.is_empty())
                    .unwrap_or(NO_STACK)
                    .to_owned(),
                os: os.into(),
                terminal: terminal.into(),
                selected: 0,
                copy: CopyState::Idle,
                offset: 0,
            }
        }

        #[must_use]
        pub const fn palette(&self) -> CrashPalette {
            self.mode.palette()
        }

        #[must_use]
        pub const fn mode(&self) -> CrashMode {
            self.mode
        }

        #[must_use]
        pub fn message(&self) -> &str {
            &self.message
        }

        #[must_use]
        pub fn stack(&self) -> &str {
            &self.stack
        }

        #[must_use]
        pub const fn selected(&self) -> usize {
            self.selected
        }

        #[must_use]
        pub const fn selected_action(&self) -> CrashAction {
            CrashAction::all()[self.selected % CrashAction::all().len()]
        }

        #[must_use]
        pub const fn copy_state(&self) -> CopyState {
            self.copy
        }

        #[must_use]
        pub const fn offset(&self) -> usize {
            self.offset
        }

        /// `contentWidth()` (`error-component.tsx:104`).
        #[must_use]
        pub fn content_width(cols: u16) -> u16 {
            CONTENT_MAX_WIDTH.min(CONTENT_MIN_WIDTH.max(cols.saturating_sub(4)))
        }

        /// `showSubtext()` (`error-component.tsx:105`).
        #[must_use]
        pub const fn show_subtext(rows: u16) -> bool {
            rows >= SUBTEXT_MIN_HEIGHT
        }

        /// `showFooter()` (`error-component.tsx:106`).
        #[must_use]
        pub const fn show_footer(rows: u16) -> bool {
            rows >= FOOTER_MIN_HEIGHT
        }

        /// Footer copy hint (`error-component.tsx:204-212`).
        #[must_use]
        pub const fn footer_hint(&self) -> &'static str {
            match self.copy {
                CopyState::Copied => "Report copied — paste it into a new GitHub issue.",
                CopyState::Failed => {
                    "Clipboard write failed. Try again or report the crash manually."
                }
                CopyState::Idle => "Copy the report and open a GitHub issue to help us fix this.",
            }
        }

        /// Footer version line (`error-component.tsx:213`).
        #[must_use]
        pub fn version_line(&self) -> String {
            format!("OpenCode {}", self.version)
        }

        /// Stack rows available below the fixed chrome, never under the
        /// three-row minimum (`error-component.tsx:183`).
        #[must_use]
        pub fn stack_viewport(&self, rows: u16) -> usize {
            let chrome = 1 // headline
                + 2 // error panel borders
                + 2 // action row + key row
                + if Self::show_subtext(rows) { 1 } else { 0 }
                + if Self::show_footer(rows) { 2 } else { 0 };
            usize::from(rows.saturating_sub(chrome).max(STACK_MIN_HEIGHT))
        }

        fn stack_rows(&self) -> Vec<&str> {
            self.stack.split('\n').collect()
        }

        /// Largest valid stack offset.
        #[must_use]
        pub fn stack_max_offset(&self, rows: u16) -> usize {
            self.stack_rows().len().saturating_sub(self.stack_viewport(rows))
        }

        /// Current stack window.
        #[must_use]
        pub fn stack_window(&self, rows: u16) -> Vec<&str> {
            let all = self.stack_rows();
            let viewport = self.stack_viewport(rows);
            let start = self.offset.min(self.stack_max_offset(rows));
            all[start..(start + viewport).min(all.len())].to_vec()
        }

        fn move_selection(&mut self, delta: isize) -> CrashOutcome {
            let len = CrashAction::all().len() as isize;
            let next = (self.selected as isize + delta).rem_euclid(len);
            self.selected = next as usize;
            CrashOutcome::Moved
        }

        fn scroll(&mut self, delta: isize, rows: u16) -> CrashOutcome {
            let max = self.stack_max_offset(rows);
            let next = if delta.is_negative() {
                self.offset.saturating_sub(delta.unsigned_abs())
            } else {
                self.offset.saturating_add(delta.unsigned_abs()).min(max)
            };
            let moved = next != self.offset;
            self.offset = next;
            if moved {
                CrashOutcome::Moved
            } else {
                CrashOutcome::Ignored
            }
        }

        /// Route a key (`error-component.tsx:69-100`).
        pub fn handle(&mut self, key: CrashKey, rows: u16) -> CrashOutcome {
            match key {
                CrashKey::CtrlC => CrashOutcome::Exit,
                CrashKey::Enter => CrashOutcome::Run(self.selected_action()),
                CrashKey::Left => self.move_selection(-1),
                CrashKey::Right | CrashKey::Tab => self.move_selection(1),
                CrashKey::ShiftTab => self.move_selection(-1),
                CrashKey::Up => self.scroll(-1, rows),
                CrashKey::Down => self.scroll(1, rows),
                CrashKey::PageUp => {
                    let page = self.stack_viewport(rows) as isize;
                    self.scroll(-page, rows)
                }
                CrashKey::PageDown => {
                    let page = self.stack_viewport(rows) as isize;
                    self.scroll(page, rows)
                }
                CrashKey::Home => {
                    self.offset = 0;
                    CrashOutcome::Moved
                }
                CrashKey::End => {
                    self.offset = self.stack_max_offset(rows);
                    CrashOutcome::Moved
                }
                CrashKey::Char('c') => CrashOutcome::Run(CrashAction::Copy),
                CrashKey::Char('r') => CrashOutcome::Run(CrashAction::Restart),
                CrashKey::Char('q') => CrashOutcome::Run(CrashAction::Quit),
                CrashKey::Char(_) => CrashOutcome::Ignored,
            }
        }

        /// Record the clipboard outcome (`error-component.tsx:47-53`).
        pub fn mark_copy(&mut self, copied: bool) {
            self.copy = if copied {
                CopyState::Copied
            } else {
                CopyState::Failed
            };
        }

        /// Issue URL with the stack truncated to fit
        /// (`error-component.tsx:221-257`).
        #[must_use]
        pub fn issue_url(&self) -> String {
            let head = format!(
                "The OpenCode TUI crashed with an unexpected error.\n\n**Error:** {}\n\n**Stack trace:**\n",
                self.message
            );
            let marker = TRUNCATION_MARKER;
            let build = |stack: &str| {
                let mut url = String::from(ISSUE_URL);
                url.push_str("&title=");
                url.push_str(&encode(&format!("TUI crash: {}", self.message)));
                url.push_str("&opencode-version=");
                url.push_str(&encode(&self.version));
                url.push_str("&os=");
                url.push_str(&encode(&self.os));
                url.push_str("&terminal=");
                url.push_str(&encode(&self.terminal));
                url.push_str("&reproduce=");
                url.push_str(&encode(
                    "Reported automatically from the OpenCode crash screen. If you can, describe what you were doing when it crashed.",
                ));
                url.push_str("&description=");
                url.push_str(&encode(&format!("{head}```\n{stack}\n```")));
                url
            };
            let full = build(&self.stack);
            if full.len() <= MAX_URL_LENGTH {
                return full;
            }
            // Largest raw stack prefix whose encoded URL (with the marker)
            // still fits.
            let mut lo = 0usize;
            let mut hi = self.stack.len();
            while lo < hi {
                let mid = (lo + hi + 1) / 2;
                let boundary = floor_char_boundary(&self.stack, mid);
                let candidate = build(&format!("{}{marker}", &self.stack[..boundary]));
                if candidate.len() <= MAX_URL_LENGTH {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            build(&format!(
                "{}{marker}",
                &self.stack[..floor_char_boundary(&self.stack, lo)]
            ))
        }
    }

    fn encode(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for byte in text.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char);
                }
                b' ' => out.push_str("%20"),
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        out
    }
}

#[cfg(test)]
mod error_and_crash_tests {
    use super::crash::*;
    use super::error_details::*;

    fn details() -> ErrorDetails {
        ErrorDetails::new(
            "Plugin failed",
            "connect ECONNREFUSED 127.0.0.1:4096\nwhile loading the plugin",
            Some("plugin: demo".to_owned()),
            Some("ref-42".to_owned()),
        )
        .with_source("~/.config/opencode/plugin.ts")
    }

    #[test]
    fn copied_text_and_prompt_match_upstream_order() {
        let details = details();
        assert_eq!(
            details.text(),
            "Plugin failed\nplugin: demo\nError: connect ECONNREFUSED 127.0.0.1:4096\nwhile loading the plugin\nReference: ref-42"
        );
        let prompt = details.prompt();
        assert!(prompt.starts_with(INVESTIGATE_HEADING));
        assert!(prompt.contains("Error: connect ECONNREFUSED"));
        assert!(prompt.contains("reference ref-42"));
        assert!(prompt.ends_with(INVESTIGATE_INSTRUCTION));
        assert_eq!(details.source(), Some("~/.config/opencode/plugin.ts"));
        assert_eq!(details.diagnostic_ref(), Some("ref-42"));
    }

    #[test]
    fn body_wraps_by_words_and_caps_at_twenty_rows() {
        let error = (0..80)
            .map(|n| format!("word{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let details = ErrorDetails::new("t", error, None, None);
        let rows = details.body_rows(20);
        assert!(rows.len() > MAX_BODY_HEIGHT);
        assert!(rows.iter().all(|row| row.chars().count() <= 20));
        assert_eq!(details.visible_rows(20).len(), MAX_BODY_HEIGHT);
    }

    #[test]
    fn scrolling_is_one_twenty_and_bounds_aware() {
        let error = (0..60).map(|n| format!("line{n}")).collect::<Vec<_>>().join("\n");
        let mut details = ErrorDetails::new("t", error, None, None);
        assert_eq!(details.body_rows(10).len(), 60);
        assert_eq!(details.max_offset(10), 40);
        assert_eq!(details.handle(ErrorKey::Down, 10), ErrorOutcome::Scrolled);
        assert_eq!(details.offset(), 1);
        assert_eq!(details.handle(ErrorKey::PageDown, 10), ErrorOutcome::Scrolled);
        assert_eq!(details.offset(), 21);
        assert_eq!(details.handle(ErrorKey::End, 10), ErrorOutcome::Scrolled);
        assert_eq!(details.offset(), 40);
        assert_eq!(details.handle(ErrorKey::Down, 10), ErrorOutcome::Ignored);
        assert_eq!(details.handle(ErrorKey::PageUp, 10), ErrorOutcome::Scrolled);
        assert_eq!(details.offset(), 20);
        assert_eq!(details.handle(ErrorKey::Home, 10), ErrorOutcome::Scrolled);
        assert_eq!(details.offset(), 0);
        assert_eq!(details.handle(ErrorKey::Up, 10), ErrorOutcome::Ignored);
        let rows = details.visible_rows(10);
        assert_eq!(rows.first().map(String::as_str), Some("line0"));
    }

    #[test]
    fn copy_investigate_and_back_actions_route_in_source_order() {
        let mut details = details();
        assert_eq!(details.handle(ErrorKey::Copy, 40), ErrorOutcome::Copy);
        assert_eq!(details.handle(ErrorKey::Investigate, 40), ErrorOutcome::Investigate);
        assert_eq!(details.handle(ErrorKey::Back, 40), ErrorOutcome::Back);
        assert!(!details.copied());
        assert_eq!(
            details.actions(),
            vec![
                ErrorAction { title: "investigate".to_owned(), key: Some('i') },
                ErrorAction { title: "copy details".to_owned(), key: Some('c') },
            ]
        );
        details.mark_copied();
        assert_eq!(details.actions()[1].title, "✓ copied");
        assert_eq!(details.actions()[1].key, None);
    }

    #[test]
    fn crash_content_width_and_responsive_thresholds() {
        assert_eq!(CrashScreen::content_width(200), 84);
        assert_eq!(CrashScreen::content_width(68), 64);
        assert_eq!(CrashScreen::content_width(20), 24);
        assert!(!CrashScreen::show_subtext(17));
        assert!(CrashScreen::show_subtext(18));
        assert!(!CrashScreen::show_footer(19));
        assert!(CrashScreen::show_footer(20));
    }

    #[test]
    fn crash_palette_is_local_and_mode_specific() {
        let dark = CrashMode::Dark.palette();
        let light = CrashMode::Light.palette();
        assert_ne!(dark.bg, light.bg);
        assert_eq!(dark.error, [0xe0, 0x6c, 0x75, 0xff]);
        assert_eq!(light.error, [0xd1, 0x38, 0x3d, 0xff]);
        assert_eq!(dark.on_primary, [0x0a, 0x0a, 0x0a, 0xff]);
    }

    #[test]
    fn crash_actions_move_horizontally_and_run_selected() {
        let mut screen = CrashScreen::new(CrashMode::Dark, "1.2.3", Some("boom"), None, "macos", "vt");
        assert_eq!(screen.message(), "boom");
        assert_eq!(screen.stack(), NO_STACK);
        assert_eq!(screen.selected_action(), CrashAction::Copy);
        assert_eq!(screen.handle(CrashKey::Right, 24), CrashOutcome::Moved);
        assert_eq!(screen.selected_action(), CrashAction::Restart);
        assert_eq!(screen.handle(CrashKey::Left, 24), CrashOutcome::Moved);
        assert_eq!(screen.selected_action(), CrashAction::Copy);
        // Wraps backwards to the last action.
        assert_eq!(screen.handle(CrashKey::Left, 24), CrashOutcome::Moved);
        assert_eq!(screen.selected_action(), CrashAction::Quit);
        assert_eq!(screen.handle(CrashKey::ShiftTab, 24), CrashOutcome::Moved);
        assert_eq!(screen.selected_action(), CrashAction::Restart);
        assert_eq!(screen.handle(CrashKey::Enter, 24), CrashOutcome::Run(CrashAction::Restart));
        assert_eq!(screen.handle(CrashKey::Char('c'), 24), CrashOutcome::Run(CrashAction::Copy));
        assert_eq!(screen.handle(CrashKey::Char('r'), 24), CrashOutcome::Run(CrashAction::Restart));
        assert_eq!(screen.handle(CrashKey::Char('q'), 24), CrashOutcome::Run(CrashAction::Quit));
        assert_eq!(screen.handle(CrashKey::Char('x'), 24), CrashOutcome::Ignored);
        assert_eq!(screen.handle(CrashKey::CtrlC, 24), CrashOutcome::Exit);
        // Labels and footer follow the copy state.
        assert_eq!(CrashAction::Copy.label(screen.copy_state()), "Copy report");
        screen.mark_copy(true);
        assert_eq!(CrashAction::Copy.label(screen.copy_state()), "✓ Copied");
        assert!(screen.footer_hint().contains("paste it into a new GitHub issue"));
        screen.mark_copy(false);
        assert_eq!(CrashAction::Copy.label(screen.copy_state()), "Copy failed");
        assert!(screen.footer_hint().contains("Clipboard write failed"));
        assert_eq!(screen.version_line(), "OpenCode 1.2.3");
    }

    #[test]
    fn crash_stack_scrolls_and_keeps_a_three_row_minimum() {
        let stack = (0..60).map(|n| format!("frame{n}")).collect::<Vec<_>>().join("\n");
        let mut screen =
            CrashScreen::new(CrashMode::Dark, "1", Some("boom"), Some(&stack), "macos", "vt");
        // Tiny terminal: the panel still reserves three rows.
        assert_eq!(screen.stack_viewport(1), 3);
        let viewport = screen.stack_viewport(24);
        assert!(viewport >= 3);
        assert_eq!(screen.stack_max_offset(24), 60 - viewport);
        assert_eq!(screen.handle(CrashKey::Down, 24), CrashOutcome::Moved);
        assert_eq!(screen.offset(), 1);
        assert_eq!(screen.handle(CrashKey::PageDown, 24), CrashOutcome::Moved);
        assert_eq!(screen.offset(), 1 + viewport);
        assert_eq!(screen.handle(CrashKey::End, 24), CrashOutcome::Moved);
        assert_eq!(screen.offset(), 60 - viewport);
        assert_eq!(screen.handle(CrashKey::End, 24), CrashOutcome::Moved);
        assert_eq!(screen.handle(CrashKey::Up, 24), CrashOutcome::Moved);
        assert_eq!(screen.handle(CrashKey::Home, 24), CrashOutcome::Moved);
        assert_eq!(screen.offset(), 0);
        let window = screen.stack_window(24);
        assert_eq!(window.first().copied(), Some("frame0"));
        assert_eq!(window.len(), viewport);
    }

    #[test]
    fn crash_issue_url_carries_the_report_and_truncates_the_stack() {
        let screen = CrashScreen::new(
            CrashMode::Dark,
            "1.2.3",
            Some("boom & bang"),
            Some("at a\nat b"),
            "macos",
            "vt",
        );
        let url = screen.issue_url();
        assert!(url.starts_with(ISSUE_URL));
        assert!(url.contains("title=TUI%20crash%3A%20boom%20%26%20bang"));
        assert!(url.contains("opencode-version=1.2.3"));
        assert!(url.contains("os=macos"));
        assert!(url.contains("terminal=vt"));
        assert!(url.contains("at%20a"));
        assert!(url.len() <= MAX_URL_LENGTH);

        let huge = (0..4_000)
            .map(|n| format!("frame {n} padding padding padding"))
            .collect::<Vec<_>>()
            .join("\n");
        let screen =
            CrashScreen::new(CrashMode::Dark, "1.2.3", Some("boom"), Some(&huge), "macos", "vt");
        let url = screen.issue_url();
        assert!(url.len() <= MAX_URL_LENGTH, "{}", url.len());
        let marker = "%0A%E2%80%A6%20%28truncated%29";
        assert!(url.contains(marker), "missing truncation marker");
    }

    #[test]
    fn crash_issue_url_truncates_multibyte_stack_without_panicking() {
        let multibyte = "日本語".repeat(2_000);
        let screen = CrashScreen::new(
            CrashMode::Dark,
            "1.2.3",
            Some("boom"),
            Some(&multibyte),
            "macos",
            "vt",
        );
        let url = screen.issue_url();
        assert!(url.len() <= MAX_URL_LENGTH, "{}", url.len());
        let marker = "%0A%E2%80%A6%20%28truncated%29";
        assert!(url.contains(marker), "missing truncation marker");

        let emoji = "é".repeat(4_000);
        let screen =
            CrashScreen::new(CrashMode::Dark, "1.2.3", Some("boom"), Some(&emoji), "macos", "vt");
        let url = screen.issue_url();
        assert!(url.len() <= MAX_URL_LENGTH, "{}", url.len());
        assert!(url.contains(marker), "missing truncation marker");
    }

    /// DIALOG-20: `open_crash` seeds the root overlay model, routes keys into
    /// it, and never lets the crash overlay reach the decision path.
    #[test]
    fn open_crash_seeds_the_overlay_and_routes_keys() {
        use super::{bounded_crash_text, DialogError, DialogKind, DialogView};

        let mut view = DialogView::new();
        view.open_crash(Some("boom"), Some("at a\nat b"), "1.2.3", CrashMode::Dark, "vt")
            .unwrap();
        assert!(view.is_open());
        assert_eq!(view.open_kind(), Some(DialogKind::Crash));
        let screen = view.crash_screen().expect("crash model seeded");
        assert_eq!(screen.message(), "boom");
        assert_eq!(screen.stack(), "at a\nat b");
        assert_eq!(screen.version_line(), "OpenCode 1.2.3");
        // Keys route into the crash model (copy/restart/quit), not the dialog
        // decision path.
        assert_eq!(view.handle_crash_key(CrashKey::Right, 24), Some(CrashOutcome::Moved));
        assert_eq!(
            view.handle_crash_key(CrashKey::Enter, 24),
            Some(CrashOutcome::Run(CrashAction::Restart))
        );
        assert_eq!(view.handle_crash_key(CrashKey::CtrlC, 24), Some(CrashOutcome::Exit));
        // A crash overlay is neither answerable nor cancellable.
        assert_eq!(view.answer().unwrap_err(), DialogError::NotAnswerable);
        assert_eq!(view.cancel().unwrap_err(), DialogError::NotAnswerable);

        // The layout fallback surfaces the bounded message and version, and
        // suppresses the generic dialog footer (the crash module owns it).
        let rows = view.render(80, 24);
        assert!(rows.iter().any(|row| row == "boom"), "{rows:?}");
        assert!(rows.iter().any(|row| row == "OpenCode 1.2.3"), "{rows:?}");
        assert!(!rows.iter().any(|row| row.contains("Enter")), "{rows:?}");

        // `bounded_crash_text` bounds by bytes and neutralizes control chars
        // (except newline/tab) without splitting a multi-byte scalar.
        assert_eq!(bounded_crash_text("a\u{1}b\tc\nd", 64), "a b\tc\nd");
        assert_eq!(bounded_crash_text("\u{1}\u{2}\u{3}tail", 3), "   ");
        assert_eq!(bounded_crash_text("ééé", 4), "éé");
    }

    /// DIALOG-20: the fallback message uses the upstream placeholder when the
    /// panic payload is absent or empty, and `handle_crash_key` is a no-op
    /// before the overlay opens.
    #[test]
    fn crash_overlay_uses_placeholders_and_is_inert_before_open() {
        use super::DialogView;

        let mut view = DialogView::new();
        assert_eq!(view.crash_screen(), None);
        assert_eq!(view.handle_crash_key(CrashKey::Enter, 24), None);

        view.open_crash(None, Some(""), "9.9.9", CrashMode::Light, "vt").unwrap();
        let screen = view.crash_screen().expect("crash model seeded");
        assert_eq!(screen.message(), UNKNOWN_MESSAGE);
        assert_eq!(screen.stack(), NO_STACK);
        assert_eq!(screen.mode(), CrashMode::Light);
    }

    /// DIALOG-20: a live panic is captured once and can seed, render, and
    /// dismiss the same crash surface used by the native loop.
    #[test]
    fn dialog20_live_panic_capture_opens_and_closes_crash_surface() {
        use super::{crash, DialogKind, DialogView};

        crash::install_panic_hook();
        // The panic slot is process-global. Drain it before catch_unwind so
        // this test cannot consume a panic recorded by an earlier test.
        let _ = crash::take_panic_capture();
        let result = std::panic::catch_unwind(|| panic!("dialog20 boom"));
        assert!(result.is_err());
        let message = crash::take_panic_capture().expect("panic hook captured payload");
        assert!(message.starts_with("dialog20 boom"), "{message}");
        assert!(message.contains("native_view_dialogs.rs"), "{message}");

        let mut view = DialogView::new();
        view.open_crash(Some(&message), None, "1.2.3", CrashMode::Dark, "vt")
            .unwrap();
        assert_eq!(view.open_kind(), Some(DialogKind::Crash));
        assert!(view.render(120, 32).iter().any(|row| row.contains("dialog20 boom")));
        assert_eq!(
            view.handle_crash_key(CrashKey::Char('r'), 32),
            Some(CrashOutcome::Run(CrashAction::Restart))
        );
        view.close_crash();
        assert_eq!(view.open_kind(), None);
        assert!(view.crash_screen().is_none());
    }

    /// DIALOG-20: the live render arm paints the full upstream layout
    /// (subtext, ` Error ` panel, actions row with shortcut keys, stack
    /// panel, footer hint + version) from a controlled payload through
    /// `render_with_metrics`, the public render API the native loop's modal
    /// painter uses (`tui_entry.rs`).
    #[test]
    fn dialog20_render_paints_full_upstream_layout() {
        use super::{DialogLayoutMetrics, DialogViewportPolicy, DialogView};

        let stack = (0..40)
            .map(|frame| format!("frame {frame}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = DialogView::new();
        view.open_crash(Some("boom"), Some(&stack), "1.2.3", CrashMode::Dark, "vt")
            .unwrap();

        let metrics = DialogLayoutMetrics::new(110, 30, 30, DialogViewportPolicy::ContentBudget);
        let rows = view.render_with_metrics(metrics);
        // Headline and subtext (`error-component.tsx:119-123`).
        assert!(rows.iter().any(|row| row == HEADLINE), "{rows:?}");
        assert!(rows.iter().any(|row| row == SUBTEXT), "{rows:?}");
        // Error message panel (`error-component.tsx:129-141`).
        assert!(rows.iter().any(|row| row == " Error "), "{rows:?}");
        assert!(rows.iter().any(|row| row == "boom"), "{rows:?}");
        // Actions row + centered shortcut-key row
        // (`error-component.tsx:143-179`), first action selected.
        let labels_index = rows
            .iter()
            .position(|row| row.contains("Copy report") && row.contains("Restart") && row.contains("Quit"))
            .expect("actions row");
        assert!(rows[labels_index].contains("[ Copy report ]"), "{:?}", rows[labels_index]);
        let keys: Vec<&str> = rows[labels_index + 1].split_whitespace().collect();
        assert_eq!(keys, vec!["c", "r", "q"], "{:?}", rows[labels_index + 1]);
        // Stack panel paints the model's window (`error-component.tsx:183-202`).
        assert!(rows.iter().any(|row| row == " Stack trace "), "{rows:?}");
        assert!(rows.iter().any(|row| row == "frame 0"), "{rows:?}");
        // Footer (`error-component.tsx:204-216`): hint then version; the
        // generic dialog footer stays suppressed.
        let hint = view.crash_screen().unwrap().footer_hint();
        assert!(rows.iter().any(|row| row == hint), "{rows:?}");
        assert!(rows.iter().any(|row| row == "OpenCode 1.2.3"), "{rows:?}");
        assert!(!rows.iter().any(|row| row.contains("click here confirm")), "{rows:?}");
    }

    /// DIALOG-20: the responsive thresholds hide subtext under 18 rows and
    /// the footer under 20, while the stack panel keeps its three-row
    /// minimum even on tiny terminals.
    #[test]
    fn dialog20_render_respects_subtext_and_footer_thresholds() {
        use super::{DialogLayoutMetrics, DialogViewportPolicy, DialogView};

        let long_stack = (0..30)
            .map(|frame| format!("frame {frame}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = DialogView::new();
        view.open_crash(Some("boom"), Some(&long_stack), "1.2.3", CrashMode::Dark, "vt")
            .unwrap();

        let render = |rows: u16| {
            view.render_with_metrics(DialogLayoutMetrics::new(90, usize::from(rows), usize::from(rows), DialogViewportPolicy::ContentBudget))
        };
        // Short: no subtext, no footer, message + keyed actions still painted.
        let short = render(12);
        assert!(!short.iter().any(|row| row == SUBTEXT), "{short:?}");
        assert!(!short.iter().any(|row| row == "OpenCode 1.2.3"), "{short:?}");
        assert!(short.iter().any(|row| row == "boom"), "{short:?}");
        assert!(short.iter().any(|row| row == " Stack trace "), "{short:?}");
        let stack_rows = short
            .iter()
            .skip_while(|row| row.as_str() != " Stack trace ")
            .skip(1)
            .filter(|row| row.starts_with("frame "))
            .count();
        assert_eq!(stack_rows, 5, "{short:?}");
        // Medium (18..20): subtext appears, footer still absent.
        let medium = render(18);
        assert!(medium.iter().any(|row| row == SUBTEXT), "{medium:?}");
        assert!(!medium.iter().any(|row| row == "OpenCode 1.2.3"), "{medium:?}");
        // Tall: footer returns.
        let tall = render(20);
        assert!(tall.iter().any(|row| row == "OpenCode 1.2.3"), "{tall:?}");
    }

    /// DIALOG-20: render reflects live model movement - the bracketed action
    /// follows selection and the stack window follows scroll offset, the
    /// same way `handle_crash_key` drives the live overlay
    /// (`tui_entry.rs` crash routing).
    #[test]
    fn dialog20_render_tracks_selection_and_stack_scroll() {
        use super::{DialogLayoutMetrics, DialogViewportPolicy, DialogView};

        let stack = (0..30)
            .map(|frame| format!("frame {frame}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = DialogView::new();
        view.open_crash(Some("boom"), Some(&stack), "1.2.3", CrashMode::Dark, "vt")
            .unwrap();
        let metrics = DialogLayoutMetrics::new(110, 30, 30, DialogViewportPolicy::ContentBudget);

        // Selection moves the bracket; the shortcut-key row is unchanged.
        assert_eq!(
            view.handle_crash_key(CrashKey::Right, 30),
            Some(CrashOutcome::Moved)
        );
        let rows = view.render_with_metrics(metrics);
        let labels_index = rows
            .iter()
            .position(|row| row.contains("Copy report") && row.contains("Restart"))
            .expect("actions row");
        assert!(rows[labels_index].contains("[ Restart ]"), "{:?}", rows[labels_index]);
        assert!(!rows[labels_index].contains("[ Copy report ]"), "{:?}", rows[labels_index]);

        // Scrolling advances the stack window past the first frame
        // (`error-component.tsx:84-91`, two-panel scrollbox).
        assert!(rows.iter().any(|row| row == "frame 0"), "{rows:?}");
        assert_eq!(
            view.handle_crash_key(CrashKey::Down, 30),
            Some(CrashOutcome::Moved)
        );
        let scrolled = view.render_with_metrics(metrics);
        assert!(!scrolled.iter().any(|row| row == "frame 0"), "{scrolled:?}");
        assert!(scrolled.iter().any(|row| row == "frame 1"), "{scrolled:?}");
    }
}

#[cfg(test)]
mod f002r3_permission_navigation_tests {
    use super::DialogView;

    /// F002-R3: upstream registers `left,h` / `right,l` only when
    /// `keys.length > 1` (`permission.tsx:465-485`). A single-option prompt is
    /// inert; a two-plus option prompt cycles.
    #[test]
    fn f002r3_h_l_cycle_only_when_more_than_one_option() {
        // 2 options (no `save` rules): `once`, `reject`.
        let mut single = DialogView::new();
        single
            .open_permission_with_persistent("p", "s", "bash", false)
            .unwrap();
        assert_eq!(single.permission_option_count(), Some(2));
        let before = single.focused_choice().map(str::to_owned);
        assert!(single.permission_cycle(true), "two options cycle forward");
        assert_ne!(single.focused_choice().map(str::to_owned), before);

        // A one-option permission prompt never moves.
        let mut one = DialogView::new();
        one.open_request(super::DialogRequest {
            id: "p1".into(),
            session_id: "s".into(),
            kind: super::DialogKind::Permission,
            title: "t".into(),
            choices: vec!["Allow once".into()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
        .unwrap();
        assert_eq!(one.permission_option_count(), Some(1));
        let before = one.focused_choice().map(str::to_owned);
        assert!(!one.permission_cycle(true), "single option is inert");
        assert_eq!(one.focused_choice().map(str::to_owned), before);

        // A non-permission dialog reports no permission option count.
        let mut select = DialogView::new();
        assert_eq!(select.permission_option_count(), None);
        assert!(!select.permission_cycle(true));
    }
}

#[cfg(test)]
mod diff_dialog_tests {
    use super::diff_dialog::*;

    fn file(path: &str, status: DiffStatus, lines: usize) -> DiffFile {
        let patch = (0..lines)
            .map(|n| format!("+line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        DiffFile::new(path, 3, 1, status).with_patch(patch)
    }

    fn two_files() -> DiffDialog {
        DiffDialog::new(vec![
            file("src/alpha.rs", DiffStatus::Modified, 4),
            file("src/beta.rs", DiffStatus::Modified, 3),
        ])
    }

    #[test]
    fn full_layout_boundary_is_eighty_eight() {
        let dialog = two_files();
        assert!(dialog.layout(87).compact);
        assert!(!dialog.layout(88).compact);
        assert_eq!(dialog.layout(87).inset, 0);
        assert_eq!(dialog.layout(88).inset, 2);
        // A docked pane is always compact, however wide it is.
        let mut docked = two_files();
        docked.set_docked(true);
        assert!(docked.layout(200).compact);
        assert_eq!(docked.layout(200).inset, 0);
    }

    #[test]
    fn tree_width_is_thirty_to_forty_and_patch_pane_splits_at_100() {
        assert_eq!(file_tree_width(88), 30);
        assert_eq!(file_tree_width(120), 30);
        assert_eq!(file_tree_width(160), 40);
        assert_eq!(file_tree_width(400), 40);
        // Tree + insets: the tree grows with the terminal, so the boundary is
        // 137 -> 99 (unified), 138 -> 100 (split).
        assert_eq!(file_tree_width(137), 34);
        assert_eq!(patch_pane_width(137, Some(file_tree_width(137)), 2), 99);
        assert!(!two_files().layout(137).split_available);
        assert_eq!(patch_pane_width(138, Some(file_tree_width(138)), 2), 100);
        assert!(two_files().layout(138).split_available);
        // No tree on a narrow full screen: 103 -> 99, 104 -> 100.
        let mut no_tree = two_files();
        no_tree.set_tree_enabled(false);
        assert!(!no_tree.layout(103).split_available);
        assert!(no_tree.layout(104).split_available);
        assert_eq!(no_tree.layout(104).tree_width, None);
        // Empty payloads never show the tree.
        assert!(!DiffDialog::new(Vec::new()).layout(200).tree_width.is_some());
    }

    #[test]
    fn added_and_deleted_files_always_render_unified() {
        let mut dialog = DiffDialog::new(vec![
            file("new.rs", DiffStatus::Added, 2),
            file("gone.rs", DiffStatus::Deleted, 2),
            file("edit.rs", DiffStatus::Modified, 2),
        ]);
        dialog.set_tree_enabled(false);
        let layout = dialog.layout(200);
        assert!(layout.split_available);
        assert_eq!(layout.view, DiffView::Split);
        assert_eq!(dialog.view_for(&layout, 0), DiffView::Unified);
        assert_eq!(dialog.view_for(&layout, 1), DiffView::Unified);
        assert_eq!(dialog.view_for(&layout, 2), DiffView::Split);
    }

    #[test]
    fn toggle_view_is_refused_below_the_split_boundary() {
        let mut dialog = two_files();
        assert!(!dialog.toggle_view(100));
        assert_eq!(dialog.view_override(), None);
        assert_eq!(dialog.layout(100).view, DiffView::Unified);
        assert!(dialog.toggle_view(140));
        assert_eq!(dialog.layout(140).view, DiffView::Unified);
        assert!(dialog.toggle_view(140));
        assert_eq!(dialog.layout(140).view, DiffView::Split);
    }

    #[test]
    fn sticky_header_offsets_are_clamped_to_their_own_card() {
        // 2 header rows + 8 body rows = 10.
        assert_eq!(sticky_header_offset(0, 0, 10), 0);
        assert_eq!(sticky_header_offset(1, 0, 10), 1);
        assert_eq!(sticky_header_offset(8, 0, 10), 8);
        assert_eq!(sticky_header_offset(50, 0, 10), 8);
        // A card that starts below the viewport does not stick yet.
        assert_eq!(sticky_header_offset(5, 10, 10), 0);
        // A header-only card has no travel.
        assert_eq!(sticky_header_offset(5, 0, 2), 0);
    }

    #[test]
    fn sticky_header_pins_the_card_while_its_body_scrolls() {
        let dialog = DiffDialog::new(vec![file("src/alpha.rs", DiffStatus::Modified, 20)]);
        assert_eq!(dialog.pinned_file(0), None);
        assert_eq!(dialog.pinned_file(3), Some(0));
        let rows = dialog.render(134, 10, 3, &DiffKeys::default());
        assert!(rows[0].text.contains("src/alpha.rs"), "{:?}", rows[0]);
        assert_eq!(rows[0].surface, DiffSurface::Context);
        // Once the header has reached its card bottom it scrolls away.
        assert_eq!(dialog.pinned_file(22), None);
    }

    #[test]
    fn reviewed_files_collapse_their_body_and_mark_the_card() {
        let mut dialog = two_files();
        let effect = dialog.toggle_reviewed(0);
        assert!(effect.reviewed);
        assert_eq!(dialog.review_count(), "1/2");
        let rows = dialog.render(134, 40, 0, &DiffKeys::default());
        let header = rows
            .iter()
            .position(|row| row.text.contains("src/alpha.rs"))
            .expect("first header");
        assert_eq!(rows[header].surface, DiffSurface::Reviewed);
        assert!(rows[header].text.contains('✓'), "{}", rows[header].text);
        // The header's bottom padding row is the reviewed half-block gap.
        assert!(rows[header + 1].text.starts_with('▀'), "{:?}", rows[header + 1]);
        // No patch body for the reviewed file: the next row is the separator.
        assert_eq!(rows[header + 2].text.chars().next(), Some(CARD_GAP_TOP));
        // The unreviewed file keeps its body.
        assert!(rows.iter().any(|row| row.text.contains("+line 0")));
        // Unreviewing restores the body and the counter.
        dialog.toggle_reviewed(0);
        assert_eq!(dialog.review_count(), "0/2");
        let rows = dialog.render(134, 40, 0, &DiffKeys::default());
        assert!(!rows[1].text.starts_with('▀'), "{:?}", rows[1]);
    }

    #[test]
    fn file_menu_is_nineteen_by_one_and_clamped() {
        let mut dialog = two_files();
        assert!(dialog.open_menu(1, 5, 5));
        assert_eq!(dialog.menu_label(), Some("Mark complete"));
        let rect = dialog.menu().unwrap().rect(80, 24);
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (5, 6, 19, 1));
        // Clamped inside the terminal.
        assert_eq!(dialog.menu().unwrap().rect(20, 24).width, 19);
        dialog.open_menu(1, 79, 30);
        let rect = dialog.menu().unwrap().rect(80, 24);
        assert_eq!((rect.x, rect.y), (61, 23));
        dialog.open_menu(1, 0, 30);
        assert_eq!(dialog.menu().unwrap().rect(80, 24).y, 23);
        // Unknown indexes never open a menu.
        assert!(!dialog.open_menu(9, 0, 0));
        assert_eq!(FILE_MENU_Z_INDEX, 2600);
    }

    #[test]
    fn running_the_file_menu_closes_it_before_toggling() {
        let mut dialog = two_files();
        dialog.open_menu(0, 0, 0);
        let effect = dialog.run_menu().expect("menu ran");
        assert_eq!(dialog.menu(), None);
        assert!(effect.reviewed);
        assert!(dialog.is_reviewed(0));
        // Reopening shows the inverse label.
        dialog.open_menu(0, 0, 0);
        assert_eq!(dialog.menu_label(), Some("Mark incomplete"));
        dialog.close_menu();
        assert_eq!(dialog.menu(), None);
        assert!(dialog.run_menu().is_none());
    }

    #[test]
    fn single_patch_review_advances_or_exhausts() {
        let mut dialog = two_files();
        dialog.set_single_patch(true);
        let effect = dialog.toggle_reviewed(0);
        assert_eq!(effect.advanced, Some(1));
        assert_eq!(dialog.selected(), Some(1));
        assert_eq!(dialog.visible_files(), vec![1]);
        // A single-file viewer exhausts instead of advancing.
        let mut single = DiffDialog::new(vec![file("only.rs", DiffStatus::Modified, 2)]);
        single.set_single_patch(true);
        let effect = single.toggle_reviewed(0);
        assert!(effect.exhausted);
        assert!(single.mark_reviewed_blocked());
        single.select(0);
        assert!(!single.mark_reviewed_blocked());
        // Menu-driven review of a different file does not navigate.
        let mut other = two_files();
        other.set_single_patch(true);
        other.select(0);
        other.open_menu(1, 0, 0);
        let effect = other.run_menu().unwrap();
        assert!(effect.reviewed);
        assert_eq!(effect.advanced, None);
        assert_eq!(other.selected(), Some(0));
    }

    #[test]
    fn compact_footer_and_full_help_widget() {
        let mut dialog = two_files();
        dialog.set_docked(true);
        let layout = dialog.layout(120);
        let footer = dialog.footer_rows(&layout, &DiffKeys::default());
        assert_eq!(footer.len(), 3);
        assert!(footer[0].text.chars().all(|c| c == FOOTER_TRANSITION));
        assert!(footer[1].text.contains("focus prompt"), "{:?}", footer[1]);
        assert!(footer[1].text.contains("see all"), "{:?}", footer[1]);
        assert_eq!(footer[2].text, "");
        // A narrow full screen keeps the footer but not the prompt hint.
        let mut narrow = two_files();
        narrow.set_docked(false);
        let layout = narrow.layout(80);
        let footer = narrow.footer_rows(&layout, &DiffKeys::default());
        assert_eq!(footer.len(), 1);
        assert!(footer[0].text.contains(" close"), "{:?}", footer[0]);
        assert!(narrow.help_widget(&layout).is_none());
        // Full layout without the tree: no footer, help widget top-right.
        let mut full = two_files();
        full.set_tree_enabled(false);
        let layout = full.layout(200);
        assert!(!full.tree_enabled());
        assert!(full.footer_rows(&layout, &DiffKeys::default()).is_empty());
        let help = full.help_widget(&layout).expect("help widget");
        assert_eq!((help.x, help.y, help.width, help.height), (layout.patch_width - 1, 0, 1, 1));
        // With the tree shown the widget lives in the tree header instead.
        assert!(two_files().help_widget(&two_files().layout(200)).is_none());
    }

    #[test]
    fn no_patch_and_empty_states_render_stable_text() {
        let dialog = DiffDialog::new(vec![DiffFile::new("bin.dat", 0, 0, DiffStatus::Modified)]);
        let rows = dialog.render(134, 20, 0, &DiffKeys::default());
        assert!(rows.iter().any(|row| row.text.contains(NO_PATCH)));
        let empty = DiffDialog::new(Vec::new());
        let rows = empty.render(134, 4, 0, &DiffKeys::default());
        assert_eq!(rows.len(), 4);
        assert!(rows[0].text.contains(NO_CHANGES));
        // A zero-height frame paints nothing.
        assert!(empty.render(134, 0, 0, &DiffKeys::default()).is_empty());
    }

    #[test]
    fn review_counter_and_source_picker_match_upstream() {
        let mut dialog = two_files();
        assert_eq!(dialog.review_count(), "0/2");
        dialog.toggle_reviewed(1);
        assert_eq!(dialog.review_count(), "1/2");
        assert_eq!(DiffSource::Branch.label(), "All");
        assert_eq!(DiffSource::Turn.label(), "Last turn");
        assert_eq!(DiffSource::Working.description(), "Local changes only");
        assert!(DiffSource::Committed.needs_base());
        assert!(!DiffSource::Working.needs_base());
        assert_eq!(DiffSource::sources(false).len(), 3);
        assert_eq!(DiffSource::sources(true).len(), 4);
        assert_eq!(DiffStatus::Added.code(), 'A');
        assert_eq!(DiffStatus::Deleted.code(), 'D');
        assert_eq!(DiffStatus::Modified.code(), 'M');
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
    fn save_ineligible_prompt_keeps_once_and_reject_only() {
        let mut dialog = DialogView::new();
        dialog.open_permission_with_persistent("request", "session", "permission", false).unwrap();
        let rows = dialog.render(80, 24);
        assert!(rows.iter().any(|row| row.contains("Allow once")));
        assert!(rows.iter().any(|row| row.contains("Reject")));
        // The save gate drops ONLY the persistent `always` option
        // (`permission.tsx:232-240`); the two non-persistent choices remain.
        assert!(!rows.iter().any(|row| row.contains("Always allow")));
        assert!(!rows.iter().any(|row| row.contains("for this session")));
        assert!(!rows.iter().any(|row| row.contains("Always deny")));
        assert!(dialog.select(1));
        assert!(matches!(dialog.answer().unwrap(), DecisionIntent::Reject { .. }));
    }
    #[test]
    fn save_eligible_prompt_exposes_once_always_reject_in_order() {
        let mut dialog = DialogView::new();
        dialog.open_permission_with_persistent("request", "session", "permission", true).unwrap();
        let rows = dialog.render(80, 24);
        let once = rows.iter().position(|row| row.contains("Allow once")).unwrap();
        let always = rows.iter().position(|row| row.contains("Always allow")).unwrap();
        let reject = rows.iter().position(|row| row.contains("Reject")).unwrap();
        assert!(once < always && always < reject, "upstream option order once, always, reject");
        // Index 1 is `always`: a persistent, scoped allow for the project.
        assert!(dialog.select(1));
        assert!(matches!(dialog.answer().unwrap(), DecisionIntent::ScopedPermission {
            allow: true, persistent: true, ..
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
            description: None,
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
            description: None,
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
            description: None,
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
            description: None,
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

    // DIALOG-02: the live new-designs shell threads through the dialog layout.
    // `ui/dialog.tsx:109` drops the title bold weight under the experiment
    // (`attributes={newDesigns() ? undefined : TextAttributes.BOLD}`), painting
    // it in the base text color. The Permission heading keeps its Warning role:
    // that is a security signal, not a decoration the experiment toggles. The
    // footer gap row is NOT toggled: `dialog.tsx:159` yields `0` for the default
    // `footerGap` of 1 in both states and `1` for the `footerGap={2}` callers,
    // so new-designs never removes it.
    #[test]
    fn new_designs_shell_changes_title_role() {
        // Legacy default: bold Heading title.
        let mut legacy = DialogView::new();
        legacy.open_request(DialogRequest {
            id: "d02-legacy".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Confirm,
            title: "Confirm change".to_string(),
            choices: vec!["yes".to_string(), "no".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        }).unwrap();
        assert!(!legacy.new_designs());
        let legacy_rows = legacy.render_styled(56, 12);
        assert_eq!(legacy_rows[0].role, crate::native_ui_style::Role::Heading);
        let legacy_footer = legacy_rows.iter().position(|l| l.text.contains("Enter")).unwrap();
        assert_eq!(legacy_rows[legacy_footer - 1].text, "", "legacy keeps the gap row");

        // New designs: base-color (unbolded) title; the gap row is unchanged.
        let mut designed = DialogView::new();
        designed.set_new_designs(true);
        designed.open_request(DialogRequest {
            id: "d02-new".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Confirm,
            title: "Confirm change".to_string(),
            choices: vec!["yes".to_string(), "no".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        }).unwrap();
        assert!(designed.new_designs());
        let designed_rows = designed.render_styled(56, 12);
        assert_eq!(designed_rows[0].role, crate::native_ui_style::Role::Item);
        let designed_footer = designed_rows.iter().position(|l| l.text.contains("Enter")).unwrap();
        assert_eq!(designed_rows[designed_footer - 1].text, "", "gap row is state-independent");
        // The permission heading is still a security signal under new designs.
        let mut perm = open_permission();
        perm.set_new_designs(true);
        assert_eq!(perm.render_styled(56, 12)[0].role, crate::native_ui_style::Role::Warning);
    }

    #[test]
    fn dialog02_live_render_paints_description_only_for_new_designs() {
        // DIALOG-02: exercise the production request/layout/render path, not
        // only the palette helper. Upstream `ui/dialog.tsx:105-119` shows the
        // description directly below the title only when both the experiment
        // and the caller-provided description are present.
        let request = || DialogRequest {
            id: "d02-description".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Confirm,
            title: "Confirm change".to_string(),
            choices: vec!["yes".to_string(), "no".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: Some("This action changes the project".to_string()),
        };

        let mut designed = DialogView::new();
        designed.set_new_designs(true);
        designed.open_request(request()).unwrap();
        let designed_rows = designed.render_styled(56, 12);
        assert_eq!(designed_rows[0].text, "Confirm change");
        assert_eq!(designed_rows[1].text, "This action changes the project");
        assert_eq!(designed_rows[1].role, crate::native_ui_style::Role::Description);

        let mut legacy = DialogView::new();
        legacy.open_request(request()).unwrap();
        let legacy_rows = legacy.render_styled(56, 12);
        assert_eq!(legacy_rows[0].text, "Confirm change");
        assert!(
            legacy_rows.iter().all(|line| line.text != "This action changes the project"),
            "legacy dialog must not paint the description"
        );

        let mut no_description = DialogView::new();
        no_description.set_new_designs(true);
        let mut without_description = request();
        without_description.description = None;
        no_description.open_request(without_description).unwrap();
        let rows = no_description.render_styled(56, 12);
        assert!(rows.iter().all(|line| line.text != "This action changes the project"));
        assert_eq!(rows[1].text, "");
    }

    #[test]
    fn dialog02_prompt_footer_gap_adds_body_padding_under_new_designs() {
        // DIALOG-02: `ui/dialog.tsx:159` — a footer with `footerGap={2}` (the
        // live `dialog-prompt.tsx:133` caller) adds one body bottom padding row
        // under the new-designs experiment, and none in legacy. The live layout
        // now reads the tested `dialog::body_bottom_padding` helper.
        let mut legacy = DialogView::new();
        legacy.open_prompt("p", "s", "Rename", "value", None).unwrap();
        let legacy_rows = legacy.render_styled(56, 14);
        let legacy_footer = legacy_rows
            .iter()
            .position(|l| l.text.contains("Enter"))
            .expect("prompt footer");
        assert_eq!(legacy_rows[legacy_footer - 1].text, "", "legacy keeps only the gap row");

        let mut designed = DialogView::new();
        designed.set_new_designs(true);
        designed.open_prompt("p", "s", "Rename", "value", None).unwrap();
        let designed_rows = designed.render_styled(56, 14);
        let designed_footer = designed_rows
            .iter()
            .position(|l| l.text.contains("Enter"))
            .expect("prompt footer");
        assert_eq!(designed_rows[designed_footer - 1].text, "", "gap row");
        assert_eq!(designed_rows[designed_footer - 2].text, "", "body padding row");
        assert_eq!(
            designed_footer,
            legacy_footer + 1,
            "footerGap=2 adds exactly one body padding row"
        );
    }

    #[test]
    fn prompt_dialog_edits_value_and_reports_busy_state() {
        // DIALOG-10: `ui/dialog-prompt.tsx` — a one-row text input seeded with
        // an initial value. Enter submits the current value; Escape cancels;
        // both are suppressed while busy (`:43-46`, `:93-100`, `:168-170`).
        let mut view = DialogView::new();
        view.open_prompt("rename", "sess-1", "Rename session", "Old title", None)
            .unwrap();
        assert_eq!(view.open_kind(), Some(DialogKind::Prompt));
        assert!(view.intercepts_input());
        assert_eq!(view.prompt_value(), Some("Old title"));
        // Editing: append then delete.
        assert!(view.prompt_push('!'));
        assert_eq!(view.prompt_value(), Some("Old title!"));
        assert!(view.prompt_backspace());
        assert_eq!(view.prompt_value(), Some("Old title"));
        // A second open is refused while one is visible.
        assert_eq!(
            view.open_prompt("rename", "sess-2", "Rename session", "x", None),
            Err(DialogError::AlreadyOpen)
        );
        // Rendered value + the exact upstream footer.
        let rows = view.render(56, 12);
        assert!(rows.iter().any(|row| row.contains("Rename session")), "{rows:?}");
        assert!(rows.iter().any(|row| row.contains("Old title")), "{rows:?}");
        assert!(
            rows.iter().any(|row| row.contains("Enter: save · Esc: cancel")),
            "{rows:?}"
        );
        // Enter submits the current value while focused.
        assert_eq!(view.prompt_submit().as_deref(), Some("Old title"));
        assert!(view.prompt_escape());
        // Busy suspends editing, blocks submit and suppresses Escape.
        view.prompt_set_busy(true);
        assert_eq!(view.prompt_submit(), None);
        assert!(!view.prompt_escape());
        assert!(!view.prompt_push('x'));
        assert!(!view.prompt_backspace());
        let busy_rows = view.render(56, 12);
        assert!(
            busy_rows.iter().any(|row| row.contains("Working…")),
            "{busy_rows:?}"
        );
        // Cancel clears the whole dialog.
        view.close();
        assert!(!view.is_open());
        assert_eq!(view.prompt_value(), None);
    }

    #[test]
    fn mcp_dialog_lists_sorted_servers_with_status_words_and_details() {
        // SIDE-08: the row-click dialog is a local select list of servers
        // (`component/dialog-mcp.tsx`): the clicked server is focused, each
        // row carries the exact upstream status word, and a failed server's
        // error is the details body. Enter is not a decision; Esc closes
        // locally.
        let mut view = DialogView::new();
        let servers = vec![
            ("alpha".to_string(), "Connected ✓".to_string()),
            ("beta".to_string(), "Failed !".to_string()),
        ];
        view.open_mcp(&servers, Some(1), Some("boom".to_string()))
            .unwrap();
        assert_eq!(view.open_kind(), Some(DialogKind::Mcp));
        assert!(view.intercepts_input());
        assert_eq!(view.focused_choice(), Some("beta"));
        let rows = view.render(56, 14);
        assert!(rows.iter().any(|row| row.contains("MCP servers")), "{rows:?}");
        assert!(
            rows.iter().any(|row| row.contains("alpha") && row.contains("Connected ✓")),
            "{rows:?}"
        );
        assert!(
            rows.iter().any(|row| row.contains("> beta") && row.contains("Failed !")),
            "{rows:?}"
        );
        assert!(rows.iter().any(|row| row.contains("boom")), "{rows:?}");
        // Enter is not a permission decision for this kind; the caller reads
        // `focused_choice` and performs the toggle itself.
        assert_eq!(view.answer().unwrap_err(), DialogError::NotAnswerable);
        assert!(view.is_open(), "a refused answer must not close the list");
        // Esc closes locally (no pending decision, no fabricated intent).
        view.close();
        assert!(!view.is_open());
        assert_eq!(view.open_kind(), None);
    }

    #[test]
    fn mcp_dialog_without_focus_selects_the_first_server() {
        // `DialogSelect` seeds `selected: 0` when no `initialServer` matches
        // (`dialog-select.tsx:152`).
        let mut view = DialogView::new();
        let servers = vec![
            ("alpha".to_string(), "Disabled ○".to_string()),
            ("beta".to_string(), "Sign in required →".to_string()),
        ];
        view.open_mcp(&servers, None, None).unwrap();
        assert_eq!(view.focused_choice(), Some("alpha"));
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
            description: None,
        }
    }

    fn open_permission() -> DialogView {
        let mut view = DialogView::new();
        view.open_request(permission_request())
            .expect("valid request");
        view
    }

    #[test]
    fn execute_kind_is_content_only_and_not_answerable() {
        // `dialog-execute.tsx:28` is a content viewer: empty choices open, and
        // neither answer nor cancel produces a decision intent.
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "exe-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Execute,
            title: "execute".to_string(),
            choices: Vec::new(),
            message: Some("print(1)\n\n1".to_string()),
            disabled: Vec::new(),
            error: None,
            description: None,
        })
        .expect("execute viewer opens without choices");
        assert!(view.is_open());
        assert_eq!(
            view.production_viewport_policy(),
            DialogViewportPolicy::ContentBudget
        );
        assert_eq!(view.answer().unwrap_err(), DialogError::NotAnswerable);
        assert_eq!(view.cancel().unwrap_err(), DialogError::NotAnswerable);
        assert!(view.is_open());
    }

    #[test]
    fn select_kind_opens_with_terminal_viewport_and_answers_like_question() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "sel-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Select,
            title: "Pick a model".to_string(),
            choices: vec!["gpt".to_string(), "claude".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
        .expect("select request valid");
        assert!(view.is_open());
        assert_eq!(
            view.production_viewport_policy(),
            DialogViewportPolicy::SelectTerminal
        );
        view.next();
        let intent = view.answer().expect("select answer");
        assert!(
            matches!(intent, DecisionIntent::Answer { choice_index: 1, .. }),
            "select answer carries the choice index, got {intent:?}"
        );
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
                description: None,
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
            description: None,
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
                description: None,
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
            description: None,
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
            description: None,
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
                description: None,
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
            description: None,
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
            description: None,
            ..permission_request()
        };
        assert_eq!(
            bounded.open_request(too_many),
            Err(DialogError::TooManyChoices)
        );
        let long_id = DialogRequest {
            id: "i".repeat(MAX_ID_LEN + 1),
            description: None,
            ..permission_request()
        };
        assert_eq!(bounded.open_request(long_id), Err(DialogError::IdTooLong));
        let long_title = DialogRequest {
            title: "t".repeat(MAX_TITLE_LEN + 1),
            description: None,
            ..permission_request()
        };
        assert_eq!(
            bounded.open_request(long_title),
            Err(DialogError::TitleTooLong)
        );
        let long_choice = DialogRequest {
            choices: vec!["c".repeat(MAX_CHOICE_LEN + 1)],
            description: None,
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
                description: None,
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
            DialogError::LockedSelection,
        ] {
            assert!(code.code().starts_with("dialog."));
        }
        assert_eq!(
            DialogError::UnknownPermissionChoice.code(),
            "dialog.unknown_permission_choice"
        );
        assert_eq!(DialogError::LockedSelection.code(), "dialog.locked_selection");
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
                message: None,
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
                description: None,
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
                description: None,
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
            description: None,
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
                description: None,
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
                description: None,
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
                message: None,
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
                description: None,
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

    // FORM-003 — regex pattern validation, including invalid patterns, and JS
    // undefined-vs-empty-array option semantics (`form.ts:38,82-88`; `form.tsx:116`).
    #[test]
    fn form_003_pattern_and_undefined_options_semantics() {
        let patterned = FormField::new("code", FieldKind::Text).with_pattern(r"item-[0-9]+");
        assert_eq!(validate(&patterned, Some(&FormValue::Text("prefix item-42 suffix".into()))), None);
        assert_eq!(
            validate(&patterned, Some(&FormValue::Text("prefix item-x suffix".into()))).as_deref(),
            Some("Must match pattern: item-[0-9]+"),
        );
        let invalid_pattern = FormField::new("bad", FieldKind::Text).with_pattern("[");
        assert_eq!(
            validate(&invalid_pattern, Some(&FormValue::Text("value".into()))).as_deref(),
            Some("Invalid pattern: ["),
        );
        // Fail closed on engine-unsupported JS constructs (look-around): a
        // pattern JS would compile is rejected as "Invalid pattern" rather
        // than silently accepted. Documented subset decision at `validate`.
        let lookaround = FormField::new("la", FieldKind::Text).with_pattern("foo(?=bar)");
        assert_eq!(
            validate(&lookaround, Some(&FormValue::Text("foobar".into()))).as_deref(),
            Some("Invalid pattern: foo(?=bar)"),
        );

        let free_text = FormField::new("free", FieldKind::Text);
        assert!(free_text.is_textual());
        assert_eq!(validate(&free_text, Some(&FormValue::Text("anything".into()))), None);
        let empty_options = FormField::new("choices", FieldKind::Text).with_options(vec![]);
        assert!(!empty_options.is_textual());
        assert!(FormPrompt::new("f-3-options", "Form", None, vec![empty_options.clone()]).single());
        assert_eq!(
            validate(&empty_options, Some(&FormValue::Text("anything".into()))).as_deref(),
            Some("Select an available option"),
        );
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

/// The dialog view owns a `ModalLayer`; these lock the derivation so a future
/// edit cannot reintroduce a second "is this open" flag or a paint path that
/// reads geometry from somewhere other than the stack.
///
/// Upstream: `packages/tui/src/ui/dialog.tsx:306-439` (stack, `size`/
/// `centered` store, focus save/restore, `modal` keymap layer) and `:461-479`
/// (the provider paints only `stack.at(-1)`).
#[cfg(test)]
mod modal_integration_tests {
    use super::*;
    use crate::native_modal_layer::{
        ManualClock, DIALOG_INSET, MODAL_BACKDROP_ALPHA, MODAL_Z_INDEX,
    };

    fn request() -> DialogRequest {
        DialogRequest {
            id: "req-1".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Permission,
            title: "Run shell command".to_string(),
            choices: vec!["allow once".to_string(), "deny".to_string()],
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        }
    }

    fn answer_attempt(view: &mut DialogView) -> u64 {
        view.next();
        view.answer().expect("selection answers").attempt()
    }

    #[test]
    fn opening_installs_exactly_one_entry_and_closing_clears_it() {
        let mut view = DialogView::new();
        assert!(DialogView::default().stack().is_empty());
        assert!(view.stack().is_empty());
        assert!(!view.is_open());
        assert_eq!(view.painted_dialogs(), 0);

        view.open_request(request()).expect("valid");
        assert_eq!(view.stack().len(), 1);
        assert_eq!(view.painted_dialogs(), 1);
        assert_eq!(view.top_dialog(), Some(view.stack()[0]));
        assert_eq!(view.top_dialog(), view.stack().last().copied());
        assert!(view.modal().is_open());
        assert!(view.shell_keys_blocked());
        assert!(view.intercepts_input());
        // Opening is not a close (`onClose` runs for previous entries only).
        assert_eq!(view.modal().closed(), 0);

        // A second open is refused and must not touch the stack.
        assert_eq!(view.open_request(request()), Err(DialogError::AlreadyOpen));
        assert_eq!(view.stack().len(), 1);
        assert_eq!(view.modal().closed(), 0);

        view.close();
        assert!(view.stack().is_empty());
        assert!(!view.is_open());
        assert!(!view.shell_keys_blocked());
        assert_eq!(view.modal().closed(), 1);
        assert_eq!(view.painted_dialogs(), 0);
        // Nothing to paint when the stack is empty.
        assert!(view.paint(100, 30, 10).is_none());
        assert!(view.compose(100, 30, 10).is_none());
    }

    #[test]
    fn refused_open_leaves_the_stack_and_focus_guard_untouched() {
        let mut view = DialogView::new();
        view.set_focus_target(ModalFocus::new(9));
        let unknown = DialogRequest {
            choices: vec!["do not approve".to_string()],
            description: None,
            ..request()
        };
        assert_eq!(
            view.open_request(unknown),
            Err(DialogError::UnknownPermissionChoice)
        );
        let too_long = DialogRequest {
            id: "i".repeat(MAX_ID_LEN + 1),
            description: None,
            ..request()
        };
        assert_eq!(view.open_request(too_long), Err(DialogError::IdTooLong));
        // `replace()` never ran (`ui/dialog.tsx:396-419`), so nothing was
        // saved, blurred, closed or installed.
        assert!(view.stack().is_empty());
        assert!(view.modal().saved_focus().is_none());
        assert!(!view.modal().focused_blurred());
        assert_eq!(view.modal().closed(), 0);
        assert_eq!(view.modal().refocus_requests(), 0);
        assert!(!view.shell_keys_blocked());
        // The refused view is still usable.
        view.open_request(request()).expect("valid after refusals");
        assert_eq!(view.stack().len(), 1);
    }

    #[test]
    fn first_open_saves_and_blurs_focus_and_close_restores_it_after_1ms() {
        let mut view = DialogView::new();
        let focus = ModalFocus::new(7);
        view.set_focus_target(focus);
        view.open_request(request()).expect("valid");
        assert!(view.focused_blurred());
        assert_eq!(view.saved_focus(), Some(focus));
        assert_eq!(view.modal().refocus_requests(), 0);

        view.close();
        assert_eq!(view.modal().refocus_requests(), 1);
        // Not before the 1 ms timer (`ui/dialog.tsx:326-343`).
        assert!(view.take_refocus(0).is_none());
        assert_eq!(view.take_refocus(1), Some(focus));
        assert!(!view.focused_blurred());
        assert!(view.saved_focus().is_none());
        // One restore per close.
        assert!(view.take_refocus(2).is_none());

        // Reopening before the timer fires restores nothing (`:330-332`).
        view.open_request(request()).expect("valid");
        view.close();
        view.open_request(request()).expect("valid");
        assert!(view.take_refocus(5).is_none());
        assert_eq!(view.stack().len(), 1);
    }

    #[test]
    fn destroyed_or_detached_focus_is_not_restored() {
        let mut view = DialogView::new();
        let mut destroyed = ModalFocus::new(3);
        destroyed.destroyed = true;
        view.set_focus_target(destroyed);
        view.open_request(request()).expect("valid");
        view.close();
        assert!(view.take_refocus(1).is_none());

        let mut detached = ModalFocus::new(4);
        detached.attached = false;
        view.set_focus_target(detached);
        view.open_request(request()).expect("valid");
        view.close();
        assert!(view.take_refocus(2).is_none());
    }

    #[test]
    fn pending_decision_holds_the_entry_against_every_close_path() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let attempt = answer_attempt(&mut view);
        assert!(view.pending());
        assert!(view.is_open());
        assert_eq!(view.stack().len(), 1);

        // A dispatched decision cannot be discarded locally.
        view.close();
        assert!(view.is_open());
        assert!(view.pending());
        view.backdrop_mouse_down(false);
        assert_eq!(view.backdrop_mouse_up(), BackdropRelease::Suppressed);
        assert!(view.is_open());
        let escape = view.escape_route(false);
        assert_eq!(
            escape,
            ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: ModalKey::Ignored
            }
        );
        let ctrl_c = view.ctrl_c_route(false, false);
        assert_eq!(
            ctrl_c,
            ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: ModalKey::Ignored
            }
        );
        assert_eq!(view.stack().len(), 1);

        // The matching ACK closes through `clear()`.
        assert_eq!(
            view.ack(DialogAck {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                accepted: true,
                attempt,
            }),
            Ok(AckOutcome::Closed)
        );
        assert!(view.stack().is_empty());
        assert!(!view.is_open());
        assert!(!view.pending());
        assert_eq!(view.modal().closed(), 1);
    }

    #[test]
    fn rejected_ack_keeps_the_entry_open_and_close_still_clears_it() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let attempt = answer_attempt(&mut view);
        assert_eq!(
            view.ack(DialogAck {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                accepted: false,
                attempt,
            }),
            Ok(AckOutcome::Rejected)
        );
        assert!(view.is_open());
        assert!(!view.pending());
        assert_eq!(view.stack().len(), 1);
        assert_eq!(view.modal().closed(), 0);
        // The reopened choice is answerable again, with a higher attempt.
        let retry = answer_attempt(&mut view);
        assert_eq!(retry, attempt + 1);
        assert_eq!(
            view.ack(DialogAck {
                request_id: "req-1".to_string(),
                session_id: "ses-1".to_string(),
                accepted: false,
                attempt: retry,
            }),
            Ok(AckOutcome::Rejected)
        );
        assert_eq!(view.stack().len(), 1);

        // `close()` after an open (non-pending) decision clears the entry.
        view.close();
        assert!(view.stack().is_empty());
        assert_eq!(view.modal().closed(), 1);
    }

    #[test]
    fn escape_dismisses_a_chrome_layer_before_the_dialog() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        // The diff file menu pushes its own keymap layer on top
        // (`diff-viewer-file-menu.tsx:23-30`).
        view.activate_chrome(ChromeLayer::DiffFileMenu);
        assert_eq!(view.activations(), &[ChromeLayer::Dialog, ChromeLayer::DiffFileMenu]);
        assert_eq!(view.top_interactive(), Some(ChromeLayer::DiffFileMenu));

        let route = view.escape_route(false);
        assert_eq!(route.layer, Some(ChromeLayer::DiffFileMenu));
        assert_eq!(route.key, ModalKey::Closed);
        assert!(view.is_open(), "the dialog survives the menu dismissal");
        assert_eq!(view.top_interactive(), Some(ChromeLayer::Dialog));

        let route = view.escape_route(false);
        assert_eq!(route.layer, Some(ChromeLayer::Dialog));
        assert_eq!(route.key, ModalKey::Closed);
        assert!(view.stack().is_empty());
        assert!(!view.is_open());
        assert_eq!(view.modal().closed(), 1);
        // The content went with the entry, so no stale dialog can be queried.
        assert_eq!(view.answer(), Err(DialogError::NotOpen));
        assert!(view.compose(100, 30, 10).is_none());
    }

    #[test]
    fn escape_clears_a_selection_without_closing_the_entry() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let route = view.escape_route(true);
        assert_eq!(route.layer, Some(ChromeLayer::Dialog));
        assert_eq!(route.key, ModalKey::SelectionCleared);
        assert!(view.is_open());
        assert_eq!(view.modal().selection_clears(), 1);
        assert_eq!(view.modal().closed(), 0);
    }

    #[test]
    fn ctrl_c_clears_the_editor_before_closing_the_entry() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let route = view.ctrl_c_route(true, true);
        assert_eq!(route.key, ModalKey::SelectionCleared);
        assert!(view.is_open());
        let route = view.ctrl_c_route(false, true);
        assert_eq!(route.key, ModalKey::EditorCleared);
        assert!(view.is_open());
        assert_eq!(view.modal().editor_clears(), 1);
        let route = view.ctrl_c_route(false, false);
        assert_eq!(route.key, ModalKey::Closed);
        assert!(!view.is_open());
        assert_eq!(view.modal().closed(), 1);
    }

    #[test]
    fn compose_returns_the_top_entry_rect_and_rows_together() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let frame = view.compose(100, 30, 10).expect("open dialog composes");
        assert_eq!(Some(frame.entry), view.top_dialog());
        // `alignItems="center"` (`:63`) and `paddingTop = height / 4` plus the
        // panel's own `paddingTop={1}` (`:67,83`).
        assert_eq!(
            frame.panel,
            DialogRect { x: 20, y: 8, width: 60, height: 11 }
        );
        assert_eq!(frame.rows.len(), 10);
        assert_eq!(frame.hits.len(), 10);
        assert_eq!(frame.size, DialogSize::Medium);
        assert!(!frame.centered);
        // The rows are the same rows the standalone renderer produces at the
        // inset body width, so paint and hit testing cannot diverge.
        let inset_width = usize::from(60u16 - 2 * DIALOG_INSET);
        let standalone = view.render_styled_with_metrics(DialogLayoutMetrics::new(
            inset_width,
            10,
            11,
            DialogViewportPolicy::ContentBudget,
        ));
        assert_eq!(frame.rows.len(), standalone.len());
        for (frame_row, standalone_row) in frame.rows.iter().zip(standalone.iter()) {
            assert_eq!(frame_row.text, standalone_row.text);
            assert_eq!(frame_row.role, standalone_row.role);
        }
        assert_eq!(
            frame.footer_row,
            standalone.iter().position(|row| row.text.contains("click here confirm"))
        );
    }

    #[test]
    fn compose_geometry_follows_size_and_centered_from_the_stack() {
        let mut view = DialogView::new();
        view.open_request_sized(request(), DialogSize::Large, true)
            .expect("valid");
        assert_eq!(view.dialog_size(), DialogSize::Large);
        assert!(view.centered());
        let frame = view.compose(100, 40, 10).expect("centered frame");
        assert_eq!(frame.size, DialogSize::Large);
        assert!(frame.centered);
        assert_eq!(frame.panel.width, 88);
        assert_eq!(frame.panel.x, 6);
        // `justifyContent="center"` with `paddingTop={0}`: (40 - 11) / 2.
        assert_eq!(frame.panel.y, 14);

        let paint = view.paint(100, 40, 10).expect("paint");
        assert_eq!(paint.z_index, MODAL_Z_INDEX);
        assert_eq!(paint.backdrop_alpha, MODAL_BACKDROP_ALPHA);
        assert_eq!(
            paint.backdrop,
            DialogRect { x: 0, y: 0, width: 100, height: 40 }
        );
        assert_eq!(paint.panel, frame.panel);

        // `maxWidth={dimensions().width - 2}` caps the large presets too.
        view.set_dialog_size(DialogSize::XLarge);
        let narrow = view.compose(100, 40, 10).expect("capped frame");
        assert_eq!(narrow.panel.width, 98);
        assert_eq!(narrow.panel.x, 1);
    }

    #[test]
    fn compose_rows_are_inset_by_the_shared_dialog_inset() {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            title: "T".repeat(200),
            description: None,
            ..request()
        })
        .expect("valid");
        let frame = view.compose(100, 30, 10).expect("frame");
        assert_eq!(frame.panel.width, 60);
        // `DialogLayout` padding is `DIALOG_INSET` per side
        // (`ui/dialog.tsx:163-166`).
        let inner = usize::from(60u16 - 2 * DIALOG_INSET);
        assert!(frame.rows.iter().all(|row| row.text.chars().count() <= inner));
        assert_eq!(frame.rows[0].text.chars().count(), inner);
    }

    #[test]
    fn compose_at_uses_the_caller_rect_and_still_reads_the_stack() {
        let closed = DialogView::new();
        assert!(closed
            .compose_at(DialogRect { x: 0, y: 0, width: 40, height: 8 }, 8)
            .is_none());

        let mut view = DialogView::new();
        view.open_request_sized(request(), DialogSize::Large, true)
            .expect("valid");
        // A composition root with its own capture-backed origin.
        let panel = DialogRect { x: 30, y: 5, width: 40, height: 12 };
        let frame = view.compose_at(panel, 8).expect("frame");
        assert_eq!(frame.panel, panel);
        assert_eq!(frame.rows.len(), 8);
        let inner = usize::from(40u16 - 2 * DIALOG_INSET);
        assert!(frame.rows.iter().all(|row| row.text.chars().count() <= inner));
        // Presentation still comes from the stack, not from the rect.
        assert_eq!(frame.size, DialogSize::Large);
        assert!(frame.centered);
    }

    #[test]
    fn replace_and_clear_reset_presentation_like_upstream() {
        let mut view = DialogView::new();
        view.open_request_sized(request(), DialogSize::Large, true)
            .expect("valid");
        view.close();
        // `clear()` resets the store (`ui/dialog.tsx:389-398`).
        assert_eq!(view.dialog_size(), DialogSize::Medium);
        assert!(!view.centered());

        // A presentation set while closed is overwritten by `replace()`
        // (`:406-408`); upstream callers re-apply it after opening.
        view.set_dialog_size(DialogSize::XLarge);
        view.set_centered(true);
        view.open_request(request()).expect("valid");
        assert_eq!(view.dialog_size(), DialogSize::Medium);
        assert!(!view.centered());
    }

    #[test]
    fn open_request_sized_applies_presentation_after_replace() {
        let mut view = DialogView::new();
        view.open_request_sized(request(), DialogSize::Large, true)
            .expect("valid");
        // `dialog.show()` then `dialog.set()` (`plugin/api.tsx:334-341`).
        assert_eq!(view.dialog_size(), DialogSize::Large);
        assert!(view.centered());
        assert_eq!(view.stack().len(), 1);

        // A refused request never applies the presentation either.
        let mut pending = DialogView::new();
        let refused = DialogRequest {
            choices: vec!["nope".to_string()],
            description: None,
            ..request()
        };
        assert_eq!(
            pending.open_request_sized(refused, DialogSize::Large, true),
            Err(DialogError::UnknownPermissionChoice)
        );
        assert_eq!(pending.dialog_size(), DialogSize::Medium);
        assert!(!pending.centered());
    }

    #[test]
    fn backdrop_release_closes_the_entry_but_a_selection_drag_does_not() {
        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        // Panel release: a selection bubbles to copy-on-select, anything else
        // stops at the panel (`ui/dialog.tsx:73-79`).
        assert_eq!(view.panel_mouse_up(true), PanelRelease::Bubbled);
        assert!(view.is_open());
        assert_eq!(view.panel_mouse_up(false), PanelRelease::Stopped);
        assert!(view.is_open());
        // An armed backdrop release is swallowed once (`:51-60`).
        view.backdrop_mouse_down(true);
        assert_eq!(view.backdrop_mouse_up(), BackdropRelease::Suppressed);
        assert!(view.is_open());
        // A plain backdrop click closes the whole stack.
        view.backdrop_mouse_down(false);
        assert_eq!(view.backdrop_mouse_up(), BackdropRelease::Closed);
        assert!(view.stack().is_empty());
        assert!(!view.is_open());
        assert_eq!(view.modal().closed(), 1);
        assert!(view.answer().is_err());
    }

    #[test]
    fn refocus_reads_the_injected_clock() {
        let mut view = DialogView::new();
        let focus = ModalFocus::new(2);
        view.set_focus_target(focus);
        view.open_request(request()).expect("valid");
        view.close();
        let mut clock = ManualClock::new(0);
        assert!(view.take_refocus_at(&clock).is_none());
        clock.advance(1);
        assert_eq!(view.take_refocus_at(&clock).map(|saved| saved.id), Some(2));
    }

    #[test]
    fn chrome_frame_composes_over_the_stack_the_view_owns() {
        use crate::native_modal_layer::ChromeModel;

        let mut view = DialogView::new();
        view.open_request(request()).expect("valid");
        let chrome = ChromeModel::new();
        let frame = chrome.frame_with(view.modal());
        assert!(frame.paints(ChromeLayer::Dialog));
        assert_eq!(frame.input_owner, Some(ChromeLayer::Dialog));
        assert_eq!(frame.backdrop, Some((ChromeLayer::Dialog, MODAL_BACKDROP_ALPHA)));

        // The menu opened through the view's ordering is painted above the
        // dialog and owns Escape (`diff-viewer-file-menu.tsx:23-30`).
        view.activate_chrome(ChromeLayer::DiffFileMenu);
        let frame = chrome.frame_with(view.modal());
        assert!(frame.paints(ChromeLayer::DiffFileMenu));
        assert_eq!(frame.input_owner, Some(ChromeLayer::DiffFileMenu));
        // Dismissing it through the same route restores the dialog's keys.
        assert_eq!(
            view.escape_route(false).layer,
            Some(ChromeLayer::DiffFileMenu)
        );
        let frame = chrome.frame_with(view.modal());
        assert!(!frame.paints(ChromeLayer::DiffFileMenu));
        assert_eq!(frame.input_owner, Some(ChromeLayer::Dialog));

        view.close();
        assert!(!chrome.frame_with(view.modal()).paints(ChromeLayer::Dialog));
    }
}

/// DIALOG-06..09: the live select surface. Every assertion goes through the
/// same `DialogView` methods the native loop calls, and option ids are the
/// ORIGINAL choice indexes.
#[cfg(test)]
mod select_live_tests {
    use super::*;

    fn select_view(choices: &[&str], disabled: &[usize]) -> DialogView {
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "sel-live".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Select,
            title: "Working directory".to_string(),
            choices: choices.iter().map(|choice| (*choice).to_string()).collect(),
            message: None,
            disabled: disabled.to_vec(),
            error: None,
            description: None,
        })
        .expect("select request valid");
        view
    }

    #[test]
    fn dialog09_live_select_reads_experiment_markers_and_preserves_legacy_rows() {
        let mut legacy = select_view(&["Copy path", "Open folder"], &[]);
        assert!(legacy
            .render(56, 12)
            .iter()
            .any(|row| row == " > 1. Copy path"));

        let mut designed = select_view(&["Copy path", "Open folder"], &[]);
        designed.set_new_designs(true);
        let rows = designed.render(56, 12);
        assert!(rows.iter().any(|row| row == "┃▪ 1. Copy path"), "{rows:?}");
        assert!(rows.iter().any(|row| row == "   2. Open folder"), "{rows:?}");
    }

    #[test]
    fn dialog06_typing_filters_rows_and_confirm_returns_the_original_index() {
        let mut view = select_view(&["Copy path", "Open folder", "Workspaces"], &[]);
        assert_eq!(view.focused_choice(), Some("Copy path"));
        // Only `Workspaces` survives the query, and the selection follows the
        // visible row while `answer` still carries original index 2.
        for ch in "works".chars() {
            assert!(view.select_push_filter(ch), "filter accepts {ch:?}");
        }
        let rows = view.render(56, 12);
        assert!(
            rows.iter().any(|row| row.contains("3. Workspaces")),
            "{rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.contains("Copy path")
                || row.contains("Open folder")),
            "hidden rows are not painted: {rows:?}"
        );
        assert!(rows.iter().any(|row| row.contains("select: works")), "{rows:?}");
        let intent = view.answer().expect("filtered selection is answerable");
        assert!(
            matches!(
                intent,
                DecisionIntent::Answer {
                    choice_index: 2,
                    ref choice,
                    ..
                } if choice == "Workspaces"
            ),
            "original choice identity, got {intent:?}"
        );
    }

    #[test]
    fn dialog06_backspace_restores_the_full_list_and_identity() {
        let mut view = select_view(&["Copy path", "Open folder", "Workspaces"], &[]);
        for ch in "open".chars() {
            assert!(view.select_push_filter(ch));
        }
        assert_eq!(view.focused_choice(), Some("Open folder"));
        for _ in 0.."open".len() {
            assert!(view.select_backspace(), "backspace removes one scalar");
        }
        assert!(!view.select_backspace(), "empty filter has nothing to remove");
        assert_eq!(view.focused_choice(), Some("Copy path"));
        view.next();
        let intent = view.answer().expect("restored list answerable");
        assert!(
            matches!(
                intent,
                DecisionIntent::Answer {
                    choice_index: 1,
                    ref choice,
                    ..
                } if choice == "Open folder"
            ),
            "got {intent:?}"
        );
    }

    #[test]
    fn dialog06_disabled_rows_never_enter_the_filtered_navigation() {
        let mut view = select_view(&["alpha", "beta", "gamma"], &[1]);
        assert_eq!(view.focused_choice(), Some("alpha"));
        view.next();
        assert_eq!(
            view.focused_choice(),
            Some("gamma"),
            "filtered navigation skips the disabled middle row"
        );
        view.select_push_filter('g');
        assert_eq!(view.focused_choice(), Some("gamma"));
        let intent = view.answer().expect("answerable");
        assert!(
            matches!(intent, DecisionIntent::Answer { choice_index: 2, .. }),
            "got {intent:?}"
        );
    }

    #[test]
    fn dialog08_page_keys_move_ten_rows_and_wrap() {
        let choices: Vec<String> = (0..25).map(|index| format!("row {index}")).collect();
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "sel-page".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Select,
            title: "Paged".to_string(),
            choices,
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
        .expect("valid");
        assert_eq!(view.focused_choice(), Some("row 0"));
        assert!(view.select_page(true));
        assert_eq!(view.focused_choice(), Some("row 10"));
        assert!(view.select_page(true));
        assert_eq!(view.focused_choice(), Some("row 20"));
        assert!(view.select_page(true));
        assert_eq!(
            view.focused_choice(),
            Some("row 0"),
            "page movement wraps the endpoints"
        );
        assert!(view.select_page(false));
        assert_eq!(
            view.focused_choice(),
            Some("row 24"),
            "page-up wraps the first endpoint to the last row"
        );
    }

    #[test]
    fn dialog09_empty_and_no_match_states_are_painted() {
        // Every option disabled: the model has no flat rows at all.
        let mut empty = select_view(&["alpha", "beta"], &[0, 1]);
        let rows = empty.render(56, 12);
        assert!(rows.iter().any(|row| row.contains("No items available")), "{rows:?}");
        assert!(!empty.select_page(true), "nothing to page through");

        // A query that matches nothing paints the distinct no-match view.
        let mut no_match = select_view(&["alpha", "beta"], &[]);
        assert!(no_match.select_push_filter('z'));
        let rows = no_match.render(56, 12);
        assert!(rows.iter().any(|row| row.contains("No results found")), "{rows:?}");
        assert!(
            !rows.iter().any(|row| row.contains("1. alpha")),
            "no stale row survives: {rows:?}"
        );
    }

    /// Regression: a filter that matches nothing must CLEAR the selection.
    /// Keeping the previous row would let Enter confirm a choice the user can
    /// no longer see.
    #[test]
    fn dialog06_no_match_clears_the_selection_and_cannot_confirm() {
        let mut view = select_view(&["alpha", "beta"], &[]);
        assert_eq!(view.focused_choice(), Some("alpha"));
        assert!(view.select_push_filter('z'));
        let rows = view.render(56, 12);
        assert!(rows.iter().any(|row| row.contains("No results found")), "{rows:?}");
        assert_eq!(
            view.focused_choice(),
            None,
            "no visible row means no focused choice"
        );
        // A stale mirror must not resurrect a choice absent from the model.
        view.selection = Some(0);
        assert_eq!(view.focused_choice(), None);
        assert_eq!(view.answer().unwrap_err(), DialogError::NoSelection);
        // Navigation on an empty list never resurrects a hidden row.
        view.next();
        view.prev();
        assert_eq!(view.focused_choice(), None);
        assert_eq!(view.answer().unwrap_err(), DialogError::NoSelection);
        // Removing the query restores a real selection and a real answer.
        assert!(view.select_backspace());
        assert_eq!(view.focused_choice(), Some("alpha"));
        assert!(matches!(
            view.answer().expect("restored selection answers"),
            DecisionIntent::Answer { choice_index: 0, .. }
        ));
    }
    /// A locked `DialogSelect` suppresses pointer/navigation and submit
    /// (`dialog-select.tsx:405-412,464-470,785-801`). The highlight stays on
    /// the painted row, but answer is refused until the select is unlocked.
    #[test]
    fn dialog06_locked_select_ignores_pointer_click_and_keeps_the_painted_row() {
        let mut view = select_view(&["Copy path", "Open folder", "Workspaces"], &[]);
        assert_eq!(view.focused_choice(), Some("Copy path"));
        let painted = view.render(56, 12);
        view.dialog
            .as_mut()
            .and_then(|dialog| dialog.select.as_mut())
            .expect("select model")
            .set_locked(true);
        // A pointer click on a hidden-by-lock row refuses instead of moving
        // the model behind the painted marker.
        assert!(!view.select(1), "a locked select refuses a pointer click");
        assert_eq!(view.focused_choice(), Some("Copy path"));
        // Down/Up cannot move it either, and a suppressed move must not
        // desynchronize the mirror from the model.
        view.next();
        view.prev();
        assert_eq!(view.focused_choice(), Some("Copy path"));
        assert_eq!(view.render(56, 12), painted, "locked row remains painted");
        assert_eq!(view.answer(), Err(DialogError::LockedSelection));
        assert!(!view.pending(), "locked select emits no intent or pending state");

        view.dialog
            .as_mut()
            .and_then(|dialog| dialog.select.as_mut())
            .expect("select model")
            .set_locked(false);
        assert!(matches!(
            view.answer().expect("unlocked select answers the painted row"),
            DecisionIntent::Answer { choice_index: 0, attempt: 1, ref choice, .. }
                if choice == "Copy path"
        ));
        assert!(view.pending());
    }

    /// `focused_choice` reads the select MODEL, so a mirror that was never
    /// updated (or is stale) can never make the row that answers differ from
    /// the row the model holds.
    #[test]
    fn dialog06_focused_choice_reads_the_model_not_a_stale_mirror() {
        let mut view = select_view(&["Copy path", "Open folder", "Workspaces"], &[]);
        for ch in "works".chars() {
            assert!(view.select_push_filter(ch));
        }
        assert_eq!(view.focused_choice(), Some("Workspaces"));
        // Simulate a mirror that a path outside `DialogView` left behind.
        view.selection = Some(0);
        assert_eq!(
            view.focused_choice(),
            Some("Workspaces"),
            "the model, not the mirror, decides the focused row"
        );
        assert!(matches!(
            view.answer().expect("model row answers"),
            DecisionIntent::Answer { choice_index: 2, .. }
        ));
    }
}

/// F002-R2: the child-session rejection stage. Upstream
/// `routes/session/permission.tsx:144-155,243-254` opens a nested reason editor
/// ONLY for a child session (`session.parentID`); a root-session reject replies
/// immediately. Confirming sends `reply("reject", message || undefined)`
/// (`:148-153`); Escape cancels the stage back to the options (`:299-302`);
/// Ctrl-C clears a non-empty reason before it cancels (`:288-298`).
#[cfg(test)]
mod f002r2_reject_stage_tests {
    use super::{AckOutcome, DecisionIntent, DialogAck, DialogKind, DialogView};

    /// A child-session permission prompt with the `reject` row selected.
    fn child_permission() -> DialogView {
        let mut view = DialogView::new();
        view.open_permission_with_persistent("req-1", "ses-1", "bash", true)
            .expect("permission opens");
        view.set_permission_session_has_parent(true);
        // The persistent prompt is `[Allow once, Always allow, Reject]`.
        assert!(view.select(2), "the reject row is selectable");
        view
    }

    /// A root-session permission prompt with the `reject` row selected.
    fn root_permission() -> DialogView {
        let mut view = DialogView::new();
        view.open_permission_with_persistent("req-1", "ses-1", "bash", true)
            .expect("permission opens");
        assert!(view.select(2));
        view
    }

    #[test]
    fn f002r2_child_reject_opens_the_reason_editor() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage(), "child reject opens the stage");
        assert!(view.reject_stage());
        assert!(!view.pending(), "no decision is in flight yet");
        assert_eq!(view.open_kind(), Some(DialogKind::Prompt));
        assert!(view.intercepts_input(), "the editor owns input");
    }

    #[test]
    fn f002r2_root_reject_replies_immediately() {
        let mut view = root_permission();
        assert!(!view.begin_reject_stage(), "root reject has no nested stage");
        assert!(!view.reject_stage());
        match view.answer().expect("root reject answers") {
            DecisionIntent::Reject { message, .. } => assert_eq!(message, None),
            other => panic!("expected reject, got {other:?}"),
        }
    }

    #[test]
    fn f002r2_confirm_carries_the_typed_reason() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage());
        for ch in "use cargo".chars() {
            assert!(view.prompt_push(ch));
        }
        match view.confirm_reject_stage().expect("confirm emits intent") {
            DecisionIntent::Reject {
                request_id,
                session_id,
                message,
                ..
            } => {
                assert_eq!(request_id, "req-1");
                assert_eq!(session_id, "ses-1");
                assert_eq!(message.as_deref(), Some("use cargo"));
            }
            other => panic!("expected reject, got {other:?}"),
        }
        assert!(view.pending());
        assert!(!view.reject_stage(), "the stage is consumed on confirm");
    }

    #[test]
    fn f002r2_confirm_with_empty_reason_omits_the_message() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage());
        match view.confirm_reject_stage().expect("confirm emits intent") {
            DecisionIntent::Reject { message, .. } => assert_eq!(message, None),
            other => panic!("expected reject, got {other:?}"),
        }
    }

    #[test]
    fn f002r2_cancel_restores_the_permission_card() {
        let mut view = child_permission();
        let before = view.render(80, 24);
        assert!(view.begin_reject_stage());
        assert!(view.reject_stage());
        assert!(view.cancel_reject_stage(), "cancel restores the card");
        assert!(!view.reject_stage());
        assert_eq!(view.open_kind(), Some(DialogKind::Permission));
        assert_eq!(view.render(80, 24), before, "the card is restored verbatim");
        assert!(
            view.begin_reject_stage(),
            "a second reject re-enters the editor"
        );
    }

    #[test]
    fn f002r2_ctrl_c_clears_then_cancels() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage());
        assert!(!view.reject_clear_reason(), "nothing to clear yet");
        assert!(view.prompt_push('x'));
        assert_eq!(view.reject_reason(), "x");
        assert!(view.reject_clear_reason(), "clears the reason");
        assert_eq!(view.reject_reason(), "");
        assert!(view.reject_stage(), "still in the stage after a clear");
        assert!(!view.reject_clear_reason(), "nothing left to clear");
        assert!(view.cancel_reject_stage());
        assert!(!view.reject_stage());
    }

    #[test]
    fn f002r2_accepted_ack_closes_the_editor() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage());
        assert!(view.prompt_push('x'));
        let intent = view.confirm_reject_stage().unwrap();
        let ack = DialogAck {
            request_id: intent.request_id().to_owned(),
            session_id: intent.session_id().to_owned(),
            accepted: true,
            attempt: intent.attempt(),
        };
        assert_eq!(view.ack(ack).unwrap(), AckOutcome::Closed);
        assert!(!view.is_open());
        assert!(!view.reject_stage());
    }

    #[test]
    fn f002r2_rejected_ack_restores_the_permission_options() {
        let mut view = child_permission();
        assert!(view.begin_reject_stage());
        let intent = view.confirm_reject_stage().unwrap();
        let ack = DialogAck {
            request_id: intent.request_id().to_owned(),
            session_id: intent.session_id().to_owned(),
            accepted: false,
            attempt: intent.attempt(),
        };
        assert_eq!(view.ack(ack).unwrap(), AckOutcome::Rejected);
        assert_eq!(view.open_kind(), Some(DialogKind::Permission));
        assert!(!view.reject_stage());
    }

    #[test]
    fn f002r2_non_reject_selection_stays_a_plain_answer() {
        let mut view = DialogView::new();
        view.open_permission_with_persistent("req-1", "ses-1", "bash", true)
            .unwrap();
        view.set_permission_session_has_parent(true);
        assert!(view.select(0)); // allow once
        assert!(!view.begin_reject_stage());
        assert!(matches!(
            view.answer().unwrap(),
            DecisionIntent::Approve { .. }
        ));
    }

    #[test]
    fn f002r2_child_always_selection_stays_a_plain_answer() {
        let mut view = DialogView::new();
        view.open_permission_with_persistent("req-1", "ses-1", "bash", true)
            .unwrap();
        view.set_permission_session_has_parent(true);
        assert!(view.select(1)); // always allow
        assert!(!view.begin_reject_stage());
        assert!(matches!(
            view.answer().unwrap(),
            DecisionIntent::ScopedPermission { .. }
        ));
    }

    /// Upstream `escapeKey="reject"` routes Escape through the same
    /// `onSelect("reject")` as the reject row (`permission.tsx:241,437`), so a
    /// child-session Escape opens the reason editor regardless of which row is
    /// highlighted.
    #[test]
    fn f002r2_escape_on_child_opens_the_editor_regardless_of_highlight() {
        let mut view = DialogView::new();
        view.open_permission_with_persistent("req-1", "ses-1", "bash", true)
            .unwrap();
        view.set_permission_session_has_parent(true);
        assert!(view.select(0)); // `allow once` is highlighted, not `reject`
        assert!(view.escape_reject_stage(), "child Escape opens the stage");
        assert!(view.reject_stage());
        assert_eq!(view.open_kind(), Some(DialogKind::Prompt));
        assert!(!view.pending(), "no decision is in flight yet");
        assert!(view.intercepts_input(), "the editor owns input");
    }

    /// A root session has no nested stage, so Escape falls through to the
    /// immediate reject (`permission.tsx:249-253`).
    #[test]
    fn f002r2_escape_on_root_rejects_immediately() {
        let mut view = root_permission();
        assert!(
            !view.escape_reject_stage(),
            "a root session has no nested stage"
        );
        assert!(!view.reject_stage());
        match view.cancel().expect("root escape rejects") {
            DecisionIntent::Reject { message, .. } => assert_eq!(message, None),
            other => panic!("expected reject, got {other:?}"),
        }
    }

    /// Escape toggles between the reason editor and the permission card: the
    /// first Escape opens the editor, the next cancels it back to the card
    /// (`permission.tsx:152,299-302`), and a third re-opens it.
    #[test]
    fn f002r2_escape_toggles_stage_and_card() {
        let mut view = child_permission();
        let before = view.render(80, 24);
        assert!(view.escape_reject_stage(), "escape opens the editor");
        assert!(view.reject_stage());
        assert!(view.cancel_reject_stage(), "escape again restores the card");
        assert!(!view.reject_stage());
        assert_eq!(view.open_kind(), Some(DialogKind::Permission));
        assert_eq!(view.render(80, 24), before, "the card is restored verbatim");
        assert!(
            view.escape_reject_stage(),
            "escape re-opens the editor from the card"
        );
        assert!(view.reject_stage());
    }

    /// Once the decision is pending the stage is retained only for ACK
    /// correlation; Escape can neither re-enter nor cancel it.
    #[test]
    fn f002r2_escape_is_inert_once_the_decision_is_pending() {
        let mut view = child_permission();
        assert!(view.escape_reject_stage());
        let _ = view.confirm_reject_stage().expect("confirm emits intent");
        assert!(view.pending());
        assert!(!view.escape_reject_stage(), "no re-entry while pending");
        assert!(!view.cancel_reject_stage(), "no cancel while pending");
    }

    /// With no permission dialog open, Escape has no stage to open.
    #[test]
    fn f002r2_escape_without_a_permission_is_inert() {
        let mut view = DialogView::new();
        assert!(!view.escape_reject_stage());
        assert!(!view.reject_stage());
    }
}

/// DIFF-001/DIFF-003: the live `DialogDiffViewer` patch viewer
/// (`feature-plugins/system/diff-viewer.tsx`, `component/patch-diff.tsx`).
/// These tests pin the open-time split resolution and the scroll-driven
/// virtualization window through the live render arm.
#[cfg(test)]
mod diff_tests {
    use super::*;

    fn texts(lines: &[crate::native_ui_style::Line]) -> Vec<String> {
        lines.iter().map(|line| line.text.clone()).collect()
    }

    /// DIFF-001: `open_diff_viewer` resolves the split rule once at open time
    /// (`diff-viewer.tsx:45,349`: never split below 100 columns, split from
    /// 100 up) and the live render arm branches on that stored mode.
    #[test]
    fn diff001_viewer_mode_is_resolved_at_open_time_and_drives_the_render() {
        let patch = "@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n";
        // Width 99 -> the viewer never splits below 100 columns (unified).
        let mut narrow = DialogView::new();
        narrow.open_diff_viewer(patch.to_string(), 99).expect("opens");
        assert_eq!(narrow.open_kind(), Some(DialogKind::DiffViewer));
        let narrow_texts = texts(&narrow.render_styled(99, 20));
        assert!(narrow_texts.iter().any(|t| t == "-old"), "{narrow_texts:?}");
        assert!(narrow_texts.iter().any(|t| t == "+new"), "{narrow_texts:?}");
        // Width 100 -> split; the paired columns join with a space and the
        // unified raw pair is gone.
        let mut wide = DialogView::new();
        wide.open_diff_viewer(patch.to_string(), 100).expect("opens");
        let wide_texts = texts(&wide.render_styled(100, 20));
        assert!(
            wide_texts
                .iter()
                .any(|t| t.contains("- old") && t.contains("+ new")),
            "{wide_texts:?}"
        );
        assert!(!wide_texts.iter().any(|t| t == "-old"), "{wide_texts:?}");
    }

    /// DIFF-003: the diff body scrolls in rendered rows and the live render
    /// arm derives its chunk window from that offset
    /// (`patch-diff.tsx:164-181`); offscreen chunks collapse to a placeholder.
    #[test]
    fn diff003_scroll_moves_the_virtualized_window() {
        let mut patch = String::from("@@ -0,0 +1,3200 @@\n");
        for n in 1..=3200 {
            patch.push_str(&format!("+line{n}\n"));
        }
        let mut view = DialogView::new();
        // Width 80 -> unified, so the virtualized branch is live.
        view.open_diff_viewer(patch, 80).expect("opens");
        let (start, max) = view.execute_scroll_state(80, 20);
        assert_eq!(start, 0);
        assert!(max > 0, "virtualized body must overflow the viewport");
        let top = texts(&view.render_styled(80, 20));
        assert!(top.iter().any(|t| t == "+line1"), "{top:?}");
        assert!(
            !top.iter().any(|t| t == "+line1153"),
            "offscreen chunk hidden: {top:?}"
        );
        // Scroll into chunk 10 (rows 1280..1408); the window follows. The
        // range keeps one chunk of overscan above (`patch-diff.tsx:164-181`),
        // so the top painted chunk is 9 at this offset.
        view.set_execute_scroll(1280);
        assert_eq!(view.execute_scroll_state(80, 20).0, 1280);
        let scrolled = texts(&view.render_styled(80, 20));
        assert!(scrolled.iter().any(|t| t == "+line1153"), "{scrolled:?}");
        assert!(
            !scrolled.iter().any(|t| t == "+line1"),
            "top chunk scrolled away: {scrolled:?}"
        );
    }

    /// DIFF-001 review fix: the read-only diff viewer has no answer/cancel
    /// intent. Both must be refused with `NotAnswerable` before any attempt is
    /// consumed — the backdrop mouse-up cancel (`tui_entry.rs` kind-2 arm) and
    /// the footer confirm must never reach the `unreachable!` arm.
    #[test]
    fn diff002_diffviewer_cancel_and_answer_are_inert() {
        let patch = "@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n";
        let mut view = DialogView::new();
        view.open_diff_viewer(patch.to_string(), 80).expect("opens");
        assert_eq!(view.open_kind(), Some(DialogKind::DiffViewer));
        assert_eq!(view.answer().unwrap_err(), DialogError::NotAnswerable);
        assert_eq!(view.cancel().unwrap_err(), DialogError::NotAnswerable);
        // Inert: the dialog is still open and no attempt was consumed.
        assert!(view.is_open());
    }

    /// DIFF-001 review fix: the patch body is painted once. The generic
    /// two-space-prefixed `message_lines` block must not re-append the patch
    /// the DiffViewer arm already rendered.
    #[test]
    fn diff002_small_patch_paints_the_body_once() {
        let patch = "@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n";
        let mut view = DialogView::new();
        view.open_diff_viewer(patch.to_string(), 80).expect("opens");
        let rendered = texts(&view.render_styled(80, 20));
        let old_rows = rendered.iter().filter(|t| t.trim() == "-old").count();
        let new_rows = rendered.iter().filter(|t| t.trim() == "+new").count();
        assert_eq!(old_rows, 1, "patch body painted once: {rendered:?}");
        assert_eq!(new_rows, 1, "patch body painted once: {rendered:?}");
    }

    /// DIFF-001 review fix: a backdrop mouse-up closes the read-only diff
    /// viewer locally (`ui/dialog.tsx:54-60`). Its `cancel()` is inert, so the
    /// backdrop must take the same local-close path Esc does, not emit a
    /// decision intent.
    #[test]
    fn diff002_backdrop_closes_the_viewer_locally() {
        let mut view = DialogView::new();
        assert!(!view.backdrop_closes_locally(), "no dialog -> no local close");
        view.open_diff_viewer("@@ -1 +1 @@\n-a\n+b\n".to_string(), 80)
            .expect("opens");
        assert!(
            view.backdrop_closes_locally(),
            "the diff viewer closes locally on a backdrop release"
        );
        view.close();
        assert!(!view.backdrop_closes_locally(), "closed -> no local close");
    }

    /// DIFF-001 review fix: the scroll maximum is the real painted row count
    /// minus the viewport, not `chunks * VIRTUAL_CHUNK_LINES - viewport` (the
    /// final chunk is partial, so the product over-counts).
    #[test]
    fn diff003_scroll_max_matches_painted_rows() {
        let mut patch = String::from("@@ -0,0 +1,3300 @@\n");
        for n in 1..=3300 {
            patch.push_str(&format!("+line{n}\n"));
        }
        let total_rows = crate::native_diff_view::virtual_added_patch(&patch, true)
            .expect("virtualized")
            .total_rows;
        // The product over-counts: 26 chunks * 128 = 3328 > 3300 rows.
        assert!(total_rows < 26 * crate::native_diff_view::VIRTUAL_CHUNK_LINES);
        let mut view = DialogView::new();
        view.open_diff_viewer(patch, 80).expect("opens");
        let (_, max) = view.execute_scroll_state(80, 20);
        assert_eq!(max, total_rows - 18, "clamped to painted rows, not chunks");
    }
}

/// TOOL-011: the live `DialogExecute` content viewer
/// (`packages/tui/src/routes/session/dialog-execute.tsx`). The model is
/// `native_tool_rows::execute_dialog`; these tests pin the live dialog render,
/// the scroll window, the copy selection and the content-only contract.
#[cfg(test)]
mod tool011_tests {
    use super::*;
    use crate::tui_entry::native_turn::native_tool_rows::{ExecuteDialog, execute_dialog};

    fn model() -> ExecuteDialog {
        execute_dialog(
            "completed",
            r#"{"code":"const a = 1\nreturn a"}"#,
            "{\"ok\":true}\n\nLogs:\nline",
            false,
            None,
        )
    }

    fn texts(lines: &[crate::native_ui_style::Line]) -> Vec<String> {
        lines.iter().map(|line| line.text.clone()).collect()
    }

    /// `dialog-execute.tsx:222-282`: the status line, the gutter-numbered code
    /// block, the shared-gutter JSON output and the `Warnings:`/`Logs:` tail
    /// are all painted live.
    #[test]
    fn tool011_open_execute_renders_status_code_and_json() {
        let mut view = DialogView::new();
        view.open_execute(model()).expect("execute viewer opens");
        assert_eq!(view.open_kind(), Some(DialogKind::Execute));
        let texts = texts(&view.render_styled(80, 20));
        assert!(texts.iter().any(|t| t == "Completed"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "1 const a = 1"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "2 return a"), "{texts:?}");
        // `GutteredCode` numbers each block from 1 (`dialog-execute.tsx:251`),
        // so the JSON output restarts at 1 even though `digits` is shared.
        assert!(texts.iter().any(|t| t == "1 {\"ok\":true}"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Logs:"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "line"), "{texts:?}");
    }

    /// The viewer is content-only (`dialog-execute.tsx:28`): no decision
    /// intent, and Escape closes it locally.
    #[test]
    fn tool011_execute_is_content_only_and_closes_locally() {
        let mut view = DialogView::new();
        view.open_execute(model()).expect("opens");
        assert_eq!(view.answer().unwrap_err(), DialogError::NotAnswerable);
        assert_eq!(view.cancel().unwrap_err(), DialogError::NotAnswerable);
        assert!(view.is_open());
        view.close();
        assert!(!view.is_open());
        assert_eq!(view.open_kind(), None);
    }

    /// `dialog-execute.tsx:76-89`: up/down move the body one row and the
    /// window is clamped to the body length.
    #[test]
    fn tool011_scroll_window_moves() {
        let mut view = DialogView::new();
        let code: String = (1..=40).map(|n| format!("line{n}")).collect::<Vec<_>>().join("\n");
        let args = serde_json::json!({ "code": code }).to_string();
        view.open_execute(execute_dialog("completed", &args, "", false, None))
            .expect("opens");
        let (start, max) = view.execute_scroll_state(80, 10);
        assert_eq!(start, 0);
        assert!(max > 0, "body must overflow the viewport");
        let first = texts(&view.render_styled(80, 10));
        // 40 lines -> `digits = 2`, so the gutter is right-padded: `" 1 line1"`.
        assert!(first.iter().any(|t| t == " 1 line1"), "{first:?}");
        view.set_execute_scroll(2);
        assert_eq!(view.execute_scroll_state(80, 10).0, 2);
        let scrolled = texts(&view.render_styled(80, 10));
        assert!(!scrolled.iter().any(|t| t == " 1 line1"), "{scrolled:?}");
        // Beyond the end clamps to `max`, never past it.
        view.set_execute_scroll(usize::MAX);
        assert_eq!(view.execute_scroll_state(80, 10).0, max);
    }

    /// `dialog-execute.tsx:96-104`: `c` copies the code block and `o` the
    /// output; an empty block or a closed dialog yields `None`.
    #[test]
    fn tool011_copy_text_selects_code_or_output() {
        let mut view = DialogView::new();
        view.open_execute(model()).expect("opens");
        assert_eq!(view.execute_copy_text(true).as_deref(), Some("const a = 1\nreturn a"));
        assert_eq!(
            view.execute_copy_text(false).as_deref(),
            Some("{\"ok\":true}\n\nLogs:\nline")
        );
        view.close();
        assert_eq!(view.execute_copy_text(true), None);
        assert_eq!(view.execute_copy_text(false), None);
    }
}


#[cfg(test)]
mod dialog07_tests {
    use super::*;
    use crate::native_palette::select::{SelectDialog, SelectOption};

    /// Open a live `DialogKind::Select` and inject a `SelectDialog` whose
    /// options carry footers. Option ids are the original choice indexes so
    /// the render's `id.parse::<usize>()` mapping keeps working.
    fn select_view(options: Vec<SelectOption>, flat: bool) -> DialogView {
        let choices: Vec<String> = options.iter().map(|option| option.title.clone()).collect();
        let mut view = DialogView::new();
        view.open_request(DialogRequest {
            id: "d07".to_string(),
            session_id: "ses-1".to_string(),
            kind: DialogKind::Select,
            title: "Pick".to_string(),
            choices,
            message: None,
            disabled: Vec::new(),
            error: None,
            description: None,
        })
        .expect("select request valid");
        view.dialog.as_mut().unwrap().select = Some(SelectDialog::new(options).with_flat(flat));
        view
    }

    /// `dialog-select.tsx:806-809`: a grouped row paints its own `footer`
    /// right-aligned at the row's right edge, separated from the title by at
    /// least one space.
    #[test]
    fn dialog07_row_footer_paints_right_aligned() {
        let view = select_view(
            vec![
                SelectOption::new("0", "First").with_footer("f0"),
                SelectOption::new("1", "Second").with_footer("f1"),
            ],
            false,
        );
        let rows = view.render(40, 12);
        let row = rows
            .iter()
            .find(|row| row.contains("First"))
            .expect("first row painted");
        assert!(row.ends_with("f0"), "footer right-aligned: {row:?}");
        assert_eq!(row.chars().count(), 40, "row fills the content width: {row:?}");
        assert!(row.contains("First  "), "title then gap then footer: {row:?}");
    }

    /// While flattened (`flat(true)` + a filter) the footer is
    /// `searchFooter ?? category ?? footer`; grouped, only `footer` paints.
    #[test]
    fn dialog07_flatten_uses_search_footer_then_category() {
        let mut view = select_view(
            vec![
                SelectOption::new("0", "First")
                    .with_category("Group")
                    .with_footer("plain")
                    .with_search_footer("search"),
                SelectOption::new("1", "Second").with_category("Group"),
            ],
            true,
        );
        let grouped = view.render(40, 12);
        let row = grouped
            .iter()
            .find(|row| row.contains("First"))
            .expect("grouped row");
        assert!(row.ends_with("plain"), "grouped row shows footer: {row:?}");

        assert!(
            view.dialog
                .as_mut()
                .unwrap()
                .select
                .as_mut()
                .unwrap()
                .set_filter("first"),
            "filter admitted"
        );
        let flattened = view.render(40, 12);
        let row = flattened
            .iter()
            .find(|row| row.contains("First"))
            .expect("flattened row");
        assert!(
            row.ends_with("search"),
            "flattened footer = searchFooter: {row:?}"
        );

        // No `searchFooter`: the category is the fallback.
        let mut view = select_view(
            vec![
                SelectOption::new("0", "First").with_category("Group"),
                SelectOption::new("1", "Second").with_category("Group"),
            ],
            true,
        );
        assert!(
            view.dialog
                .as_mut()
                .unwrap()
                .select
                .as_mut()
                .unwrap()
                .set_filter("first")
        );
        let rows = view.render(40, 12);
        let row = rows
            .iter()
            .find(|row| row.contains("First"))
            .expect("flattened row");
        assert!(
            row.ends_with("Group"),
            "flattened footer falls back to category: {row:?}"
        );
    }

    /// A title that collides with the footer is truncated; the footer stays
    /// visible and the row still fills the content width.
    #[test]
    fn dialog07_long_title_truncates_and_keeps_footer() {
        let view = select_view(
            vec![SelectOption::new("0", "A".repeat(60)).with_footer("tail")],
            false,
        );
        let rows = view.render(30, 12);
        let row = rows
            .iter()
            .find(|row| row.contains("tail"))
            .expect("footer survives truncation");
        assert!(row.ends_with("tail"), "footer kept: {row:?}");
        assert_eq!(row.chars().count(), 30, "row fills the content width: {row:?}");
        assert!(
            !row.contains(&"A".repeat(40)),
            "title truncated to make room: {row:?}"
        );
    }
}
