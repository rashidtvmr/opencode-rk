#![forbid(unsafe_code)]
//! Route-aware native Home, conversation, sidebar and modal presentation.
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
use crate::native_tabs::TabReservation;
use crate::native_view_dialogs::DialogView;
use crate::native_view_layout::{compose_frame, fit, Layout};
use crate::native_view_pages::{Page, PageRow, PageState};
use crate::native_view_sessions::{SessionRow, SessionView};
use crate::native_view_status::StatusView;
use serde_json::Value;

/// Draft bytes carried into one render (paint sanitizes control chars and
/// clips to pane width; this bound just keeps allocation fixed-size).
const MAX_RENDER_DRAFT_CHARS: usize = 4096;
const MAX_FRAME_CELLS: usize = 1 << 20;

/// Pinned Home prompt max-width configuration (home.tsx:33-36).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PromptWidth {
    #[default]
    Default,
    Auto,
    Fixed(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogSize {
    #[default]
    Medium,
    Large,
    XLarge,
}

impl DialogSize {
    fn width(self) -> u16 {
        match self {
            Self::Medium => 60,
            Self::Large => 88,
            Self::XLarge => 116,
        }
    }
}

/// Shared modal geometry; panel position and inner padding are distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalPresentation {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub content_x: u16,
    pub content_y: u16,
    pub content_width: u16,
    pub bottom_padding: u16,
    pub backdrop_alpha: u8,
}

impl ModalPresentation {
    pub fn content_point(self, x: u16, y: u16, height: usize) -> Option<(u16, usize)> {
        let column = x.checked_sub(self.content_x)?;
        let row = usize::from(y.checked_sub(self.content_y)?);
        (column < self.content_width && row < height).then_some((column, row))
    }
}

pub struct NativeView {
    layout: Layout,
    sessions: SessionView,
    chat: ChatView,
    pages: PageState,
    memory: crate::native_memory::Memory,
    compaction: crate::native_compaction_settings::CompactionSettingsView,
    context: crate::native_context::ContextView,
    command_palette: crate::native_command_palette::CommandPalette,
    status: StatusView,
    /// Held from day one; `DialogView::request/answer` wiring is explicitly
    /// pending until the dialogs part delivers its request/answer API
    /// (inventing calls here would not compile). The live `/connect`,
    /// `/models` and `/connect` credential dialogs remain the existing
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
    /// Session route fact: true when the main column shows the active
    /// session (transcript + composer); false = home (no active
    /// conversation). Set explicitly by the input loop from the real
    /// transcript/backend state; the view never guesses it from
    /// startup memory/error text.
    session_route: bool,
    /// Sidebar hidden by explicit user toggle (`session.sidebar.toggle`).
    sidebar_hidden: bool,
    sidebar_open: bool,
    /// Active session is a child (`parentID` present): upstream suppresses
    /// the sidebar entirely.
    parent_session: bool,
    prompt_width: PromptWidth,
    shell_mode: bool,
    placeholder_index: usize,
    dialog_size: DialogSize,
    composer_agent: Option<String>,
    send_pointer: Option<(u16, u16)>,
    /// Bounds from the same frame that paints the Context Usage footer label.
    context_usage_hit: Option<(u16, u16, u16)>,
    composer_variant: Option<String>,
    composer_model: Option<(String, String)>,
    composer_effort: Option<String>,
    chat_row_classes: Vec<crate::native_view_chat::ChatRowClass>,
    chat_text_bounds: Vec<Option<(u16, u16)>>,
    /// Persisted global + per-box appearance; the framed composer reads the
    /// `BoxId::ChatComposer` effective values (default = upstream sharp).
    box_appearance: crate::native_box_style::BoxAppearance,
    context_window: Option<u64>,
    ui_regions: Vec<crate::native_ui_style::Region>,
    modal_underlay_rows: Vec<String>,
    overlay_roles: Vec<crate::native_ui_style::Role>,
    /// Busy state of the most recently rendered frame.  Native painting uses
    /// `composer_region` for its optional center loader overlay; while a turn
    /// is busy the footer is the sole loader surface, so that overlay must be
    /// suppressed without changing the composer rows themselves.
    frame_busy: bool,
    mcp_hit: Vec<(String, bool, u16, u16, u16)>,
    sidebar_content_height: usize,
    /// Frame reservation owned by the live session-tab shell.
    tab_reservation: TabReservation,
}

#[cfg(test)]
mod page_spacing_tests {
    use super::*;
    #[test]
    fn sidebar_click_authority_is_retracted_before_hidden_or_rebound_frames() {
        let mut view = NativeView::new(148, 30);
        view.set_session_route(true);
        view.apply_mcp(&serde_json::json!({
            "status": {"alpha": {"status": "connected"}},
            "included": {"alpha": true}
        }))
        .unwrap();
        let frame = view.render("draft", &[], false, &[]);
        let y = frame
            .iter()
            .position(|line| line.contains("• alpha"))
            .unwrap() as u16;
        let x = view.presentation().sidebar_offset.unwrap() + 3;
        assert_eq!(view.mcp_sidebar_action(x, y), Some(("alpha".into(), true)));
        view.set_sidebar_hidden(true);
        assert!(
            view.mcp_sidebar_action(x, y).is_none(),
            "a hidden pane must not retain clickable rows"
        );
        view.set_sidebar_hidden(false);
        view.set_session_binding(
            Some(opencode_rk_contracts::SessionId::new().to_string()),
            Some("New session".into()),
            Some(0),
        );
        assert!(
            view.mcp_sidebar_action(x, y).is_none(),
            "old-session inclusion cannot authorize a new-session click"
        );
    }

    #[test]
    fn sidebar_scrolling_keeps_the_last_server_visible_and_clickable() {
        for (cols, framed) in [(148, false), (100, false), (148, true)] {
            let mut view = NativeView::new(cols, 30);
            view.set_session_route(true);
            if view.presentation().sidebar_offset.is_none() {
                view.toggle_sidebar();
            }
            if framed {
                view.set_box_appearance(crate::native_box_style::BoxAppearance {
                    preset: crate::native_box_style::RadiusPreset::IosSquircle,
                    ..Default::default()
                });
            }
            let status: serde_json::Map<String, Value> = (0..64)
                .map(|index| {
                    (
                        format!("server-{index:02}"),
                        serde_json::json!({"status":"connected"}),
                    )
                })
                .collect();
            let included: serde_json::Map<String, Value> = status
                .keys()
                .map(|name| (name.clone(), serde_json::json!(false)))
                .collect();
            view.apply_mcp(&serde_json::json!({"status":status,"included":included}))
                .unwrap();
            view.render("unchanged draft", &[], false, &[]);
            let x = view.presentation().sidebar_offset.unwrap() + 3;
            assert!(
                !view.scroll_sidebar_at(x, 29, isize::MAX),
                "footer is not sidebar content"
            );
            assert!(view.scroll_sidebar_at(x, 2, isize::MAX));
            let frame = view.render("unchanged draft", &[], false, &[]);
            let y = frame
                .iter()
                .position(|line| line.contains("server-63"))
                .expect("last server must remain reachable") as u16;
            assert!(y < 29);
            assert_eq!(
                view.mcp_sidebar_action(x, y),
                Some(("server-63".into(), false))
            );
        }
    }

    #[test]
    fn accepted_projection_invalidates_old_sidebar_coordinates_until_repaint() {
        let mut view = NativeView::new(148, 30);
        view.set_session_route(true);
        view.apply_mcp(
            &serde_json::json!({"status":{"beta":{"status":"connected"}},"included":{"beta":true}}),
        )
        .unwrap();
        let frame = view.render("draft", &[], false, &[]);
        let y = frame
            .iter()
            .position(|line| line.contains("• beta"))
            .unwrap() as u16;
        let x = view.presentation().sidebar_offset.unwrap() + 3;
        assert_eq!(view.mcp_sidebar_action(x, y), Some(("beta".into(), true)));
        view.apply_mcp(&serde_json::json!({
            "status":{"alpha":{"status":"connected"},"beta":{"status":"connected"}},
            "included":{"alpha":true,"beta":true}
        }))
        .unwrap();
        assert!(
            view.mcp_sidebar_action(x, y).is_none(),
            "a new projection cannot authorize stale coordinates"
        );
        view.render("draft", &[], false, &[]);
        assert_eq!(view.mcp_sidebar_action(x, y), Some(("alpha".into(), true)));
    }

    #[test]
    fn hiding_and_reshowing_sidebar_requires_fresh_painted_hit_geometry() {
        let mut view = NativeView::new(148, 30);
        view.set_session_route(true);
        view.apply_mcp(&serde_json::json!({"status":{"alpha":{"status":"connected"}},"included":{"alpha":true}})).unwrap();
        let frame = view.render("draft", &[], false, &[]);
        let y = frame
            .iter()
            .position(|line| line.contains("• alpha"))
            .unwrap() as u16;
        let x = view.presentation().sidebar_offset.unwrap() + 3;
        assert!(view.mcp_sidebar_action(x, y).is_some());
        view.set_sidebar_hidden(true);
        view.set_sidebar_hidden(false);
        assert!(
            view.mcp_sidebar_action(x, y).is_none(),
            "visibility changes invalidate the painted hit generation"
        );
        view.render("draft", &[], false, &[]);
        view.toggle_sidebar();
        view.toggle_sidebar();
        assert!(view.mcp_sidebar_action(x, y).is_none());
    }
    #[test]
    fn memory_scroll_uses_the_rendered_panel_viewport_at_normal_and_wide_widths() {
        for width in [100, 180] {
            let mut view = NativeView::new(width, 30);
            view.open_page(Page::Memory);
            let content = format!("{}\nFINAL_MEMORY_LINE", "instruction line\n".repeat(100));
            view.load_memory(
                &serde_json::json!({"sources":[{
                    "path":"/fixture/AGENTS.md","content":content,"included":true,"reason":null
                }]})
                .to_string(),
            )
            .unwrap();
            view.scroll_memory_content(isize::MAX);
            let frame = view.render("preserved draft", &[], false, &[]).join("\n");
            assert!(
                frame.contains("FINAL_MEMORY_LINE"),
                "last source line is unreachable at width {width}"
            );
        }
    }
    #[test]
    fn every_page_has_outer_space_heading_hierarchy_and_selected_row_roles() {
        for page in [
            Page::Sessions,
            Page::Help,
            Page::Models,
            Page::Providers,
            Page::Agents,
            Page::Workspaces,
            Page::Status,
            Page::Mcp,
            Page::Diff,
            Page::Context,
            Page::Commands,
            Page::Memory,
        ] {
            let mut view = NativeView::new(100, 30);
            view.open_page(page);
            view.set_page_rows(vec![PageRow {
                id: "one".into(),
                label: "Example item".into(),
                detail: "Secondary description".into(),
                enabled: true,
            }]);
            if page == Page::Memory {
                view.load_memory(r#"{"sources":[{"path":"/fixture/AGENTS.md","content":"Example instruction","included":true,"reason":null}]}"#).unwrap();
            }
            let frame = view.render("unsent", &[], false, &[]);
            assert!(frame[0].trim().is_empty(), "{page:?}: outer top margin");
            let regions = view.ui_regions();
            assert!(
                regions
                    .iter()
                    .any(|row| row.role == crate::native_ui_style::Role::Heading),
                "{page:?}: heading role"
            );
            if page != Page::Status {
                let selected = regions
                    .iter()
                    .find(|row| row.role == crate::native_ui_style::Role::Selected)
                    .expect("selection role");
                assert!(
                    selected.x >= 4 && selected.width <= 92,
                    "{page:?}: horizontal insets"
                );
            }
        }
    }
    #[test]
    fn selected_page_row_stays_visible_after_many_arrow_moves_and_resize() {
        let mut view = NativeView::new(80, 16);
        view.open_page(Page::Sessions);
        view.set_page_rows(
            (0..30)
                .map(|i| PageRow {
                    id: format!("id-{i}"),
                    label: format!("Item {i}"),
                    detail: "details".into(),
                    enabled: true,
                })
                .collect(),
        );
        for _ in 0..25 {
            view.page_next();
        }
        let frame = view.render("", &[], false, &[]).join("\n");
        assert!(frame.contains("Item 25"));
        let regions = view.ui_regions();
        assert!(regions
            .iter()
            .any(|row| row.role == crate::native_ui_style::Role::Selected && row.y < 16));
    }
}

#[cfg(test)]
mod native_home_feedback_tests {
    use super::*;
    #[test]
    fn native_home_displays_rejected_turn_without_hiding_the_draft() {
        let mut view = NativeView::new(100, 30);
        view.set_session_route(false);
        let messages = [crate::native_view_chat::ChatMessage::text(
            crate::native_view_chat::ChatRole::System,
            "error: provider HTTP 429; prompt preserved",
        )];
        let rows = view
            .render_typed("retained café", &messages, false, &[])
            .join("\n");
        assert!(rows.contains("HTTP 429"));
        assert!(rows.contains("retained café"));
        assert!(!view.session_route);
    }
}

/// Region geometry for the current frame, without ANSI. Downstream style
/// writers derive cell positions from this instead of parsing the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewPresentation {
    /// Viewport width in cells.
    pub cols: u16,
    /// Viewport height in rows.
    pub rows: u16,
    /// Main-column start cell (always 0: no permanent left pane).
    pub main_offset: u16,
    /// Main-column outer width in cells.
    pub main_width: u16,
    /// Upstream view horizontal padding applied inside the main column.
    pub main_padding: u16,
    /// Sidebar start cell, or `None` when the sidebar is not shown.
    pub sidebar_offset: Option<u16>,
    /// Sidebar width in cells (`0` when not shown).
    pub sidebar_width: u16,
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
    pub fn ui_regions(&self) -> Vec<crate::native_ui_style::Region> {
        self.ui_regions.clone()
    }
    pub fn set_overlay_roles(&mut self, roles: Vec<crate::native_ui_style::Role>) {
        self.overlay_roles = roles;
    }

    pub fn page_panel(&self) -> Option<(u16, u16, u16, u16)> {
        if matches!(self.page(), Page::Chat | Page::Theme) {
            return None;
        }
        let presentation = self.presentation();
        let width = presentation
            .main_width
            .saturating_sub(if presentation.main_width >= 20 { 4 } else { 0 })
            .min(104);
        let y = u16::from(self.render_height() >= 8);
        Some((
            presentation.main_offset + (presentation.main_width - width) / 2,
            y.saturating_add(self.tab_reservation.y_offset()),
            width,
            (self.render_height() as u16).saturating_sub(2 * y),
        ))
    }

    pub fn open_command_palette(&mut self, has_session: bool) {
        self.command_palette.open(has_session);
    }
    pub fn command_palette_open(&self) -> bool {
        self.command_palette.is_open()
    }
    pub fn close_command_palette(&mut self) {
        self.command_palette.close();
    }
    pub fn command_palette_push(&mut self, ch: char) -> bool {
        self.command_palette.push_query(ch)
    }
    pub fn command_palette_backspace(&mut self) -> bool {
        self.command_palette.backspace()
    }
    pub fn command_palette_next(&mut self) {
        self.command_palette.next();
    }
    pub fn command_palette_prev(&mut self) {
        self.command_palette.prev();
    }
    pub fn command_palette_activate(&self) -> Option<crate::native_command_palette::PaletteAction> {
        self.command_palette.activate()
    }
    pub fn command_palette_render(&self) -> Vec<String> {
        let geometry = self.modal_presentation();
        self.command_palette
            .render(geometry.content_width, self.modal_content_height())
    }

