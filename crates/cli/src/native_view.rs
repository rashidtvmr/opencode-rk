#![forbid(unsafe_code)]
//! `NativeView` facade: the visible three-pane native shell.
//!
//! This file is integration-owned glue over the six `native_view_*` part
//! modules (layout/sessions/chat/pages/status/dialogs). It exposes only
//! startup/resize/render plus page/session action intents. Deliberately
//! IO-free: fetching (`GET /api/sessions`, `GET /api/mcp`) and every HTTP
//! transition stay in `tui_entry`; render functions only format bounded
//! strings for the preserved native bridge paint path.
//!
//! Honest-capability rule: labels/`Unavailable` lines are emitted from real
//! connection/backend facts only — never dummy data.

use crate::native_view_chat::ChatView;
use crate::native_view_dialogs::DialogView;
use crate::native_view_layout::{compose_frame, Layout};
use crate::native_view_pages::{Page, PageRow, PageState};
use crate::native_view_sessions::{SessionRow, SessionView};
use crate::native_view_status::StatusView;
use serde_json::Value;

/// Product banner (frozen PTY foundation control: `control.banner`).
const BANNER: &str = "OpenCode RK (native)";
/// Draft bytes carried into one render (paint sanitizes control chars and
/// clips to pane width; this bound just keeps allocation fixed-size).
const MAX_RENDER_DRAFT_CHARS: usize = 4096;

pub struct NativeView {
    layout: Layout,
    sessions: SessionView,
    chat: ChatView,
    pages: PageState,
    status: StatusView,
    /// Held from day one; `DialogView::request/answer` wiring is explicitly
    /// pending until the dialogs part delivers its request/answer API
    /// (inventing calls here would not compile). The live `/connect`,
    /// `/models`, Ctrl-P credential dialogs remain the existing
    /// `native_setup` center-dialog path in `tui_entry`.
    pub dialogs: DialogView,
    /// Facade-owned permission retry correlation (NOT part of the approved
    /// `DialogView`): the attempt number of the last dispatched reply whose
    /// acknowledgement did NOT legitimately resolve the decision
    /// (`AckOutcome::Rejected` — the module reopened the choice). While
    /// set, the empty-GET poll must never close the dialog: the decision
    /// awaits an EXPLICIT user retry (Esc re-sends reject), never an
    /// automatic resend, auto-approve or silent local close.
    permission_retry_attempt: Option<u64>,
    workspace: String,
    /// Active submit keymap fact (set once by `tui_entry`): keeps the
    /// Pages Help rows truthful about which key submits vs inserts newline.
    submit_with_ctrl_j: bool,
}

/// Captured binding identity of one in-flight turn worker. A worker is
/// launched under one `(session, generation)` binding and its events may be
/// applied only while both still match the current binding; a superseded
/// (stale) worker's events are drained and discarded, never rendered.
pub struct OwnedWorker<W> {
    pub worker: W,
    session_id: String,
    generation: u64,
}

impl<W> OwnedWorker<W> {
    pub fn new(worker: W, session_id: impl Into<String>, generation: u64) -> Self {
        Self {
            worker,
            session_id: session_id.into(),
            generation,
        }
    }

    /// True only while this worker still owns the current binding: same
    /// active session AND same binding generation (generations never wrap:
    /// they are reserved through [`reserve_generation`] before any bind
    /// side effect and committed only after a successful bind).
    pub fn owns(&self, active: Option<&str>, generation: u64) -> bool {
        self.generation == generation && active == Some(self.session_id.as_str())
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Reserve the NEXT binding generation before any bind side effect (fetch,
/// create POST, switch). `None` on `u64` exhaustion: the caller fails closed
/// with zero mutation and no `OwnedWorker` identity is ever constructed; a
/// generation is committed (`binding_generation = prospective`) ONLY after a
/// successful bind, so identities are never reused or wrapped.
pub fn reserve_generation(current: u64) -> Option<u64> {
    current.checked_add(1)
}

/// Maximum characters in the facade-owned page query buffer (Sessions
/// filter / Ctrl-K command buffer). Together with the pages module's own
/// [`crate::native_view_pages::MAX_QUERY_BYTES`] byte cap this keeps the
/// applied query always byte-identical to the typed buffer — the facade
/// refuses characters past the bound instead of letting the part module
/// silently re-clip. The buffer is never the composer draft.
const MAX_PAGE_QUERY_CHARS: usize = 256;

/// Bind preflight bound: mirrors the sessions module row cap
/// (`SessionView::set_rows` keeps at most its own first-cap entries). A
/// fresh listing at or below this bound can never be truncated by the
/// refresh, so the proved target membership provably survives `set_rows`
/// (canonical-UUID ids are sanitize-stable and dedup keeps at least one
/// occurrence). A larger listing is refused with zero mutation instead of
/// risking a truncated refresh that silently breaks the active
/// acknowledgement. ponytail: if the module row cap changes, widen in
/// lockstep.
const MAX_BIND_FRESH_ROWS: usize = 256;

/// Successful bind commit: the newly active session id and the composer
/// draft restored for it (the store keeps its own copy — nothing is
/// cleared by the view).
pub struct BindCommit {
    pub session_id: String,
    pub draft: String,
}

impl NativeView {
    /// Startup composition for the given kernel geometry.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            layout: Layout::new(cols, rows),
            sessions: SessionView::default(),
            chat: ChatView::default(),
            pages: PageState::default(),
            status: StatusView::default(),
            dialogs: DialogView::default(),
            permission_retry_attempt: None,
            workspace: String::new(),
            submit_with_ctrl_j: false,
        }
    }

    /// Kernel geometry changed: rebuild layout, keep scroll/page state.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.layout = Layout::new(cols, rows);
    }

    // ---------- startup facts (set from tui_entry, no IO here) ----------

    pub fn set_model(&mut self, model: Option<String>) {
        self.status.set_model(model);
    }

    /// Model state was never observed (failed catalogue fetch): honest
    /// `Unavailable`, never `None` (which would claim "confirmed unset").
    pub fn set_model_unknown(&mut self) {
        self.status.set_model_unknown();
    }

    pub fn set_connection(&mut self, connected: bool) {
        self.status.set_connection(connected);
    }

    /// Connection never observed (failed fetch): `Unavailable`, not a
    /// fabricated connected/disconnected claim.
    pub fn set_connection_unknown(&mut self) {
        self.status.set_connection_unknown();
    }

    /// Active submit keymap fact for truthful Help rows.
    pub fn set_submit_with_ctrl_j(&mut self, submit_with_ctrl_j: bool) {
        self.submit_with_ctrl_j = submit_with_ctrl_j;
    }

    pub fn set_session(&mut self, session: Option<String>) {
        self.status.set_session(session);
    }

    /// Forward an ACCEPTED authenticated binding (id + title + confirmed
    /// persisted message count from the SAME accepted snapshot) into the
    /// status projection. See
    /// [`crate::native_view_status::StatusView::set_session_binding`].
    pub fn set_session_binding(
        &mut self,
        session: Option<String>,
        title: Option<String>,
        message_count: Option<usize>,
    ) {
        self.status
            .set_session_binding(session, title, message_count);
    }

    pub fn set_workspace(&mut self, workspace: String) {
        self.workspace = workspace;
    }

    pub fn set_session_rows(&mut self, rows: Vec<SessionRow>, active: Option<String>) {
        self.sessions.set_rows(rows, active);
    }

    pub fn set_page_rows(&mut self, rows: Vec<PageRow>) {
        self.pages.set_rows(rows);
    }

    /// Backend MCP snapshot (`GET /api/mcp`) projected into the right pane.
    pub fn apply_mcp(&mut self, value: &Value) -> Result<(), String> {
        self.status.apply_mcp(value)
    }

    // ---------- page action intents (pure state; HTTP stays outside) ----

    pub fn open_page(&mut self, page: Page) {
        self.pages.open(page);
    }

    pub fn close_page(&mut self) {
        self.pages.close();
    }

    pub fn page(&self) -> Page {
        self.pages.page()
    }

    pub fn page_next(&mut self) {
        self.pages.next();
    }

    pub fn page_prev(&mut self) {
        self.pages.prev();
    }

    pub fn page_selected(&self) -> Option<&PageRow> {
        self.pages.selected()
    }

