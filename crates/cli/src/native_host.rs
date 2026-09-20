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
    Resize { cols: u16, rows: u16 },
    Paste(String),
    Submit(String),
    TokenDelta { bytes: usize },
    TimelineItem,
    DaemonLive,
    DaemonDown,
    OnboardingAdvance,
    OnboardingError,
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
        }
    }

    fn step_key(&mut self, c: char) -> HostAction {
        match c {
            'q' | '\x03' | '\x04' => {
                self.quit = true;
                HostAction::Quit
            }
            '\t' => {
                let mut next = self.focus.next();
                if next == Focus::Sidebar && !self.sidebar_visible {
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

    fn resize(&mut self, cols: u16, rows: u16) {
        self.compact = cols < MIN_FULL_WIDTH || rows < MIN_FULL_HEIGHT;
        self.sidebar_visible = !self.compact;
        if self.focus == Focus::Sidebar && !self.sidebar_visible {
            self.focus = Focus::Status;
        }
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
        self.pending_frames = self.pending_frames.saturating_add(1).min(MAX_PENDING_FRAMES);
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
        assert_eq!(
            host.step(HostEvent::TimelineItem),
            HostAction::RequestFrame
        );
        assert_eq!(host.routed_items(), 1);
        assert!(host.needs_render());
        assert!(host.take_frame());
        assert!(!host.needs_render());
        assert_eq!(describe_ownership().find("tui_entry::run"), Some(7));
    }
}
