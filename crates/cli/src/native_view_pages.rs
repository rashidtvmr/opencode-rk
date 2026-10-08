#![forbid(unsafe_code)]
//! Reachable page state and render for the native three-pane shell.
//!
//! SOURCE-PROPOSAL-UNINTEGRATED — proposal only. This file is not wired into
//! any crate, module list, build, test target, runtime or backend; the lane
//! integrator owns the actual worktree, routing and dispatch. Nothing here
//! executes against a daemon, DB, network or user configuration.
//!
//! Pure state + pure string render: std only, no external crates, no IO, no
//! threads, no clock, no persistence. Mirrors the intent style of
//! `crates/cli/src/native_status.rs` and `crates/cli/src/native_navigation.rs`
//! modules: a page selection, a bounded backend-fed row list, a byte-safe
//! search filter, a stable-id selection, and an optional [`PageAction`] intent
//! the caller must confirm and get a backend [`Ack`] for before any real
//! provider/model/session/setting change happens.
//!
//! Honesty rules encoded here (availability, never fake metadata):
//! - unsupported providers render `enabled: false` with an
//!   `unavailable: <reason>` detail (transport failures never become rows);
//! - agent/workspace backends that do not exist feed [`unavailable_rows`];
//! - credentials never appear in rows: only id/label/detail of non-secret
//!   entries; masking is the caller's separate prompt, and errors must not be
//!   built from secret material;
//! - Help rows mirror the actual keybindings (`footer_hints` /
//!   `keybinding_help` in `crates/sessions/src/tui_state.rs`), nothing more;
//! - theme markers are the ones the existing palette actually supports
//!   (`native_palette::Theme::parse`: "dark" | "light" | "nocolor"), stored in
//!   this frontend state only — no fake persistence;
//! - Commands are the static local routes listed in [`COMMAND_ROUTES`];
//! - an unsupported activation returns [`PageAction::Unsupported`], never a
//!   pretend ACK;
//! - `close()`/cancel abandons a pending intent and changes nothing else;
//! - an error retains the last backend rows; only a successful `set_rows`
//!   replaces them.
//!
//! Base SHA: 2c30425a29f0cf4ea9e359bf3c62991c970b0ca5 (guard GREEN at proposal
//! time). Pinned upstream evidence: 95daf90670b7c039c436c85537da5fbfe2205b41
//! (`packages/tui/src/config/keybind.ts`: dialog.select.* navigation, model
//! dialog, theme_list, help dialog) — used for page/dialog behavior only.
//!
//! v2 (pages-v2, repairs for `pages-review-v1/VERDICT.md: NEEDS_FIX`):
//! 1. Snapshot identity: `try_set_rows` validates **exact** row ids (unique,
//!    non-empty, ≤ `MAX_ID_BYTES`, no control chars) BEFORE any clipping or
//!    state assignment. An invalid snapshot is refused as a whole: previous
//!    backend rows, selection id and pending intent stay intact, and only a
//!    bounded static error (the `ERR_*` consts, never raw ids) is shown. Ids
//!    are never clipped or renamed (the v1 id clip could alias two distinct
//!    ids into one), and duplicates never resolve by first-wins — including
//!    when a duplicate row is disabled. Identity is exact and case-preserving;
//!    no provider normalization is invented. The void `set_rows` keeps its
//!    signature and delegates to `try_set_rows`; a refusal never triggers a
//!    backend request.
//! 2. Themes: `SUPPORTED_THEMES` now includes the real `high-contrast`
//!    builtin, consistent with `native_theme::builtin_for_marker`
//!    (`"dark" | "light" | "high-contrast"`); `"nocolor"` stays because
//!    `native_palette::Theme::parse` supports it. `Page::Theme` rows with id
//!    `high-contrast` are enabled choices that activate to
//!    `PageAction::SetTheme` under the same frontend-only ownership — still
//!    no fake persistence and no backend theme endpoint.
//! 3. Help keymap adapter: `help_rows_for_submit_mode(submit_with_ctrl_j)`
//!    builds the binding rows for the active `Composer::SubmitKeymap`;
//!    `help_rows_default()` mirrors the actual current default
//!    (`SubmitKeymap::Enter`, see `native_composer.rs::with_keymap`), and the
//!    original `help_rows(submit_is_enter)` delegates to the adapter. The
//!    integrated Help page and footer must both call the adapter with the
//!    active keymap (integration owner; core Help const/main/footer are not
//!    touched here). Unwired upstream command keys are not advertised.
//! 3b. v3 (pages-v3, repair for the remaining review finding: `take(MAX_ROWS)`
//!    in `try_set_rows` silently dropped an invalid tail): an incoming
//!    snapshot with `len() > MAX_ROWS` is now refused up front with the
//!    bounded static `ERR_TOO_MANY_ROWS` — only the error field changes;
//!    prior rows, selection, query, pending intent, `rows_page` and current
//!    page are untouched. The silent `take` is gone; validated length is
//!    bounded by `MAX_ROWS`, so no pagination API is needed.
//! 4. v4 (pages-v4, installed-UI integration gap): additive [`Page::Sessions`].
//!    `/sessions` now resolves to `Page::Sessions` (the Commands entry is an
//!    enabled route) instead of being mapped onto `Chat`. The page renders
//!    only caller-provided rows fed from the real backend (`GET /api/sessions`):
//!    `id` = backend session UUID, `label` = title, `detail` = active marker,
//!    `enabled` = true — this module hardcodes no Chat/dummy session rows.
//!    With no rows loaded the page is honestly empty (`PageAction::None`, an
//!    explicit render note), never a fake-functional shortcut. Activating a
//!    selected session emits the existing generic
//!    [`PageAction::Request { page: Page::Sessions, id }]` shape with the real
//!    session id; the integrator owns `GET` session/history/listing, draft
//!    preservation on ACK, and the `PageState` ack. `Chat` stays the default
//!    and `close()` target and now carries no static route (home page), so
//!    `from_route("/sessions")` is unambiguous.

/// Maximum rows retained after `set_rows` (first 512 kept, deterministically).
pub const MAX_ROWS: usize = 512;
/// Maximum bytes per row: `label + detail` after sanitation (32 KiB).
pub const MAX_ROW_BYTES: usize = 32 * 1024;
/// Maximum bytes per row id (identity for stable selection).
pub const MAX_ID_BYTES: usize = 256;
/// Maximum bytes kept for the search text and for error messages.
pub const MAX_QUERY_BYTES: usize = 512;
/// Default frontend theme marker; matches `native_palette::Theme` defaults.
pub const DEFAULT_THEME: &str = "dark";

/// Theme markers the existing surfaces actually support: the
/// `native_palette::Theme::parse` set (`dark`/`light`/`nocolor`) plus the
/// real `high-contrast` builtin of `native_theme::builtin_for_marker`
/// (`"dark" => opencode_dark, "light" => opencode_light, "high-contrast" =>
/// opencode_high_contrast`). Anything else is rejected; this module never
/// persists a theme (frontend state only).
pub const SUPPORTED_THEMES: [&str; 4] = ["dark", "light", "nocolor", "high-contrast"];

/// Refusal: two rows share the exact same id. The whole snapshot is rejected
/// (no first-wins), previous rows/selection/pending stay intact.
pub const ERR_DUPLICATE_ID: &str =
    "refused: duplicate row id in backend snapshot; previous rows retained";
