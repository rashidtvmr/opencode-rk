#![forbid(unsafe_code)]
//! Native event-loop owner (gap G4/G8).
//!
//! One owner joins the pure-state orphans into a single loop:
//! `NativeApp` (focus/view/freshness/scheduler) + `HostLoop` (bounded queue,
//! alt-screen obligation, frame coalescing) + transcript/timeline/composer
//! routing counters + onboarding [`SetupStep`] phase gate.
//!
//! std only, no IO, no threads, no FFI, no `crate::` imports: compiles both
//! as `crate::native_host` (once the integrator adds `mod native_host;` to
//! `main.rs`) and standalone via `rustc --edition 2021 --test
//! crates/cli/src/native_host.rs`.
//!
//! Ownership (exactly one caller per orphan, enforced by construction):
//! - `tui_entry::run` constructs [`NativeHost`] and drives
//!   [`NativeHost::step`] in its interactive loop; no other caller may own
//!   the loop. See [`describe_ownership`].
//! - Pure-state modules stay pure: `native_app`, `terminal_host`
//!   (`HostLoop`), `native_transcript`, `native_timeline`,
//!   `native_composer`, `native_shell`, `onboarding` expose state; only
//!   `NativeHost` combines them into loop decisions. Host keeps routing
//!   counters only — retained buffers stay in their owner modules.
//!
//! Integrator wiring (not this lane): add `mod native_host;` to `main.rs`,
//! then in `tui_entry::run` replace/augment the line-only
//! `interactive_loop` with `NativeHost::new(cfg)` + `step()` per event.
//!
//! Pane/overlay integration (P04, upstream pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! - `packages/tui/src/component/session-frame.tsx:386-398` — a manual sidebar
//!   below the 120-cell wide threshold paints as an overlay and reserves no
//!   transcript width; `routes/session/index.tsx:253-258` closes it with
//!   `escape,ctrl+c`, so those keys must reach the overlay before the global
//!   quit latch.
//! - `packages/tui/src/context/panel.tsx:15-16` — a plugin panel is fullscreen
//!   below `SESSION_PANEL_SPLIT_MIN_WIDTH` (108); `app.tsx:615-619` hides tabs
//!   only while the panel is fullscreen.
//! - `packages/tui/src/component/session-frame.tsx:287-318` — the pane focus
//!   guard consumes Enter and click releases aimed at a covered permission
//!   prompt but forwards wheel events inside the transcript viewport.
//! - `packages/tui/src/ui/dialog.tsx:320-324,345-347` — while a modal stack is
//!   non-empty its keymap layer owns keys; the host therefore blocks Submit
//!   and routes Escape/Ctrl+C to the dialog instead of quitting.
//!
//! The detailed state machines live in `native_pane_resize` and
//! `native_modal_layer`; this file mirrors only the scalar thresholds it
//! needs because it must stay standalone-compilable (`rustc --test
//! crates/cli/src/native_host.rs`).

/// Minimum viewport for the full multi-column shell (mirrors
/// `native_app::MIN_FULL_WIDTH/HEIGHT`; duplicated so this file is
/// standalone-compilable with no `crate::` dependency).
pub const MIN_FULL_WIDTH: u16 = 80;
/// Minimum viewport height for the full shell.
pub const MIN_FULL_HEIGHT: u16 = 24;
/// Upper bound on coalesced pending frame requests (mirrors
/// `native_app::MAX_PENDING_FRAMES` / `terminal_host` re-export).
pub const MAX_PENDING_FRAMES: u32 = 16;
/// Upper bound on retained view-transition history entries.
pub const MAX_HISTORY: usize = 32;
/// Upper bound on routed token deltas counted per session (host counts,
/// `native_transcript` retains).
pub const MAX_ROUTED_DELTAS: u64 = 65_536;
/// Upper bound on routed timeline items counted per session (host counts,
/// `native_timeline` retains).
pub const MAX_ROUTED_ITEMS: usize = 4_096;
/// Narrow-sidebar overlay ceiling: `width - verticalTabs > 120` is the wide
/// (docked) test (`component/session-frame.tsx:107`). Mirrors
/// `native_view_layout::SESSION_WIDE_THRESHOLD`.
pub const SIDEBAR_NARROW_MAX_WIDTH: u16 = 120;
/// Plugin-panel split minimum: preferred content 64 + minimum content 44
/// (`ui/layout.ts:9`, `context/panel.tsx:15`). Mirrors
/// `native_view_layout::SESSION_PANEL_SPLIT_MIN_WIDTH`.
pub const PANEL_SPLIT_MIN_WIDTH: u16 = 108;
/// Docked sidebar width (`ui/layout.ts:1`).
pub const SIDEBAR_WIDTH: u16 = 42;