    /// Startup composition for the given kernel geometry.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            layout: Layout::new(cols, rows),
            sessions: SessionView::default(),
            chat: ChatView::default(),
            pages: PageState::default(),
            memory: crate::native_memory::Memory::new(),
            compaction: crate::native_compaction_settings::CompactionSettingsView::new(),
            context: crate::native_context::ContextView::new(),
            status: StatusView::default(),
            dialogs: DialogView::default(),
            command_palette: crate::native_command_palette::CommandPalette::default(),
            permission_retry_attempt: None,
            workspace: String::new(),
            submit_with_ctrl_j: false,
            session_route: true,
            sidebar_hidden: false,
            sidebar_open: false,
            parent_session: false,
            prompt_width: PromptWidth::Default,
            shell_mode: false,
            placeholder_index: 0,
            dialog_size: DialogSize::Medium,
            composer_agent: None,
            send_pointer: None,
            context_usage_hit: None,
            composer_variant: None,
            composer_model: None,
            composer_effort: None,
            chat_row_classes: Vec::new(),
            chat_text_bounds: Vec::new(),
            box_appearance: crate::native_box_style::BoxAppearance::default(),
            context_window: None,
            ui_regions: Vec::new(),
            modal_underlay_rows: Vec::new(),
            overlay_roles: Vec::new(),
            frame_busy: false,
            mcp_hit: Vec::new(),
            sidebar_content_height: 0,
            tab_reservation: TabReservation::default(),
        }
    }

    /// Kernel geometry changed: rebuild layout, keep scroll/page state.
    /// Install the reservation computed for the current renderer frame.
    /// Every geometry query below observes this same value.
    pub fn set_tab_reservation(&mut self, reservation: TabReservation) {
        if self.tab_reservation != reservation {
            self.mcp_hit.clear();
            self.context_usage_hit = None;
        }
        self.tab_reservation = reservation;
    }

    #[must_use]
    pub fn tab_reservation(&self) -> TabReservation {
        self.tab_reservation
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.mcp_hit.clear();
        self.context_usage_hit = None;
        self.sidebar_content_height = 0;
        self.layout = Layout::new(cols, rows);
    }

    // ---------- startup facts (set from tui_entry, no IO here) ----------

    pub fn set_model(&mut self, model: Option<String>) {
        self.composer_model = None;
        self.status.set_model(model);
    }

    pub fn set_context_window(&mut self, window: Option<u64>) {
        self.context_window = window;
        self.status.set_context_window(window);
    }
    pub fn set_usage(
        &mut self,
        input: u64,
        output: u64,
        cached: Option<u64>,
        reasoning: Option<u64>,
    ) {
        self.status
            .set_usage(input, output, cached, reasoning, self.context_window);
    }

    /// Compatibility shim for the first layout handoff.  Busy state is
    /// deliberately frame-authoritative: callers must pass the current value
    /// through `render`/`render_typed`; retaining this value would leave a
    /// stale spinner after cancellation or error.
    #[deprecated(note = "pass the authoritative busy value to render instead")]
    pub fn set_loading(&mut self, loading: bool) {
        let _ = loading;
    }

    /// The directory shown by the footer is the backend-selected project
    /// directory supplied by the caller.  This explicit name is an alias for
    /// the existing workspace setter so integrations need not infer it from
    /// the process cwd.
    pub fn set_project_directory(&mut self, directory: String) {
        self.set_workspace(directory);
    }

    /// Catalogue display names are presentation only. The status retains the
    /// qualified identity used by the daemon and persisted configuration.
    pub fn set_composer_model(&mut self, model: &str, provider: &str) {
        self.composer_model = if [model, provider].iter().all(|name| {
            !name.trim().is_empty() && name.len() <= 256 && !name.chars().any(char::is_control)
        }) {
            Some((model.to_owned(), provider.to_owned()))
        } else {
            None
        };
    }

    /// Model state was never observed (failed catalogue fetch): honest
    /// `Unavailable`, never `None` (which would claim "confirmed unset").
    pub fn set_model_unknown(&mut self) {
        self.composer_model = None;
        self.status.set_model_unknown();
    }

    pub fn set_connection(&mut self, connected: bool) {
        self.status.set_connection(connected);
    }

    pub fn modal_underlay_rows(&self) -> Vec<String> {
        self.modal_underlay_rows.clone()
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

    pub fn set_prompt_width(&mut self, value: PromptWidth) {
        self.prompt_width = value;
    }
    pub fn chat_row_classes(&self) -> &[crate::native_view_chat::ChatRowClass] {
        &self.chat_row_classes
    }
    pub fn chat_text_bounds(&self) -> &[Option<(u16, u16)>] {
        &self.chat_text_bounds
    }

    pub fn set_prompt_mode(&mut self, shell: bool) {
        self.shell_mode = shell;
    }

    pub fn set_placeholder_index(&mut self, index: usize) {
        self.placeholder_index = index % 3;
    }

    pub fn set_dialog_size(&mut self, size: DialogSize) {
        self.dialog_size = size;
    }

    pub fn set_composer_agent(&mut self, agent: Option<String>, variant: Option<String>) {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|text| {
                    crate::native_transcript::strip_ansi(&text)
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(128)
                        .collect::<String>()
                })
                .filter(|text| !text.trim().is_empty())
        }
        self.composer_agent = clean(agent);
        self.composer_variant = clean(variant);
    }

    /// Set the provider effort actually selected for the next request.  The
    /// caller must pass an observed/catalogued value; this view never invents
    /// a default when the daemon did not report one.
    pub fn set_composer_effort(&mut self, effort: Option<String>) {
        self.composer_effort = effort.and_then(|value| {
            let cleaned = crate::native_transcript::strip_ansi(&value)
                .chars()
                .filter(|c| !c.is_control())
                .take(32)
                .collect::<String>();
            (!cleaned.trim().is_empty()).then_some(cleaned)
        });
    }

    fn render_height(&self) -> usize {
        let cols = usize::from(self.layout.cols);
        if cols == 0 {
            return 0;
        }
        usize::from(self.layout.rows)
            .saturating_sub(usize::from(self.tab_reservation.y_offset()))
            .min(256)
            .min(MAX_FRAME_CELLS / cols)
    }

    pub fn home_prompt_width(&self) -> u16 {
        let width = self.presentation().main_width;
        let requested = match self.prompt_width {
            PromptWidth::Default => 75,
            PromptWidth::Auto => 75usize
                .max(usize::from(width) * 7 / 10)
                .min(u16::MAX as usize) as u16,
            PromptWidth::Fixed(value) => value.max(1),
        };
        width.saturating_sub(4).min(requested)
    }

    pub fn modal_presentation(&self) -> ModalPresentation {
        // The final row is the shared native footer.  Modal bounds must be
        // calculated against the rows above it, otherwise modal compositing
        // paints over the footer and leaves hit regions inconsistent with the
        // visible action rows.
        let available_height = self.render_height().saturating_sub(1);
        let content_cols = self.layout.cols.saturating_sub(self.tab_reservation.x_offset());
        let width = content_cols.saturating_sub(2).min(self.dialog_size.width());
        let y = (available_height as u16).div_ceil(4);
        let box_id = if self.command_palette.is_open() {
            crate::native_box_style::BoxId::Menu
        } else {
            crate::native_box_style::BoxId::Dialog
        };
        let padding = u16::from(
            self.box_appearance
                .effective(box_id)
                .padding
                .min(crate::native_box_style::MAX_PADDING),
        );
        let x = self
            .tab_reservation
            .x_offset()
            .saturating_add(content_cols.saturating_sub(width) / 2);
        let inset = 2u16.saturating_add(padding).min(width);
        ModalPresentation {
            x,
            y: y.saturating_add(self.tab_reservation.y_offset()),
            width,
            content_x: x.saturating_add(inset),
            content_y: y
                .saturating_add(1)
                .saturating_add(padding)
                .saturating_add(self.tab_reservation.y_offset()),
            content_width: width.saturating_sub(2 * inset),
            bottom_padding: padding,
            backdrop_alpha: 150,
        }
    }

    pub fn sidebar_backdrop_alpha(&self) -> u8 {
        if self.sidebar_visible() && self.layout.cols <= 120 {
            70
        } else {
            0
        }
    }

    /// Window modal content below its top inset and above its bottom border.
    pub fn modal_content_height(&self) -> usize {
        let modal = self.modal_presentation();
        if modal.content_width == 0 {
            return 0;
        }
        self.render_height()
            .saturating_sub(1)
            .saturating_sub(
                usize::from(modal.content_y.saturating_sub(self.tab_reservation.y_offset()))
                    + 1
                    + usize::from(modal.bottom_padding),
            )
    }

    fn composer_metadata(&self) -> String {
        if self.shell_mode {
            return self.composer_agent.as_deref().unwrap_or("Shell").to_owned();
        }

        let terminal_width = usize::from(self.layout.cols);
        let available = terminal_width.saturating_sub(if terminal_width < 44 { 9 } else { 13 });
        let agent = (terminal_width >= 44)
            .then_some(self.composer_agent.as_deref())
            .flatten();
        let (model, provider) = self
            .composer_model
            .as_ref()
            .map_or((self.status.model_label(), ""), |(model, provider)| {
                (model.as_str(), provider.as_str())
            });
        let provider = if terminal_width < 44 { "" } else { provider };
        let provider_short = provider.rsplit(" / ").next().unwrap_or(provider);
        let variant = self.composer_variant.as_deref();
        let effort = self.composer_effort.as_deref();
        let format = |provider: &str, model: &str| {
            let mut parts = Vec::new();
            if let Some(agent) = agent {
                parts.push(agent.to_owned());
            }
            if !model.is_empty() {
                if agent.is_some() {
                    parts.push("·".to_owned());
                }
                parts.push(model.to_owned());
                if !provider.is_empty() {
                    parts.push(provider.to_owned());
                }
            }
            if let Some(variant) = variant {
                parts.push("·".to_owned());
                parts.push(variant.to_owned());
            }
            if let Some(effort) = effort {
                parts.push("·".to_owned());
                parts.push(effort.to_owned());
            }
            parts.join(" ")
        };

        // Upstream `auto` comes from `local.permission.mode`; no confirmed
        // mode fact reaches this view, so it must not fabricate that label.
        for candidate in [
            format(provider, model),
            format(provider_short, model),
            format("", model),
        ] {
            if prompt_metadata_cells(&candidate) <= available {
                return candidate;
            }
        }

        let prefix = agent.map_or_else(String::new, |agent| format!("{agent} · "));
        let suffix = [variant, effort]
            .into_iter()
            .flatten()
            .map(|value| format!(" · {value}"))
            .collect::<String>();
        let model_width = available
            .saturating_sub(prompt_metadata_cells(&prefix))
            .saturating_sub(prompt_metadata_cells(&suffix))
            .max(9);
        let model = truncate_prompt_metadata(model, model_width);
        format("", &model)
    }

    // ---------- route / sidebar facts (set from the input loop) ----------

    /// Main-column route: `true` shows the active session (transcript +
    /// composer), `false` shows Home. Set explicitly from the real
    /// transcript/backend state; startup memory/error text must never force
    /// this on its own.
    /// Install the persisted global + per-box appearance. The native
    /// composer frame resolves `BoxId::ChatComposer` from this value.
    pub fn set_box_appearance(&mut self, appearance: crate::native_box_style::BoxAppearance) {
        if self.box_appearance != appearance {
            self.mcp_hit.clear();
        }
        self.box_appearance = appearance;
    }

    pub fn set_session_route(&mut self, session_route: bool) {
        if self.session_route != session_route {
            self.mcp_hit.clear();
        }
        self.session_route = session_route;
    }

    /// Toggle the optional wide sidebar (`session.sidebar.toggle`).
    pub fn toggle_sidebar(&mut self) {
        self.mcp_hit.clear();
        let visible = self.sidebar_visible();
        self.sidebar_hidden = visible;
        self.sidebar_open = !visible;
    }

    /// Force the sidebar hidden/shown. Hidden wins over the wide auto rule.
    pub fn set_sidebar_hidden(&mut self, hidden: bool) {
        self.mcp_hit.clear();
        self.sidebar_hidden = hidden;
        self.sidebar_open = false;
    }

    pub fn sidebar_hidden(&self) -> bool {
        self.sidebar_hidden
    }

    /// Mark the active session as a child (`parentID` present): upstream
    /// suppresses the sidebar regardless of width or toggle.
    pub fn set_parent_session(&mut self, parent_session: bool) {
        if self.parent_session != parent_session {
            self.mcp_hit.clear();
        }
        self.parent_session = parent_session;
    }

    /// Current region geometry (no ANSI): positions for the style writer.
    #[must_use]
    pub fn presentation(&self) -> ViewPresentation {
        let sidebar = self.sidebar_visible();
        let inline = sidebar && self.layout.cols > 120;
        let sidebar_width = if sidebar { self.layout.cols.min(42) } else { 0 };
        let main_width = self
            .layout
            .cols
.saturating_sub(self.tab_reservation.x_offset())
            .saturating_sub(if inline { sidebar_width } else { 0 });
        ViewPresentation {
            cols: self.layout.cols,
            rows: self.layout.rows,
            main_offset: self.tab_reservation.x_offset(),
            main_width,
            main_padding: 2,
            sidebar_offset: if sidebar {
                Some(self.layout.cols.saturating_sub(sidebar_width))
            } else {
                None
            },
            sidebar_width,
        }
    }

    fn sidebar_visible(&self) -> bool {
        self.session_route
            && !self.parent_session
            && self.layout.rows >= 2
            && (self.sidebar_open || (!self.sidebar_hidden && self.layout.cols > 120))
    }

    /// Composer box coordinates shared with the painter. The box is derived
    /// from route and bounded draft rows, never from visible text matching.
    pub fn composer_region(&self, draft: &str) -> Option<(u16, u16, u16, u16)> {
        let height = self.render_height();
        let width = self.presentation().main_width;
        if height == 0 || width <= 4 || !matches!(self.pages.page(), Page::Chat | Page::Theme) {
            return None;
        }
        if self.session_route {
            let body_height = if height >= 4 { height - 2 } else { height };
            let frame = self
                .box_appearance
                .effective(crate::native_box_style::BoxId::ChatComposer);
            // Shared with the actual ChatComposer renderer; Home's legacy
            // rail remains a separate layout path.
            let layout = crate::native_view_chat::composer_layout(
                usize::from(width - 4),
                body_height,
                draft,
                frame,
            );
            let composer_height = layout.height;
            Some((
                self.tab_reservation.x_offset().saturating_add(2),
                // The shared body budget includes the header; the composer
                // starts immediately after its remaining transcript rows.
                body_height
                    .saturating_sub(composer_height)
                    .saturating_add(usize::from(self.tab_reservation.y_offset())) as u16,
                width - 4,
                composer_height as u16,
            ))
        } else {
            let (count, _, input_top) = self.home_positions(draft);
            let prompt_width = self.home_prompt_width();
            Some((
                self.tab_reservation
                    .x_offset()
                    .saturating_add((width - prompt_width).div_ceil(2)),
                input_top
                    .saturating_sub(1)
                    .saturating_add(usize::from(self.tab_reservation.y_offset())) as u16,
                prompt_width,
                (count + 4).min(height - input_top.saturating_sub(1)) as u16,
            ))
        }
    }

    pub fn home_logo_region(&self, draft: &str) -> Option<(u16, u16)> {
        let height = self.render_height();
        let width = self.presentation().main_width;
        let (count, top, _) = self.home_positions(draft);
        if self.session_route
            || !matches!(self.pages.page(), Page::Chat | Page::Theme)
            || width < 43
            || height < count + 15
        {
            return None;
        }
        Some((
            self.tab_reservation.x_offset().saturating_add((width - 39).div_ceil(2)),
            (top as u16).saturating_add(self.tab_reservation.y_offset()),
        ))
    }

    /// The same cell rectangle drives both drawing and pointer admission.
    pub fn send_button_region(&self, draft: &str) -> Option<(u16, u16, u16)> {
        if self.pages.page() != Page::Chat {
            return None;
        }
        let (left, top, width, height) = self.composer_region(draft)?;
        if width < 14 || height < 3 {
            return None;
        }
        let row = top.checked_add(height.checked_sub(if self.session_route { 1 } else { 2 })?)?;
        Some((
            left.checked_add(width.checked_sub(8)?)?,
            row,
            6,
        ))
    }

    pub fn send_button_hit(&self, draft: &str, x: u16, y: u16) -> bool {
        if self.dialogs.is_open() || self.command_palette.is_open() {
            return false;
        }
        if self.layout.cols <= 120
            && self.sidebar_visible()
            && self
                .presentation()
                .sidebar_offset
                .is_some_and(|left| x >= left)
        {
            return false;
        }
        self.send_button_region(draft)
            .is_some_and(|(left, row, width)| {
                y == row && x >= left && x < left.saturating_add(width)
            })
    }

    /// Actual Context Usage footer cells, absent until a matching frame renders.
    pub fn context_usage_region(&self) -> Option<(u16, u16, u16)> {
        self.context_usage_hit
    }

    /// Admit a click only within the exact last-rendered usage label.
    pub fn context_usage_hit(&self, x: u16, y: u16) -> bool {
        !self.dialogs.intercepts_input()
            && !self.command_palette_open()
            && self.context_usage_hit.is_some_and(|(left, row, width)| {
                width > 0 && y == row && x >= left && x < left.saturating_add(width)
            })
    }

    /// Same destination as `/context`; does not mutate composer or binding state.
    pub fn open_context_usage(&mut self) {
        self.open_page(Page::Context);
    }

    pub fn set_send_pointer(&mut self, draft: &str, x: u16, y: u16) -> bool {
        let old = self
            .send_pointer
            .is_some_and(|(x, y)| self.send_button_hit(draft, x, y));
        self.send_pointer = Some((x, y));
        old != self.send_button_hit(draft, x, y)
    }

    pub fn home_cursor(&self, draft: &str) -> Option<(u16, u16)> {
        if self.session_route {
            return None;
        }
        let (left, top, width, height) = self.composer_region(draft)?;
        if width <= 3 || height <= 3 {
            return None;
        }
        let bounded: String = draft.chars().take(MAX_RENDER_DRAFT_CHARS).collect();
        let clean = crate::native_transcript::strip_ansi(&bounded);
        let mut lines = clean.split('\n').take(129);
        let mut last = lines.next().unwrap_or("");
        let mut row = 0u16;
        for line in lines {
            row += 1;
            last = line;
        }
        if row >= height.saturating_sub(4) {
            return None;
        }
        let cells = crate::native_view_layout::visit_unpadded_cells(last, usize::from(width - 3), |_, _| {}) as u16;
        Some((left + 3 + cells.min(width - 4), top + 1 + row))
    }

    pub fn set_session(&mut self, session: Option<String>) {
        self.status.set_session(session);
    }

    /// Return the visual shell to Home after the client closes its last tab.
    /// This is deliberately only a client-side projection: it does not delete
    /// or mutate the daemon session, and the caller owns the tab/worker state.
    pub fn reset_to_home_after_tab_close(&mut self) {
        self.close_page();
        self.set_session_route(false);
        self.set_session(None);
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
        if self.status.session_id() != session.as_deref() {
            self.status.clear_usage();
            self.status.clear_mcp_inclusion();
        }
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

    pub fn load_memory(&mut self, body: &str) -> Result<(), String> {
        self.memory.load_response(body)
    }
    pub fn set_memory_error(&mut self, error: &str) {
        self.memory.set_error(error);
    }
    pub fn memory_mut(&mut self) -> &mut crate::native_memory::Memory {
        &mut self.memory
    }
    pub fn memory(&self) -> &crate::native_memory::Memory {
        &self.memory
    }
    pub fn compaction(&self) -> &crate::native_compaction_settings::CompactionSettingsView {
        &self.compaction
    }
    pub fn compaction_mut(
        &mut self,
    ) -> &mut crate::native_compaction_settings::CompactionSettingsView {
        &mut self.compaction
    }
    pub fn load_context(&mut self, body: &str) -> Result<(), String> {
        self.context.load_response(body)
    }
    pub fn set_context_error(&mut self, error: &str) {
        self.context.set_error(error);
    }
    pub fn invalidate_context(&mut self) {
        self.context.invalidate();
    }
    pub fn context_mut(&mut self) -> &mut crate::native_context::ContextView {
        &mut self.context
    }
    pub fn context(&self) -> &crate::native_context::ContextView {
        &self.context
    }
    pub fn select_context_step(&mut self, down: bool) {
        self.context.select_step(down);
    }
    pub fn scroll_context_detail(&mut self, down: bool) {
        self.context.scroll_detail(down);
    }
    pub fn scroll_memory_content(&mut self, lines: isize) {
        let Some((_, _, width, height)) = self.page_panel() else {
            return;
        };
        let inset = if width >= 8 { 2 } else { 0 };
        let vertical = u16::from(height >= 4);
        self.memory.scroll_content(
            lines,
            usize::from(width.saturating_sub(2 * inset)),
            usize::from(height.saturating_sub(2 * vertical)),
        );
    }

    /// Backend MCP snapshot (`GET /api/mcp`) projected into the right pane.
    pub fn apply_mcp(&mut self, value: &Value) -> Result<(), String> {
        self.status.apply_mcp(value)?;
        self.mcp_hit.clear();
        Ok(())
    }

    pub fn apply_todos(&mut self, value: &Value) -> Result<(), String> {
        let result = self.status.apply_todos(value);
        self.mcp_hit.clear();
        result
    }

    pub fn set_todos_error(&mut self, reason: &str) {
        self.status.set_todos_error(reason);
        self.mcp_hit.clear();
    }

    pub fn mcp_sidebar_action(&self, x: u16, y: u16) -> Option<(String, bool)> {
        if self.presentation().sidebar_offset.is_none()
            || self.dialogs.intercepts_input()
            || self.command_palette_open()
        {
            return None;
        }
        self.mcp_hit
            .iter()
            .find(|(_, _, left, top, width)| {
                x >= *left && x < left.saturating_add(*width) && y == *top
            })
            .filter(|(name, included, _, _, _)| self.status.mcp_included(name) == Some(*included))
            .map(|(name, included, _, _, _)| (name.clone(), *included))
    }

    pub fn scroll_sidebar_at(&mut self, x: u16, y: u16, lines: isize) -> bool {
        let presentation = self.presentation();
        let Some(left) = presentation.sidebar_offset else {
            return false;
        };
        let Some(content_y) = y.checked_sub(self.tab_reservation.y_offset()) else {
            return false;
        };
        if x < left
            || x >= left.saturating_add(presentation.sidebar_width)
            || usize::from(content_y) >= self.render_height().saturating_sub(1)
            || self.sidebar_content_height == 0
            || self.dialogs.intercepts_input()
            || self.command_palette_open()
        {
            return false;
        }
        self.status
            .scroll_sidebar(lines, self.sidebar_content_height);
        self.mcp_hit.clear();
        true
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

    pub fn theme_dialog(&self) -> Vec<String> {
        if self.pages.page() != Page::Theme {
            return Vec::new();
        }
        let geometry = self.modal_presentation();
        let height = self
            .render_height()
            .saturating_sub(usize::from(geometry.content_y))
            .saturating_sub(1)
            .min(16);
        let mut rows = self
            .pages
            .render(usize::from(geometry.content_width), height);
        if let Some(title) = rows.first_mut() {
            *title = "Themes".into();
        }
        if let Some(search) = rows.get_mut(1) {
            *search = format!("Search: {}", self.pages.query());
        }
        rows
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

    /// Tail-relative chat viewport state owned by the active client tab.
    #[must_use]
    pub fn chat_scroll_offset(&self) -> usize {
        self.chat.scroll_offset()
    }

    /// Restore a previously accepted tab viewport without changing the
    /// pagination policy.
    pub fn set_chat_scroll_offset(&mut self, rows: usize) {
        self.chat.set_scroll(rows);
    }

    pub fn chat_at_scroll_ceiling(&self) -> bool {
        self.chat.at_scroll_ceiling()
    }

    pub fn chat_at_tail(&self) -> bool {
        self.chat.at_tail()
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

    /// Compose one bounded frame: the main column (chat/page/dialog), plus
    /// the optional fixed 42-column sidebar only when `cols > 120`. There is
    /// no permanent left all-sessions pane and no permanent right status
    /// pane (upstream `session/index.tsx:255-278`). Returns at most
    /// `layout.rows` lines, ready for the preserved `native_paint` path.
    pub fn render(
        &mut self,
        draft: &str,
        transcript: &[String],
        busy: bool,
        overlay: &[String],
    ) -> Vec<String> {
        self.render_content(draft, transcript, None, busy, overlay, None)
    }

    pub fn render_typed(
        &mut self,
        draft: &str,
        messages: &[crate::native_view_chat::ChatMessage],
        busy: bool,
        overlay: &[String],
    ) -> Vec<String> {
        self.render_content(draft, &[], Some(messages), busy, overlay, None)
    }

    /// Renders a session-route permission prompt in the chat body. The
    /// dialog remains owned by `DialogView`, so keyboard input and broker ACK
    /// correlation are unchanged; this method only changes placement.
    pub fn render_typed_permission(
        &mut self,
        draft: &str,
        messages: &[crate::native_view_chat::ChatMessage],
        busy: bool,
        overlay: &[String],
        permission_rows: &[String],
    ) -> Vec<String> {
        self.render_content(
            draft,
            &[],
            Some(messages),
            busy,
            overlay,
            Some(permission_rows),
        )
    }

    /// Exact styled transcript height for a bounded message batch. Used by
    /// history pagination to preserve a rendered-row anchor across eviction.
    pub fn chat_message_rows(&self, messages: &[crate::native_view_chat::ChatMessage]) -> usize {
        let width = self.presentation().main_width.saturating_sub(4) as usize;
        self.chat
            .styled_message_rows(width, messages, &self.box_appearance)
    }

    pub fn chat_rendered_anchor(
        &self,
        entries: &[(String, crate::native_view_chat::ChatMessage)],
    ) -> Option<crate::native_view_chat::RenderedChatAnchor> {
        let width = self.presentation().main_width.saturating_sub(4) as usize;
        let ids = entries.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let messages = entries
            .iter()
            .map(|(_, message)| message.clone())
            .collect::<Vec<_>>();
        self.chat
            .rendered_anchor(width, &messages, &ids, &self.box_appearance)
    }

    pub fn restore_chat_rendered_anchor(
        &mut self,
        entries: &[(String, crate::native_view_chat::ChatMessage)],
        anchor: &crate::native_view_chat::RenderedChatAnchor,
    ) -> bool {
        let width = self.presentation().main_width.saturating_sub(4) as usize;
        let ids = entries.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let messages = entries
            .iter()
            .map(|(_, message)| message.clone())
            .collect::<Vec<_>>();
        let Some(scroll) =
            self.chat
                .scroll_for_anchor(width, &messages, &ids, &self.box_appearance, anchor, 0)
        else {
            return false;
        };
        self.chat.set_scroll(scroll);
        true
    }

    fn render_content(
        &mut self,
        draft: &str,
        transcript: &[String],
        messages: Option<&[crate::native_view_chat::ChatMessage]>,
        busy: bool,
        overlay: &[String],
        permission_rows: Option<&[String]>,
    ) -> Vec<String> {
        // A provider turn owns the footer loader.  Approval/setup overlays
        // retain their existing center loader semantics and must not be
        // relabeled as a conversation spinner.
        // Thinking is a main-pane header status. It must not suppress the
        // composer hit region or move interaction geometry into the footer.
        self.frame_busy = false;
        self.chat_row_classes.clear();
        self.chat_text_bounds.clear();
        self.context_usage_hit = None;
        self.ui_regions.clear();
        self.modal_underlay_rows.clear();
        let height = self.render_height();
        if self.layout.cols == 0 || height == 0 {
            return Vec::new();
        }
        let presentation = self.presentation();
        let width = presentation.main_width;
        let inner_width = width.saturating_sub(4);
        let is_chat = matches!(self.pages.page(), Page::Chat | Page::Theme);
        let mut center = if !is_chat {
            let (left, top, panel_width, panel_height) = self.page_panel().unwrap();
            let inset = if panel_width >= 8 { 2 } else { 0 };
            let content_width = panel_width.saturating_sub(2 * inset);
            let vertical = usize::from(panel_height >= 4);
            let content_height = usize::from(panel_height).saturating_sub(2 * vertical);
            let lines = if self.pages.page() == Page::Status {
                self.status
                    .render_styled(content_width as usize, content_height)
            } else if self.pages.page() == Page::Memory {
                self.memory
                    .render(content_width as usize, content_height)
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| crate::native_ui_style::Line {
                        text,
                        role: if index == 0 {
                            crate::native_ui_style::Role::Heading
                        } else if index == 1
                            && self.memory.selected().is_some()
                            && self.memory.error().is_none()
                        {
                            crate::native_ui_style::Role::Selected
                        } else {
                            crate::native_ui_style::Role::Item
                        },
                        detail_from: None,
                    })
                    .collect()
            } else if self.pages.page() == Page::Compaction {
                self.compaction
                    .render(content_width as usize, content_height)
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| crate::native_ui_style::Line {
                        text,
                        role: if index == 0 {
                            crate::native_ui_style::Role::Heading
                        } else {
                            crate::native_ui_style::Role::Item
                        },
                        detail_from: None,
                    })
                    .collect()
            } else if self.pages.page() == Page::Context {
                self.context
                    .render(content_width as usize, content_height)
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| crate::native_ui_style::Line {
                        text,
                        role: if index == 0 {
                            crate::native_ui_style::Role::Heading
                        } else if index == 1 {
                            crate::native_ui_style::Role::Selected
                        } else {
                            crate::native_ui_style::Role::Item
                        },
                        detail_from: None,
                    })
                    .collect()
            } else {
                self.pages
                    .render_styled(content_width as usize, content_height)
            };
            let mut center = vec![String::new(); height];
            for (index, line) in lines.into_iter().enumerate().take(content_height) {
                let row = usize::from(top) + vertical + index;
                if row >= height {
                    break;
                }
                let x = left + inset;
                center[row] = format!(
                    "{}{}",
                    " ".repeat(usize::from(x)),
                    fit(&line.text, content_width)
                );
                self.ui_regions.push(crate::native_ui_style::Region {
                    x,
                    y: row as u16,
                    width: content_width,
                    role: line.role,
                    detail_from: line.detail_from,
                    modal: false,
                });
            }
            center
        } else if self.session_route {
            let body_height = if height >= 4 { height - 2 } else { height };
            let mut body = if let Some(messages) = messages {
                let rows = self.chat.render_styled(
                    inner_width as usize,
                    body_height,
                    &self.workspace,
                    messages,
                    draft,
                    busy,
                    &self.box_appearance,
                );
                self.chat_row_classes
                    .extend(rows.iter().map(|(_, class, _)| *class));
                self.chat_text_bounds
                    .extend(rows.iter().map(|(_, _, bounds)| {
                        bounds.map(|(left, width)| (left.saturating_add(2), width))
                    }));
                rows.into_iter()
                    .map(|(text, _, _)| text)
                    .collect::<Vec<_>>()
            } else {
                self.chat.render(
                    inner_width as usize,
                    body_height,
                    &self.workspace,
                    transcript,
                    draft,
                    busy,
                )
            };
            // The upstream scrollbox starts with a spacer, rather than a
            // permanent workspace/header row. The composer stays at its foot.
            if body.len() > 1 {
                // Keep the confirmed selected model/provider visible in the
                // conversation at narrow widths too. The footer is reserved
                // for directory and turn/status chrome, and the optional
                // sidebar is unavailable at these widths. Only replace the
                // upstream spacer when an actual model observation exists;
                // unknown state remains an empty spacer.
                body[0] = match &self.composer_model {
                    Some(_) => padded(
                        &if busy {
                            format!("Thinking… · {}", self.composer_metadata())
                        } else {
                            self.composer_metadata()
                        },
                        width,
                    ),
                    None if self.status.model_label() != "Unavailable" => padded(
                        &if busy {
                            format!("Thinking… · {}", self.composer_metadata())
                        } else {
                            self.composer_metadata()
                        },
                        width,
                    ),
                    None if busy => padded("Thinking…", width),
                    None => String::new(),
                };
            }
            let has_permission = permission_rows.is_some();
            // Upstream `SessionQuestion` (`permission.tsx:514-523`) renders the
            // permission prompt inline at the conversation bottom with
            // `maxHeight: 15` and REPLACES the composer (index.tsx Switch at
            // 1610-1621: composer.open OR permission — never both). Reserve
            // that bounded inline height BEFORE rendering the chat so the
            // transcript foot is the prompt, not the composer footer, and do
            // NOT append the composer metadata/blank separator when present.
            let permission_height = has_permission.then(|| {
                permission_rows
                    .map(|rows| rows.iter().take(15).count())
                    .unwrap_or(0)
            });
            let body_height = if height >= 4 {
                height.saturating_sub(2).saturating_sub(permission_height.unwrap_or(0))
            } else {
                height
            };
            let mut body = if let Some(messages) = messages {
                let rows = self.chat.render_styled(
                    inner_width as usize,
                    body_height,
                    &self.workspace,
                    messages,
                    draft,
                    busy,
                    &self.box_appearance,
                );
                self.chat_row_classes
                    .extend(rows.iter().map(|(_, class, _)| *class));
                self.chat_text_bounds
                    .extend(rows.iter().map(|(_, _, bounds)| {
                        bounds.map(|(left, width)| (left.saturating_add(2), width))
                    }));
                rows.into_iter()
                    .map(|(text, _, _)| text)
                    .collect::<Vec<_>>()
            } else {
                self.chat.render(
                    inner_width as usize,
                    body_height,
                    &self.workspace,
                    transcript,
                    draft,
                    busy,
                )
            };
            // The upstream scrollbox starts with a spacer, rather than a
            // permanent workspace/header row. The composer stays at its foot.
            if body.len() > 1 {
                // Keep the confirmed selected model/provider visible in the
                // conversation at narrow widths too. The footer is reserved
                // for directory and turn/status chrome, and the optional
                // sidebar is unavailable at these widths. Only replace the
                // upstream spacer when an actual model observation exists;
                // unknown state remains an empty spacer.
                body[0] = match &self.composer_model {
                    Some(_) => padded(
                        &if busy {
                            format!("Thinking… · {}", self.composer_metadata())
                        } else {
                            self.composer_metadata()
                        },
                        width,
                    ),
                    None if self.status.model_label() != "Unavailable" => padded(
                        &if busy {
                            format!("Thinking… · {}", self.composer_metadata())
                        } else {
                            self.composer_metadata()
                        },
                        width,
                    ),
                    None if busy => padded("Thinking…", width),
                    None => String::new(),
                };
            }
            let mut body: Vec<_> = body.into_iter().map(|line| padded(&line, width)).collect();
            if let Some(rows) = permission_rows {
                body.extend(rows.iter().take(15).map(|row| padded(row, width)));
            }
            if has_permission {
                // Permission prompt replaces the composer — no composer
                // metadata / blank separator row beneath it (upstream Switch).
            } else if height >= 4 {
                body.push(padded(&self.composer_metadata(), width));
                body.push(String::new());
            }
            body
        } else {
            let mut home = self.home_column(width, height, draft);
            let notice = transcript.last().map(String::as_str).or_else(|| {
                let last = messages?.last()?;
                if last.role != crate::native_view_chat::ChatRole::System {
                    return None;
                }
                last.parts.iter().rev().find_map(|part| match part {
                    crate::native_view_chat::ChatPart::Text(text) => Some(text.as_str()),
                    _ => None,
                })
            });
            if height >= 2 {
                if let Some(notice) = notice
                    .filter(|notice| notice.starts_with("error:") || notice.starts_with("offline:"))
                {
                    home[height - 2] = padded(&crate::native_transcript::strip_ansi(notice), width);
                }
            }
            home
        };
        self.sidebar_content_height = if presentation.sidebar_width == 0 {
            0
        } else if presentation.sidebar_width >= 4
            && height >= 3
            && self
                .box_appearance
                .full_outline(crate::native_box_style::BoxId::Sidebar)
        {
            height.saturating_sub(3)
        } else {
            height.saturating_sub(1)
        };
        let sidebar_styled = if presentation.sidebar_width > 0 {
            // Backend facts remain available in the optional session sidebar;
            // the all-sessions picker is a separate page, never sidebar chrome.
            self.status
                .render_sidebar_styled(38, self.sidebar_content_height)
        } else {
            Vec::new()
        };
        self.mcp_hit.clear();
        let mut sidebar: Vec<String> = sidebar_styled
            .iter()
            .map(|line| padded(&line.text, presentation.sidebar_width))
            .collect();
        let mut sidebar_inset = 2;
        let mut sidebar_top = 0;
        let mut sidebar_text_width = presentation.sidebar_width.saturating_sub(3);
        let mut sidebar_frame_painted = false;
        if presentation.sidebar_width >= 4
            && height >= 3
            && self
                .box_appearance
                .full_outline(crate::native_box_style::BoxId::Sidebar)
        {
            let style = self
                .box_appearance
                .effective(crate::native_box_style::BoxId::Sidebar);
            // The sidebar's fill, border and text clipping must all be
            // derived from one live component rectangle.  In particular,
            // do not compute the border from `sidebar_width` and clip text
            // independently: that was the source of one-cell overflow at
            // the viewport edge (R1).  `BoxGeometry` is intentionally used
            // here at the production render call site, not only in helper
            // tests.
            // `sidebar` is passed to `compose_frame` as a local pane: its
            // origin is (0, 0), and compose_frame supplies the absolute
            // x-offset by concatenating it after the main column. Keep the
            // geometry in that same local coordinate system; absolute
            // coordinates are derived only when publishing ui_regions.
            let geometry = crate::native_box_style::BoxGeometry::for_border(
                style.border_style,
                0,
                0,
                presentation.sidebar_width,
                // The shell reserves its final row for the shared footer
                // composition.  The sidebar status source is likewise
                // requested at `height - 1`; use that same actual painted
                // component height instead of allowing a border row to be
                // clipped/replaced during final frame composition.
                height.saturating_sub(1).min(u16::MAX as usize) as u16,
                style.padding,
            )
            .and_then(|box_geometry| {
                box_geometry.clipped(presentation.sidebar_width, height as u16)
            });
            if let Some(geometry) = geometry {
                let inside = usize::from(geometry.width.saturating_sub(2));
                let pad = usize::from(geometry.pad).min(inside.saturating_sub(1) / 2);
                let frame = crate::native_box_style::TextFrame::new(
                    style,
                    usize::from(geometry.width),
                    pad,
                )
                .expect("BoxGeometry::framed guarantees minimum TextFrame width");
                sidebar_inset = geometry.x.saturating_add(1 + pad as u16);
                sidebar_top = geometry.y;
                let text_width = geometry
                    .text_area()
                    .map(|(_, _, width, _)| width)
                    .unwrap_or(0);
                sidebar_text_width = text_width;
                sidebar_frame_painted = true;
                let mut framed = vec![frame.edge(false)];
                // The box owns the full component height, even when the
                // status renderer returns fewer rows.  Always materialize
                // the complete interior so the bottom border stays on the
                // geometry's bottom edge rather than immediately following
                // the last non-empty status line.
                for index in 0..usize::from(geometry.height.saturating_sub(2)) {
                    let row = sidebar.get(index).map_or("", String::as_str);
                    framed.push(frame.row(row));
                }
                if geometry.height >= 2 {
                    framed.push(frame.edge(true));
                }
                sidebar = framed;
            } else {
                // A clipped/tiny sidebar is not frameable; fail closed
                // rather than inventing a partial border or background.
                sidebar.clear();
                self.sidebar_content_height = 0;
            }
        }
        if let Some(left) = presentation.sidebar_offset {
            // Do not publish regions for a frame that failed closed. For a
            // normal unframed pane, the existing local content inset is the
            // two-cell status-panel inset; framed panes use text_area() from
            // the same BoxGeometry that generated their rows.
            let region_count = if sidebar_frame_painted {
                // `BoxGeometry::framed` uses height - 1 because the shell's
                // last row is the shared footer. Exclude both sidebar
                // border rows when publishing text regions.
                sidebar_styled.len().min(height.saturating_sub(3))
            } else if sidebar.is_empty() {
                0
            } else {
                sidebar_styled.len().min(height)
            };
            for (index, line) in sidebar_styled.iter().enumerate().take(region_count) {
                self.ui_regions.push(crate::native_ui_style::Region {
                    // `sidebar_inset` is local to the right pane; add the
                    // pane origin exactly once. Text starts after the top
                    // border row, not at the pane's row zero corner.
                    x: left.saturating_add(sidebar_inset),
                    y: if sidebar_frame_painted {
                        sidebar_top.saturating_add(1).saturating_add(index as u16)
                    } else {
                        index as u16
                    },
                    width: sidebar_text_width,
                    role: line.role,
                    detail_from: None,
                    modal: false,
                });
                if let Some((name, included)) =
                    self.status
                        .mcp_sidebar_hit(38, self.sidebar_content_height, index)
                {
                    self.mcp_hit.push((
                        name.to_owned(),
                        included,
                        left.saturating_add(sidebar_inset),
                        if sidebar_frame_painted {
                            sidebar_top.saturating_add(1).saturating_add(index as u16)
                        } else {
                            index as u16
                        },
                        sidebar_text_width,
                    ));
                }
            }
        }
        let inline = presentation.sidebar_width > 0 && self.layout.cols > 120;
        let effective = Layout {
            center_width: width,
            right_width: if inline {
                presentation.sidebar_width
            } else {
                0
            },
            rows: height as u16,
            ..self.layout
        };
        let mut frame = compose_frame(&effective, &[], &center, &sidebar);
        if self.tab_reservation.width > 0 || self.tab_reservation.height > 0 {
            let x = usize::from(self.tab_reservation.x_offset());
            let y = usize::from(self.tab_reservation.y_offset());
            let rows = usize::from(self.layout.rows).min(256);
            let width = usize::from(self.layout.cols);
            let mut shifted = vec![String::new(); y.min(rows)];
            for line in frame.into_iter().take(rows.saturating_sub(y)) {
                let mut row = " ".repeat(x.min(width));
                row.push_str(&line);
                shifted.push(fit(&row, self.layout.cols));
            }
            shifted.resize(rows, String::new());
            frame = shifted;
            for region in &mut self.ui_regions {
                region.x = region.x.saturating_add(self.tab_reservation.x_offset());
                region.y = region.y.saturating_add(self.tab_reservation.y_offset());
            }
            for hit in &mut self.mcp_hit {
                hit.2 = hit.2.saturating_add(self.tab_reservation.x_offset());
                hit.3 = hit.3.saturating_add(self.tab_reservation.y_offset());
            }
        }
        if let Some((left, row, button_width)) = self.send_button_region(draft) {
            if let Some(line) = frame.get_mut(usize::from(row)) {
                let right = left.saturating_add(button_width);
                *line = format!(
                    "{}[Send]{}",
                    fit(line, left),
                    crate::native_view_layout::slice_cells(
                        line,
                        right,
                        self.layout.cols.saturating_sub(right)
                    )
                );
                let enabled = !busy && !draft.trim().is_empty();
                let hovered = self
                    .send_pointer
                    .is_some_and(|(x, y)| self.send_button_hit(draft, x, y));
                self.ui_regions.push(crate::native_ui_style::Region {
                    x: left,
                    y: row,
                    width: button_width,
                    role: if !enabled {
                        crate::native_ui_style::Role::Hint
                    } else if hovered {
                        crate::native_ui_style::Role::Selected
                    } else {
                        crate::native_ui_style::Role::Heading
                    },
                    detail_from: None,
                    modal: false,
                });
            }
        }
        if !inline {
            if let Some(offset) = presentation.sidebar_offset {
                for (row, line) in sidebar.iter().enumerate().take(frame.len()) {
                    frame[row] = fit(
                        &format!(
                            "{}{}",
                            fit(&frame[row], offset),
                            fit(line, presentation.sidebar_width)
                        ),
                        self.layout.cols,
                    );
                }
            }
        }
        // Modal geometry is viewport-owned, rather than restricted to the
        // conversation column: dialog.tsx medium=60, maxWidth=cols-2, top=h/4.
        let modal_geometry = self.modal_presentation();
        let dialog_width = modal_geometry.width;
        let modal_lines = if self.dialogs.is_open() && permission_rows.is_none() {
            self.dialogs.render_styled(
                modal_geometry.content_width as usize,
                self.modal_content_height(),
            )
        } else if self.command_palette.is_open() {
            self.command_palette
                .render_styled(modal_geometry.content_width, self.modal_content_height())
        } else if self.pages.page() == Page::Theme {
            let height = self.modal_content_height().min(16);
            let mut lines = self
                .pages
                .render_styled(usize::from(modal_geometry.content_width), height);
            if let Some(first) = lines.first_mut() {
                first.text = "Themes".into();
            }
            lines
        } else {
            // Generic overlays use the exact same content budget as native
            // dialogs.  In particular, never let an untrusted/oversized
            // overlay consume the reserved footer row.
            overlay
                .iter()
                .take(self.modal_content_height())
                .enumerate()
                .map(|(index, text)| crate::native_ui_style::Line {
                    text: text.clone(),
                    role: self
                        .overlay_roles
                        .get(index)
                        .copied()
                        .unwrap_or(if index == 0 {
                            crate::native_ui_style::Role::Heading
                        } else {
                            crate::native_ui_style::Role::Description
                        }),
                    detail_from: None,
                })
                .collect()
        };
        let modal_content_height = self.modal_content_height();
        for (index, line) in modal_lines.iter().enumerate().take(modal_content_height) {
            let y = usize::from(modal_geometry.content_y).saturating_add(index);
            if y >= height.saturating_sub(1) {
                break;
            }
            self.ui_regions.push(crate::native_ui_style::Region {
                x: modal_geometry.content_x,
                y: modal_geometry
                    .content_y
                    .saturating_add(index.min(u16::MAX as usize) as u16),
                width: modal_geometry.content_width,
                role: line.role,
                detail_from: line.detail_from,
                modal: true,
            });
        }
        let modal: Vec<String> = modal_lines.into_iter().map(|line| line.text).collect();
        let content_rows = modal
            .iter()
            .take(modal_content_height)
            .rposition(|line| !line.trim().is_empty())
            .map_or(0, |i| i + 1)
            .min(modal_content_height);
        if content_rows > 0 {
            // Preserve the bounded composed viewport before modal text replaces
            // its cells. The painter needs actual underlying corner glyphs.
            self.modal_underlay_rows = frame.clone();
            let left = self.layout.cols.saturating_sub(dialog_width) / 2;
            let top = usize::from(modal_geometry.content_y);
            // The panel's inner top padding is an empty panel row, not
            // transcript text behind the dialog.
            for row in usize::from(modal_geometry.y)
                ..height.saturating_sub(1).min(
                    top.saturating_add(content_rows)
                        .saturating_add(1 + usize::from(modal_geometry.bottom_padding)),
                )
            {
                let line = row
                    .checked_sub(top)
                    .and_then(|index| modal.get(index))
                    .filter(|_| row < top + content_rows);
                let right = crate::native_view_layout::slice_cells(
                    &frame[row],
                    left + dialog_width,
                    self.layout.cols.saturating_sub(left + dialog_width),
                );
                frame[row] = format!(
                    "{}{}{}",
                    fit(&frame[row], left),
                    format!(
                        "{}{}{}",
                        " ".repeat(usize::from(modal_geometry.content_x.saturating_sub(left))),
                        fit(
                            line.map_or("", String::as_str),
                            modal_geometry.content_width
                        ),
                        " ".repeat(usize::from(
                            dialog_width
                                .saturating_sub(modal_geometry.content_x.saturating_sub(left))
                                .saturating_sub(modal_geometry.content_width)
                        ))
                    ),
                    right
                );
            }
        }
        // Paint the shared footer last, after modal composition.  The modal
        // geometry reserves this row, so it is never part of modal content or
        // hit regions.  Very short chat frames retain the composer instead of
        // replacing the user's draft with chrome.
        if self.session_route && height >= 4 {
            // The footer is the shared shell row, not a main-column row.  In
            // wide inline-sidebar mode `width` excludes the 42-cell sidebar;
            // using it here leaves the final composed row short and breaks
            // the cell-exact full-frame contract.  Paint the footer against
            // the complete viewport width after sidebar/modal composition.
            // `busy` is the authoritative provider-turn fact.  Keep it on
            // the footer even when a modal/overlay is present; `frame_busy`
            // is only the separate center-composer suppression projection.
            let (footer, context_hit) = self.footer_line(self.layout.cols, busy);
            frame[height - 1] = footer;
            self.context_usage_hit = context_hit.map(|(left, width)| {
                (left, height.saturating_sub(1) as u16, width)
            });
        }
        frame
    }

    fn footer_line(&self, width: u16, loading: bool) -> (String, Option<(u16, u16)>) {
        let inner = width.saturating_sub(4);
        if inner == 0 {
            return (padded("", width), None);
        }
        // Active Thinking is rendered by the bounded conversation header
        // (`native_view_chat::render_header`), before sidebar composition. The
        // shared shell footer remains directory + MCP/version status and never
        // places a loader beneath the right sidebar.
        let _ = loading;
        let mut right = env!("CARGO_PKG_VERSION").to_owned();
        if self.status.mcp_acknowledged() && !self.status.entries().is_empty() {
            let connected = self
                .status
                .entries()
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.status,
                        crate::native_view_status::McpStatus::Connected
                    )
                })
                .count();
            right = format!("⊙ {connected} MCP /status  {right}");
        }
        let context_label = self
            .status
            .context_usage_label()
            .filter(|label| label.chars().count().saturating_add(1) <= usize::from(inner));
        let context_width = context_label
            .as_ref()
            .map_or(0, |label| label.chars().count()) as u16;
        let context_gap = u16::from(context_width > 0);
        let right_width = right
            .chars()
            .count()
            .min(usize::from(inner.saturating_sub(context_width).saturating_sub(context_gap)))
            as u16;
        let left_width = usize::from(inner)
            .saturating_sub(usize::from(right_width))
            .saturating_sub(usize::from(context_width))
            .saturating_sub(usize::from(context_gap))
            .saturating_sub(1);
        let gap = usize::from(inner).saturating_sub(
            left_width
                + usize::from(right_width)
                + usize::from(context_width)
                + usize::from(context_gap),
        );
        let padding = (width / 2).min(2);
        let context_hit = (context_width > 0).then_some((
            padding.saturating_add(inner).saturating_sub(context_width),
            context_width,
        ));
        let directory = fit(
            &footer_directory(&self.workspace, left_width as u16),
            left_width as u16,
        );
        let footer = padded(
            &format!(
                "{}{}{}{}{}",
                directory,
                " ".repeat(gap),
                fit(&right, right_width),
                " ".repeat(usize::from(context_gap)),
                context_label
                    .as_ref()
                    .map(|label| fit(label, context_width))
                    .unwrap_or_default()
            ),
            width,
        );
        (footer, context_hit)
    }

    // home.tsx: two flex-grow spacers, four fixed rows before the logo,
    // one spacer and one prompt padding row. Keep painter and text geometry
    // derived from the same bounded draft, including compact terminals.
    fn home_positions(&self, draft: &str) -> (usize, usize, usize) {
        let height = self.render_height();
        let bounded: String = draft.chars().take(MAX_RENDER_DRAFT_CHARS).collect();
        let clean = crate::native_transcript::strip_ansi(&bounded);
        let count = clean.split('\n').take((height / 2).clamp(1, 12)).count();
        let required = count + 15;
        if height >= required {
            let top = 4 + (height - required) / 2;
            (count, top, top + 7)
        } else {
            (
                count,
                0,
                height
                    .saturating_sub(count + 4)
                    .max(usize::from(height > 1)),
            )
        }
    }

    /// Pinned Home composition: centered four-row logo and a 75-cell maximum
    /// prompt, constrained by the parent's two-cell horizontal padding.
    fn home_column(&self, width: u16, height: usize, draft: &str) -> Vec<String> {
        let mut rows = vec![String::new(); height];
        if height == 0 || width == 0 {
            return rows;
        }
        let prompt_width = self.home_prompt_width();
        let left = width.saturating_sub(prompt_width).div_ceil(2);
        let draft: String = draft.chars().take(MAX_RENDER_DRAFT_CHARS).collect();
        let draft = crate::native_transcript::strip_ansi(&draft);
        let input: Vec<_> = draft.split('\n').take((height / 2).clamp(1, 12)).collect();
        let (_, top, input_top) = self.home_positions(&draft);
        // logo.ts marker substitutions match component/logo.tsx's glyphs;
        // marker-specific shading is applied by the native paint layer.
        let logo = [
            ("                   ", "             ▄     "),
            ("█▀▀█ █▀▀█ █▀▀█ █▀▀▄", "█▀▀▀ █▀▀█ █▀▀█ █▀▀█"),
            ("█__█ █__█ █^^^ █__█", "█___ █__█ █__█ █^^^"),
            ("▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀", "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀"),
        ];
        if height >= input.len() + 15 {
            let logo_width = width.saturating_sub(4).min(39);
            let logo_left = width.saturating_sub(logo_width).div_ceil(2);
            for (index, (left_line, right_line)) in logo.iter().enumerate() {
                let line: String = format!("{left_line} {right_line}")
                    .chars()
                    .map(|c| match c {
                        '_' => ' ',
                        '^' | '~' => '▀',
                        ',' => '▄',
                        other => other,
                    })
                    .collect();
                rows[top + index] = format!(
                    "{}{}",
                    " ".repeat(logo_left as usize),
                    fit(&line, logo_width)
                );
            }
        }
        let border_top = input_top.saturating_sub(1);
        let box_bottom = input_top.saturating_add(input.len() + 2);
        // Default Home keeps upstream's left rail + 2-cell padding exactly;
        // a configured appearance swaps in its preset corners/line weight.
        let home_effective = self
            .box_appearance
            .effective(crate::native_box_style::BoxId::ChatComposer);
        let home_frame = self.box_appearance.home_composer_outline();
        let home_corners = home_effective.corner_glyphs();
        let (home_h, home_v) = home_effective.border_glyphs();
        let home_v = if !home_effective.has_border() {
            ' '
        } else if home_frame {
            home_v
        } else {
            '┃'
        };
        for index in border_top..box_bottom.min(height) {
            if self.box_appearance != crate::native_box_style::BoxAppearance::default()
                && !home_effective.has_border()
            {
                continue;
            }
            let rail = if home_frame && index == border_top {
                home_corners.top_left
            } else {
                home_v
            };
            rows[index] = if home_frame && prompt_width >= 2 {
                if index == border_top {
                    format!(
                        "{}{}{}{}",
                        " ".repeat(left as usize),
                        home_corners.top_left,
                        home_h
                            .to_string()
                            .repeat(prompt_width.saturating_sub(2) as usize),
                        home_corners.top_right
                    )
                } else {
                    format!(
                        "{}{rail}{}{home_v}",
                        " ".repeat(left as usize),
                        " ".repeat(prompt_width.saturating_sub(2) as usize)
                    )
                }
            } else {
                format!(
                    "{}{}{}",
                    " ".repeat(left as usize),
                    rail,
                    " ".repeat(prompt_width.saturating_sub(1) as usize)
                )
            };
        }
        for (index, line) in input.iter().enumerate() {
            if input_top + index < height {
                let normal = [
                    "Fix a TODO in the codebase",
                    "What is the tech stack of this project?",
                    "Fix broken tests",
                ];
                let shell = ["ls -la", "git status", "pwd"];
                let placeholder = if self.shell_mode {
                    format!("Run a command… \"{}\"", shell[self.placeholder_index])
                } else {
                    format!("Ask anything… \"{}\"", normal[self.placeholder_index])
                };
                let label = if draft.is_empty() {
                    placeholder.as_str()
                } else {
                    line
                };
                rows[input_top + index] = if home_frame && prompt_width >= 5 {
                    format!(
                        "{}{home_v}  {}{home_v}",
                        " ".repeat(left as usize),
                        fit(label, prompt_width.saturating_sub(4))
                    )
                } else {
                    format!(
                        "{}{}  {}",
                        " ".repeat(left as usize),
                        home_v,
                        fit(label, prompt_width.saturating_sub(3))
                    )
                };
            }
        }
        let model_row = input_top + input.len() + 1;
        if model_row < height {
            rows[model_row] = if home_frame && prompt_width >= 5 {
                format!(
                    "{}{home_v}  {}{home_v}",
                    " ".repeat(left as usize),
                    fit(&self.composer_metadata(), prompt_width.saturating_sub(4))
                )
            } else {
                format!(
                    "{}{}  {}",
                    " ".repeat(left as usize),
                    home_v,
                    fit(&self.composer_metadata(), prompt_width.saturating_sub(3))
                )
            };
        }
        if box_bottom < height && home_effective.has_border() {
            rows[box_bottom] = if home_frame && prompt_width >= 2 {
                format!(
                    "{}{}{}{}",
                    " ".repeat(left as usize),
                    home_corners.bottom_left,
                    home_h
                        .to_string()
                        .repeat(prompt_width.saturating_sub(2) as usize),
                    home_corners.bottom_right
                )
            } else {
                format!(
                    "{}╹{}",
                    " ".repeat(left as usize),
                    "▀".repeat(prompt_width.saturating_sub(1) as usize)
                )
            };
        }
        if box_bottom + 1 < height.saturating_sub(1) {
            let hint = "ctrl+p commands";
            rows[box_bottom + 1] = format!(
                "{}{}",
                " ".repeat(usize::from(
                    left + prompt_width.saturating_sub(hint.len() as u16)
                )),
                hint
            );
        }
        let mut right = env!("CARGO_PKG_VERSION").to_owned();
        if self.status.mcp_acknowledged() && !self.status.entries().is_empty() {
            let connected = self
                .status
                .entries()
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.status,
                        crate::native_view_status::McpStatus::Connected
                    )
                })
                .count();
            right = format!("⊙ {connected} MCP /status  {right}");
        }
        let inner = width.saturating_sub(4);
        let right_width = (right.chars().count().min(usize::from(inner))) as u16;
        let left_width = inner.saturating_sub(right_width).saturating_sub(2);
        let footer = format!(
            "{}{}{}",
            fit(&self.workspace, left_width),
            " ".repeat(usize::from(inner.saturating_sub(left_width + right_width))),
            fit(&right, right_width)
        );
        rows[height - 1] = padded(&footer, width);
        rows
    }
}