/// Refusal: a row id is empty (never materialized into an alias id).
pub const ERR_EMPTY_ID: &str = "refused: empty row id in backend snapshot; previous rows retained";
/// Refusal: a row id exceeds `MAX_ID_BYTES` (never clipped into an alias).
pub const ERR_OVERSIZED_ID: &str =
    "refused: oversized row id in backend snapshot; previous rows retained";
/// Refusal: a row id contains control characters (malformed identity).
pub const ERR_MALFORMED_ID: &str =
    "refused: malformed row id in backend snapshot; previous rows retained";
/// Refusal: the snapshot exceeds `MAX_ROWS`; it is rejected whole before any
/// id validation or truncation (no silently dropped tail).
pub const ERR_TOO_MANY_ROWS: &str =
    "refused: too many rows in backend snapshot; previous rows retained";

use std::collections::HashSet;

/// Static local command routes (user-requested local routes). The integrator
/// binds each route to real navigation; nothing here fetches or mutates.
pub const COMMAND_ROUTES: [(&str, &str); 11] = [
    ("/sessions", "Sessions"),
    ("/models", "Models"),
    ("/connect", "Providers"),
    ("/help", "Help"),
    ("/theme", "Theme"),
    ("/status", "Status"),
    ("/mcp", "Mcp"),
    ("/context", "Context"),
    ("/diff", "Diff"),
    ("/workspace", "Workspaces"),
    ("/agents", "Agents"),
];

// ── Page ──────────────────────────────────────────────────────────────

/// A reachable page of the shell. `Chat` is the default and `close()` target
/// (the home page carries no static route); [`Page::Sessions`] owns
/// `/sessions` and is fed by the real sessions backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Chat,
    Sessions,
    Help,
    Models,
    Providers,
    Agents,
    Workspaces,
    Theme,
    Status,
    Mcp,
    Diff,
    Context,
    Commands,
}

impl Page {
    /// Tab/page cycling order.
    pub const ORDER: [Page; 13] = [
        Page::Chat,
        Page::Sessions,
        Page::Help,
        Page::Models,
        Page::Providers,
        Page::Agents,
        Page::Workspaces,
        Page::Theme,
        Page::Status,
        Page::Mcp,
        Page::Diff,
        Page::Context,
        Page::Commands,
    ];

    /// Page title (a page always renders rows too — a heading alone never
    /// claims completion).
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Page::Chat => "Chat",
            Page::Sessions => "Sessions",
            Page::Help => "Help",
            Page::Models => "Models",
            Page::Providers => "Providers",
            Page::Agents => "Agents",
            Page::Workspaces => "Workspaces",
            Page::Theme => "Theme",
            Page::Status => "Status",
            Page::Mcp => "MCP",
            Page::Diff => "Diff",
            Page::Context => "Context",
            Page::Commands => "Commands",
        }
    }

    /// Static local route for this page (`None` for the `Chat` home page and
    /// for `Commands` itself).
    #[must_use]
    pub const fn route(self) -> Option<&'static str> {
        match self {
            Page::Chat => None,
            Page::Sessions => Some("/sessions"),
            Page::Help => Some("/help"),
            Page::Models => Some("/models"),
            Page::Providers => Some("/connect"),
            Page::Agents => Some("/agents"),
            Page::Workspaces => Some("/workspace"),
            Page::Theme => Some("/theme"),
            Page::Status => Some("/status"),
            Page::Mcp => Some("/mcp"),
            Page::Diff => Some("/diff"),
            Page::Context => Some("/context"),
            Page::Commands => None,
        }
    }

    /// Resolve a static local route back to its page.
    #[must_use]
    pub fn from_route(route: &str) -> Option<Page> {
        Page::ORDER
            .into_iter()
            .find(|page| page.route() == Some(route))
    }

    /// Next page in tab order (wraps).
    #[must_use]
    pub const fn next(self) -> Page {
        match self {
            Page::Chat => Page::Sessions,
            Page::Sessions => Page::Help,
            Page::Help => Page::Models,
            Page::Models => Page::Providers,
            Page::Providers => Page::Agents,
            Page::Agents => Page::Workspaces,
            Page::Workspaces => Page::Theme,
            Page::Theme => Page::Status,
            Page::Status => Page::Mcp,
            Page::Mcp => Page::Diff,
            Page::Diff => Page::Context,
            Page::Context => Page::Commands,
            Page::Commands => Page::Chat,
        }
    }

    /// Previous page in tab order (wraps).
    #[must_use]
    pub const fn prev(self) -> Page {
        match self {
            Page::Chat => Page::Commands,
            Page::Sessions => Page::Chat,
            Page::Help => Page::Sessions,
            Page::Models => Page::Help,
            Page::Providers => Page::Models,
            Page::Agents => Page::Providers,
            Page::Workspaces => Page::Agents,
            Page::Theme => Page::Workspaces,
            Page::Status => Page::Theme,
            Page::Mcp => Page::Status,
            Page::Diff => Page::Mcp,
            Page::Context => Page::Diff,
            Page::Commands => Page::Context,
        }
    }
}

// ── Rows ──────────────────────────────────────────────────────────────

/// One selectable row on a page. `detail` carries availability text for
/// disabled rows (`unavailable: <reason>`); it never carries credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRow {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub enabled: bool,
}

/// Build an enabled row (proposal helper for adapters and tests).
#[must_use]
pub fn row(id: &str, label: &str, detail: &str) -> PageRow {
    PageRow {
        id: id.to_owned(),
        label: label.to_owned(),
        detail: detail.to_owned(),
        enabled: true,
    }
}

/// Help rows from the actual active keymap: `submit_is_enter` selects the
/// submit/newline pair exactly like `footer_hints` does. Delegates to the
/// keymap adapter [`help_rows_for_submit_mode`]; kept for API stability.
#[must_use]
pub fn help_rows(submit_is_enter: bool) -> Vec<PageRow> {
    help_rows_for_submit_mode(!submit_is_enter)
}

/// Bounded adapter for the active `Composer::SubmitKeymap`
/// (`crates/cli/src/native_composer.rs`, default `SubmitKeymap::Enter`).
/// `submit_with_ctrl_j == false` is the Enter keymap (submit `Enter`,
/// newline `Shift+Enter / Ctrl+J`); `true` is the Ctrl-J keymap (submit
/// `Ctrl+J`, newline `Enter`). The integrated Help page and the footer must
/// both call this with the active mode so their labels stay truthful; this
/// helper invents no new key protocol and advertises no unwired keys.
#[must_use]
pub fn help_rows_for_submit_mode(submit_with_ctrl_j: bool) -> Vec<PageRow> {
    let (submit, newline) = if submit_with_ctrl_j {
        ("Ctrl+J", "Enter")
    } else {
        ("Enter", "Shift+Enter / Ctrl+J")
    };
    vec![
        row("key-submit", submit, "submit composer draft"),
        row("key-newline", newline, "insert newline"),
        row("key-model", "Ctrl+P", "open model switcher"),
        row("key-context", "Ctrl+T", "open context detail"),
        row("key-help", "?", "show this help"),
    ]
    .into_iter()
    .take(MAX_ROWS)
    .collect()
}

/// Help rows for the actual current default keymap
/// (`Composer::with_keymap(SubmitKeymap::Enter)`). Callers on a non-default
/// keymap must use [`help_rows_for_submit_mode`] with the live mode instead.
#[must_use]
pub fn help_rows_default() -> Vec<PageRow> {
    help_rows_for_submit_mode(false)
}