/// Focusable shell region. Exactly one holds focus at a time (mirrors
/// `native_app::Focus` so the host loop needs no crate import).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Focus {
    Composer,
    Transcript,
    Sidebar,
    Status,
}

impl Focus {
    /// Next region in Tab order.
    #[must_use]
    pub const fn next(self) -> Focus {
        match self {
            Focus::Composer => Focus::Transcript,
            Focus::Transcript => Focus::Sidebar,
            Focus::Sidebar => Focus::Status,
            Focus::Status => Focus::Composer,
        }
    }
}

/// Whole-shell view. Only [`AppView::Actionable`] accepts session input
/// (mirrors `native_app::AppView`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AppView {
    Empty,
    Loading,
    Offline,
    Error,
    Actionable,
}

impl AppView {
    /// True only for the interactive view.
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        matches!(self, AppView::Actionable)
    }

    /// Human label; all five differ so states stay distinguishable.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            AppView::Empty => "empty",
            AppView::Loading => "loading",
            AppView::Offline => "offline",
            AppView::Error => "error",
            AppView::Actionable => "ready",
        }
    }
}

/// First-run onboarding phase gate (mirrors `onboarding::SetupStep`
/// ordering; host advances only via [`SetupStep::next`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SetupStep {
    Welcome,
    ProviderSelect,
    CredentialEntry,
    ModelSelect,
    Done,
}

impl SetupStep {
    /// Next step, or `None` once [`SetupStep::Done`].
    #[must_use]
    pub const fn next(self) -> Option<SetupStep> {
        match self {
            SetupStep::Welcome => Some(SetupStep::ProviderSelect),
            SetupStep::ProviderSelect => Some(SetupStep::CredentialEntry),
            SetupStep::CredentialEntry => Some(SetupStep::ModelSelect),
            SetupStep::ModelSelect => Some(SetupStep::Done),
            SetupStep::Done => None,
        }
    }

    /// True only for the terminal step.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, SetupStep::Done)
    }
}

/// Host phase: onboarding gates the main shell until setup is done.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HostPhase {
    Onboarding,
    Main,
}

/// Events the single loop owner routes. Pure data; IO stays in `tui_entry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostEvent {
    Key(char),
    Resize {
        cols: u16,
        rows: u16,
    },
    Paste(String),
    Submit(String),
    TokenDelta {
        bytes: usize,
    },
    TimelineItem,
    DaemonLive,
    DaemonDown,
    OnboardingAdvance,
    OnboardingError,
    /// Enter pressed outside the prompt handler. The pane focus guard may eat
    /// it (`component/session-frame.tsx:287-318`).
    Enter,
    /// Focus moved into the right pane (`focusRightPane`).
    PaneFocusRight,
    /// Focus returned to the session pane (`focusSession`).
    PaneFocusSession,
    /// Mouse wheel at frame cell `(x, y)`.
    Wheel {
        x: u16,
        y: u16,
    },
    /// A modal layer opened (`ui/dialog.tsx:400-419`).
    ModalOpen,
    /// Escape delivered to the modal layer (`ui/dialog.tsx:349-363`).
    ModalEscape,
    /// Ctrl+C delivered to the modal layer (`ui/dialog.tsx:364-384`).
    ModalCtrlC,
    /// `session.sidebar.toggle` (`component/session-frame.tsx:181-201`).
    ToggleSidebar,
    /// A plugin panel opening/closing (`context/panel.tsx:23-42`).
    TogglePanel,
}

/// Dispatch outcome for one stepped event.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HostAction {
    Quit,
    CycleFocus,
    RequestFrame,
    Ignored,
    NeedsOnboarding,
}

/// Loop construction config, supplied by the `tui_entry::run` caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HostConfig {
    pub cols: u16,
    pub rows: u16,
    pub skip_onboarding: bool,
}

impl HostConfig {
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            skip_onboarding: false,
        }
    }
}