/// Footer directory projection keeps the selected project's basename visible.
/// Unlike the general leading `fit`, this drops the oldest path clusters and
/// prefixes an ellipsis when the backend-confirmed directory exceeds the
/// reserved left segment. Clusters are collected through the same cell-aware
/// native layout scanner, so Unicode and combining sequences are never split.
fn footer_directory(path: &str, width: u16) -> String {
    if width == 0 {
        return String::new();
    }
    let bounded: String = crate::native_transcript::strip_ansi(path)
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_RENDER_DRAFT_CHARS)
        .collect();
    let mut clusters: Vec<(String, u16)> = Vec::new();
    // `visit_cells` uses its width argument as the bounded `fit` budget and
    // pads to that many cells.  Do not pass `u16::MAX`: that would materialize
    // tens of thousands of padding cells and make the reverse walk discard
    // the actual path. Two cells per scalar is a safe upper bound for this
    // renderer's width model, while keeping the scan bounded by the already
    // capped path.
    let scan_width = bounded
        .chars()
        .count()
        .saturating_mul(2)
        .min(usize::from(u16::MAX)) as u16;
    crate::native_view_layout::visit_cells(&bounded, scan_width, |_, glyph, cells| {
        clusters.push((glyph.to_owned(), cells));
    });
    // Remove only the scanner's synthetic suffix, while retaining genuine
    // trailing spaces from the backend-confirmed path.
    let trailing_spaces = bounded.chars().rev().take_while(|c| *c == ' ').count();
    while clusters.last().is_some_and(|(glyph, _)| glyph == " ") {
        clusters.pop();
    }
    for _ in 0..trailing_spaces {
        clusters.push((" ".to_owned(), 1));
    }
    let total: u16 = clusters
        .iter()
        .fold(0, |sum, (_, cells)| sum.saturating_add(*cells));
    if total <= width {
        return clusters.into_iter().map(|(glyph, _)| glyph).collect();
    }
    let mut used = 1u16;
    let mut tail = Vec::new();
    for (glyph, cells) in clusters.into_iter().rev() {
        if used.saturating_add(cells) > width {
            break;
        }
        used = used.saturating_add(cells);
        tail.push(glyph);
    }
    tail.reverse();
    format!("…{}", tail.concat())
}