/// Static command rows for the [`Page::Commands`] page. Ids are the routes.
#[must_use]
pub fn command_rows() -> Vec<PageRow> {
    COMMAND_ROUTES
        .iter()
        .map(|(route, page)| PageRow {
            id: (*route).to_owned(),
            label: (*route).to_owned(),
            detail: format!("open the {page} page"),
            enabled: true,
        })
        .collect()
}

/// Provider rows from real availability: `(supported, id, name, reason)`.
/// Unsupported providers are disabled with an availability reason; a
/// transport or metadata problem never becomes a row here.
#[must_use]
pub fn provider_rows(entries: &[(bool, &str, &str, &str)]) -> Vec<PageRow> {
    entries
        .iter()
        .map(|(supported, id, name, reason)| {
            let detail = if *supported {
                format!("available ({id})")
            } else {
                format!("unavailable: {reason}")
            };
            PageRow {
                id: (*id).to_owned(),
                label: (*name).to_owned(),
                detail,
                enabled: *supported,
            }
        })
        .collect()
}

/// A single disabled `Unavailable` row carrying the honest reason — used when
/// the real backend for a page (agents, or an unconfigured workspace
/// catalog) does not exist. Never fabricate selectable options.
#[must_use]
pub fn unavailable_rows(reason: &str) -> Vec<PageRow> {
    vec![PageRow {
        id: "unavailable".to_owned(),
        label: "Unavailable".to_owned(),
        detail: format!("unavailable: {reason}"),
        enabled: false,
    }]
}

// ── Intents ───────────────────────────────────────────────────────────

/// Why a pending request cannot run. Returned instead of a fake ACK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageAction {
    /// Nothing to do (no selection, or an informational page such as Help).
    None,
    /// A static local route (Commands page); no backend involved.
    Route(&'static str),
    /// A frontend-only theme change (already applied to this state; the
    /// caller persists through its own supported path, if any).
    SetTheme(String),
    /// Request a real change (provider/model/session/setting). The caller
    /// must confirm with the user and receive a backend [`Ack::Ok`] before
    /// applying anything.
    Request { page: Page, id: String },
    /// The action is unsupported/disabled; carries the availability reason.
    Unsupported {
        page: Page,
        id: String,
        reason: String,
    },
}

/// Backend verdict for a pending [`PageAction::Request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ack {
    /// Backend applied the change; the caller then reloads via `set_rows`.
    Ok,
    /// Backend refused; rows and selection are retained, the reason is
    /// recorded as the visible error.
    Rejected(String),
}

// ── State ─────────────────────────────────────────────────────────────

/// Page state: current page, bounded backend rows, byte-safe search, stable
/// id selection, frontend theme, last error, pending intent.
#[derive(Clone, Debug)]
pub struct PageState {
    current: Page,
    rows: Vec<PageRow>,
    /// Which page `rows` were loaded for (foreign rows never render).
    rows_page: Page,
    search: String,
    selected_id: Option<String>,
    theme: String,
    error: Option<String>,
    pending: Option<PageAction>,
}

impl Default for PageState {
    fn default() -> Self {
        Self {
            current: Page::Chat,
            rows: Vec::new(),
            rows_page: Page::Chat,
            search: String::new(),
            selected_id: None,
            theme: DEFAULT_THEME.to_owned(),
            error: None,
            pending: None,
        }
    }
}

impl PageState {
    /// Fresh state (default page `Chat`, default theme `dark`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to a page. Clears search and any pending intent (nothing is
    /// applied by switching); the selection survives only when it still
    /// belongs to that page's loaded rows.
    pub fn open(&mut self, page: Page) {
        if self.rows_page != page {
            self.selected_id = None;
        }
        self.current = page;
        self.search.clear();
        self.pending = None;
        self.clamp_selection();
    }

    /// Close the current page (and cancel): back to `Chat`, pending intent
    /// abandoned. Rows, selection, search, theme and error are untouched —
    /// cancel has no side effects.
    pub fn close(&mut self) {
        self.current = Page::Chat;
        self.pending = None;
    }

    /// Replace the rows for the current page from a successful backend read.
    /// Void signature preserved: delegates to [`Self::try_set_rows`] and
    /// records its bounded static refusal reason as the visible error (it
    /// never issues a backend request of its own). On success labels/details
    /// are bounded to [`MAX_ROW_BYTES`], the error clears and the selection
    /// is restored by stable id.
    pub fn set_rows(&mut self, rows: Vec<PageRow>) {
        if let Err(reason) = self.try_set_rows(rows) {
            // `try_set_rows` already recorded the bounded static reason;
            // kept explicit so a void caller can never observe a silent drop.
            self.error = Some(reason);
        }
    }

    /// Validating variant (additive; the void [`Self::set_rows`] calls it).
    /// Refuses any snapshot with more than [`MAX_ROWS`] rows **first**, with
    /// the bounded static [`ERR_TOO_MANY_ROWS`] — no silent `take`, no tail
    /// drop, only the error field changes. Then validates **exact** ids —
    /// unique, non-empty, ≤ [`MAX_ID_BYTES`], no control characters — BEFORE
    /// any label/detail clipping, row assignment or selection mutation. An
    /// invalid snapshot is refused as a whole: previous backend rows, the
    /// selected id, the query, any pending intent, `rows_page` and the
    /// current page stay intact, and only the bounded static `ERR_*` reason
    /// is recorded (raw ids are never echoed). No first-wins for duplicates,
    /// no id clipping/renaming into aliases, and no backend request is made.
    /// `Ok` commits the snapshot (validated length ≤ [`MAX_ROWS`]).
    pub fn try_set_rows(&mut self, rows: Vec<PageRow>) -> Result<(), String> {
        if rows.len() > MAX_ROWS {
            return self.refuse(ERR_TOO_MANY_ROWS);
        }
        for row in &rows {
            if row.id.is_empty() {
                return self.refuse(ERR_EMPTY_ID);
            }
            if row.id.len() > MAX_ID_BYTES {
                return self.refuse(ERR_OVERSIZED_ID);
            }
            if row.id.chars().any(char::is_control) {
                return self.refuse(ERR_MALFORMED_ID);
            }
        }
        // Exact-identity duplicate detection with a set bounded by MAX_ROWS
        // (ids are case-preserving: "OpenAI" and "openai" stay distinct).
        let mut seen: HashSet<&str> = HashSet::with_capacity(rows.len());
        for row in &rows {
            if !seen.insert(row.id.as_str()) {
                return self.refuse(ERR_DUPLICATE_ID);
            }
        }
        self.rows = rows.into_iter().map(Self::sanitize_row).collect();
        self.rows_page = self.current;
        self.error = None;
        self.clamp_selection();
        Ok(())
    }

    /// Record a backend error while retaining the last rows and selection.
    pub fn set_error(&mut self, message: &str) {
        self.error = Some(clip_bytes(message, MAX_QUERY_BYTES));
    }

    /// Set the search text (byte-bounded) and re-clamp the selection onto the
    /// first still-matching row.
    pub fn search(&mut self, text: &str) {
        self.search = clip_bytes(text, MAX_QUERY_BYTES);
        self.clamp_selection();
    }

    /// Move the selection down one visible row; clamps at the end.
    pub fn next(&mut self) {
        self.step(true);
    }

    /// Move the selection up one visible row; clamps at the start.
    pub fn prev(&mut self) {
        self.step(false);
    }

    /// The selected visible row (defaults to the first match), `None` when
    /// nothing is visible for the current page/filter.
    #[must_use]
    pub fn selected(&self) -> Option<&PageRow> {
        let visible = self.visible();
        let position = self.pos_of(&visible).unwrap_or(0);
        visible.get(position).map(|&index| &self.rows[index])
    }