/// Single event-loop owner: app shell + host loop + transcript/timeline/
/// composer routing + onboarding phase. Pure state; the caller (`tui_entry`)
/// owns all IO around [`NativeHost::step`].
#[derive(Clone, Debug)]
pub struct NativeHost {
    focus: Focus,
    view: AppView,
    live: bool,
    compact: bool,
    pending_frames: u32,
    quit: bool,
    phase: HostPhase,
    setup: SetupStep,
    sidebar_visible: bool,
    history: Vec<AppView>,
    routed_deltas: u64,
    routed_items: usize,
    dropped_routes: u64,
    cols: u16,
    rows: u16,
    /// Explicit `session.sidebar.toggle` state; separate from the automatic
    /// wide-viewport sidebar (`component/session-frame.tsx:108-113`).
    manual_sidebar: bool,
    /// Active plugin panel (`context/panel.tsx:12-24`).
    panel_open: bool,
    /// Measured panel width (0 when closed). Follows the terminal width, as
    /// `app.tsx:623` feeds `panels.setWidth(...)`.
    panel_width: u16,
    /// Requested docked presentation; below the split minimum the panel is
    /// fullscreen regardless (`context/panel.tsx:15-16`).
    panel_dock_requested: bool,
    /// Whether the tabs rail exists at all (`app.tsx:613-619`).
    tabs_available: bool,
    /// `activePane() === "right"` (`component/session-frame.tsx:87`).
    right_pane_focus: bool,
    /// A modal stack is non-empty (`ui/dialog.tsx:345-347`).
    modal_open: bool,
    /// Wheel events forwarded to the transcript while the guard is up.
    transcript_scrolls: u32,
}

impl NativeHost {
    /// Build the loop owner. Starts in `Onboarding` unless
    /// `cfg.skip_onboarding`; view starts `Loading`, never live.
    #[must_use]
    pub fn new(cfg: HostConfig) -> Self {
        let compact = cfg.cols < MIN_FULL_WIDTH || cfg.rows < MIN_FULL_HEIGHT;
        Self {
            focus: Focus::Composer,
            view: AppView::Loading,
            live: false,
            compact,
            pending_frames: 1,
            quit: false,
            phase: if cfg.skip_onboarding {
                HostPhase::Main
            } else {
                HostPhase::Onboarding
            },
            setup: SetupStep::Welcome,
            sidebar_visible: !compact,
            history: vec![AppView::Loading],
            routed_deltas: 0,
            routed_items: 0,
            dropped_routes: 0,
            cols: cfg.cols,
            rows: cfg.rows,
            manual_sidebar: false,
            panel_open: false,
            panel_width: cfg.cols,
            panel_dock_requested: true,
            tabs_available: true,
            right_pane_focus: false,
            modal_open: false,
            transcript_scrolls: 0,
        }
    }