#[cfg(test)]
mod tab_interaction_geometry_tests {
    use super::*;
    use crate::native_tabs::{TabOrientation, TabReservation};

    fn views(orientation: TabOrientation) -> (NativeView, NativeView, TabReservation) {
        let reservation = TabReservation::for_view(orientation, 150, 32);
        let mut translated = NativeView::new(150, 32);
        translated.set_session_route(false);
        translated.set_tab_reservation(reservation);
        let mut plain = NativeView::new(150 - reservation.x_offset(), 32 - reservation.y_offset());
        plain.set_session_route(false);
        (translated, plain, reservation)
    }

    #[test]
    fn modal_rectangles_translate_once_and_stay_inside_content_viewport() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let (view, plain, reservation) = views(orientation);
            let actual = view.modal_presentation();
            let expected = plain.modal_presentation();
            assert_eq!(actual.x, expected.x + reservation.x_offset());
            assert_eq!(actual.y, expected.y + reservation.y_offset());
            assert_eq!(actual.content_x, expected.content_x + reservation.x_offset());
            assert_eq!(actual.content_y, expected.content_y + reservation.y_offset());
            assert_eq!(actual.width, expected.width);
            assert_eq!(actual.content_width, expected.content_width);
            assert!(actual.x + actual.width <= 150);
            assert!(actual.content_y < 31, "modal content must remain above the footer");
        }
    }

    #[test]
    fn modal_content_budget_is_independent_of_absolute_tab_offset() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let (view, plain, _) = views(orientation);
            assert_eq!(view.modal_content_height(), plain.modal_content_height());
            assert!(view.modal_content_height() > 0);
        }
    }

    #[test]
    fn home_logo_and_page_panel_translate_like_the_content_viewport() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let (mut view, mut plain, reservation) = views(orientation);
            let (x, y) = plain.home_logo_region("draft").unwrap();
            assert_eq!(view.home_logo_region("draft"), Some((x + reservation.x_offset(), y + reservation.y_offset())));
            view.open_page(Page::Status);
            plain.open_page(Page::Status);
            let (x, y, width, height) = plain.page_panel().unwrap();
            assert_eq!(view.page_panel(), Some((x + reservation.x_offset(), y + reservation.y_offset(), width, height)));
        }
    }

    #[test]
    fn sidebar_stays_right_aligned_after_either_tab_reservation() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let (mut view, _, _) = views(orientation);
            view.set_session_route(true);
            let presentation = view.presentation();
            assert_eq!(presentation.sidebar_width, 42);
            assert_eq!(presentation.sidebar_offset, Some(108));
            assert_eq!(presentation.sidebar_offset.unwrap() + presentation.sidebar_width, 150);
        }
    }

    #[test]
    fn sidebar_scroll_admits_body_rows_and_excludes_tab_strip_and_footer() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            let (mut view, _, reservation) = views(orientation);
            view.set_session_route(true);
            let _ = view.render_typed("draft", &[], false, &[]);
            if reservation.y_offset() > 0 {
                assert!(!view.scroll_sidebar_at(108, 0, 1), "tab strip does not belong to sidebar scrolling");
            }
            assert!(view.scroll_sidebar_at(108, reservation.y_offset(), 1));
            assert!(view.scroll_sidebar_at(108, 30, 1), "last body row remains interactive");
            assert!(!view.scroll_sidebar_at(108, 31, 1), "footer does not belong to sidebar scrolling");
        }
    }
}