    /// Confirmation handler: open the selected (enabled) page row, close on
    /// `Chat`. Disabled rows stay closed — their `detail` already states the
    /// explicit unsupported/unavailable reason.
    pub fn open_selected(&mut self) {
        let id = self
            .pages
            .selected()
            .map(|row| (row.id.clone(), row.enabled));
        match id {
            Some((_, false)) => {}
            Some((row_id, true)) => match row_id.as_str() {
                "chat" => self.pages.close(),
                // NOTE: no "sessions" arm — the real `Page::Sessions` is
                // opened by the `/sessions` command, which also loads its
                // authenticated rows (open first, then load: rows bind to
                // the page current at load time). No fake route (e.g. via
                // Chat) exists and the index carries no dead session row.
                "status" => self.pages.open(Page::Status),
                "mcp" => self.pages.open(Page::Mcp),
                "models" => self.pages.open(Page::Models),
                "providers" => self.pages.open(Page::Providers),
                "workspaces" => self.pages.open(Page::Workspaces),
                "agents" => self.pages.open(Page::Agents),
                "theme" => self.pages.open(Page::Theme),
                "diff" => self.pages.open(Page::Diff),
                "context" => self.pages.open(Page::Context),
                "commands" => self.open_commands_page(),
                "help" => {
                    // Help page rows come from the approved keymap adapter
                    // with the ACTIVE submit mode; keys not wired in this
                    // native loop are marked disabled with the honest reason
                    // (no unsupported key is advertised as actionable).
                    self.pages.open(Page::Help);
                    let mut rows = crate::native_view_pages::help_rows_for_submit_mode(
                        self.submit_with_ctrl_j,
                    );
                    for row in &mut rows {
                        if matches!(row.id.as_str(), "key-context" | "key-help") {
                            row.enabled = false;
                            row.detail = "not wired in the native loop".to_owned();
                        }
                    }
                    self.pages.set_rows(rows);
                }
                _ => {}
            },
            None => self.pages.close(),
        }
    }

    /// Page confirmation (Enter on an open page) — the SINGLE production
    /// confirm path. Pure page state only: the composer draft is passed in
    /// deliberately so the one place a confirm could ever touch it can be
    /// asserted byte-identical afterwards. No confirm path may clear the
    /// draft; only an accepted turn does.
    pub fn confirm_page(&mut self, composer: &mut String) {
        let before = composer.clone();
        self.open_selected();
        debug_assert_eq!(
            *composer, before,
            "page confirm must never mutate the composer draft"
        );
    }

    /// Build the real intent for the selected row of the open page. For a
    /// caller-fed backend page (`Page::Sessions` with authenticated rows)
    /// this becomes a pending [`PageAction::Request`] the caller must run
    /// through its own guarded bind transaction and resolve with a real
    /// backend verdict — never a pretend ACK.
    pub fn activate_open_page(&mut self) -> crate::native_view_pages::PageAction {
        self.pages.activate()
    }

    /// Apply a real backend verdict to the pending page request. `Ack::Ok`
    /// (only after an actual bind success) returns the request for the
    /// caller to apply and reload rows with `set_page_rows`; `Ack::Rejected`
    /// retains rows/selection, records the bounded reason as the visible
    /// page error and returns [`PageAction::Unsupported`].
    pub fn resolve_page_ack(
        &mut self,
        ack: crate::native_view_pages::Ack,
    ) -> Option<crate::native_view_pages::PageAction> {
        self.pages.resolve_pending(ack)
    }

    // ---------- page query (typed filter / command buffer) ----------

    /// Current bounded page query: the Sessions filter, or the typed
    /// command buffer while the Commands page owns focus. A facade-owned
    /// buffer living in `PageState` — never the composer draft, never a
    /// per-session stored draft.
    pub fn page_query(&self) -> &str {
        self.pages.query()
    }

    /// Append one character to the page query. The buffer is bounded to
    /// [`MAX_PAGE_QUERY_CHARS`] characters AND the pages module's byte
    /// cap, so the applied query is always byte-identical to what was
    /// typed (the part module never has to re-clip). Returns `false` when
    /// the bound refuses the character; the query is then left intact.
    pub fn page_query_push(&mut self, character: char) -> bool {
        let query = self.pages.query();
        if query.chars().count() + 1 > MAX_PAGE_QUERY_CHARS
            || query.len() + character.len_utf8() > crate::native_view_pages::MAX_QUERY_BYTES
        {
            return false;
        }
        let mut text = query.to_owned();
        text.push(character);
        self.pages.search(&text);
        true
    }

    /// Remove the last character from the page query. Returns `false`
    /// when the buffer is already empty; the selection is only re-clamped
    /// onto the rows the new filter makes visible.
    pub fn page_query_backspace(&mut self) -> bool {
        let mut text = self.pages.query().to_owned();
        if text.pop().is_none() {
            return false;
        }
        self.pages.search(&text);
        true
    }

    /// Open the Commands page with its static route rows bound to the
    /// current page (open FIRST, then load: rows bind at load time — the
    /// same ordering as `/sessions`). The SINGLE production open path:
    /// the `/commands` route, the Ctrl-K command input and the index row
    /// all route through here, so the page is never reachable without its
    /// rows.
    pub fn open_commands_page(&mut self) {
        self.pages.open(Page::Commands);
        self.pages
            .set_rows(crate::native_view_pages::command_rows());
    }