    /// Route one event; returns the loop action. Exactly-once owner of all
    /// orphan transitions: focus cycle (Tab, sidebar skipped when compact),
    /// quit latch (`q`/Ctrl-C/Ctrl-D), resize (recompute compact, redirect
    /// stranded sidebar focus), live/disconnect badge flips, onboarding
    /// advance gate, transcript/timeline/composer routing counters.
    pub fn step(&mut self, event: HostEvent) -> HostAction {
        match event {
            HostEvent::Key(c) => self.step_key(c),
            HostEvent::Resize { cols, rows } => {
                self.resize(cols, rows);
                HostAction::RequestFrame
            }
            HostEvent::Paste(_) => {
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::Submit(_) => {
                if self.phase != HostPhase::Main {
                    return HostAction::NeedsOnboarding;
                }
                // A modal layer owns keys while it is open, and the pane focus
                // guard keeps Enter away from a covered permission prompt.
                if self.modal_open || self.right_pane_focus {
                    return HostAction::Ignored;
                }
                if self.view.is_actionable() && self.focus == Focus::Composer {
                    self.mark_frame();
                    HostAction::RequestFrame
                } else {
                    HostAction::Ignored
                }
            }
            HostEvent::TokenDelta { bytes } => {
                if bytes == 0 || self.routed_deltas >= MAX_ROUTED_DELTAS {
                    self.dropped_routes = self.dropped_routes.saturating_add(1);
                    return HostAction::Ignored;
                }
                self.routed_deltas += 1;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::TimelineItem => {
                if self.routed_items >= MAX_ROUTED_ITEMS {
                    self.dropped_routes = self.dropped_routes.saturating_add(1);
                    return HostAction::Ignored;
                }
                self.routed_items += 1;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::DaemonLive => {
                self.live = true;
                self.set_view(AppView::Actionable);
                HostAction::RequestFrame
            }
            HostEvent::DaemonDown => {
                self.live = false;
                self.set_view(AppView::Offline);
                HostAction::RequestFrame
            }
            HostEvent::OnboardingAdvance => {
                if self.phase == HostPhase::Main {
                    return HostAction::Ignored;
                }
                match self.setup.next() {
                    Some(next) => {
                        self.setup = next;
                        self.mark_frame();
                        HostAction::NeedsOnboarding
                    }
                    None => {
                        self.setup = SetupStep::Done;
                        self.phase = HostPhase::Main;
                        self.mark_frame();
                        HostAction::RequestFrame
                    }
                }
            }
            HostEvent::OnboardingError => {
                if self.phase == HostPhase::Main {
                    return HostAction::Ignored;
                }
                self.mark_frame();
                HostAction::NeedsOnboarding
            }
            HostEvent::Enter => {
                if self.modal_open || self.right_pane_focus {
                    return HostAction::Ignored;
                }
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::PaneFocusRight => {
                if self.modal_open {
                    return HostAction::Ignored;
                }
                self.right_pane_focus = true;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::PaneFocusSession => {
                self.right_pane_focus = false;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::Wheel { x, y } => {
                if self.modal_open {
                    return HostAction::Ignored;
                }
                if self.right_pane_focus && !self.transcript_contains(x, y) {
                    return HostAction::Ignored;
                }
                self.transcript_scrolls = self.transcript_scrolls.saturating_add(1);
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::ModalOpen => {
                self.modal_open = true;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::ModalEscape | HostEvent::ModalCtrlC => {
                if !self.modal_open {
                    return HostAction::Ignored;
                }
                self.modal_open = false;
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::ToggleSidebar => {
                if self.modal_open {
                    return HostAction::Ignored;
                }
                self.manual_sidebar = !self.manual_sidebar;
                if self.manual_sidebar {
                    // Opening the sidebar closes an active panel
                    // (`component/session-frame.tsx:121-127,195-200`).
                    self.panel_open = false;
                }
                self.mark_frame();
                HostAction::RequestFrame
            }
            HostEvent::TogglePanel => {
                if self.modal_open {
                    return HostAction::Ignored;
                }
                self.panel_open = !self.panel_open;
                if self.panel_open {
                    // Opening a panel closes the manual sidebar
                    // (`component/session-frame.tsx:93-106`).
                    self.manual_sidebar = false;
                }
                self.mark_frame();
                HostAction::RequestFrame
            }
        }
    }

    fn step_key(&mut self, c: char) -> HostAction {
        match c {
            'q' => {
                self.quit = true;
                HostAction::Quit
            }
            '\x1b' => self.escape(),
            '\r' | '\n' => {
                if self.modal_open || self.right_pane_focus {
                    return HostAction::Ignored;
                }
                self.mark_frame();
                HostAction::RequestFrame
            }
            '\x03' | '\x04' => self.ctrl_c(),
            '\t' => {
                if self.modal_open {
                    return HostAction::Ignored;
                }
                let mut next = self.focus.next();
                if next == Focus::Sidebar && !self.sidebar_present() {
                    next = Focus::Status;
                }
                if next != self.focus {
                    self.focus = next;
                    self.mark_frame();
                }
                HostAction::CycleFocus
            }
            _ => HostAction::Ignored,
        }
    }

    /// Escape scope (`ui/dialog.tsx:349-363` over
    /// `routes/session/index.tsx:253-258`): an open modal wins, then the
    /// narrow sidebar overlay; otherwise the host ignores it.
    fn escape(&mut self) -> HostAction {
        if self.modal_open {
            self.modal_open = false;
            self.mark_frame();
            return HostAction::RequestFrame;
        }
        if self.narrow_sidebar_overlay() {
            self.manual_sidebar = false;
            self.mark_frame();
            return HostAction::RequestFrame;
        }
        HostAction::Ignored
    }

    /// Ctrl+C scope (`ui/dialog.tsx:364-384` over
    /// `routes/session/index.tsx:253-258`): modal, then narrow sidebar
    /// overlay, then the global quit latch.
    fn ctrl_c(&mut self) -> HostAction {
        if self.modal_open {
            self.modal_open = false;
            self.mark_frame();
            return HostAction::RequestFrame;
        }
        if self.narrow_sidebar_overlay() {
            self.manual_sidebar = false;
            self.mark_frame();
            return HostAction::RequestFrame;
        }
        self.quit = true;
        HostAction::Quit
    }

    /// A manually opened sidebar below the wide threshold is painted as an
    /// overlay (`component/session-frame.tsx:386-398`).
    fn narrow_sidebar_overlay(&self) -> bool {
        self.manual_sidebar && self.cols <= SIDEBAR_NARROW_MAX_WIDTH
    }

    /// Auto-wide or manually opened sidebar.
    fn sidebar_present(&self) -> bool {
        self.sidebar_visible || self.manual_sidebar
    }

    /// Coarse transcript viewport: everything below the first frame row and
    /// above the last. The exact rectangle lives in `native_pane_resize`.
    fn transcript_contains(&self, x: u16, y: u16) -> bool {
        x < self.cols && y >= 1 && y < self.rows.saturating_sub(1)
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.compact = cols < MIN_FULL_WIDTH || rows < MIN_FULL_HEIGHT;
        self.sidebar_visible = !self.compact;
        if self.focus == Focus::Sidebar && !self.sidebar_present() {
            self.focus = Focus::Status;
        }
        // `panels.setWidth(dimensions - tabs)` follows the terminal width.
        self.panel_width = cols;
        self.mark_frame();
    }

    fn set_view(&mut self, view: AppView) {
        if self.view != view {
            self.view = view;
            self.history.push(view);
            if self.history.len() > MAX_HISTORY {
                let overflow = self.history.len() - MAX_HISTORY;
                self.history.drain(..overflow);
            }
            if !view.is_actionable() {
                self.live = false;
            }
            self.mark_frame();
        }
    }

    fn mark_frame(&mut self) {
        self.pending_frames = self
            .pending_frames
            .saturating_add(1)
            .min(MAX_PENDING_FRAMES);
    }

    /// True once a quit key has been stepped.
    #[must_use]
    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    /// True whenever the coalesced scheduler owes a frame.
    #[must_use]
    pub const fn needs_render(&self) -> bool {
        self.pending_frames > 0
    }

    /// Consume pending marks as one frame. False when idle.
    pub fn take_frame(&mut self) -> bool {
        if self.pending_frames == 0 {
            return false;
        }
        self.pending_frames = 0;
        true
    }

    /// Live data may be shown only when connected AND actionable.
    #[must_use]
    pub const fn shows_live_data(&self) -> bool {
        self.live && matches!(self.view, AppView::Actionable)
    }

    /// Paint badge derived from [`shows_live_data`](Self::shows_live_data).
    #[must_use]
    pub const fn data_badge(&self) -> &'static str {
        if self.shows_live_data() {
            "live"
        } else {
            "stale"
        }
    }

    #[must_use]
    pub const fn focus(&self) -> Focus {
        self.focus
    }

    #[must_use]
    pub const fn view(&self) -> AppView {
        self.view
    }

    #[must_use]
    pub const fn phase(&self) -> HostPhase {
        self.phase
    }

    #[must_use]
    pub const fn setup_step(&self) -> SetupStep {
        self.setup
    }

    #[must_use]
    pub const fn is_compact(&self) -> bool {
        self.compact
    }

    /// A manually opened sidebar is painted as an overlay at or below the
    /// 120-cell wide threshold (`component/session-frame.tsx:107,386-398`).
    #[must_use]
    pub const fn narrow_sidebar_open(&self) -> bool {
        self.manual_sidebar && self.cols <= SIDEBAR_NARROW_MAX_WIDTH
    }

    #[must_use]
    pub const fn manual_sidebar_open(&self) -> bool {
        self.manual_sidebar
    }

    #[must_use]
    pub const fn panel_open(&self) -> bool {
        self.panel_open
    }

    #[must_use]
    pub const fn panel_width(&self) -> u16 {
        self.panel_width
    }

    /// Below 108 cells a panel is fullscreen regardless of the requested
    /// presentation (`context/panel.tsx:15-16`).
    #[must_use]
    pub const fn panel_fullscreen(&self) -> bool {
        self.panel_open && (self.panel_width < PANEL_SPLIT_MIN_WIDTH || !self.panel_dock_requested)
    }

    /// Tabs hide only while the active panel is fullscreen
    /// (`app.tsx:615-619`).
    #[must_use]
    pub const fn tabs_visible(&self) -> bool {
        self.tabs_available && !self.panel_fullscreen()
    }

    /// True while a modal layer owns the keyboard
    /// (`ui/dialog.tsx:320-324,345-347`).
    #[must_use]
    pub const fn modal_open(&self) -> bool {
        self.modal_open
    }

    /// True while the pane focus guard is up
    /// (`component/session-frame.tsx:287-318`).
    #[must_use]
    pub const fn right_pane_focus(&self) -> bool {
        self.right_pane_focus
    }

    /// Wheel events forwarded to the transcript.
    #[must_use]
    pub const fn transcript_scrolls(&self) -> u32 {
        self.transcript_scrolls
    }

    #[must_use]
    pub fn history(&self) -> &[AppView] {
        &self.history
    }

    #[must_use]
    pub const fn routed_deltas(&self) -> u64 {
        self.routed_deltas
    }

    #[must_use]
    pub const fn routed_items(&self) -> usize {
        self.routed_items
    }

    #[must_use]
    pub const fn dropped_routes(&self) -> u64 {
        self.dropped_routes
    }
}

/// Single-caller ownership statement: the TUI path constructs exactly one
/// `NativeHost` via `tui_entry::run`; pure-state orphans
/// (`native_app`, `terminal_host::HostLoop`, `native_transcript`,
/// `native_timeline`, `native_composer`, `native_shell`,
/// `onboarding::SetupStep`) have no other event-loop caller.
#[must_use]
pub const fn describe_ownership() -> &'static str {
    "owner: tui_entry::run -> NativeHost::new + step loop; orphans native_app, \
     terminal_host::HostLoop, native_transcript, native_timeline, \
     native_composer, native_shell, onboarding::SetupStep route only via NativeHost::step"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn main_host() -> NativeHost {
        let mut cfg = HostConfig::new(120, 40);
        cfg.skip_onboarding = true;
        NativeHost::new(cfg)
    }

    #[test]
    fn quit_keys_latch_once() {
        for key in ['q', '\x03', '\x04'] {
            let mut host = main_host();
            assert_eq!(host.step(HostEvent::Key(key)), HostAction::Quit);
            assert!(host.should_quit());
        }
        let mut host = main_host();
        assert_eq!(host.step(HostEvent::Key('x')), HostAction::Ignored);
        assert!(!host.should_quit());
    }

    #[test]
    fn tab_cycle_skips_collapsed_sidebar() {
        let mut host = NativeHost::new(HostConfig::new(40, 10));
        assert!(host.is_compact());
        assert_eq!(host.step(HostEvent::Key('\t')), HostAction::CycleFocus);
        assert_eq!(host.focus(), Focus::Transcript);
        assert_eq!(host.step(HostEvent::Key('\t')), HostAction::CycleFocus);
        assert_eq!(host.focus(), Focus::Status);
        let mut wide = main_host();
        wide.step(HostEvent::Key('\t'));
        wide.step(HostEvent::Key('\t'));
        assert_eq!(wide.focus(), Focus::Sidebar);
    }

    #[test]
    fn live_and_disconnect_flip_badge() {
        let mut host = main_host();
        assert_eq!(host.data_badge(), "stale");
        assert_eq!(host.step(HostEvent::DaemonLive), HostAction::RequestFrame);
        assert!(host.shows_live_data());
        assert_eq!(host.data_badge(), "live");
        host.step(HostEvent::DaemonDown);
        assert!(!host.shows_live_data());
        assert_eq!(host.view(), AppView::Offline);
        assert_eq!(host.view().label(), "offline");
    }

    #[test]
    fn onboarding_gates_submit_until_done() {
        let mut host = NativeHost::new(HostConfig::new(120, 40));
        assert_eq!(host.phase(), HostPhase::Onboarding);
        host.step(HostEvent::DaemonLive);
        assert_eq!(
            host.step(HostEvent::Submit("hi".into())),
            HostAction::NeedsOnboarding
        );
        for _ in 0..5 {
            host.step(HostEvent::OnboardingAdvance);
        }
        assert_eq!(host.phase(), HostPhase::Main);
        assert_eq!(host.setup_step(), SetupStep::Done);
        assert!(host.setup_step().is_terminal());
        assert_eq!(
            host.step(HostEvent::Submit("hi".into())),
            HostAction::RequestFrame
        );
    }

    #[test]
    fn routing_counters_bound_and_drop() {
        let mut host = main_host();
        host.step(HostEvent::DaemonLive);
        assert_eq!(
            host.step(HostEvent::TokenDelta { bytes: 3 }),
            HostAction::RequestFrame
        );
        assert_eq!(host.routed_deltas(), 1);
        assert_eq!(
            host.step(HostEvent::TokenDelta { bytes: 0 }),
            HostAction::Ignored
        );
        assert_eq!(host.step(HostEvent::TimelineItem), HostAction::RequestFrame);
        assert_eq!(host.routed_items(), 1);
        assert!(host.needs_render());
        assert!(host.take_frame());
        assert!(!host.needs_render());
        assert_eq!(describe_ownership().find("tui_entry::run"), Some(7));
    }

    // ---- P04: panes, overlay and modal/focus guard ----------------------

    fn host_at(cols: u16, rows: u16) -> NativeHost {
        let mut cfg = HostConfig::new(cols, rows);
        cfg.skip_onboarding = true;
        NativeHost::new(cfg)
    }

    #[test]
    fn lay08_narrow_manual_sidebar_escape_closes_before_quit() {
        let mut host = host_at(100, 30);
        assert_eq!(
            host.step(HostEvent::ToggleSidebar),
            HostAction::RequestFrame
        );
        assert!(host.manual_sidebar_open());
        assert!(host.narrow_sidebar_open());

        // Escape belongs to the overlay's scoped layer, not the global quit.
        assert_eq!(host.step(HostEvent::Key('\x1b')), HostAction::RequestFrame);
        assert!(!host.manual_sidebar_open());
        assert!(!host.should_quit());
        // With the overlay closed Escape is ignored by the host.
        assert_eq!(host.step(HostEvent::Key('\x1b')), HostAction::Ignored);

        // Ctrl+C works the same way and only quits after the overlay closes.
        let mut host = host_at(100, 30);
        host.step(HostEvent::ToggleSidebar);
        assert_eq!(host.step(HostEvent::Key('\x03')), HostAction::RequestFrame);
        assert!(!host.should_quit());
        assert!(!host.manual_sidebar_open());
        assert_eq!(host.step(HostEvent::Key('\x03')), HostAction::Quit);
    }

    #[test]
    fn lay08_wide_manual_sidebar_docks_and_is_not_overlaid() {
        let mut host = host_at(140, 40);
        host.step(HostEvent::ToggleSidebar);
        assert!(host.manual_sidebar_open());
        // Above 120 the sidebar docks; Escape does not scope to it.
        assert!(!host.narrow_sidebar_open());
        assert_eq!(host.step(HostEvent::Key('\x1b')), HostAction::Ignored);
        assert!(host.manual_sidebar_open());
    }

    #[test]
    fn lay08_manual_sidebar_and_panel_are_mutually_exclusive() {
        let mut host = host_at(100, 30);
        host.step(HostEvent::ToggleSidebar);
        assert!(host.manual_sidebar_open());
        host.step(HostEvent::TogglePanel);
        assert!(host.panel_open());
        assert!(!host.manual_sidebar_open());
        host.step(HostEvent::ToggleSidebar);
        assert!(host.manual_sidebar_open());
        assert!(!host.panel_open());
    }

    #[test]
    fn lay10_panel_107_fullscreen_hides_tabs_and_108_docks() {
        let mut narrow = host_at(107, 40);
        assert!(!narrow.panel_fullscreen());
        assert!(narrow.tabs_visible());
        narrow.step(HostEvent::TogglePanel);
        assert!(narrow.panel_open());
        assert!(narrow.panel_fullscreen());
        assert!(!narrow.tabs_visible());

        let mut at_threshold = host_at(108, 40);
        at_threshold.step(HostEvent::TogglePanel);
        assert!(at_threshold.panel_open());
        assert!(!at_threshold.panel_fullscreen());
        assert!(at_threshold.tabs_visible());

        // Reflow re-evaluates the threshold without closing the panel.
        at_threshold.step(HostEvent::Resize {
            cols: 107,
            rows: 40,
        });
        assert!(at_threshold.panel_open());
        assert!(at_threshold.panel_fullscreen());
        assert!(!at_threshold.tabs_visible());
        at_threshold.step(HostEvent::Resize {
            cols: 108,
            rows: 40,
        });
        assert!(!at_threshold.panel_fullscreen());
        assert!(at_threshold.tabs_visible());
    }

    #[test]
    fn lay12_right_pane_guard_blocks_enter_and_submit() {
        let mut host = main_host();
        host.step(HostEvent::DaemonLive);
        assert_eq!(
            host.step(HostEvent::PaneFocusRight),
            HostAction::RequestFrame
        );
        assert!(host.right_pane_focus());
        assert_eq!(host.step(HostEvent::Enter), HostAction::Ignored);
        assert_eq!(
            host.step(HostEvent::Submit("rm -rf".into())),
            HostAction::Ignored
        );
        // Returning focus restores prompt input.
        host.step(HostEvent::PaneFocusSession);
        assert!(!host.right_pane_focus());
        assert_eq!(
            host.step(HostEvent::Submit("hello".into())),
            HostAction::RequestFrame
        );
        assert_eq!(host.step(HostEvent::Enter), HostAction::RequestFrame);
    }

    #[test]
    fn lay12_guard_keeps_transcript_wheel_usable() {
        let mut host = main_host();
        host.step(HostEvent::PaneFocusRight);
        // Inside the transcript viewport: forwarded.
        assert_eq!(
            host.step(HostEvent::Wheel { x: 5, y: 5 }),
            HostAction::RequestFrame
        );
        assert_eq!(host.transcript_scrolls(), 1);
        // Frame row 0 and out-of-bounds columns are consumed by the guard.
        assert_eq!(
            host.step(HostEvent::Wheel { x: 5, y: 0 }),
            HostAction::Ignored
        );
        assert_eq!(
            host.step(HostEvent::Wheel { x: 200, y: 5 }),
            HostAction::Ignored
        );
        assert_eq!(host.transcript_scrolls(), 1);
        // Without the guard the transcript always receives the wheel.
        host.step(HostEvent::PaneFocusSession);
        assert_eq!(
            host.step(HostEvent::Wheel { x: 0, y: 0 }),
            HostAction::RequestFrame
        );
    }

    #[test]
    fn lay12_modal_layer_owns_keys_and_does_not_quit_on_ctrl_c() {
        let mut host = main_host();
        host.step(HostEvent::DaemonLive);
        host.step(HostEvent::ModalOpen);
        assert!(host.modal_open());
        assert_eq!(host.step(HostEvent::Enter), HostAction::Ignored);
        assert_eq!(
            host.step(HostEvent::Submit("blocked".into())),
            HostAction::Ignored
        );
        assert_eq!(host.step(HostEvent::Key('\t')), HostAction::Ignored);
        // Ctrl+C closes the dialog instead of latching quit.
        assert_eq!(host.step(HostEvent::Key('\x03')), HostAction::RequestFrame);
        assert!(!host.modal_open());
        assert!(!host.should_quit());

        // Explicit modal events are idempotent when no dialog is open.
        assert_eq!(host.step(HostEvent::ModalEscape), HostAction::Ignored);
        host.step(HostEvent::ModalOpen);
        assert_eq!(host.step(HostEvent::ModalEscape), HostAction::RequestFrame);
        assert!(!host.modal_open());
    }

    #[test]
    fn lay12_modal_blocks_pane_toggles_until_closed() {
        let mut host = host_at(100, 30);
        host.step(HostEvent::ModalOpen);
        assert_eq!(host.step(HostEvent::ToggleSidebar), HostAction::Ignored);
        assert!(!host.manual_sidebar_open());
        assert_eq!(host.step(HostEvent::TogglePanel), HostAction::Ignored);
        assert!(!host.panel_open());
        host.step(HostEvent::ModalCtrlC);
        assert_eq!(
            host.step(HostEvent::ToggleSidebar),
            HostAction::RequestFrame
        );
        assert!(host.manual_sidebar_open());
    }
}