#[cfg(test)]
mod framed_composer_tests {
    use super::*;

    #[test]
    fn reservation_translates_live_view_geometry_and_preserves_boundaries() {
        use crate::native_tabs::{TabOrientation, TabReservation};
        // EXPLICIT expected x/y per upstream layout.ts boundary:
        //  - 105 cols: Vertical does not fit (42 + 64 = 106 boundary), falls back
        //    to Horizontal topbar -> x0 y1
        //  - 106 cols: Vertical fits -> rail -> x42 y0 (SESSION_SIDEBAR_WIDTH=42)
        for (cols, expected, expected_x, expected_y) in [
            (105, TabOrientation::Horizontal, 0, 1),
            (106, TabOrientation::Vertical, 42, 0),
        ] {
            let mut view = NativeView::new(cols, 12);
            let reservation = TabReservation::for_view(TabOrientation::Vertical, cols, 12);
            view.set_tab_reservation(reservation);
            assert_eq!(reservation.orientation, Some(expected));
            let presentation = view.presentation();
            assert_eq!(
                presentation.main_offset, expected_x,
                "main_offset must reflect the translated x offset, not reservation.width"
            );
            assert_eq!(
                reservation.x_offset(), expected_x,
                "reservation.x_offset must match presentation main_offset"
            );
            assert_eq!(
                reservation.y_offset(), expected_y,
                "reservation.y_offset must match the orientation boundary contract"
            );
            assert_eq!(presentation.sidebar_offset, None);
            let frame = view.render_typed("draft", &[], false, &[]);
            assert_eq!(frame.len(), 12, "frame height equals render height");
            assert!(
                frame.iter().all(|line| crate::native_view_layout::visit_unpadded_cells(line, cols as usize, |_, _| {}).le(&(cols as usize))),
                "every rendered line fits within cols boundary"
            );
        }
        let mut tiny = NativeView::new(1, 1);
        tiny.set_tab_reservation(TabReservation::for_view(TabOrientation::Horizontal, 1, 1));
        assert!(tiny.render_typed("x", &[], false, &[]).len() <= 1);
    }