    /// Command-mode dispatch on the open Commands page. The typed query
    /// IS the command buffer: independent of, and never touching, the
    /// composer draft. An exact recognized local route typed in the
    /// buffer wins; otherwise the selected (filtered) row's static route;
    /// `None` when neither resolves — the caller reports an honest
    /// unknown-command and no route is ever fabricated.
    pub fn dispatch_command(&mut self) -> Option<&'static str> {
        let typed = self.pages.query().trim().to_owned();
        if let Some(page) = Page::from_route(&typed) {
            return page.route();
        }
        match self.pages.activate() {
            crate::native_view_pages::PageAction::Route(route) => Some(route),
            _ => None,
        }
    }

    // ---------- chat scroll intents ----------

    pub fn scroll_up(&mut self, lines: usize) {
        self.chat.scroll_up(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.chat.scroll_down(lines);
    }

    pub fn follow_tail(&mut self) {
        self.chat.follow_tail();
    }

    // ---------- session picker intents ----------

    /// Move the highlight only. The active session never changes here;
    /// [`session_confirm`] applies a backend-ACKed switch.
    pub fn session_next(&mut self) -> Option<String> {
        self.sessions.select_next()
    }

    pub fn session_prev(&mut self) -> Option<String> {
        self.sessions.select_prev()
    }

    /// Backend ACK applied: the highlighted session is now active.
    pub fn session_confirm(&mut self, id: &str) {
        self.sessions.acknowledge_active(id);
    }

    // ---------- permission reply retry correlation (facade-owned) ----------

    /// Correlated acknowledgement: the real `DialogView::ack` plus the
    /// retry-await flag in the same production call the dispatch uses.
    /// Records the flag ONLY on the actual [`AckOutcome::Rejected`] outcome
    /// (dialog reopened after a failed or non-accepting ACK) — never
    /// merely because the intent kind was `DecisionIntent::Reject`: a
    /// successful user deny answered with an empty `204` legitimately
    /// closes ([`AckOutcome::Closed`], flag cleared) and is never retried.
    /// A stale attempt or mismatched identity is refused by the module
    /// with no state change; this wrapper then preserves the flag and
    /// focus untouched.
    pub fn ack_permission_reply(
        &mut self,
        ack: crate::native_view_dialogs::DialogAck,
    ) -> Result<crate::native_view_dialogs::AckOutcome, crate::native_view_dialogs::DialogError>
    {
        let attempt = ack.attempt;
        let outcome = self.dialogs.ack(ack)?;
        match outcome {
            crate::native_view_dialogs::AckOutcome::Closed => {
                self.permission_retry_attempt = None;
            }
            crate::native_view_dialogs::AckOutcome::Rejected => {
                self.permission_retry_attempt = Some(attempt);
            }
        }
        Ok(outcome)
    }

    /// True while an open decision awaits an EXPLICIT user retry after a
    /// non-accepted acknowledgement. Never true for an unsubmitted dialog
    /// (nothing was dispatched) and never after a legitimate resolution.
    pub fn permission_retry_open(&self) -> bool {
        self.permission_retry_attempt.is_some()
    }

    /// Clear the retry-await flag. Only reachable after a legitimate
    /// resolution: an empty-`204` ack (`AckOutcome::Closed`) or a fresh
    /// prompt opening a new decision cycle (the flag survives only while
    /// its own dialog stays open — an open dialog blocks every close path
    /// except those explicit resolutions).
    pub fn permission_retry_clear(&mut self) {
        self.permission_retry_attempt = None;
    }

    /// Empty-GET poll resolution: an open dialog closes ONLY when it has
    /// no dispatched reply awaiting explicit retry (an unsubmitted dialog
    /// whose backend request was resolved elsewhere — the legitimate
    /// authenticated-GET resolution). Returns true only when this call
    /// actually closed the dialog; never closes a reopened (rejected-ACK)
    /// decision and no-ops when nothing is open.
    pub fn permission_backend_resolved_close(&mut self) -> bool {
        if self.permission_retry_attempt.is_some() || !self.dialogs.is_open() {
            return false;
        }
        self.dialogs.close();
        true
    }

    // ---------- two-phase session bind (draft transaction) ----------

    /// One transactional commit for a backend-ACKed session bind. Pure (the
    /// caller has already performed the authenticated fetches); all-or-nothing:
    ///
    /// 0. the fresh listing must contain the target as an actual row —
    ///    validated BEFORE the first mutation, so a listing that cannot
    ///    prove membership refuses with zero state change;
    /// 0b. the target must parse as a canonical session id (the same
    ///     contracts parser the authenticated listing fetch uses);
    /// 0c. the accepted fresh listing must fit the row bound — at or below
    ///     it the post-refresh effective rows provably still contain the
    ///     target, so the active acknowledgement can never silently no-op;
    /// 1. a fetch error refuses with **zero** mutation (rows, draft store,
    ///    active marker, status and transcript are untouched);
    /// 2. the outgoing session's unsent global draft is saved under its own
    ///    id in the real bounded `SessionView` store — unknown id / oversize
    ///    / capacity refuses the whole switch instead of silently
    ///    discarding (that check-then-write refuses BEFORE any mutation);
    /// 3. the authenticated fresh listing is registered for the current
    ///    active marker; detached drafts (an outgoing session absent from
    ///    the fresh listing) are RETAINED — a refresh never prunes, only an
    ///    explicit user discard may remove a draft;
    /// 4. the target's stored draft is restored into the returned value —
    ///    the store keeps its copy (never cleared here);
    /// 5. only then do the active marker, status session and transcript
    ///    commit. After a successful draft save NOTHING can fail: the
    ///    refresh, restore and acknowledgement are infallible given the
    ///    preflight proofs (0/0b/0c) — a `debug_assert` documents that
    ///    invariant instead of manufacturing an error after the commit.
    pub fn bind_commit(
        &mut self,
        // (target id, history, fresh listing rows, confirmed persisted
        // message_count of the SAME authenticated snapshot). The count is
        // only ever projected from this accepted tuple — never derived
        // from transcript rows or provider deltas.
        fetched: Result<(String, Vec<String>, Vec<SessionRow>, usize), String>,
        global_draft: &str,
        transcript: &mut Vec<String>,
        append: fn(&mut Vec<String>, String),
    ) -> Result<BindCommit, String> {
        let (target, history, fresh_rows, confirmed_count) =
            fetched.map_err(|error| format!("bind refused, state unchanged: {error}"))?;
        // ---------- preflight: every acknowledgement condition is proved
        // BEFORE the first mutation (the outgoing draft save). Afterwards
        // nothing can fail, so no half-committed state can exist.
        // Step 0: target membership proven by THIS authenticated listing
        // before the old-draft save, row registration or any
        // status/transcript change.
        if !fresh_rows.iter().any(|row| row.id == target) {
            return Err(format!(
                "bind refused, state unchanged: target session {target} missing from fresh listing"
            ));
        }
        // Step 0b: the target must be a canonical session id — the SAME
        // contracts parser `parse_session_listing` and the sessions module
        // commit path already use. Canonical UUIDs are sanitize-stable, so
        // the refresh can never alter or empty this id on the way to the
        // acknowledgement.
        if target.parse::<opencode_rk_contracts::SessionId>().is_err() {
            return Err(format!(
                "bind refused, state unchanged: target session {target} is not a canonical session id"
            ));
        }
        // Step 0c: the accepted listing must fit the row bound. Above it
        // the refresh keeps only its first cap entries, so the target (or
        // every row before it) could be truncated away and the active
        // acknowledgement would silently no-op mid-commit. At or below it
        // the post-refresh effective rows provably still contain the
        // target: canonical ids survive sanitize and dedup keeps at least
        // one occurrence.
        if fresh_rows.len() > MAX_BIND_FRESH_ROWS {
            return Err(format!(
                "bind refused, state unchanged: fresh listing too large ({} rows, bound {MAX_BIND_FRESH_ROWS})",
                fresh_rows.len()
            ));
        }
        // Phase A: save the outgoing draft under the old *known* id using the
        // current listing. Nothing above this line mutates; set_draft itself
        // is check-then-write (all-or-nothing).
        if let Some(old) = self.sessions.active_id().map(str::to_owned) {
            let stored = self.sessions.draft(&old);
            let needs_save =
                stored != Some(global_draft) && !(global_draft.is_empty() && stored.is_none());
            if needs_save {
                self.sessions
                    .set_draft(&old, global_draft)
                    .map_err(|error| format!("refusing switch: {error}"))?;
            }
        }
        // Phase B: register the acknowledged membership from the
        // authenticated listing (preflight 0/0b/0c proved it contains the
        // target and that the refresh keeps it). Infallible. Drafts are
        // NOT pruned here: a detached draft (its session absent from the
        // fresh listing) is retained per the existing user contract — a
        // refresh never silently deletes; only an explicit user discard
        // removes a draft.
        let active = self.sessions.active_id().map(str::to_owned);
        // The SAME accepted authenticated listing that proved membership
        // (preflight 0) carries the bound title: `bind_session` builds the
        // `LiveSnapshot` from this exact payload, so row title and
        // snapshot title are the identical authenticated value. Extract it
        // before the rows are consumed by `set_rows`.
        let bound_title = fresh_rows
            .iter()
            .find(|row| row.id == target)
            .map(|row| row.title.clone());
        self.sessions.set_rows(fresh_rows, active);
        // Phase C/D: restore the target draft (store copy retained), then
        // commit the active marker, status and transcript. All infallible:
        // preflight proved canonical id + bounded-listing membership and
        // `set_rows` cannot drop the target, so the acknowledgement cannot
        // no-op — the debug assertion documents the invariant instead of
        // manufacturing an error after the commit.
        let draft = self.sessions.draft(&target).unwrap_or_default().to_owned();
        self.sessions.acknowledge_active(&target);
        debug_assert_eq!(
            self.sessions.active_id(),
            Some(target.as_str()),
            "preflight proved canonical id + bounded membership; ack cannot no-op"
        );
        // Identity + accepted title + confirmed persisted count commit
        // together: all three come from this one successful bind
        // transaction's authenticated snapshot, so the rendered tuple can
        // never be a stale title or count on a new id (a bind whose
        // listing title is unusable keeps the id and confirmed count with
        // no live claim).
        self.status
            .set_session_binding(Some(target.clone()), bound_title, Some(confirmed_count));
        transcript.clear();
        for line in history {
            append(transcript, line);
        }
        Ok(BindCommit {
            session_id: target,
            draft,
        })
    }

    // ---------- render ----------

    /// Compose one bounded frame: left Sessions, center workspace/composer
    /// (or the open page, or the active center dialog overlay), right
    /// Status + MCP. Returns at most `layout.rows` lines, ready for the
    /// preserved `native_paint` bridge path.
    pub fn render(
        &mut self,
        draft: &str,
        transcript: &[String],
        busy: bool,
        overlay: &[String],
    ) -> Vec<String> {
        let rows = self.layout.rows as usize;
        let height = rows;
        let w_left = self.layout.left_width as usize;
        let w_center = self.layout.center_width as usize;
        let w_right = self.layout.right_width as usize;

        // Left: backend session listing (rows come from GET /api/sessions).
        // Module output is rendered as-is: no synthesized headings or
        // fallback rows that could conceal a part-module defect.
        let mut left = self.sessions.render(w_left, height);
        // Shell chrome (always-true product banner), not module data.
        left.insert(0, BANNER.to_owned());

        // Right: Status + MCP snapshot, exactly as the status module
        // renders it from the applied real state.
        let right = self.status.render(w_right, height);

        // Center: an open permission decision dialog owns focus (rendered
        // from the approved DialogView), else the active native_setup
        // center dialog overlay, else the open page, else the chat
        // workspace.
        let is_chat = matches!(self.pages.page(), Page::Chat);
        let dialog_lines = if self.dialogs.is_open() {
            Some(self.dialogs.render(w_center, height))
        } else {
            None
        };
        let mut center = if let Some(mut dialog_lines) = dialog_lines {
            let mut center: Vec<String> = dialog_lines
                .drain(..)
                .take(height.saturating_sub(1))
                .collect();
            center.push(format!("> {}", first_line(draft)));
            center
        } else if !overlay.is_empty() {
            let mut center: Vec<String> = overlay
                .iter()
                .take(height.saturating_sub(1))
                .cloned()
                .collect();
            center.push(format!("> {}", first_line(draft)));
            center
        } else if is_chat {
            // Global chrome: the product banner owns frame row 0 (painted
            // by the left pane), so the chat center column starts with ONE
            // blank continuation row — the workspace header lands on frame
            // row 1, the transcript viewport on rows 2..=rows-2 and the
            // composer on the last row, all derived from the real layout
            // height (no hardcoded geometry). Only a non-compact shell
            // with at least 3 usable body rows reserves that chrome row;
            // compact or tiny viewports keep whole-frame chat priority
            // (workspace + draft first), per the available-later contract.
            let chat_height = height.saturating_sub(1);
            if !self.layout.compact && chat_height >= 3 {
                let mut center: Vec<String> = vec![String::new()];
                center.extend(self.chat.render(
                    w_center,
                    chat_height,
                    &self.workspace,
                    transcript,
                    draft,
                    busy,
                ));
                center
            } else {
                self.chat
                    .render(w_center, height, &self.workspace, transcript, draft, busy)
            }
        } else {
            self.pages.render(w_center, height)
        };
        center.truncate(height);

        let mut frame = compose_frame(&self.layout, &left, &center, &right);
        frame.truncate(rows);
        frame
    }
}