    /// The visible (filtered) rows for the current page, in display order.
    #[must_use]
    pub fn filtered(&self) -> Vec<&PageRow> {
        self.visible()
            .into_iter()
            .map(|index| &self.rows[index])
            .collect()
    }

    /// All retained backend rows (bounded; last successful load).
    #[must_use]
    pub fn rows(&self) -> &[PageRow] {
        &self.rows
    }

    /// The current page.
    #[must_use]
    pub fn page(&self) -> Page {
        self.current
    }

    /// The stored frontend theme marker (no persistence from here).
    #[must_use]
    pub fn theme(&self) -> &str {
        &self.theme
    }

    /// Store a supported theme marker in frontend state. `false` (state
    /// unchanged) for markers the existing palette does not support.
    pub fn set_theme(&mut self, marker: &str) -> bool {
        if theme_supported(marker) {
            self.theme = marker.to_owned();
            true
        } else {
            false
        }
    }

    /// The last backend error, if any (rows are retained alongside it).
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The current search text.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.search
    }

    /// The pending request awaiting confirmation plus backend ACK.
    #[must_use]
    pub fn pending(&self) -> Option<&PageAction> {
        self.pending.as_ref()
    }

    /// Build the intent for the selected row. Disabled rows yield
    /// [`PageAction::Unsupported`] (never a request, never a pretend ACK);
    /// Commands yields a static [`PageAction::Route`]; Theme applies a
    /// supported marker locally and yields [`PageAction::SetTheme`]; Help is
    /// informational ([`PageAction::None`]); everything else — including
    /// [`Page::Sessions`] with a caller-fed, selected backend session row —
    /// becomes a pending [`PageAction::Request`] the caller must confirm and
    /// ACK. With no loaded row (e.g. the sessions backend still loading) the
    /// result is an honest [`PageAction::None`], never a fake shortcut.
    #[must_use]
    pub fn activate(&mut self) -> PageAction {
        let page = self.current;
        let selected = self.selected().cloned();
        let Some(row) = selected else {
            self.pending = None;
            return PageAction::None;
        };
        if !row.enabled {
            self.pending = None;
            return PageAction::Unsupported {
                page,
                id: row.id,
                reason: if row.detail.is_empty() {
                    "unavailable: disabled by the backend".to_owned()
                } else {
                    row.detail
                },
            };
        }
        let action = match page {
            Page::Commands => match Page::from_route(&row.id).and_then(Page::route) {
                Some(route) => PageAction::Route(route),
                None => PageAction::Unsupported {
                    page,
                    id: row.id,
                    reason: "unknown local route".to_owned(),
                },
            },
            Page::Theme => {
                if theme_supported(&row.id) {
                    self.theme = row.id.clone();
                    PageAction::SetTheme(row.id)
                } else {
                    PageAction::Unsupported {
                        page,
                        id: row.id,
                        reason: "unsupported theme marker".to_owned(),
                    }
                }
            }
            Page::Help => PageAction::None,
            _ => PageAction::Request { page, id: row.id },
        };
        self.pending = match &action {
            PageAction::Request { .. } => Some(action.clone()),
            _ => None,
        };
        action
    }

    /// Apply the backend verdict to the pending request: `Ok` returns the
    /// request for the caller to apply (then reload with `set_rows`);
    /// `Rejected` records the reason, retains the rows and returns an
    /// [`PageAction::Unsupported`] instead of any applied change.
    pub fn resolve_pending(&mut self, ack: Ack) -> Option<PageAction> {
        let action = self.pending.take()?;
        match ack {
            Ack::Ok => Some(action),
            Ack::Rejected(reason) => {
                let reason = clip_bytes(&reason, MAX_QUERY_BYTES);
                let unsupported = match &action {
                    PageAction::Request { page, id } => PageAction::Unsupported {
                        page: *page,
                        id: id.clone(),
                        reason: reason.clone(),
                    },
                    other => other.clone(),
                };
                self.error = Some(reason);
                Some(unsupported)
            }
        }
    }

    /// Render the current page as bounded plain lines: title, query/status,
    /// a scrolling row window around the selection, then a footer. At most
    /// `height` lines, each at most `width` characters (never split mid
    /// character). A title alone never appears without page content or an
    /// explicit empty/unavailable note.
    #[must_use]
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        if height == 0 || width == 0 {
            return Vec::new();
        }
        let visible = self.visible();
        let mut out: Vec<String> = Vec::with_capacity(height.min(64));
        out.push(clip_chars(
            &format!("== {} ==", self.current.title()),
            width,
        ));
        if height == 1 {
            return out;
        }
        out.push(clip_chars(
            &format!(
                "/{} · {} of {} rows · theme {}",
                self.search,
                visible.len(),
                self.rows.len(),
                self.theme
            ),
            width,
        ));
        if height == 2 {
            return out;
        }
        let budget = height - 3;
        let position = self.pos_of(&visible).unwrap_or(0);
        if visible.is_empty() {
            if budget > 0 {
                let note = if self.rows_page != self.current {
                    "  (no rows loaded for this page)"
                } else if !self.search.is_empty() {
                    "  (no rows match)"
                } else {
                    "  (no rows)"
                };
                out.push(clip_chars(note, width));
            }
        } else if budget > 0 {
            let start = if position >= budget {
                position + 1 - budget
            } else {
                0
            };
            let end = (start + budget).min(visible.len());
            for (offset, &index) in visible[start..end].iter().enumerate() {
                let row = &self.rows[index];
                let cursor = if start + offset == position {
                    "> "
                } else {
                    "  "
                };
                let body = if row.enabled {
                    format!("{cursor}{}", row.label)
                } else {
                    format!("{cursor}{} [disabled]", row.label)
                };
                let line = if row.detail.is_empty() {
                    body
                } else {
                    format!("{body} · {}", row.detail)
                };
                out.push(clip_chars(&line, width));
            }
        }
        while out.len() + 1 < height {
            out.push(String::new());
        }
        let footer = match &self.error {
            Some(message) => format!("error: {message}"),
            None => {
                let disabled = visible
                    .iter()
                    .filter(|&&index| !self.rows[index].enabled)
                    .count();
                format!(
                    "{} rows, {} disabled · tab: next page · esc: close · enter: select",
                    visible.len(),
                    disabled
                )
            }
        };
        out.push(clip_chars(&footer, width));
        out
    }

    // ── internals ─────────────────────────────────────────────────────

    /// Visible row indices for the current page and search filter.
    fn visible(&self) -> Vec<usize> {
        if self.rows_page != self.current {
            return Vec::new();
        }
        let query = self.search.to_lowercase();
        if query.is_empty() {
            return (0..self.rows.len()).collect();
        }
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                let haystack = format!("{} {} {}", row.id, row.label, row.detail).to_lowercase();
                haystack.contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Display position of the selected id inside the visible list.
    fn pos_of(&self, visible: &[usize]) -> Option<usize> {
        let selected = self.selected_id.as_deref()?;
        visible
            .iter()
            .position(|&index| self.rows[index].id == selected)
    }

    /// Keep the selection on a visible row (first row as the default).
    fn clamp_selection(&mut self) {
        let visible = self.visible();
        if visible.is_empty() {
            self.selected_id = None;
            return;
        }
        if self.pos_of(&visible).is_none() {
            let id = self.rows[visible[0]].id.clone();
            self.selected_id = Some(id);
        }
    }

    fn step(&mut self, forward: bool) {
        let visible = self.visible();
        if visible.is_empty() {
            self.selected_id = None;
            return;
        }
        let current = match self.pos_of(&visible) {
            Some(position) => position,
            None => {
                let id = self.rows[visible[0]].id.clone();
                self.selected_id = Some(id);
                return;
            }
        };
        let next = if forward {
            (current + 1).min(visible.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        let id = self.rows[visible[next]].id.clone();
        self.selected_id = Some(id);
    }

    /// Record a refusal: only the bounded static reason changes; rows,
    /// selection and pending intent are left untouched.
    fn refuse(&mut self, reason: &'static str) -> Result<(), String> {
        let bounded = clip_bytes(reason, MAX_QUERY_BYTES);
        self.error = Some(bounded.clone());
        Err(bounded)
    }

    /// Clip display text only. Row ids are validated verbatim by
    /// [`Self::try_set_rows`] and are never clipped or renamed here.
    fn sanitize_row(mut row: PageRow) -> PageRow {
        row.label = clip_bytes(&row.label, MAX_ROW_BYTES);
        let detail_budget = MAX_ROW_BYTES.saturating_sub(row.label.len());
        row.detail = clip_bytes(&row.detail, detail_budget);
        row
    }
}

/// Whether the existing palette supports this theme marker.
#[must_use]
pub fn theme_supported(marker: &str) -> bool {
    SUPPORTED_THEMES
        .iter()
        .any(|supported| *supported == marker)
}

/// Byte-bounded cut that never splits a UTF-8 character (storage bounds).
fn clip_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let mut cut = max_bytes;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_owned()
}

/// Column-bounded cut by characters for rendered lines (never splits a
/// character; a wide character counts as one column here).
fn clip_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_enabled(id: &str, label: &str, detail: &str, enabled: bool) -> PageRow {
        PageRow {
            id: id.to_owned(),
            label: label.to_owned(),
            detail: detail.to_owned(),
            enabled,
        }
    }

    #[test]
    fn search_is_unicode_byte_safe_and_filters() {
        let mut state = PageState::new();
        state.set_rows(vec![
            row("a", "Café ☕", "latin"),
            row("b", "日本語モデル", "cjk"),
            row("c", "naïve", "latin"),
        ]);
        state.search("café");
        let ids: Vec<&str> = state.filtered().iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["a"]);
        state.search("語");
        assert_eq!(state.filtered().len(), 1);
        state.search("☕");
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("a"));
        // Multi-byte queries that match nothing leave an empty view, no panic.
        state.search("قاعدة");
        assert!(state.filtered().is_empty());
        assert!(state.selected().is_none());
        // Search text is byte-bounded.
        state.search(&"é".repeat(MAX_QUERY_BYTES));
        assert!(state.search.len() <= MAX_QUERY_BYTES);
        state.search("");
        assert_eq!(state.filtered().len(), 3);
    }

    #[test]
    fn render_is_width_height_bounded_with_multibyte_content() {
        let mut state = PageState::new();
        state.set_rows(vec![
            row("a", "日本語のとても長いラベル", "詳細な説明テキスト"),
            row("b", "emoji 🚀 rocket", "detail"),
            row_enabled(
                "c",
                "unsupported provider",
                "unavailable: no native turn adapter",
                false,
            ),
        ]);
        for width in [0, 1, 3, 7, 40] {
            for height in [0, 1, 2, 3, 5, 50] {
                let lines = state.render(width, height);
                if width == 0 || height == 0 {
                    assert!(lines.is_empty());
                    continue;
                }
                assert!(lines.len() <= height, "w={width} h={height}");
                for line in &lines {
                    assert!(line.chars().count() <= width, "{line:?}");
                }
            }
        }
        // Disabled rows are visibly marked, with availability as detail.
        let lines = state.render(120, 12);
        let disabled_line = lines
            .iter()
            .find(|line| line.contains("[disabled]"))
            .expect("disabled row rendered");
        assert!(disabled_line.contains("unavailable: no native turn adapter"));
        assert!(lines.last().unwrap().contains("disabled"));
    }

    #[test]
    fn disabled_rows_select_but_activation_returns_unsupported_intent() {
        let mut state = PageState::new();
        state.open(Page::Providers);
        state.set_rows(provider_rows(&[
            (true, "openai", "OpenAI", ""),
            (false, "anthropic", "Anthropic", "no native turn adapter"),
        ]));
        // Selection may land on the disabled row...
        state.next();
        let selected = state.selected().expect("row selected");
        assert!(!selected.enabled);
        assert!(selected.detail.contains("unavailable"));
        // ...but activation yields a disabled intent, never a pretend ACK.
        let action = state.activate();
        assert_eq!(
            action,
            PageAction::Unsupported {
                page: Page::Providers,
                id: "anthropic".to_owned(),
                reason: "unavailable: no native turn adapter".to_owned(),
            }
        );
        assert!(state.pending().is_none());
        // The supported row requests a real change instead.
        state.prev();
        let action = state.activate();
        assert_eq!(
            action,
            PageAction::Request {
                page: Page::Providers,
                id: "openai".to_owned(),
            }
        );
        assert!(state.pending().is_some());
    }

    #[test]
    fn cancel_close_and_reload_retain_state() {
        let mut state = PageState::new();
        state.open(Page::Models);
        state.set_rows(vec![
            row("gpt-5.6", "GPT-5.6", "openai"),
            row("gpt-4.1", "GPT-4.1", "openai"),
        ]);
        state.next();
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("gpt-4.1"));
        let action = state.activate();
        assert!(matches!(action, PageAction::Request { .. }));
        assert!(state.pending().is_some());

        // Cancel/close: pending intent dropped, rows retained untouched
        // (they belong to Models, so Chat renders no rows for them).
        state.close();
        assert_eq!(state.page(), Page::Chat);
        assert!(state.pending().is_none());
        assert_eq!(state.rows().len(), 2);
        assert!(state.filtered().is_empty());

        // Error retains the last backend rows and selection.
        state.open(Page::Models);
        state.set_error("daemon unavailable");
        assert_eq!(state.error(), Some("daemon unavailable"));
        assert_eq!(state.rows().len(), 2);
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("gpt-4.1"));

        // Reload keeps selection by stable id, then clamps when it is gone.
        state.set_rows(vec![
            row("gpt-4.1", "GPT-4.1", "openai"),
            row("o3", "o3", "openai"),
        ]);
        assert!(state.error().is_none());
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("gpt-4.1"));
        state.set_rows(vec![row("o3", "o3", "openai")]);
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("o3"));

        // Opening another page hides foreign rows instead of misrendering.
        state.open(Page::Mcp);
        assert!(state.filtered().is_empty());
        let lines = state.render(80, 6);
        assert!(lines.iter().any(|l| l.contains("no rows loaded")));
    }

    #[test]
    fn rows_and_labels_are_bounded() {
        let mut state = PageState::new();
        // Cap guarantee: exactly MAX_ROWS is accepted.
        let exact: Vec<PageRow> = (0..MAX_ROWS)
            .map(|index| row(&format!("id-{index}"), &format!("label {index}"), "d"))
            .collect();
        state.set_rows(exact);
        assert_eq!(state.rows().len(), MAX_ROWS);
        let prior = state.rows().to_vec();

        // v3: an oversize snapshot is refused WHOLE (no silent take/tail
        // drop); prior state is retained and only the error field changes.
        let many: Vec<PageRow> = (0..(MAX_ROWS + 80))
            .map(|index| row(&format!("id-{index}"), &format!("label {index}"), "d"))
            .collect();
        assert!(state.try_set_rows(many).is_err());
        assert_eq!(state.error(), Some(ERR_TOO_MANY_ROWS));
        assert_eq!(state.rows(), prior.as_slice());
        assert_eq!(state.rows().len(), MAX_ROWS);

        let long_label = "é".repeat(MAX_ROW_BYTES + 100);
        state.set_rows(vec![row("a", &long_label, &long_label)]);
        assert!(state.rows()[0].label.len() <= MAX_ROW_BYTES);
        assert!(state.rows()[0].label.len() + state.rows()[0].detail.len() <= MAX_ROW_BYTES);
        // v2: empty ids are refused as a whole (never materialized into
        // alias ids); the retained snapshot still has a non-empty id and the
        // refusal reason is bounded/static.
        assert!(state.try_set_rows(vec![row("", "label only", "")]).is_err());
        assert_eq!(state.error(), Some(ERR_EMPTY_ID));
        assert!(!state.rows()[0].id.is_empty());
        assert_eq!(state.rows()[0].id, "a");
    }

    #[test]
    fn pending_request_requires_backend_ack() {
        let mut state = PageState::new();
        state.open(Page::Models);
        state.set_rows(vec![row("gpt-5.6", "GPT-5.6", "openai")]);
        let action = state.activate();
        assert!(matches!(
            action,
            PageAction::Request {
                page: Page::Models,
                ..
            }
        ));
        assert_eq!(state.pending(), Some(&action));

        // Rejection: unsupported intent + error, rows retained, no change.
        let rejected = state
            .resolve_pending(Ack::Rejected("provider refused".to_owned()))
            .expect("intent resolved");
        assert!(matches!(rejected, PageAction::Unsupported { .. }));
        assert_eq!(state.error(), Some("provider refused"));
        assert_eq!(state.rows().len(), 1);
        assert!(state.pending().is_none());
        // A second resolve with no pending intent does nothing.
        assert!(state.resolve_pending(Ack::Ok).is_none());

        // Acceptance: the request is handed back for the caller to apply
        // after the real backend ACK; rows reload only via `set_rows`.
        let action = state.activate();
        let accepted = state.resolve_pending(Ack::Ok).expect("acked");
        assert_eq!(accepted, action);
        assert!(state.pending().is_none());
    }

    #[test]
    fn unsupported_action_is_never_a_pretend_ack() {
        let mut state = PageState::new();
        state.open(Page::Agents);
        state.set_rows(unavailable_rows(
            "no native agent registry backend is installed",
        ));
        assert!(!state.selected().expect("row").enabled);
        let action = state.activate();
        assert!(matches!(action, PageAction::Unsupported { .. }));
        assert!(state.pending().is_none());
        let rendered = state.render(100, 8);
        assert!(rendered.iter().any(|line| line.contains("Unavailable")));
        assert!(rendered
            .iter()
            .any(|line| line.contains("no native agent registry backend")));
    }

    #[test]
    fn theme_choice_is_frontend_state_with_existing_markers_only() {
        let mut state = PageState::new();
        assert_eq!(state.theme(), DEFAULT_THEME);
        assert!(theme_supported("dark"));
        assert!(theme_supported("light"));
        assert!(theme_supported("nocolor"));
        assert!(!theme_supported("neon"));

        state.open(Page::Theme);
        state.set_rows(vec![
            row("dark", "Dark", "default"),
            row("light", "Light", ""),
            row("neon", "Neon", "not installed"),
        ]);
        // Supported marker: stored locally, no fake persistence involved.
        state.next();
        assert_eq!(state.activate(), PageAction::SetTheme("light".to_owned()));
        assert_eq!(state.theme(), "light");
        // Unsupported marker: disabled intent, state unchanged.
        state.next();
        assert!(matches!(state.activate(), PageAction::Unsupported { .. }));
        assert_eq!(state.theme(), "light");
        assert!(!state.set_theme("neon"));
        assert_eq!(state.theme(), "light");
    }

    #[test]
    fn commands_page_yields_static_local_routes() {
        assert_eq!(COMMAND_ROUTES.len(), 11);
        assert_eq!(command_rows().len(), 11);
        assert_eq!(Page::from_route("/sessions"), Some(Page::Sessions));
        assert_eq!(Page::from_route("/models"), Some(Page::Models));
        assert_eq!(Page::from_route("/connect"), Some(Page::Providers));
        assert_eq!(Page::from_route("/agents"), Some(Page::Agents));
        assert_eq!(Page::from_route("/workspace"), Some(Page::Workspaces));
        assert_eq!(Page::from_route("/commands"), None);
        // v4: /sessions belongs to Page::Sessions; Chat is the routeless home.
        assert_eq!(Page::Sessions.route(), Some("/sessions"));
        assert_eq!(Page::Chat.route(), None);
        let sessions = command_rows()
            .into_iter()
            .find(|row| row.id == "/sessions")
            .expect("/sessions command exists");
        assert!(sessions.enabled, "/sessions is an enabled command");
        assert_eq!(sessions.detail, "open the Sessions page");

        let mut state = PageState::new();
        state.open(Page::Commands);
        state.set_rows(command_rows());
        assert_eq!(state.activate(), PageAction::Route("/sessions"),);
        state.search("/theme");
        assert_eq!(state.activate(), PageAction::Route("/theme"));
        assert!(state.pending().is_none(), "routes need no backend ACK");
    }

    #[test]
    fn help_rows_mirror_the_actual_keybindings_only() {
        let enter = help_rows(true);
        assert_eq!(enter.len(), 5);
        assert_eq!(enter[0].label, "Enter");
        assert_eq!(enter[1].label, "Shift+Enter / Ctrl+J");
        let ctrl_j = help_rows(false);
        assert_eq!(ctrl_j[0].label, "Ctrl+J");
        assert_eq!(ctrl_j[1].label, "Enter");
        let labels: Vec<&str> = enter.iter().map(|r| r.label.as_str()).collect();
        assert!(labels.contains(&"Ctrl+P"));
        assert!(labels.contains(&"Ctrl+T"));
        assert_eq!(enter[4].label, "?");
        assert!(enter.iter().all(|r| r.enabled));

        // Help activation is informational: no intent, no pending request.
        let mut state = PageState::new();
        state.open(Page::Help);
        state.set_rows(help_rows(true));
        assert_eq!(state.activate(), PageAction::None);
        assert!(state.pending().is_none());
    }

    #[test]
    fn page_tab_order_round_trips_and_opens_close_to_chat() {
        let mut seen = Vec::new();
        let mut page = Page::Chat;
        for _ in Page::ORDER {
            seen.push(page);
            page = page.next();
        }
        assert_eq!(seen, Page::ORDER.to_vec());
        assert_eq!(page, Page::Chat);
        for page in Page::ORDER {
            assert_eq!(page.next().prev(), page);
            assert_eq!(page.prev().next(), page);
        }
        let mut state = PageState::new();
        for page in Page::ORDER {
            state.open(page);
            assert_eq!(state.page(), page);
        }
        // Switching pages clears search and pending intent.
        state.open(Page::Models);
        state.set_rows(vec![row("m", "M", "")]);
        state.search("m");
        let _ = state.activate();
        assert!(state.pending().is_some());
        state.open(Page::Status);
        assert!(state.query().is_empty());
        assert!(state.pending().is_none());
        state.close();
        assert_eq!(state.page(), Page::Chat);
    }

    // ── v2 repairs (pages-review-v1 VERDICT: NEEDS_FIX) ────────────────

    #[test]
    fn duplicate_exact_ids_refuse_snapshot_and_keep_identity() {
        let mut state = PageState::new();
        state.open(Page::Providers);
        state.set_rows(vec![
            row("openai", "OpenAI", "available (openai)"),
            row_enabled(
                "anthropic",
                "Anthropic",
                "unavailable: no native turn adapter",
                false,
            ),
        ]);
        state.next();
        // Selection may sit on the disabled row; activation refuses it.
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("anthropic"));
        assert!(matches!(state.activate(), PageAction::Unsupported { .. }));
        // A pending intent on the supported row.
        state.prev();
        let _ = state.activate();
        assert!(state.pending().is_some());
        let rows_before = state.rows().to_vec();
        let selected_before = state.selected().map(|r| r.id.clone());

        // Duplicate exact ids (second one disabled) → whole snapshot refused,
        // never first-wins and never "the disabled twin wins".
        let refused = state.try_set_rows(vec![
            row("openai", "OpenAI EU", "available"),
            row_enabled("openai", "OpenAI EU legacy", "available", false),
        ]);
        assert!(refused.is_err());
        assert_eq!(state.error(), Some(ERR_DUPLICATE_ID));
        // Rows, selected id and pending identity all intact.
        assert_eq!(state.rows(), rows_before.as_slice());
        assert_eq!(state.selected().map(|r| r.id.clone()), selected_before);
        assert!(state.pending().is_some());
        // Activation still targets the retained valid row, never a refused one.
        assert_eq!(
            state.activate(),
            PageAction::Request {
                page: Page::Providers,
                id: "openai".to_owned(),
            }
        );
        // Exact, case-preserving identity: different case is not a duplicate
        // (no provider normalization invented).
        assert!(state
            .try_set_rows(vec![
                row("OpenAI", "OpenAI", "available"),
                row("openai", "OpenAI legacy", "available"),
            ])
            .is_ok());
        assert_eq!(state.rows().len(), 2);
    }

    #[test]
    fn empty_and_oversized_ids_refuse_instead_of_aliasing() {
        let mut state = PageState::new();
        state.set_rows(vec![row("stable-id", "Stable", "detail")]);
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("stable-id"));

        // Two distinct ids that the v1 id clip would have aliased into the
        // same 256-byte prefix: refused as a whole, never renamed/merged.
        let long_a = format!("{}-a", "x".repeat(MAX_ID_BYTES));
        let long_b = format!("{}-b", "x".repeat(MAX_ID_BYTES));
        assert!(long_a.len() > MAX_ID_BYTES && long_b.len() > MAX_ID_BYTES);
        assert!(state
            .try_set_rows(vec![row(&long_a, "A", ""), row(&long_b, "B", "")])
            .is_err());
        assert_eq!(state.error(), Some(ERR_OVERSIZED_ID));
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.rows()[0].id, "stable-id");
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("stable-id"));

        // Empty id refused; no `row-N` alias ever appears.
        assert!(state.try_set_rows(vec![row("", "label only", "")]).is_err());
        assert_eq!(state.error(), Some(ERR_EMPTY_ID));
        assert_eq!(state.rows()[0].id, "stable-id");

        // Control characters are malformed identity.
        assert!(state
            .try_set_rows(vec![row("bad\u{0}id", "Bad", "")])
            .is_err());
        assert_eq!(state.error(), Some(ERR_MALFORMED_ID));
        assert_eq!(state.rows().len(), 1);

        // The void path records the same bounded static refusal and still
        // changes nothing else.
        state.set_rows(vec![row("", "still bad", "")]);
        assert_eq!(state.error(), Some(ERR_EMPTY_ID));
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.selected().map(|r| r.id.as_str()), Some("stable-id"));
    }

    // ── v3 repair (oversize snapshot: no silent tail drop) ─────────────

    #[test]
    fn oversize_snapshot_refuses_whole_and_keeps_all_state() {
        let mut state = PageState::new();
        state.open(Page::Models);
        state.set_rows(vec![
            row("gpt-5.6", "GPT-5.6", "openai"),
            row("gpt-4.1", "GPT-4.1", "openai"),
        ]);
        state.search("gpt");
        state.next();
        let _ = state.activate();
        assert!(state.pending().is_some());
        let rows_before = state.rows().to_vec();
        let selected_before = state.selected().map(|r| r.id.clone());
        let query_before = state.query().to_owned();
        let pending_before = state.pending().cloned();
        let page_before = state.page();

        let many: Vec<PageRow> = (0..(MAX_ROWS + 80))
            .map(|index| row(&format!("id-{index}"), "l", "d"))
            .collect();
        assert!(state.try_set_rows(many).is_err());
        // Only the error field changed; every other piece of state survived.
        assert_eq!(state.error(), Some(ERR_TOO_MANY_ROWS));
        assert_eq!(state.rows(), rows_before.as_slice());
        assert_eq!(state.selected().map(|r| r.id.clone()), selected_before);
        assert_eq!(state.query(), query_before);
        assert_eq!(state.pending(), pending_before.as_ref());
        assert_eq!(state.page(), page_before);
        assert_eq!(state.page(), Page::Models);

        // `rows_page` is untouched too: after switching pages, the render
        // note still reports the foreign-page rows (it would change to
        // "(no rows match)" only if a refusal had reassigned `rows_page`).
        state.open(Page::Help); // our own op: clears search + pending
        state.search("zzz-no-match");
        let note_before: Vec<String> = state
            .render(80, 8)
            .into_iter()
            .filter(|line| line.contains("no rows"))
            .collect();
        assert_eq!(note_before.len(), 1);
        assert!(note_before[0].contains("no rows loaded for this page"));
        let help_rows_before = state.rows().to_vec();
        let many: Vec<PageRow> = (0..(MAX_ROWS + 80))
            .map(|index| row(&format!("id-{index}"), "l", "d"))
            .collect();
        assert!(state.try_set_rows(many).is_err());
        assert_eq!(state.error(), Some(ERR_TOO_MANY_ROWS));
        assert_eq!(state.page(), Page::Help);
        assert_eq!(state.query(), "zzz-no-match");
        assert_eq!(state.rows(), help_rows_before.as_slice());
        let note_after: Vec<String> = state
            .render(80, 8)
            .into_iter()
            .filter(|line| line.contains("no rows"))
            .collect();
        assert_eq!(note_before, note_after);
    }

    #[test]
    fn boundary_of_exactly_max_rows_valid_snapshot() {
        let mut state = PageState::new();
        state.set_rows(vec![row("keep", "Keep", "d")]);

        // Duplicate row at index MAX_ROWS (len MAX_ROWS+1): refused whole
        // even though the first MAX_ROWS rows are valid — no tail drop, no
        // first-wins.
        let mut dup: Vec<PageRow> = (0..MAX_ROWS)
            .map(|index| row(&format!("id-{index}"), "l", "d"))
            .collect();
        dup.push(row("id-7", "tail duplicate", "d"));
        assert_eq!(dup.len(), MAX_ROWS + 1);
        assert!(state.try_set_rows(dup).is_err());
        assert_eq!(state.error(), Some(ERR_TOO_MANY_ROWS));
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.rows()[0].id, "keep");

        // Malformed (empty) row at index MAX_ROWS: same whole refusal.
        let mut bad: Vec<PageRow> = (0..MAX_ROWS)
            .map(|index| row(&format!("id-{index}"), "l", "d"))
            .collect();
        bad.push(row("", "empty id tail", ""));
        assert!(state.try_set_rows(bad).is_err());
        assert_eq!(state.error(), Some(ERR_TOO_MANY_ROWS));
        assert_eq!(state.rows()[0].id, "keep");

        // Exactly MAX_ROWS unique valid rows: accepted, error cleared.
        let ok: Vec<PageRow> = (0..MAX_ROWS)
            .map(|index| row(&format!("id-{index}"), &format!("label {index}"), "d"))
            .collect();
        assert_eq!(ok.len(), MAX_ROWS);
        assert!(state.try_set_rows(ok).is_ok());
        assert_eq!(state.rows().len(), MAX_ROWS);
        assert!(state.error().is_none());

        // Boundary: a duplicate inside exactly MAX_ROWS is still caught by
        // id validation (ERR_DUPLICATE_ID), not by the cap check.
        let mut dup_in: Vec<PageRow> = (0..MAX_ROWS)
            .map(|index| row(&format!("id-{index}"), "l", "d"))
            .collect();
        dup_in[MAX_ROWS - 1] = row("id-3", "in-range duplicate", "d");
        assert!(state.try_set_rows(dup_in).is_err());
        assert_eq!(state.error(), Some(ERR_DUPLICATE_ID));
        assert_eq!(state.rows().len(), MAX_ROWS);
    }

    #[test]
    fn high_contrast_theme_row_is_enabled_and_accepted() {
        // Fixture update (documented): SUPPORTED_THEMES grew 3 → 4 with the
        // real high-contrast builtin of native_theme::builtin_for_marker.
        assert_eq!(SUPPORTED_THEMES.len(), 4);
        assert_eq!(
            SUPPORTED_THEMES,
            ["dark", "light", "nocolor", "high-contrast"]
        );
        let mut state = PageState::new();
        assert_eq!(state.theme(), DEFAULT_THEME);
        state.open(Page::Theme);
        state.set_rows(vec![
            row("dark", "Dark", "default"),
            row("light", "Light", ""),
            row("nocolor", "No Color", ""),
            row(
                "high-contrast",
                "High Contrast",
                "real native_theme builtin",
            ),
        ]);
        state.next();
        state.next();
        state.next();
        let selected = state.selected().expect("high-contrast selected");
        assert_eq!(selected.id, "high-contrast");
        assert!(selected.enabled, "high-contrast must be an enabled choice");
        // Selection intent: SetTheme under the same frontend ownership, no
        // backend ACK, no fake persistence.
        assert_eq!(
            state.activate(),
            PageAction::SetTheme("high-contrast".to_owned())
        );
        assert_eq!(state.theme(), "high-contrast");
        assert!(state.pending().is_none());

        // Unknown marker: refused with no effects (regression).
        state.set_rows(vec![row("neon", "Neon", "not installed")]);
        assert!(matches!(state.activate(), PageAction::Unsupported { .. }));
        assert_eq!(state.theme(), "high-contrast");
        assert!(!state.set_theme("neon"));
        assert_eq!(state.theme(), "high-contrast");
    }

    #[test]
    fn help_adapter_covers_both_submit_keymaps_truthfully() {
        let enter = help_rows_for_submit_mode(false);
        let ctrl_j = help_rows_for_submit_mode(true);
        assert_eq!(enter.len(), 5);
        assert_eq!(enter[0].label, "Enter");
        assert_eq!(enter[0].detail, "submit composer draft");
        assert_eq!(enter[1].label, "Shift+Enter / Ctrl+J");
        assert_eq!(enter[1].detail, "insert newline");
        assert_eq!(ctrl_j[0].label, "Ctrl+J");
        assert_eq!(ctrl_j[1].label, "Enter");
        assert_eq!(ctrl_j[1].detail, "insert newline");
        // Only the submit/newline pair differs between the two modes.
        assert_eq!(enter[2..], ctrl_j[2..]);
        // Default mirrors the actual current keymap (SubmitKeymap::Enter).
        assert_eq!(help_rows_default(), enter);
        // Legacy API delegates to the adapter unchanged.
        assert_eq!(help_rows(true), enter);
        assert_eq!(help_rows(false), ctrl_j);
        // Bounded: never exceeds the row bound.
        assert!(help_rows_for_submit_mode(true).len() <= MAX_ROWS);
    }

    // ── v4 (installed-UI gap: additive Page::Sessions) ──────────────────

    #[test]
    fn sessions_page_honest_when_unloaded_then_requests_real_session_id() {
        let mut state = PageState::new();
        state.open(Page::Sessions);
        // No rows yet (backend still loading): honest empty state with an
        // explicit render note — never a fake-functional shortcut.
        assert_eq!(state.activate(), PageAction::None);
        assert!(state.pending().is_none());
        assert!(state
            .render(80, 8)
            .iter()
            .any(|line| line.contains("no rows")));

        // Caller feeds rows from the real backend (GET /api/sessions):
        // id = backend session UUID, label = title, detail = active marker,
        // enabled = true. This module hardcodes no Chat/dummy session rows.
        state.set_rows(vec![
            row(
                "8b2f1c9e-4f6a-4d2b-9c3e-1a2b3c4d5e6f",
                "Bug triage",
                "active",
            ),
            row("3d7e9a10-2b4c-4e8f-a6d1-0f1e2d3c4b5a", "Docs rewrite", ""),
        ]);
        state.search("docs");
        let selected = state.selected().expect("filtered selection");
        assert_eq!(selected.id, "3d7e9a10-2b4c-4e8f-a6d1-0f1e2d3c4b5a");
        // The highlighted (cursor) row is exactly the selected, searchable one.
        let lines = state.render(80, 8);
        assert!(lines
            .iter()
            .any(|line| line.starts_with("> ") && line.contains("Docs rewrite")));

        // Existing generic Request intent shape with the real session id.
        let action = state.activate();
        assert_eq!(
            action,
            PageAction::Request {
                page: Page::Sessions,
                id: "3d7e9a10-2b4c-4e8f-a6d1-0f1e2d3c4b5a".to_owned(),
            }
        );
        assert_eq!(state.pending(), Some(&action));

        // Cancel keeps everything external: this state stores no chat draft
        // and fabricates no write; only the pending intent is abandoned.
        state.close();
        assert!(state.pending().is_none());
        assert_eq!(state.page(), Page::Chat);
        assert_eq!(state.rows().len(), 2, "cancel does not mutate backend rows");
        assert_eq!(state.query(), "docs", "cancel changes no search state");
    }

    #[test]
    fn sessions_addition_keeps_other_unsupported_paths_unsupported() {
        let mut state = PageState::new();
        // Another page's unavailable backend is still an unsupported intent.
        state.open(Page::Agents);
        state.set_rows(unavailable_rows(
            "no native agent registry backend is installed",
        ));
        assert!(matches!(
            state.activate(),
            PageAction::Unsupported {
                page: Page::Agents,
                ..
            }
        ));
        assert!(state.pending().is_none());
        // An unknown command route stays unsupported (never a pretend route).
        state.open(Page::Commands);
        state.set_rows(vec![row("/no-such-route", "/no-such-route", "")]);
        assert!(matches!(state.activate(), PageAction::Unsupported { .. }));
        // A disabled session row (backend verdict) refuses with its reason.
        state.open(Page::Sessions);
        state.set_rows(vec![PageRow {
            id: "00000000-0000-0000-0000-000000000000".to_owned(),
            label: "Archived".to_owned(),
            detail: "unavailable: session archived".to_owned(),
            enabled: false,
        }]);
        assert!(matches!(
            state.activate(),
            PageAction::Unsupported {
                page: Page::Sessions,
                ..
            }
        ));
        assert!(state.pending().is_none());
    }
}