    #[test]
    fn inline_permission_rows_are_painted_once_without_modal_regions() {
        let mut view = NativeView::new(100, 32);
        view.set_session_route(true);
        view.dialogs
            .open_permission("req-inline", "ses-inline", "Run command")
            .unwrap();
        let rows = view.dialogs.render(
            view.modal_presentation().content_width as usize,
            view.modal_content_height(),
        );
        let rendered = view.render_typed_permission("", &[], false, &[], &rows);
        let needle = "Allow once";
        assert_eq!(
            rendered.iter().filter(|line| line.contains(needle)).count(),
            1,
            "permission choice must not be duplicated by a center modal"
        );
        assert!(
            view.ui_regions().iter().all(|region| !region.modal),
            "inline permission must not publish modal hit regions"
        );
        assert!(
            view.dialogs.intercepts_input(),
            "inline placement must retain permission input focus"
        );
    }

    #[test]
    fn send_control_is_inside_home_and_session_composers() {
        for session in [false, true] {
            let mut view = NativeView::new(100, 32);
            view.set_session_route(session);
            let draft = "send fixture";
            let rows = view.render_typed(draft, &[], false, &[]);
            let (left, top, width, height) = view.composer_region(draft).unwrap();
            let found = rows
                .iter()
                .enumerate()
                .find_map(|(row, text)| {
                    let mut candidate = None;
                    let mut candidate_text = String::new();
                    crate::native_view_layout::visit_cells(
                        text,
                        view.layout.cols,
                        |column, glyph, _| {
                            if candidate_text == "[Send]" {
                                return;
                            }
                            if candidate.is_some() {
                                candidate_text.push_str(glyph);
                                if !"[Send]".starts_with(&candidate_text) {
                                    candidate = None;
                                    candidate_text.clear();
                                }
                            }
                            if candidate.is_none() && glyph == "[" {
                                candidate = Some(column);
                                candidate_text.push('[');
                            }
                        },
                    );
                    candidate.map(|column| (usize::from(column), row))
                })
                .expect("composer includes the requested Send control");
            assert!(found.1 >= usize::from(top) && found.1 < usize::from(top + height));
            assert!(found.0 >= usize::from(left) && found.0 + 6 <= usize::from(left + width));
        }
    }

    #[test]
    fn default_composer_frame_has_exact_corners_draft_and_region() {
        let mut view = NativeView::new(60, 24);
        view.set_session_route(true);
        let draft = "hello frame";
        let rows = view.render_typed(draft, &[], false, &[]);
        let (left, top, width, region_h) = view.composer_region(draft).expect("region");
        let (left, top, width) = (left as usize, top as usize, width as usize);
        let frame_h = 3usize; // one draft row + two borders (default padding 0)
        assert_eq!(width, view.presentation().main_width as usize - 4);
        assert_eq!(
            top,
            view.render_height() - 2 - frame_h,
            "composer_region follows the frame with the header inside the body budget"
        );
        assert_eq!(region_h as usize, frame_h);
        let row = |index: usize| rows[index].chars().collect::<Vec<_>>();
        assert_eq!(row(top)[left], '┌');
        assert_eq!(row(top)[left + width - 1], '┐');
        assert_eq!(row(top + frame_h - 1)[left], '└');
        assert_eq!(row(top + frame_h - 1)[left + width - 1], '┘');
        assert_eq!(row(top + 1)[left], '│');
        assert_eq!(row(top + 1)[left + width - 1], '│');
        assert!(rows[top + 1].contains(draft), "draft inside the border");
        assert!(!rows[top].contains(draft), "border row is not painted over");
    }

    #[test]
    fn composer_region_matches_rendered_frame_bottom_for_default_and_padded_styles() {
        for (appearance, draft) in [
            (
                crate::native_box_style::BoxAppearance::default(),
                "default draft",
            ),
            (
                {
                    let mut appearance = crate::native_box_style::BoxAppearance::default();
                    appearance.preset = crate::native_box_style::RadiusPreset::Rounded;
                    appearance.padding = 1;
                    appearance
                },
                "padded draft",
            ),
        ] {
            let mut view = NativeView::new(60, 24);
            view.set_session_route(true);
            view.set_box_appearance(appearance.clone());
            let rows = view.render_typed(draft, &[], false, &[]);
            let (left, top, width, height) = view.composer_region(draft).expect("composer region");
            let row = |index: usize| rows[index].chars().collect::<Vec<_>>();
            let bottom = top as usize + height as usize - 1;
            assert_eq!(
                row(top as usize)[left as usize],
                appearance
                    .effective(crate::native_box_style::BoxId::ChatComposer)
                    .corner_glyphs()
                    .top_left
            );
            assert_eq!(
                row(top as usize)[left as usize + width as usize - 1],
                appearance
                    .effective(crate::native_box_style::BoxId::ChatComposer)
                    .corner_glyphs()
                    .top_right
            );
            assert_eq!(
                row(bottom)[left as usize],
                appearance
                    .effective(crate::native_box_style::BoxId::ChatComposer)
                    .corner_glyphs()
                    .bottom_left
            );
            assert_eq!(
                row(bottom)[left as usize + width as usize - 1],
                appearance
                    .effective(crate::native_box_style::BoxId::ChatComposer)
                    .corner_glyphs()
                    .bottom_right
            );
            assert_eq!(
                bottom,
                rows.iter()
                    .position(|line| line.chars().nth(left as usize)
                        == Some(
                            appearance
                                .effective(crate::native_box_style::BoxId::ChatComposer)
                                .corner_glyphs()
                                .bottom_left
                        ))
                    .expect("rendered bottom corner")
            );
            assert!(
                bottom < rows.len() - 1,
                "composer frame must not include shared footer"
            );
        }
        let mut narrow = NativeView::new(1, 10);
        narrow.set_session_route(true);
        assert_eq!(narrow.composer_region("x"), None);
    }

    #[test]
    fn native_typed_scroll_reaches_oldest_after_repeated_wheels_and_moves_down() {
        let mut view = NativeView::new(80, 24);
        view.set_session_route(true);
        let messages: Vec<crate::native_view_chat::ChatMessage> = (0..61)
            .map(|i| {
                crate::native_view_chat::ChatMessage::text(
                    crate::native_view_chat::ChatRole::Assistant,
                    format!("entry {i:03}"),
                )
            })
            .collect();
        for _ in 0..40 {
            view.scroll_up(5);
            let _ = view.render_typed("", &messages, false, &[]);
        }
        let oldest = view.render_typed("", &messages, false, &[]);
        assert!(
            oldest.iter().any(|row| row.contains("entry 000")),
            "oldest native frame: {oldest:?}"
        );
        view.scroll_down(1);
        let next = view.render_typed("", &messages, false, &[]);
        assert!(
            next.iter().any(|row| row.contains("entry 001")),
            "one-row native frame: {next:?}"
        );
    }

    #[test]
    /// Production entrypoint replay: wheel-up/down through the ACTUAL
    /// `NativeView::render` legacy-chat pipeline that `render_content`
    /// paints in the live loop (`ChatView::render`, raw transcript strings),
    /// NOT the `render_typed` messages pipeline. 61-entry history, 55 wheel-up
    /// ticks of 3 (sixth-scroll present), then one wheel-down, then
    /// `follow_tail`, asserting the full history stays reachable and the
    /// window never freezes short of the true oldest viewport.
    fn production_render_pipeline_reaches_true_oldest_and_moves_one_step_down() {
        let mut view = NativeView::new(100, 30);
        view.set_session_route(true);
        let transcript: Vec<String> = (0..61)
            .map(|i| format!("entry {:03}: {}", i, "x".repeat(96)))
            .collect();
        // Sanity: capture the tail frame first (must show the newest entry).
        let tail = view.render("ty", &transcript, false, &[]).join("\n");
        assert!(tail.contains("entry 060"), "newest row visible at tail");
        for _ in 0..55 {
            view.scroll_up(3);
            let frame = view.render("ty", &transcript, false, &[]);
            assert_eq!(frame.len(), 30, "every render stays exactly 30 rows");
        }
        let oldest = view.render("ty", &transcript, false, &[]);
        let joined = oldest.join("\n");
        assert!(
            joined.contains("entry 000"),
            "oldest viewport after 55*3-row scroll must show entry 000, got first rows: {:?}",
            oldest.iter().take(8).collect::<Vec<_>>()
        );
        view.scroll_down(3);
        let one_down = view.render("ty", &transcript, false, &[]).join("\n");
        assert!(
            !one_down.contains("entry 000"),
            "one wheel-down from the oldest viewport must move content (repaint away from frozen entry 000)"
        );
        view.follow_tail();
        let back = view.render("ty", &transcript, false, &[]).join("\n");
        assert!(back.contains("entry 060"), "re-pin restores the true tail");
    }