/// First line only, bounded; the pane width clip happens in `native_paint`.
fn first_line(draft: &str) -> String {
    draft
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(MAX_RENDER_DRAFT_CHARS)
        .collect()
}

/// Honest startup page rows: `enabled` mirrors an actual reachable backend
/// route or an existing local capability; everything else states its exact
/// unsupported reason in `detail`.
pub fn initial_page_rows(backend: bool) -> Vec<PageRow> {
    vec![
        PageRow {
            id: "chat".to_owned(),
            label: "Chat".to_owned(),
            detail: "current transcript".to_owned(),
            enabled: true,
        },
        PageRow {
            id: "status".to_owned(),
            label: "Status".to_owned(),
            detail: "connection / model / session snapshot".to_owned(),
            enabled: true,
        },
        PageRow {
            id: "mcp".to_owned(),
            label: "MCP".to_owned(),
            detail: if backend {
                "GET /api/mcp snapshot".to_owned()
            } else {
                "unavailable: daemon offline".to_owned()
            },
            enabled: backend,
        },
        PageRow {
            id: "models".to_owned(),
            label: "Models".to_owned(),
            detail: if backend {
                "GET /api/models (picker via /models or Ctrl-P)".to_owned()
            } else {
                "unavailable: daemon offline".to_owned()
            },
            enabled: backend,
        },
        PageRow {
            id: "providers".to_owned(),
            label: "Providers".to_owned(),
            detail: "provider setup via /connect".to_owned(),
            enabled: true,
        },
        PageRow {
            id: "workspaces".to_owned(),
            label: "Workspaces".to_owned(),
            detail: if backend {
                "GET /api/workspaces (list only)".to_owned()
            } else {
                "unavailable: daemon offline".to_owned()
            },
            enabled: backend,
        },
        PageRow {
            id: "agents".to_owned(),
            label: "Agents".to_owned(),
            detail: "unavailable: no backend route in this build".to_owned(),
            enabled: false,
        },
        PageRow {
            id: "theme".to_owned(),
            label: "Theme".to_owned(),
            detail: "unavailable: theme page not wired in this build".to_owned(),
            enabled: false,
        },
        PageRow {
            id: "diff".to_owned(),
            label: "Diff".to_owned(),
            detail: "unavailable: no backend route in this build".to_owned(),
            enabled: false,
        },
        PageRow {
            id: "context".to_owned(),
            label: "Context".to_owned(),
            detail: "local breakdown only; no live turn attached".to_owned(),
            enabled: true,
        },
        PageRow {
            id: "commands".to_owned(),
            label: "Commands".to_owned(),
            detail: "local routes (open with /commands or Ctrl-K)".to_owned(),
            enabled: true,
        },
        PageRow {
            id: "help".to_owned(),
            label: "Help".to_owned(),
            detail: "keybinding reference (submit keymap aware)".to_owned(),
            enabled: true,
        },
    ]
}