    fn configured_preset_applies_rounded_corners_and_inner_padding() {
        let mut view = NativeView::new(60, 24);
        view.set_session_route(true);
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.preset = crate::native_box_style::RadiusPreset::Rounded;
        appearance.padding = 2;
        view.set_box_appearance(appearance);
        let rows = view.render_typed("rounded", &[], false, &[]);
        let (left, top, width, _) = view.composer_region("rounded").expect("region");
        let (left, top, width) = (left as usize, top as usize, width as usize);
        let row = |index: usize| rows[index].chars().collect::<Vec<_>>();
        assert_eq!(row(top)[left], '╭');
        assert_eq!(row(top)[left + width - 1], '╮');
        let frame_h = 2 + 2 * 2 + 1;
        assert_eq!(row(top + frame_h - 1)[left], '╰');
        assert_eq!(row(top + frame_h - 1)[left + width - 1], '╯');
        let pad_row: String = rows[top + 1]
            .chars()
            .skip(left + 1)
            .take(width - 2)
            .collect();
        assert!(pad_row.trim().is_empty(), "inner top padding row");
        assert!(rows[top + 3].contains("rounded"));
    }

    #[test]
    fn narrow_one_column_falls_back_and_home_border_stays_default() {
        let mut narrow = NativeView::new(1, 10);
        narrow.set_session_route(true);
        assert_eq!(narrow.render_typed("x", &[], false, &[]).len(), 10);
        assert_eq!(narrow.composer_region("x"), None);

        let mut home = NativeView::new(80, 30);
        home.set_session_route(false);
        let default_rows = home.render_typed("hi", &[], false, &[]);
        assert!(default_rows.iter().any(|row| row.contains('┃')));
        assert!(default_rows.iter().any(|row| row.contains('╹')));

        let mut rounded = NativeView::new(80, 30);
        rounded.set_session_route(false);
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.preset = crate::native_box_style::RadiusPreset::Rounded;
        rounded.set_box_appearance(appearance);
        let rounded_rows = rounded.render_typed("hi", &[], false, &[]);
        assert!(rounded_rows.iter().any(|row| row.contains('╭')));
        assert!(rounded_rows.iter().any(|row| row.contains('╰')));
        assert!(!rounded_rows.iter().any(|row| row.contains('┃')));
    }

    /// The sidebar is a local pane for `compose_frame`, while its published
    /// regions are screen-absolute. This exercises the real NativeView render
    /// path at a wide terminal and catches both coordinate conventions:
    /// regions must begin in the painted text cell, never at the pane origin,
    /// and must remain contained by the viewport.
    #[test]
    fn wide_sidebar_frame_and_regions_share_bounds_for_each_padding() {
        for padding in 0..=4u8 {
            let mut view = NativeView::new(148, 20);
            view.set_session_route(true);
            let mut appearance = crate::native_box_style::BoxAppearance::default();
            appearance.overrides.insert(
                crate::native_box_style::BoxId::Sidebar,
                crate::native_box_style::BoxOverride {
                    border_style: Some(crate::native_box_style::BorderStyle::Heavy),
                    padding: Some(padding),
                    ..Default::default()
                },
            );
            view.set_box_appearance(appearance);
            let rows = view.render_typed("draft", &[], false, &[]);
            let presentation = view.presentation();
            let left = presentation.sidebar_offset.expect("wide sidebar");
            let width = presentation.sidebar_width;
            assert_eq!(width, 42);
            let chars: Vec<Vec<char>> = rows.iter().map(|row| row.chars().collect()).collect();
            assert_eq!(chars.len(), 20);
            assert_eq!(
                chars[0][usize::from(left)],
                '┏',
                "padding {padding}: top-left"
            );
            assert_eq!(
                chars[0][usize::from(left + width - 1)],
                '┓',
                "padding {padding}: top-right"
            );
            // The shell's final row is reserved for the shared footer; the
            // actual sidebar component therefore ends at render_height - 2.
            let frame_bottom = chars.len() - 2;
            assert_eq!(
                chars[frame_bottom][usize::from(left)],
                '┗',
                "padding {padding}: bottom-left"
            );
            assert_eq!(
                chars[frame_bottom][usize::from(left + width - 1)],
                '┛',
                "padding {padding}: bottom-right"
            );

            let regions: Vec<_> = view
                .ui_regions()
                .into_iter()
                .filter(|region| region.x >= left)
                .collect();
            assert!(!regions.is_empty(), "padding {padding}: sidebar regions");
            let expected_x = left + 1 + u16::from(padding);
            let expected_width = width - 2 - 2 * u16::from(padding);
            for (index, region) in regions.into_iter().enumerate() {
                assert_eq!(region.x, expected_x, "padding {padding}: region x");
                assert_eq!(region.y, 1 + index as u16, "padding {padding}: region y");
                assert_eq!(
                    region.width, expected_width,
                    "padding {padding}: region width"
                );
                assert!(region.x < 148);
                assert!(u32::from(region.x) + u32::from(region.width) <= 148);
                assert!(
                    region.y > 0 && usize::from(region.y) < frame_bottom,
                    "padding {padding}: region avoids border rows"
                );
            }
        }
    }

    /// The manually toggled sidebar uses the same absolute placement when
    /// the pane is overlaid on a sub-wide terminal (no inline layout slot).
    /// This catches regressions where the local pane geometry is accidentally
    /// treated as a full-screen rectangle.
    #[test]
    fn toggled_sidebar_at_subwide_width_stays_contained() {
        let mut view = NativeView::new(100, 12);
        view.set_session_route(true);
        view.toggle_sidebar();
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.overrides.insert(
            crate::native_box_style::BoxId::Sidebar,
            crate::native_box_style::BoxOverride {
                border_style: Some(crate::native_box_style::BorderStyle::Double),
                padding: Some(4),
                ..Default::default()
            },
        );
        view.set_box_appearance(appearance);
        let rows = view.render_typed("draft", &[], false, &[]);
        let left = view.presentation().sidebar_offset.expect("toggled sidebar");
        let width = view.presentation().sidebar_width;
        let first: Vec<char> = rows[0].chars().collect();
        assert_eq!(first[usize::from(left)], '╔');
        assert_eq!(first[usize::from(left + width - 1)], '╗');
        let frame_bottom = rows.len() - 2;
        let bottom: Vec<char> = rows[frame_bottom].chars().collect();
        assert_eq!(bottom[usize::from(left)], '╚');
        assert_eq!(bottom[usize::from(left + width - 1)], '╝');
        assert_eq!(rows.len(), 12);
        for region in view
            .ui_regions()
            .into_iter()
            .filter(|region| region.x >= left)
        {
            assert!(u32::from(region.x) + u32::from(region.width) <= 100);
            assert!(usize::from(region.y) < frame_bottom);
            assert!(region.width > 0);
        }
    }
}

#[cfg(test)]
mod footer_layout_tests {
    use super::*;

    #[test]
    fn configurable_menu_padding_moves_content_without_moving_outer_modal() {
        let mut view = NativeView::new(100, 32);
        view.open_command_palette(false);
        let baseline = view.modal_presentation();
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.overrides.insert(
            crate::native_box_style::BoxId::Menu,
            crate::native_box_style::BoxOverride {
                padding: Some(2),
                ..Default::default()
            },
        );
        view.set_box_appearance(appearance);
        let padded = view.modal_presentation();
        assert_eq!(
            (padded.x, padded.y, padded.width),
            (baseline.x, baseline.y, baseline.width)
        );
        assert_eq!(
            padded.content_width,
            baseline.content_width.saturating_sub(4)
        );
        assert_eq!(padded.content_y, baseline.content_y + 2);
        view.render_typed("", &[], false, &[]);
        let regions: Vec<_> = view.ui_regions().into_iter().filter(|r| r.modal).collect();
        assert!(!regions.is_empty());
        assert_eq!(regions[0].x, baseline.x + 4);
        assert_eq!(regions[0].y, baseline.content_y + 2);
        assert_eq!(
            padded.content_point(regions[0].x, regions[0].y, view.modal_content_height()),
            Some((0, 0))
        );
        assert_eq!(
            padded.content_point(
                padded.content_x - 1,
                padded.content_y,
                view.modal_content_height()
            ),
            None
        );
        assert!(regions
            .iter()
            .all(|r| r.y < 31 && r.x + r.width <= padded.x + padded.width));
        view.close_command_palette();
        assert_eq!(
            view.modal_presentation().content_width,
            baseline.content_width,
            "Menu override must not change Dialog padding"
        );
    }

    #[test]
    fn configurable_dialog_padding_bounds_permission_content_and_footer() {
        let mut view = NativeView::new(80, 20);
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.overrides.insert(
            crate::native_box_style::BoxId::Dialog,
            crate::native_box_style::BoxOverride {
                padding: Some(3),
                ..Default::default()
            },
        );
        view.set_box_appearance(appearance);
        let modal = view.modal_presentation();
        let content_height = view.modal_content_height();
        assert_eq!(modal.content_x, modal.x + 5);
        assert_eq!(modal.content_y, modal.y + 4);
        assert_eq!(
            modal.content_point(modal.content_x + 2, modal.content_y + 1, content_height),
            Some((2, 1))
        );
        assert_eq!(
            modal.content_point(modal.x + 1, modal.content_y + 1, content_height),
            None
        );
        assert_eq!(
            modal.content_point(modal.content_x, 19, content_height),
            None
        );
        for (cols, rows) in [(0, 0), (4, 3), (10, 5)] {
            view.resize(cols, rows);
            let modal = view.modal_presentation();
            assert!(modal.content_x <= cols);
            assert!(view.modal_content_height() == 0);
        }
    }

    #[test]
    fn modal_underlay_snapshot_preserves_composed_cells_and_clears_after_close() {
        let mut view = NativeView::new(80, 20);
        view.set_session_route(true);
        let baseline = view.render_typed("draft", &[], false, &[]);
        assert!(view.modal_underlay_rows().is_empty());
        let overlay = vec!["Visible modal".to_owned(), "Modal action".to_owned()];
        let painted = view.render_typed("draft", &[], false, &overlay);
        let underlay = view.modal_underlay_rows();
        assert_eq!(underlay.len(), baseline.len());
        assert_eq!(
            underlay[..baseline.len() - 1],
            baseline[..baseline.len() - 1]
        );
        assert_ne!(painted, underlay);
        view.render_typed("draft", &[], false, &[]);
        assert!(view.modal_underlay_rows().is_empty());
    }

    #[test]
    fn project_directory_and_loader_share_only_the_footer_row() {
        let mut view = NativeView::new(72, 20);
        view.set_session_route(true);
        view.set_project_directory("/tmp/工作🦀/project".to_owned());
        view.set_loading(true); // deprecated no-op: frame argument is authority
        let rows = view.render_typed("draft", &[], true, &[]);
        assert_eq!(rows.len(), 20);
        assert!(rows[19].contains("/tmp/") || rows[19].contains("project"));
        assert!(rows[..19].iter().any(|row| row.contains("Thinking")));
        assert!(!rows[19].contains("Thinking"));
        let idle = view.render_typed("draft", &[], false, &[]);
        assert!(!idle[19].contains("Thinking"));
    }

    // Usability contract proposal (RED): active Thinking belongs to the
    // bounded conversation/composer column, not the global shell row beneath
    // the inline right sidebar.  The implementation wave must supersede the
    // older footer-only assertions above through independent review; these
    // tests intentionally fail against the current renderer.
    #[test]
    fn usability_thinking_is_visible_in_main_pane_at_wide_140() {
        let mut view = NativeView::new(140, 20);
        view.set_session_route(true);
        let rows = view.render_typed("draft", &[], true, &[]);
        let main_width = usize::from(view.presentation().main_width);
        let thinking = rows
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.find("Thinking").map(|x| (x, y)))
            .expect("busy native session must expose Thinking");
        assert!(
            thinking.0 < main_width,
            "Thinking must be in main pane: {thinking:?} main_width={main_width}"
        );
        assert!(
            thinking.1 < rows.len() - 1,
            "Thinking must not be the global footer row"
        );
    }

    #[test]
    fn usability_thinking_is_visible_in_main_pane_at_wide_121() {
        let mut view = NativeView::new(121, 20);
        view.set_session_route(true);
        let rows = view.render_typed("draft", &[], true, &[]);
        let main_width = usize::from(view.presentation().main_width);
        let thinking = rows
            .iter()
            .enumerate()
            .find_map(|(y, row)| row.find("Thinking").map(|x| (x, y)))
            .expect("busy native session must expose Thinking");
        assert!(
            thinking.0 < main_width,
            "Thinking must be in main pane: {thinking:?} main_width={main_width}"
        );
        assert!(
            thinking.1 < rows.len() - 1,
            "Thinking must not be the global footer row"
        );
    }

    #[test]
    fn usability_narrow_overlay_hides_thinking_behind_right_sidebar() {
        let mut view = NativeView::new(120, 20);
        view.set_session_route(true);
        view.toggle_sidebar();
        let idle = view.render_typed("draft", &[], false, &[]);
        let rows = view.render_typed("draft", &[], true, &[]);
        let right_start = usize::from(view.presentation().sidebar_offset.expect("toggled sidebar"));
        assert!(
            rows.iter().zip(idle.iter()).all(|(busy_row, idle_row)| {
                busy_row
                    .chars()
                    .skip(right_start)
                    .eq(idle_row.chars().skip(right_start))
            }),
            "busy Thinking must not paint or alter any cell in the narrow right overlay"
        );
        assert!(
            rows.iter().any(|row| {
                row.chars()
                    .take(right_start)
                    .collect::<String>()
                    .contains("Thinking")
            }),
            "busy status must remain visible in the main pane"
        );
    }

    #[test]
    fn modal_bounds_reserve_footer_and_hit_regions_stay_above_it() {
        let mut view = NativeView::new(80, 12);
        view.set_session_route(true);
        let frame = view.render_typed("", &[], true, &["Accept".to_owned(), "Cancel".to_owned()]);
        assert_eq!(frame.len(), 12);
        assert!(frame[..11].iter().any(|row| row.contains("Thinking")));
        assert!(!frame[11].contains("Thinking"));
        let modal = view.modal_presentation();
        assert!(
            modal
                .y
                .saturating_add(modal.content_y.saturating_sub(modal.y))
                < 11
        );
        assert!(view
            .ui_regions()
            .iter()
            .filter(|r| r.modal)
            .all(|r| r.y < 11));
    }

    #[test]
    fn oversized_generic_overlay_preserves_footer_and_has_no_footer_hit_region() {
        let mut view = NativeView::new(48, 7);
        view.set_session_route(true);
        view.set_overlay_roles(vec![crate::native_ui_style::Role::Selected; 64]);
        let overlay = (0..64)
            .map(|index| format!("Choice {index}"))
            .collect::<Vec<_>>();
        let rows = view.render_typed("", &[], true, &overlay);
        assert_eq!(rows.len(), 7);
        assert!(
            rows[..6].iter().any(|row| row.contains("Thinking")),
            "main Thinking must remain visible"
        );
        assert!(
            !rows[6].contains("Thinking"),
            "footer must not own Thinking"
        );
        assert!(view
            .ui_regions()
            .iter()
            .filter(|region| region.modal)
            .all(|region| region.y < 6));
    }

    #[test]
    fn unknown_context_limit_does_not_render_zero_percentage_capacity() {
        let mut view = NativeView::new(140, 24);
        view.set_context_window(None);
        view.set_usage(10, 4, None, None);
        view.set_session_route(true);
        let rows = view.render_typed("", &[], false, &[]).join("\n");
        assert!(!rows.contains("window=0"));
        assert!(!rows.contains("0%"));
    }

    #[test]
    fn active_busy_frame_has_footer_loader_only_not_composer_loader_region() {
        let mut view = NativeView::new(80, 20);
        view.set_session_route(true);
        let rows = view.render_typed("draft", &[], true, &[]);
        assert_eq!(
            rows.iter().filter(|row| row.contains("Thinking")).count(),
            1
        );
        assert!(rows[..19].iter().any(|row| row.contains("Thinking")));
        assert!(!rows[19].contains("Thinking"));
        assert!(view.composer_region("draft").is_some());
        let idle = view.render_typed("draft", &[], false, &[]);
        assert!(idle.iter().all(|row| !row.contains("Thinking")));
        assert!(view.composer_region("draft").is_some());
    }

    #[test]
    fn compact_busy_frames_keep_composer_region_for_loader_fallback() {
        for height in [1_u16, 2, 3] {
            let mut view = NativeView::new(80, height);
            view.set_session_route(true);
            let rows = view.render_typed("draft", &[], true, &[]);
            assert_eq!(rows.len(), usize::from(height));
            if height >= 4 {
                assert!(
                    rows.iter().any(|row| row.contains("Thinking")),
                    "main Thinking remains visible at height {height}"
                );
            }
            assert!(view.composer_region("draft").is_some(), "height {height}");
        }
    }

    #[test]
    fn footer_directory_tail_keeps_selected_project_name_visible() {
        let mut first = NativeView::new(64, 20);
        first.set_workspace("/very/long/root/selected-alpha".to_owned());
        let alpha = first.render_typed("", &[], false, &[]).pop().unwrap();
        let mut second = NativeView::new(64, 20);
        second.set_workspace("/very/long/root/selected-beta".to_owned());
        let beta = second.render_typed("", &[], false, &[]).pop().unwrap();
        assert!(alpha.contains("selected-alpha"));
        assert!(beta.contains("selected-beta"));
        assert_ne!(alpha, beta);
        assert_eq!(alpha.chars().count(), 64);
        let mut narrow = NativeView::new(18, 20);
        narrow.set_workspace("/長い/プロジェクト🦀/名前".to_owned());
        let row = narrow.render_typed("", &[], false, &[]).pop().unwrap();
        assert_eq!(row.chars().count(), 18);
    }

    #[test]
    fn footer_short_directory_keeps_status_at_the_right_edge() {
        let mut view = NativeView::new(64, 20);
        view.set_workspace("/x".to_owned());
        let idle = view.render_typed("", &[], false, &[]).pop().unwrap();
        assert_eq!(idle.chars().count(), 64);
        assert!(idle.ends_with(&format!("{}  ", env!("CARGO_PKG_VERSION"))));
        let busy = view.render_typed("", &[], true, &[]).pop().unwrap();
        assert_eq!(busy.chars().count(), 64);
        assert!(busy.ends_with(&format!("{}  ", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn borderless_composer_region_reclaims_unframed_content_height() {
        let mut view = NativeView::new(80, 24);
        view.set_session_route(true);
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.overrides.insert(
            crate::native_box_style::BoxId::ChatComposer,
            crate::native_box_style::BoxOverride {
                border_style: Some(crate::native_box_style::BorderStyle::None),
                padding: Some(2),
                ..Default::default()
            },
        );
        view.set_box_appearance(appearance);
        let rows = view.render_typed("draft", &[], false, &[]);
        let region = view
            .composer_region("draft")
            .expect("borderless composer region");
        assert_eq!(region.2, view.presentation().main_width - 4);
        assert!(rows
            .iter()
            .all(|row| !row.contains('┌') && !row.contains('┐')));
        assert!(region.1 + region.3 <= rows.len() as u16);
    }

    #[test]
    fn narrow_conversation_keeps_confirmed_model_and_provider_visible() {
        let mut view = NativeView::new(100, 32);
        view.set_session_route(true);
        view.set_model(Some("acme/two-client-fixture".to_owned()));
        view.set_composer_model("Two-client fixture", "acme");
        view.set_workspace("/selected/project".to_owned());
        let messages = [crate::native_view_chat::ChatMessage::text(
            crate::native_view_chat::ChatRole::Assistant,
            "first reply",
        )];
        let rows = view.render_typed("draft", &messages, false, &[]);
        let joined = rows.join("\n");
        assert!(joined.contains("Two-client fixture"));
        assert!(joined.contains("acme"));
        assert!(joined.contains("first reply"));
        assert!(rows
            .last()
            .is_some_and(|row| row.contains("/selected/project")));
        assert!(rows.iter().any(|row| row.contains("> draft")));
    }

    #[test]
    fn composer_metadata_renders_only_observed_effort() {
        let mut view = NativeView::new(100, 32);
        view.set_composer_agent(Some("build".into()), None);
        view.set_composer_model("fixture", "openai");
        view.set_composer_effort(Some("high".into()));
        assert!(view
            .render_typed("draft", &[], false, &[])
            .join("\n")
            .contains("high"));
        view.set_composer_effort(None);
        assert!(!view
            .render_typed("draft", &[], false, &[])
            .join("\n")
            .contains("high"));
    }
}

#[cfg(test)]
mod composer_metadata_regression_tests {
    use super::*;

    fn configured(width: u16, model: &str, provider: &str) -> NativeView {
        let mut view = NativeView::new(width, 32);
        view.set_session_route(true);
        view.set_composer_agent(Some("builder".into()), Some("balanced".into()));
        view.set_composer_model(model, provider);
        view.set_composer_effort(Some("high".into()));
        view
    }

    #[test]
    fn provider_fallback_boundaries_and_minimum_model_width_match_upstream() {
        let model = format!("{}x", "model".repeat(11));
        for (width, expected, expected_cells, absent) in [
            (115, format!("builder · {model} provider / vendor · balanced · high"), 102, None),
            (114, format!("builder · {model} vendor · balanced · high"), 91, None),
            (104, format!("builder · {model} vendor · balanced · high"), 91, None),
            (103, format!("builder · {model} · balanced · high"), 84, Some("vendor")),
            (44, "builder · modelmod… · balanced · high".to_owned(), 37, Some("provider")),
            (43, "modelmodelmodel… · balanced · high".to_owned(), 34, Some("builder")),
        ] {
            let mut view = configured(width, &model, "provider / vendor");
            let actual = view.composer_metadata();
            assert_eq!(actual, expected, "width {width}");
            assert_eq!(prompt_metadata_cells(&actual), expected_cells, "width {width}");
            if let Some(absent) = absent {
                assert!(!actual.contains(absent), "width {width}: {actual}");
            }
        }
    }

    #[test]
    fn unicode_measurement_truncation_and_provider_fallback_are_cell_aware() {
        assert_eq!(prompt_metadata_cells("界e\u{301}👩‍💻🇺🇳"), 7);
        assert_eq!(truncate_prompt_metadata("界e\u{301}👩‍💻Z", 4), "界e\u{301}…");

        let mut view = NativeView::new(56, 32);
        view.set_composer_agent(Some("builder".into()), None);
        view.set_composer_model("model123456789012345", "供应商名称 / 外部");
        assert_eq!(view.composer_metadata(), "builder · model123456789012345 外部");
    }

    #[test]
    fn send_hitbox_edges_and_home_cursor_follow_composer_geometry() {
        let mut session = configured(100, "model", "provider");
        let _rows = session.render_typed("draft", &[], false, &[]);
        let (x, y, width) = session.send_button_region("draft").expect("send hitbox");
        assert_eq!(width, 6);
        assert!(session.send_button_hit("draft", x, y));
        assert!(session.send_button_hit("draft", x + width - 1, y));
        assert!(!session.send_button_hit("draft", x.saturating_sub(1), y));
        assert!(!session.send_button_hit("draft", x + width, y));
        assert!(!session.send_button_hit("draft", x, y.saturating_add(1)));

        let mut home = NativeView::new(100, 32);
        home.set_session_route(false);
        home.set_composer_model("model", "provider");
        let draft = "界e\u{301}";
        let _rows = home.render_typed(draft, &[], false, &[]);
        let (left, top, _, _) = home.composer_region(draft).expect("home composer");
        assert_eq!(home.home_cursor(draft), Some((left + 6, top + 1)));
    }
}

fn prompt_metadata_cells(text: &str) -> usize {
    let mut cells = 0;
    crate::native_view_layout::visit_unpadded_cells(text, text.len(), |_, width| {
        cells += width;
    });
    cells
}

fn truncate_prompt_metadata(text: &str, width: usize) -> String {
    if prompt_metadata_cells(text) <= width {
        return text.to_owned();
    }
    match width {
        0 => return String::new(),
        1 => return "…".to_owned(),
        _ => {}
    }
    let mut output = String::new();
    crate::native_view_layout::visit_unpadded_cells(text, width - 1, |glyph, _| {
        output.push_str(glyph);
    });
    while output.chars().last().is_some_and(char::is_whitespace) {
        output.pop();
    }
    output.push('…');
    output
}

fn padded(line: &str, width: u16) -> String {
    let padding = (width / 2).min(2);
    format!(
        "{}{}{}",
        " ".repeat(padding as usize),
        fit(line, width.saturating_sub(padding.saturating_mul(2))),
        " ".repeat(padding as usize)
    )
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
                "GET /api/models (picker via /models or Commands)".to_owned()
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
        // Wide viewport: the status projection and session listing render in
        // the optional 42-column sidebar (`cols > 120`), which is the only
        // place they are painted after the main-column parity change.
        let mut view = NativeView::new(160, 30);
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
    fn native_view_applies_authenticated_session_mcp_projection_to_sidebar() {
        let mut view = NativeView::new(160, 30);
        view.set_session_route(true);
        view.set_session(Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned()));
        let session_projection = serde_json::json!({
            "status": {
                "alpha": {"status": "connected"},
                "beta": {"status": "connected"}
            },
            "included": {"alpha": false, "beta": true}
        });
        view.apply_mcp(&session_projection)
            .expect("authenticated session MCP response should reach the sidebar");
        let frame = view.render("draft", &[], false, &[]).join("\n");
        assert!(frame.contains("alpha"));
        assert!(frame.to_ascii_lowercase().contains("excluded"));
        assert!(frame.contains("beta"));
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
    // Contract supersession: the current explicit upstream-parity requirement
    // replaces banner/workspace/thirds placement, preserving bounded transcript
    // and composer priority. See upstream-ui-contract-adjudication-v1.
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
            frame[0].trim().is_empty(),
            "upstream scrollbox starts with a spacer: {frame0:?}",
            frame0 = frame[0]
        );
        assert!(
            !center(0).contains("real-project"),
            "workspace label must not ride the chrome row: {:?}",
            center(0)
        );
        // Row 1: workspace header shows the real caller path.
        assert!(
            center(1).trim().is_empty(),
            "no permanent workspace header in the upstream scrollbox: {:?}",
            center(1)
        );
        // Rows 2..=28: empty transcript viewport — blank center cells,
        // no fabricated filler, no prompt creeping up under the header.
        for i in 2..27 {
            assert_eq!(
                center(i).trim(),
                "",
                "empty transcript viewport on row {i}: {:?}",
                center(i)
            );
        }
        // Row 29: composer prompt on the last frame row.
        assert!(
            center(27).trim_start().starts_with("> "),
            "composer above model metadata and upstream bottom padding: {:?}",
            center(27)
        );

        // Tiny frame (2 rows): whole-frame chat priority — workspace
        // header and draft composer stay visible, no chrome row steals
        // the last usable row (available-later contract).
        let mut tiny = NativeView::new(120, 2);
        tiny.set_workspace("/ws".to_owned());
        let frame = tiny.render("draft here", &[], false, &[]);
        assert_eq!(frame.len(), 2);
        assert!(
            frame[0].trim().is_empty(),
            "tiny transcript spacer remains blank: {frame0:?}",
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
            frame[0].trim_start().starts_with("> "),
            "height-1 keeps composer priority: {frame0:?}",
            frame0 = frame[0]
        );
    }

    #[test]
    fn home_preserves_multiline_draft_and_does_not_reserve_sidebar() {
        let mut view = NativeView::new(160, 30);
        view.set_session_route(false);
        let frame = view.render("first line\n第二行", &[], false, &[]);
        assert!(frame.iter().any(|row| row.contains("first line")));
        assert!(frame.iter().any(|row| row.contains("第二行")));
        assert_eq!(view.presentation().main_width, 160);
        assert_eq!(view.presentation().sidebar_width, 0);
        assert!(!frame.iter().any(|row| row.contains("Sessions")));
    }

    #[test]
    fn narrow_sidebar_overlays_and_hide_reclaims_wide_main_width() {
        let mut view = NativeView::new(120, 30);
        assert_eq!(view.presentation().sidebar_width, 0);
        view.toggle_sidebar();
        assert_eq!(view.presentation().sidebar_offset, Some(78));
        assert_eq!(view.presentation().main_width, 120);
        view.resize(160, 30);
        assert_eq!(view.presentation().main_width, 118);
        view.toggle_sidebar();
        assert_eq!(view.presentation().main_width, 160);
        view.set_parent_session(true);
        view.toggle_sidebar();
        assert_eq!(view.presentation().sidebar_width, 0);
        assert_eq!(view.presentation().main_width, 160);
    }

    #[test]
    fn modal_and_manual_sidebar_rows_stay_within_tiny_ascii_viewports() {
        for cols in [0, 1, 2, 3, 4, 5, 40, 60, 120] {
            let mut view = NativeView::new(cols, 8);
            view.toggle_sidebar();
            let sidebar = view.render("draft", &[], false, &[]);
            assert!(
                sidebar
                    .iter()
                    .all(|line| line.chars().count() == cols as usize),
                "sidebar {cols}: {sidebar:?}"
            );
            let modal = view.render(
                "draft",
                &[],
                false,
                &["API key (OpenAI)".to_owned(), "masked".to_owned()],
            );
            assert!(
                modal
                    .iter()
                    .all(|line| line.chars().count() == cols as usize),
                "modal {cols}: {modal:?}"
            );
            assert_eq!(modal.len(), if cols == 0 { 0 } else { 8 });
        }
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
        // The binding tuple is exposed through the reachable Status page,
        // rather than the superseded permanently visible status column.
        view.open_page(Page::Status);
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
        view2.open_page(Page::Status);
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
        view.open_page(Page::Status);
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
        view.open_page(Page::Status);
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