/// Glue-level tests over the REAL part modules (real `SessionView` bounded
/// draft store, real `OwnedWorker` guard the event loop uses). No mirrored
/// boolean logic: each test drives the exact production entrypoints
/// (`bind_commit`, `OwnedWorker::owns`). The full event-loop effect
/// ("stale events never reach the transcript/busy flag") is enforced in
/// `tui_entry::native_loop` exclusively inside `owns_now` branches; its
/// end-to-end proof belongs to the future installed PTY test, not a fake
/// local simulation.
#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> SessionRow {
        SessionRow {
            id: id.to_owned(),
            title: format!("title {id}"),
        }
    }

    /// Real transcript consumer shape (production passes
    /// `tui_entry::append_transcript` as the same `fn` pointer type).
    fn test_append(transcript: &mut Vec<String>, line: String) {
        transcript.push(line);
    }

    fn view_with_sessions(active: &str, ids: &[&str]) -> NativeView {
        let mut view = NativeView::new(120, 30);
        let rows: Vec<SessionRow> = ids.iter().map(|id| row(id)).collect();
        view.sessions.set_rows(rows, Some(active.to_owned()));
        view.set_session(Some(active.to_owned()));
        view
    }

    #[test]
    fn failed_fetch_transaction_leaves_draft_session_transcript_unchanged() {
        let mut view = view_with_sessions("a", &["a", "b"]);
        view.sessions.set_draft("a", "unsent").unwrap();
        let mut transcript = vec!["old: line".to_owned()];
        let result = view.bind_commit(
            Err("daemon unreachable".to_owned()),
            "unsent",
            &mut transcript,
            test_append,
        );
        assert!(result.is_err());
        assert_eq!(view.sessions.active_id(), Some("a"));
        assert_eq!(view.sessions.draft("a"), Some("unsent"));
        assert_eq!(transcript, vec!["old: line".to_owned()]);
    }

    #[test]
    fn a_to_b_to_a_restores_unsent_draft() {
        // Canonical UUID fixture ids: production listings can only ever
        // contain ids that parsed through the contracts `SessionId`
        // parser, so a SUCCESSFUL bind target is the real validated shape
        // end to end (an un-canonical id is a zero-state refusal, never a
        // partial commit).
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        let mut transcript = vec!["a-old".to_owned()];
        let commit = view
            .bind_commit(
                Ok((
                    B.to_owned(),
                    vec!["b-hist".to_owned()],
                    vec![row(A), row(B)],
                    2,
                )),
                "draft-a",
                &mut transcript,
                test_append,
            )
            .expect("bind a->b");
        assert_eq!(commit.session_id, B);
        assert_eq!(commit.draft, "", "b had no stored draft");
        assert_eq!(view.sessions.draft(A), Some("draft-a"));
        // The first bind must ACTUALLY commit: the real active marker is
        // the target (a silent ACK no-op here would let the next bind
        // save "draft-b" under A's id).
        assert_eq!(view.sessions.active_id(), Some(B), "a->b commits active");
        assert_eq!(transcript, vec!["b-hist".to_owned()]);
        let commit = view
            .bind_commit(
                Ok((
                    A.to_owned(),
                    vec!["a-hist".to_owned()],
                    vec![row(A), row(B)],
                    1,
                )),
                "draft-b",
                &mut transcript,
                test_append,
            )
            .expect("bind b->a");
        assert_eq!(commit.draft, "draft-a", "A->B->A restores the unsent draft");
        assert_eq!(view.sessions.draft(B), Some("draft-b"));
        assert_eq!(
            view.sessions.draft(A),
            Some("draft-a"),
            "restoring never clears the store copy"
        );
        assert_eq!(view.sessions.active_id(), Some(A));
        assert_eq!(transcript, vec!["a-hist".to_owned()]);
    }

    #[test]
    fn draft_store_capacity_refusal_leaves_active_and_transcript_unchanged() {
        // MAX_DRAFTS = 32: fill the store for every id except the active
        // one, so the outgoing save hits real capacity and must refuse
        // instead of silently discarding. Canonical UUID ids keep this a
        // genuine CAPACITY refusal (the step-0b canonical-id guard must
        // never be the reason this test passes).
        let ids: Vec<String> = (0..34)
            .map(|i| format!("00000000-0000-4000-8000-{:012}", i))
            .collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut view = view_with_sessions(&id_refs[0], &id_refs);
        for id in &ids[1..=32] {
            view.sessions.set_draft(id, "stored").unwrap();
        }
        let fresh_rows: Vec<SessionRow> = ids.iter().map(|id| row(id)).collect();
        let mut transcript = vec!["keep".to_owned()];
        let result = view.bind_commit(
            Ok((ids[33].clone(), vec!["hist".to_owned()], fresh_rows, 1)),
            "unsent-in-s0",
            &mut transcript,
            test_append,
        );
        assert!(result.is_err(), "capacity must refuse the whole switch");
        assert_eq!(view.sessions.active_id().as_deref(), Some(id_refs[0]));
        assert_eq!(
            view.sessions.draft(&id_refs[0]),
            None,
            "no partial store write"
        );
        assert_eq!(transcript, vec!["keep".to_owned()]);
    }

    /// Detached-draft retention (existing user contract, now enforced in
    /// the bind glue): an outgoing session omitted from the fresh listing
    /// keeps its saved draft — a refresh NEVER prunes, only an explicit
    /// user discard may. The target still commits (active == B) and the
    /// owned `SessionView` guard is unchanged: the removed session accepts
    /// no NEW draft updates while its retained draft stays readable.
    #[test]
    fn omitted_outgoing_session_keeps_its_draft_and_target_commits() {
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        let mut transcript = vec!["a-old".to_owned()];
        let commit = view
            .bind_commit(
                Ok((
                    B.to_owned(),
                    vec!["b-hist".to_owned()],
                    vec![row(B)], // A is absent from the fresh listing
                    1,
                )),
                "draft-a",
                &mut transcript,
                test_append,
            )
            .expect("bind a->b succeeds with A omitted");
        assert_eq!(commit.session_id, B);
        assert_eq!(view.sessions.active_id(), Some(B), "target commits active");
        assert_eq!(
            view.sessions.draft(A),
            Some("draft-a"),
            "detached draft retained — no automatic prune on refresh"
        );
        // The owned unknown-session guard is unchanged: a session no longer
        // listed accepts NO new draft updates, and its retained draft is
        // never silently altered or deleted.
        assert!(
            view.sessions.set_draft(A, "more").is_err(),
            "removed session refuses new draft updates"
        );
        assert_eq!(view.sessions.draft(A), Some("draft-a"));
    }

    /// Preflight refusals (steps 0b/0c) fire BEFORE the first mutation:
    /// an un-canonical target or a too-large listing leaves every bit of
    /// state — active marker, draft store, transcript — byte-identical,
    /// exactly like the existing failed-fetch and missing-target refusals.
    #[test]
    fn preflight_refusals_leave_every_bit_of_state_unchanged() {
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        view.sessions.set_draft(A, "unsent").unwrap();
        let mut transcript = vec!["keep".to_owned()];
        // Current target invalid: LISTED but not a canonical session id —
        // refused ahead of the first mutation.
        let result = view.bind_commit(
            Ok((
                "not-a-canonical-id".to_owned(),
                vec!["h".to_owned()],
                vec![row(A), row("not-a-canonical-id"), row(B)],
                1,
            )),
            "unsent",
            &mut transcript,
            test_append,
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("un-canonical target must refuse before mutation"),
        };
        assert!(
            error.contains("not a canonical session id"),
            "refusal names the contract: {error}"
        );
        assert_eq!(view.sessions.active_id(), Some(A));
        assert_eq!(view.sessions.draft(A), Some("unsent"));
        assert_eq!(transcript, vec!["keep".to_owned()]);
        // Too large: beyond the accepted row bound the refresh could
        // truncate and silently drop the target — refused ahead of the
        // first mutation.
        let mut rows: Vec<SessionRow> = (0..MAX_BIND_FRESH_ROWS)
            .map(|i| row(&format!("00000000-0000-4000-8000-{i:012}")))
            .collect();
        rows.push(row(B));
        let result = view.bind_commit(
            Ok((B.to_owned(), vec!["h".to_owned()], rows, 1)),
            "unsent",
            &mut transcript,
            test_append,
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("oversize listing must refuse before mutation"),
        };
        assert!(
            error.contains("too large"),
            "refusal names the bound: {error}"
        );
        assert_eq!(view.sessions.active_id(), Some(A));
        assert_eq!(view.sessions.draft(A), Some("unsent"));
        assert_eq!(transcript, vec!["keep".to_owned()]);
    }

    #[test]
    fn stale_worker_event_never_owned_by_current_binding() {
        let owned = OwnedWorker::new((), "session-a", 3);
        assert!(owned.owns(Some("session-a"), 3), "matching binding owns");
        assert!(
            !owned.owns(Some("session-b"), 3),
            "other session never owns"
        );
        assert!(
            !owned.owns(Some("session-a"), 4),
            "later generation never owns"
        );
        assert!(
            !owned.owns(Some("session-a"), 2),
            "older generation never owns"
        );
        assert!(!owned.owns(None, 3), "no active binding owns");
        assert_eq!(owned.session_id(), "session-a");
        assert_eq!(owned.generation(), 3);
    }

    #[test]
    fn listing_missing_target_refuses_zero_state() {
        // The authenticated listing arrived and parsed, but it does NOT
        // contain the fetched target: the transaction must refuse before
        // the old-draft save, row registration, active/status/transcript
        // changes — zero mutation.
        let mut view = view_with_sessions("a", &["a", "b"]);
        view.sessions.set_draft("a", "unsent").unwrap();
        let mut transcript = vec!["old".to_owned()];
        let result = view.bind_commit(
            Ok((
                "c".to_owned(),
                vec!["c-hist".to_owned()],
                vec![row("a"), row("b")],
                1,
            )),
            "unsent",
            &mut transcript,
            test_append,
        );
        // Extract the refusal WITHOUT `expect_err`: that API demands
        // `BindCommit: Debug`, and deriving Debug on the commit would hand
        // every future formatting path a way to expose the unsent `draft`
        // field. The Ok arm panics with the identical message; the refusal
        // contract and every assertion below are byte-for-byte unchanged.
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("target missing from listing must refuse"),
        };
        assert!(
            error.contains("missing from fresh listing"),
            "refusal names the contract: {error}"
        );
        assert_eq!(view.sessions.active_id(), Some("a"));
        assert_eq!(view.sessions.draft("a"), Some("unsent"));
        assert_eq!(transcript, vec!["old".to_owned()]);
    }

    #[test]
    fn generation_max_reserve_fails_closed_no_new_worker_identity() {
        assert_eq!(reserve_generation(0), Some(1));
        assert_eq!(reserve_generation(u64::MAX - 1), Some(u64::MAX));
        // Exhaustion fails closed. Production (Ctrl-G and /new) reserves
        // BEFORE any fetch/POST, so no bind can commit, no generation is
        // reused, and no worker identity can be minted from the reservation.
        let reserved = reserve_generation(u64::MAX);
        assert!(reserved.is_none(), "u64::MAX must never wrap to 0");
        assert!(
            reserved
                .map(|generation| OwnedWorker::new((), "session-a", generation))
                .is_none(),
            "a failed reservation must never construct a new worker identity"
        );
    }

    #[test]
    fn modal_priority_render_and_dispatch_precede_setup_overlay() {
        let mut view = view_with_sessions("a", &["a"]);
        view.dialogs
            .open_permission("req-1", "session-a", "run shell: rm x")
            .expect("open pending decision");
        // The exact predicates the input loop dispatches on: the modal
        // owns input (paste/printables/pages intercepted) while open.
        assert!(view.dialogs.intercepts_input());
        assert!(view.dialogs.is_open());
        // The exact render path: the modal's real title is visible even
        // when a native_setup credential overlay is handed to the facade —
        // the API-key panel must never show behind a permission decision.
        let frame = view.render("", &[], false, &["API KEY BEHIND".to_owned()]);
        let joined = frame.join("\n");
        assert!(
            joined.contains("run shell: rm x"),
            "modal title must be visible: {joined}"
        );
        assert!(
            !joined.contains("API KEY BEHIND"),
            "setup overlay must be hidden behind the modal: {joined}"
        );
    }

    #[test]
    fn page_confirm_preserves_composer_draft() {
        let mut view = view_with_sessions("a", &["a"]);
        view.set_page_rows(initial_page_rows(true));
        view.open_page(Page::Status);
        let mut draft = "unsent chat draft".to_owned();
        // Production Enter-in-page path: applies real page state, never
        // touches the composer draft (backend/row outcomes included).
        view.confirm_page(&mut draft);
        assert_eq!(
            draft, "unsent chat draft",
            "page confirmation must preserve the composer draft"
        );
    }

    /// Installed-frame contract (PTY receipt
    /// `native-ui-installed-navigation-v2/receipts/shell-pty-result.json`
    /// rows 97-126: workspace header AND composer both rode frame row 0/1
    /// with an empty transcript below the composer): the product banner
    /// owns frame row 0, the chat workspace header — the ACTUAL caller
    /// workspace path from `set_workspace`, never a fabricated label —
    /// sits directly below on row 1, the empty transcript viewport
    /// occupies rows 2..=28 as blank center cells (no synthesized
    /// content), and the composer prompt lands on the LAST frame row.
    /// Compact/tiny frames keep whole-frame chat priority (workspace +
    /// draft visible) with no reserved chrome row.
    #[test]
    fn full_frame_chat_keeps_chrome_header_transcript_and_composer_rows() {
        let mut view = NativeView::new(120, 30);
        view.set_workspace("/private/tmp/real-project".to_owned());
        let lw = usize::from(view.layout.left_width);
        let cw = usize::from(view.layout.center_width);
        let frame = view.render("", &[], false, &[]);
        assert_eq!(frame.len(), 30, "full frame height");
        // Center column of one composed row: left segment + border, then
        // `center_width` cells (ASCII test content: chars == display cells).
        let center = |i: usize| -> String { frame[i].chars().skip(lw + 1).take(cw).collect() };
        // Row 0: global chrome — banner in the left pane, center blank.
        assert!(
            frame[0].starts_with(BANNER),
            "banner owns frame row 0: {frame0:?}",
            frame0 = frame[0]
        );
        assert!(
            !center(0).contains("real-project"),
            "workspace label must not ride the chrome row: {:?}",
            center(0)
        );
        // Row 1: workspace header shows the real caller path.
        assert!(
            center(1).contains("/private/tmp/real-project"),
            "workspace header on frame row 1: {:?}",
            center(1)
        );
        // Rows 2..=28: empty transcript viewport — blank center cells,
        // no fabricated filler, no prompt creeping up under the header.
        for i in 2..29 {
            assert_eq!(
                center(i).trim(),
                "",
                "empty transcript viewport on row {i}: {:?}",
                center(i)
            );
        }
        // Row 29: composer prompt on the last frame row.
        assert!(
            center(29).starts_with("> "),
            "composer on the last frame row: {:?}",
            center(29)
        );

        // Tiny frame (2 rows): whole-frame chat priority — workspace
        // header and draft composer stay visible, no chrome row steals
        // the last usable row (available-later contract).
        let mut tiny = NativeView::new(120, 2);
        tiny.set_workspace("/ws".to_owned());
        let frame = tiny.render("draft here", &[], false, &[]);
        assert_eq!(frame.len(), 2);
        assert!(
            frame[0].contains("/ws"),
            "tiny frame keeps the workspace header: {frame0:?}",
            frame0 = frame[0]
        );
        assert!(
            frame[1].contains("> draft here"),
            "tiny frame keeps the draft composer: {frame1:?}",
            frame1 = frame[1]
        );

        // Height 1: composer priority wins outright (compact viewport).
        let mut one = NativeView::new(120, 1);
        one.set_workspace("/ws".to_owned());
        let frame = one.render("draft", &[], false, &[]);
        assert_eq!(frame.len(), 1);
        assert!(
            frame[0].starts_with("> "),
            "height-1 keeps composer priority: {frame0:?}",
            frame0 = frame[0]
        );
    }

    #[test]
    fn sessions_page_request_acks_only_through_real_verdict() {
        let mut view = NativeView::new(120, 30);
        // Production glue order: `/sessions` opens the real pages-v5
        // `Page::Sessions` FIRST, then the authenticated listing loads as
        // its rows (rows bind to the page current at load time). Mapping:
        // id = backend UUID, label = title, detail = active marker.
        view.open_page(Page::Sessions);
        view.set_page_rows(vec![
            PageRow {
                id: "sess-a".to_owned(),
                label: "A".to_owned(),
                detail: "active".to_owned(),
                enabled: true,
            },
            PageRow {
                id: "sess-b".to_owned(),
                label: "B".to_owned(),
                detail: "".to_owned(),
                enabled: true,
            },
        ]);
        // Enter intent is a real pending Request for the SELECTED row (the
        // module clamps selection onto the first loaded row) — never a
        // fake shortcut and never an ACK.
        let action = view.activate_open_page();
        assert!(
            matches!(
            action,
            crate::native_view_pages::PageAction::Request {
                page: Page::Sessions,
                ref id,
            } if id == "sess-a"
            ),
            "expected pending Request for the selected row, got {action:?}"
        );
        // Refusal verdict (busy turn / bind error): no applied change is
        // returned as a Request, rows and selection are retained, and the
        // bounded reason becomes the page's visible error.
        let applied = view.resolve_page_ack(crate::native_view_pages::Ack::Rejected(
            "busy: turn in progress".to_owned(),
        ));
        assert!(matches!(
            applied,
            Some(crate::native_view_pages::PageAction::Unsupported {
                page: Page::Sessions,
                ..
            })
        ));
        assert_eq!(
            view.page_selected().map(|row| row.id.as_str()),
            Some("sess-a"),
            "rejection must retain rows and selection"
        );
        // A REAL bind success is the only verdict that returns the Request
        // for the caller to apply (glue commits the bind first, then ACKs
        // and reloads rows).
        let action = view.activate_open_page();
        assert!(
            matches!(
                action,
                crate::native_view_pages::PageAction::Request {
                    page: Page::Sessions,
                    ..
                }
            ),
            "got {action:?}"
        );
        let applied = view.resolve_page_ack(crate::native_view_pages::Ack::Ok);
        assert!(matches!(
            applied,
            Some(crate::native_view_pages::PageAction::Request {
                page: Page::Sessions,
                ..
            })
        ));
    }

    /// Ctrl-K command input: an independent bounded buffer that filters
    /// the Commands page while an unsent Unicode composer draft and the
    /// per-session draft store stay byte-identical — through open, typing,
    /// confirm, dispatch and cancel (Esc = `close_page` in production).
    #[test]
    fn command_input_keeps_unsent_unicode_draft_and_store() {
        let mut view = view_with_sessions("a", &["a", "b"]);
        let stored = "unsent ünïcode 🎯 draft";
        view.sessions.set_draft("a", stored).unwrap();
        let mut draft = format!("{stored} — not yet sent");
        let before = draft.clone();
        // Production Ctrl-K path: single open with static route rows.
        view.open_commands_page();
        assert_eq!(view.page(), Page::Commands);
        // Typed command buffer: Unicode round-trips, bounded push.
        for c in "/stätus".chars() {
            assert!(view.page_query_push(c));
        }
        assert_eq!(view.page_query(), "/stätus");
        assert!(view.page_query_backspace());
        assert_eq!(view.page_query(), "/stätu");
        // Confirm never mutates the composer (production `confirm_page`).
        view.confirm_page(&mut draft);
        assert_eq!(draft, before, "confirm must not touch the draft");
        // Clear the unicode text, type an exact recognized route, and
        // dispatch through the real production helper.
        while view.page_query_backspace() {}
        assert_eq!(view.page_query(), "");
        for c in "/status".chars() {
            assert!(view.page_query_push(c));
        }
        assert_eq!(view.dispatch_command(), Some("/status"));
        // Cancel (production Esc = `close_page`): query buffer leaves with
        // the page, draft and store are byte-identical.
        view.close_page();
        assert_eq!(draft, before, "cancel must restore/keep the exact draft");
        assert_eq!(view.sessions.draft("a"), Some(stored));
        assert_eq!(view.sessions.active_id(), Some("a"));
        // Bound: the 257th character is refused, buffer intact.
        view.open_commands_page();
        for _ in 0..MAX_PAGE_QUERY_CHARS {
            assert!(view.page_query_push('a'));
        }
        assert!(!view.page_query_push('a'));
        assert_eq!(view.page_query().chars().count(), MAX_PAGE_QUERY_CHARS);
    }

    /// Sessions search: a real UUID-shaped row list is filtered by the
    /// facade-owned query (selecting across filtered rows) while the chat
    /// composer draft and the per-session draft store are untouched.
    #[test]
    fn sessions_query_filters_uuid_rows_without_touching_drafts() {
        let uuid_a = "11111111-1111-1111-1111-111111111111";
        let uuid_b = "22222222-2222-2222-2222-222222222222";
        let mut view = view_with_sessions(uuid_a, &[uuid_a, uuid_b]);
        view.sessions.set_draft(uuid_a, "stored ünïcode").unwrap();
        view.open_page(Page::Sessions);
        view.set_page_rows(vec![
            PageRow {
                id: uuid_a.to_owned(),
                label: "Alpha".to_owned(),
                detail: String::new(),
                enabled: true,
            },
            PageRow {
                id: uuid_b.to_owned(),
                label: "Beta".to_owned(),
                detail: "active".to_owned(),
                enabled: true,
            },
        ]);
        let mut draft = "unsent chat draft".to_owned();
        for c in "beta".chars() {
            assert!(view.page_query_push(c));
        }
        assert_eq!(view.page_query(), "beta");
        // The filter is real: only the Beta row remains visible/selected.
        assert_eq!(
            view.page_selected().map(|row| row.label.as_str()),
            Some("Beta")
        );
        // Selection moves across the FILTERED rows (clamped to one row).
        view.page_next();
        assert_eq!(
            view.page_selected().map(|row| row.id.as_str()),
            Some(uuid_b)
        );
        view.page_prev();
        assert_eq!(
            view.page_selected().map(|row| row.id.as_str()),
            Some(uuid_b)
        );
        // Chat draft and per-session store are untouched by the query.
        assert_eq!(draft, "unsent chat draft");
        assert_eq!(view.sessions.draft(uuid_a), Some("stored ünïcode"));
        assert_eq!(view.sessions.active_id(), Some(uuid_a));
    }

    /// Permission broker ACK contract (the exact production classifier):
    /// ONLY an empty `204 No Content` accepts; `202`, `200`, `201`, `404`
    /// and a `204` carrying a body all refuse (fail closed).
    #[cfg(feature = "native")]
    #[test]
    fn permission_reply_accepts_only_empty_204() {
        use crate::tui_entry::permission_reply_accepted;
        assert!(permission_reply_accepted(204, ""));
        assert!(!permission_reply_accepted(202, ""));
        assert!(!permission_reply_accepted(200, ""));
        assert!(!permission_reply_accepted(201, ""));
        assert!(!permission_reply_accepted(404, ""));
        assert!(!permission_reply_accepted(204, "{}"));
        assert!(!permission_reply_accepted(500, ""));
    }

    /// `/new` ACK extraction (the exact production helper): only the
    /// nested `session.id` with a valid UUID is accepted; the old
    /// top-level `id` shape, a missing id, a non-UUID value and broken
    /// JSON are all static refusals — never a bind, never an invented id.
    #[cfg(feature = "native")]
    #[test]
    fn new_ack_extracts_only_nested_valid_session_id() {
        use crate::tui_entry::parse_created_session_id;
        let uuid = "11111111-1111-1111-1111-111111111111";
        let nested = format!(r#"{{"session":{{"id":"{uuid}"}},"title":"Chat"}}"#);
        assert_eq!(parse_created_session_id(&nested).as_deref(), Ok(uuid));
        // The old top-level shape must be refused even with a valid UUID.
        let top_level = format!(r#"{{"id":"{uuid}"}}"#);
        assert!(parse_created_session_id(&top_level).is_err());
        assert!(parse_created_session_id(r#"{"session":{}}"#).is_err());
        assert!(parse_created_session_id(r#"{"session":{"id":"not-a-uuid"}}"#).is_err());
        assert!(parse_created_session_id("{broken").is_err());
    }

    /// Failed-ACK empty-poll loophole: a REJECTED acknowledgement
    /// (`accepted: false` — the module reopens the choice as
    /// `AckOutcome::Rejected`) leaves the dialog OPEN, answerable and
    /// focus-owning, and the empty-GET poll must NEVER close it — not on
    /// the first None, not on any repeated None. The flag keys on the
    /// ACK outcome, never on the intent kind: this fixture dispatched an
    /// APPROVE intent whose reply was not accepted (transport failure or
    /// non-204 status — the exact production classification), and it must
    /// still await an explicit retry. Stale ACKs preserve flag and focus.
    /// No backend/network: the test drives the exact facade helpers the
    /// production dispatch and poll call.
    #[test]
    fn rejected_ack_keeps_dialog_open_through_empty_polls() {
        use crate::native_view_dialogs::{AckOutcome, DecisionIntent, DialogAck};
        let mut view = NativeView::new(120, 30);
        view.dialogs
            .open_permission("req-1", "session-1", "run shell: rm x")
            .expect("open pending decision");
        // Production Enter path: explicit selection, then the answer intent
        // carries the correlated attempt.
        view.dialogs.next();
        let intent = view.dialogs.answer().expect("answerable choice");
        assert!(
            matches!(intent, DecisionIntent::Approve { .. }),
            "approve intent: the ACK outcome, not the kind, drives the retry flag"
        );
        let attempt = intent.attempt();
        let outcome = view
            .ack_permission_reply(DialogAck {
                request_id: "req-1".to_owned(),
                session_id: "session-1".to_owned(),
                accepted: false,
                attempt,
            })
            .expect("matching ack");
        assert_eq!(
            outcome,
            AckOutcome::Rejected,
            "matching non-accepted ACK reopens, never closes"
        );
        assert!(view.permission_retry_open(), "retry awaits explicit input");
        // Empty authenticated GET, repeatedly: the poll never swallows the
        // decision — identical state every time.
        for _ in 0..2 {
            assert!(
                !view.permission_backend_resolved_close(),
                "rejected-ACK dialog is never backend-closed"
            );
            assert!(view.dialogs.is_open(), "decision stays open");
            assert!(!view.dialogs.pending(), "no in-flight ack expected");
            assert!(view.dialogs.intercepts_input(), "modal keeps focus");
        }
        // Still answerable: a fresh explicit selection produces the NEXT
        // attempt (production Esc/Enter retry — explicit user input only).
        view.dialogs.next();
        let retry = view
            .dialogs
            .answer()
            .expect("reopened choice is answerable");
        assert!(retry.attempt() > attempt, "retry carries a higher attempt");
        // A stale/mismatched ACK is refused by the module with NO state
        // change: flag and focus preserved.
        let stale = view.ack_permission_reply(DialogAck {
            request_id: "req-1".to_owned(),
            session_id: "session-1".to_owned(),
            accepted: false,
            attempt: retry.attempt() + 99,
        });
        assert!(stale.is_err(), "stale attempt must be refused");
        assert!(view.permission_retry_open(), "stale ack preserves the flag");
        assert!(view.dialogs.is_open());
    }

    /// Successful deny (empty-204 ACK accepted) is a LEGITIMATE close: the
    /// explicit Reject intent is answered, the dialog closes, the retry
    /// flag is cleared, and a later empty GET keeps it closed — never a
    /// reopen, never a retry.
    #[test]
    fn accepted_204_ack_closes_and_empty_poll_stays_closed() {
        use crate::native_view_dialogs::{AckOutcome, DecisionIntent, DialogAck};
        let mut view = NativeView::new(120, 30);
        view.dialogs
            .open_permission("req-2", "session-2", "write /tmp/x")
            .expect("open pending decision");
        // Explicit deny: select index 1 ("deny") → DecisionIntent::Reject.
        view.dialogs.next();
        view.dialogs.next();
        let intent = view.dialogs.answer().expect("explicit deny choice");
        assert!(matches!(intent, DecisionIntent::Reject { .. }));
        let attempt = intent.attempt();
        let outcome = view
            .ack_permission_reply(DialogAck {
                request_id: "req-2".to_owned(),
                session_id: "session-2".to_owned(),
                accepted: true,
                attempt,
            })
            .expect("matching ack");
        assert_eq!(
            outcome,
            AckOutcome::Closed,
            "empty-204 deny legitimately closes"
        );
        assert!(!view.permission_retry_open(), "no retry after a real close");
        assert!(!view.dialogs.is_open());
        // Empty GET stays closed: no resurrection, no reopen.
        assert!(!view.permission_backend_resolved_close());
        assert!(!view.dialogs.is_open());
        assert!(!view.dialogs.intercepts_input());
    }

    /// Unsubmitted open dialog (nothing dispatched — the backend request
    /// was resolved elsewhere) MAY close on the authenticated empty GET:
    /// that is the legitimate backend resolution, and it is the ONLY open
    /// dialog the poll is allowed to close.
    #[test]
    fn unsubmitted_open_dialog_closes_as_backend_resolution() {
        let mut view = NativeView::new(120, 30);
        view.dialogs
            .open_permission("req-3", "session-3", "net connect")
            .expect("open pending decision");
        assert!(!view.permission_retry_open(), "nothing dispatched yet");
        assert!(
            view.permission_backend_resolved_close(),
            "legitimate backend resolution closes the unsubmitted dialog"
        );
        assert!(!view.dialogs.is_open());
        assert!(
            !view.permission_backend_resolved_close(),
            "a closed dialog is a no-op for the poll helper"
        );
    }

    /// RECOVERY-DURABILITY-001: a REFUSED bind (failed fetch) keeps the
    /// previously accepted title/id pair, the drafts and the transcript
    /// atomically — status projection assertions added additively; the
    /// existing draft/state assertions are unchanged.
    #[test]
    fn refused_bind_keeps_bound_title_id_and_draft() {
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        view.sessions.set_draft(B, "b-unsent").unwrap();
        view.set_session_binding(Some(A.to_owned()), Some("Alpha".to_owned()), Some(4));
        let mut transcript = vec!["old".to_owned()];
        let result = view.bind_commit(
            Err("daemon unreachable".to_owned()),
            "a-draft",
            &mut transcript,
            test_append,
        );
        assert!(result.is_err());
        let frame = view.render("a-draft", &transcript, false, &[]);
        assert!(
            frame.iter().any(|r| r.contains("session: Alpha (live)")),
            "bound title survives a refused bind"
        );
        assert!(
            frame.iter().any(|r| r.contains("messages: 4")),
            "confirmed snapshot count survives a refused bind"
        );
        // Right pane is 39 cols at 120: `session id: ` (12 cols) + the
        // first 27 chars of the canonical UUID are the exact visible
        // prefix — clipping can only truncate, never regroup. Full
        // identity stays asserted at state level below (active_id).
        assert!(
            frame
                .iter()
                .any(|r| r.contains("session id: aaaaaaaa-aaaa-4aaa-8aaa-aaa")),
            "bound id survives a refused bind"
        );
        assert_eq!(view.sessions.active_id(), Some(A));
        assert_eq!(view.sessions.draft(B), Some("b-unsent"));
        assert_eq!(transcript, vec!["old".to_owned()]);
        // Draft-capacity refusal (32 KiB + 1) likewise mutates nothing.
        let mut view2 = view_with_sessions(A, &[A, B]);
        view2.sessions.set_draft(B, "b-unsent").unwrap();
        view2.set_session_binding(Some(A.to_owned()), Some("Alpha".to_owned()), Some(4));
        let mut transcript2 = vec!["old".to_owned()];
        let oversized = "x".repeat(32 * 1024 + 1);
        let result2 = view2.bind_commit(
            Ok((B.to_owned(), vec![], vec![row(A), row(B)], 7)),
            &oversized,
            &mut transcript2,
            test_append,
        );
        assert!(result2.is_err());
        let frame2 = view2.render("a-draft", &transcript2, false, &[]);
        assert!(
            frame2.iter().any(|r| r.contains("session: Alpha (live)")),
            "capacity refusal keeps the bound pair"
        );
        assert!(
            frame2.iter().any(|r| r.contains("messages: 4")),
            "capacity refusal keeps the confirmed count (the refused 7 is never applied)"
        );
        assert!(
            !frame2.iter().any(|r| r.contains("messages: 7")),
            "the refused snapshot count never reaches the projection"
        );
        assert_eq!(view2.sessions.active_id(), Some(A));
        assert_eq!(view2.sessions.draft(B), Some("b-unsent"));
        assert_eq!(transcript2, vec!["old".to_owned()]);
    }

    /// RECOVERY-DURABILITY-001: a successful bind commits id, title and
    /// the confirmed persisted message count TOGETHER from the one
    /// accepted snapshot tuple (the same payload the `LiveSnapshot`
    /// title/count come from) — never a stale title or count on the new
    /// id, never a bare id left bound from before the switch.
    #[test]
    fn successful_bind_commits_id_and_title_together() {
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        view.set_session_binding(Some(A.to_owned()), Some("Alpha".to_owned()), Some(4));
        let mut transcript = vec!["old".to_owned()];
        let fresh = vec![
            SessionRow {
                id: A.to_owned(),
                title: "Alpha".to_owned(),
            },
            SessionRow {
                id: B.to_owned(),
                title: "Beta".to_owned(),
            },
        ];
        let commit = view
            .bind_commit(
                Ok((B.to_owned(), vec!["h".to_owned()], fresh, 3)),
                "draft-a",
                &mut transcript,
                test_append,
            )
            .expect("bind commits");
        assert_eq!(commit.session_id, B);
        let frame = view.render("draft-a", &transcript, false, &[]);
        // `fresh` is the accepted authenticated listing — the exact payload
        // the `LiveSnapshot` title comes from — so id and title commit
        // together in this one transaction.
        assert!(
            frame.iter().any(|r| r.contains("session: Beta (live)")),
            "new bound title rendered: {frame:?}"
        );
        assert!(
            frame.iter().any(|r| r.contains("messages: 3")),
            "confirmed snapshot count rendered with the new tuple"
        );
        // Exact visible prefix of the new canonical id: 39-col right pane
        // keeps `session id: ` + the first 27 UUID chars (state-level full
        // identity is `commit.session_id` above).
        assert!(
            frame
                .iter()
                .any(|r| r.contains("session id: bbbbbbbb-bbbb-4bbb-8bbb-bbb")),
            "new identity rendered"
        );
        // No stale pair: the old bound status row is gone (the left pane's
        // row list legitimately shows the fresh titles, but the status
        // projection can never keep the old title next to the new id).
        assert!(
            !frame.contains(&"session: Alpha (live)".to_owned()),
            "no stale title on the new id"
        );
        // Full-shell rows begin with the left pane, so the single identity
        // row is counted by membership: exactly one frame row carries the
        // `session id:` segment (no duplicate identity row).
        assert_eq!(
            frame.iter().filter(|r| r.contains("session id: ")).count(),
            1
        );
        assert_eq!(transcript, vec!["h".to_owned()]);
    }

    /// RECOVERY-DURABILITY-001 (v4): a successful bind replaces the FULL
    /// accepted tuple — id, title and confirmed persisted count — in one
    /// atomic commit. A confirmed count of 0 (valid empty history) is a
    /// real accepted fact and renders truthfully; the previous count and
    /// title never survive the switch.
    #[test]
    fn successful_switch_replaces_id_title_and_count_together() {
        const A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        const B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let mut view = view_with_sessions(A, &[A, B]);
        view.set_session_binding(Some(A.to_owned()), Some("Alpha".to_owned()), Some(4));
        let mut transcript = vec!["old".to_owned()];
        let commit = view
            .bind_commit(
                Ok((
                    B.to_owned(),
                    vec![],
                    vec![
                        SessionRow {
                            id: A.to_owned(),
                            title: "Alpha".to_owned(),
                        },
                        SessionRow {
                            id: B.to_owned(),
                            title: "Beta".to_owned(),
                        },
                    ],
                    0,
                )),
                "draft-a",
                &mut transcript,
                test_append,
            )
            .expect("bind commits");
        assert_eq!(commit.session_id, B);
        let frame = view.render("draft-a", &transcript, false, &[]);
        assert!(frame.iter().any(|r| r.contains("session: Beta (live)")));
        assert!(
            frame
                .iter()
                .any(|r| r.contains("session id: bbbbbbbb-bbbb-4bbb-8bbb-bbb")),
            "new identity rendered"
        );
        assert!(
            frame.iter().any(|r| r.contains("messages: 0")),
            "confirmed empty-history count 0 is a valid accepted fact"
        );
        assert!(
            !frame.iter().any(|r| r.contains("messages: 4")),
            "no stale count on the new id"
        );
        assert!(
            !frame.iter().any(|r| r.contains("session: Alpha (live)")),
            "no stale title on the new id"
        );
    }
}
