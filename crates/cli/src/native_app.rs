#![forbid(unsafe_code)]
//! Native application shell state (TUI-003, slice: app types).
//!
//! Pure state only: focus, view, freshness, frame coalescing, tiny-terminal
//! layout. No rendering, no IO, no clock, no threads, no FFI. Caller supplies
//! all inputs (viewport size, daemon events).
//!
//! Commit: 5af7884. Evidence: `crates/sessions/src/tui_state.rs:1-69`
//! (composer/status state machines, bounded constants); card TUI-003
//! (Empty/Loading/Offline/Error/Actionable, stale-vs-live, coalescing,
//! no-overlap on tiny terminals); boundary
//! `docs/architecture/COMPLETION_NATIVE_TUI.md:24-32` (domain crates keep
//! `forbid(unsafe_code)`, bounded/coalesced events, one renderer owner).

/// Minimum viewport for the full multi-column shell.
pub const MIN_FULL_WIDTH: u16 = 80;
/// Minimum viewport height for the full shell.
pub const MIN_FULL_HEIGHT: u16 = 24;
/// Upper bound on coalesced pending frame requests.
pub const MAX_PENDING_FRAMES: u32 = 16;
/// Upper bound on retained view-transition history entries.
pub const MAX_HISTORY: usize = 32;

/// Focusable shell region. Exactly one holds focus at a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Focus {
    Composer,
    Transcript,
    Sidebar,
    Status,
}

impl Focus {
    /// Tab order used by keyboard focus cycling.
    pub const ORDER: [Focus; 4] = [
        Focus::Composer,
        Focus::Transcript,
        Focus::Sidebar,
        Focus::Status,
    ];

    #[must_use]
    pub const fn next(self) -> Focus {
        match self {
            Focus::Composer => Focus::Transcript,
            Focus::Transcript => Focus::Sidebar,
            Focus::Sidebar => Focus::Status,
            Focus::Status => Focus::Composer,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Focus {
        match self {
            Focus::Composer => Focus::Status,
            Focus::Transcript => Focus::Composer,
            Focus::Sidebar => Focus::Transcript,
            Focus::Status => Focus::Sidebar,
        }
    }
}

/// Whole-shell view. Every variant renders distinctly; only [`AppView::Actionable`]
/// accepts session input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AppView {
    /// No session/daemon data yet; shows create/open actions.
    Empty,
    /// Waiting on daemon; spinner, input held.
    Loading,
    /// No daemon link; must never present cached rows as live.
    Offline,
    /// Failed; shows retry/dismiss actions, never a live transcript.
    Error,
    /// Connected with session data; the only interactive view.
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

    /// Action hint shown for non-live views (`None` when actionable).
    #[must_use]
    pub const fn action_hint(self) -> Option<&'static str> {
        match self {
            AppView::Empty => Some("create or open a session"),
            AppView::Loading => Some("waiting for daemon"),
            AppView::Offline => Some("reconnect to daemon"),
            AppView::Error => Some("retry or dismiss"),
            AppView::Actionable => None,
        }
    }
}

/// Data freshness. Cached rows under [`Freshness::Stale`] must be badged
/// stale/offline; only [`Freshness::Live`] plus [`AppView::Actionable`]
/// may present as live.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Freshness {
    Live,
    Stale,
}

/// Integer viewport cell rectangle (origin top-left).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    #[must_use]
    pub const fn area(self) -> u32 {
        self.w as u32 * self.h as u32
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// True when interiors intersect (touching edges are fine).
    /// Zero-area rects never overlap.
    #[must_use]
    pub const fn overlaps(self, other: Rect) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.x < other.x.saturating_add(other.w)
            && other.x < self.x.saturating_add(self.w)
            && self.y < other.y.saturating_add(other.h)
            && other.y < self.y.saturating_add(self.h)
    }

    /// True when the region actually draws at least one cell.
    #[must_use]
    pub const fn is_visible(self) -> bool {
        !self.is_empty()
    }
}

/// Shell regions in paint order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellLayout {
    pub transcript: Rect,
    pub composer: Rect,
    pub sidebar: Rect,
    pub status: Rect,
    /// True on tiny viewports: single column, sidebar collapsed to zero
    /// width so regions cannot overlap and focus stays visible.
    pub compact: bool,
}

impl ShellLayout {
    /// Stacked non-overlapping layout. `sidebar_visible` is honored only
    /// when the viewport fits the full shell; tiny terminals force compact.
    #[must_use]
    pub fn compute(width: u16, height: u16, sidebar_visible: bool) -> ShellLayout {
        let compact = width < MIN_FULL_WIDTH || height < MIN_FULL_HEIGHT;
        let status_h: u16 = 1;
        let composer_h: u16 = if compact { 3 } else { 5 };
        let body_h = height.saturating_sub(status_h + composer_h);
        let status = Rect {
            x: 0,
            y: height.saturating_sub(status_h),
            w: width,
            h: status_h.min(height),
        };
        let composer = Rect {
            x: 0,
            y: height.saturating_sub(status_h + composer_h),
            w: width,
            h: composer_h.min(height.saturating_sub(status_h)),
        };
        if compact || !sidebar_visible {
            let transcript = Rect {
                x: 0,
                y: 0,
                w: width,
                h: body_h,
            };
            return ShellLayout {
                transcript,
                composer,
                sidebar: Rect {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                },
                status,
                compact: true,
            };
        }
        let sidebar_w: u16 = 30.min(width / 3).max(20.min(width));
        let transcript = Rect {
            x: 0,
            y: 0,
            w: width.saturating_sub(sidebar_w),
            h: body_h,
        };
        let sidebar = Rect {
            x: width.saturating_sub(sidebar_w),
            y: 0,
            w: sidebar_w.min(width),
            h: body_h,
        };
        ShellLayout {
            transcript,
            composer,
            sidebar,
            status,
            compact: false,
        }
    }

    /// Region rectangle for a focus target.
    #[must_use]
    pub const fn rect_of(self, focus: Focus) -> Rect {
        match focus {
            Focus::Composer => self.composer,
            Focus::Transcript => self.transcript,
            Focus::Sidebar => self.sidebar,
            Focus::Status => self.status,
        }
    }

    /// True when the focused region draws at least one cell.
    #[must_use]
    pub const fn focus_visible(self, focus: Focus) -> bool {
        !self.rect_of(focus).is_empty()
    }

    /// Remap a focus request onto a drawn region: sidebar on a
    /// compact/hidden sidebar falls back to transcript so keyboard focus
    /// can never strand on a zero-area rect.
    #[must_use]
    pub const fn resolve_focus(self, requested: Focus) -> Focus {
        match requested {
            Focus::Sidebar if self.sidebar.is_empty() => Focus::Transcript,
            other => other,
        }
    }

    /// Every non-empty region pair must be disjoint.
    #[must_use]
    pub fn has_overlap(self) -> bool {
        let rs = [
            self.transcript,
            self.composer,
            self.sidebar,
            self.status,
        ];
        for i in 0..rs.len() {
            if rs[i].area() == 0 {
                continue;
            }
            for other in rs.iter().skip(i + 1) {
                if other.area() == 0 {
                    continue;
                }
                if rs[i].overlaps(*other) {
                    return true;
                }
            }
        }
        false
    }
}

/// Coalescing frame scheduler: N dirty marks collapse into one render.
/// Bounded; idle (no pending frame) costs nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameScheduler {
    pending: u32,
    rendered: u64,
}

impl FrameScheduler {
    #[must_use]
    pub const fn new() -> FrameScheduler {
        FrameScheduler {
            pending: 0,
            rendered: 0,
        }
    }

    /// Record one dirty event; saturates at [`MAX_PENDING_FRAMES`].
    pub fn mark_dirty(&mut self) {
        self.pending = self.pending.saturating_add(1).min(MAX_PENDING_FRAMES);
    }

    /// True when a render is owed.
    #[must_use]
    pub const fn needs_render(&self) -> bool {
        self.pending > 0
    }

    /// Coalesced pending count (1..=MAX_PENDING_FRAMES).
    #[must_use]
    pub const fn pending(&self) -> u32 {
        self.pending
    }

    /// Consume all pending marks as a single frame. Returns false when idle.
    pub fn take_frame(&mut self) -> bool {
        if self.pending == 0 {
            return false;
        }
        self.pending = 0;
        self.rendered = self.rendered.saturating_add(1);
        true
    }

    #[must_use]
    pub const fn rendered_frames(&self) -> u64 {
        self.rendered
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Native shell state: focus + view + freshness + scheduler + bounded history.
#[derive(Clone, Debug)]
pub struct NativeApp {
    focus: Focus,
    view: AppView,
    /// Failure view held while Loading (debounce); applied via
    /// [`NativeApp::flush_pending_view`] so an instant Error cannot
    /// overwrite the connecting spinner.
    pending_view: Option<AppView>,
    freshness: Freshness,
    scheduler: FrameScheduler,
    layout: ShellLayout,
    /// Bounded retained view-transition history (oldest dropped).
    history: Vec<AppView>,
}

impl NativeApp {
    #[must_use]
    pub fn new(width: u16, height: u16) -> NativeApp {
        NativeApp {
            focus: Focus::Composer,
            view: AppView::Loading,
            pending_view: None,
            freshness: Freshness::Stale,
            scheduler: FrameScheduler::new(),
            layout: ShellLayout::compute(width, height, true),
            history: vec![AppView::Loading],
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
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    /// Live data may be shown only when connected AND actionable.
    /// Anything else must be badged stale/offline/loading/error.
    #[must_use]
    pub const fn shows_live_data(&self) -> bool {
        matches!(self.freshness, Freshness::Live)
            && matches!(self.view, AppView::Actionable)
    }

    #[must_use]
    pub const fn scheduler(&self) -> &FrameScheduler {
        &self.scheduler
    }

    #[must_use]
    pub const fn layout(&self) -> ShellLayout {
        self.layout
    }

    #[must_use]
    pub fn history(&self) -> &[AppView] {
        &self.history
    }

    pub fn set_focus(&mut self, focus: Focus) {
        let resolved = self.layout.resolve_focus(focus);
        if self.focus != resolved {
            self.focus = resolved;
            self.scheduler.mark_dirty();
        }
    }

    /// Tab cycle that skips the zero-area sidebar on compact layouts so
    /// focus can never hide on tiny terminals.
    pub fn cycle_focus(&mut self) {
        let mut next = self.focus.next();
        if next == Focus::Sidebar && self.layout.rect_of(next).is_empty() {
            next = Focus::Status;
        }
        self.set_focus(next);
    }

    /// Shift-Tab cycle; same compact skip as [`cycle_focus`](Self::cycle_focus).
    pub fn cycle_focus_prev(&mut self) {
        let mut prev = self.focus.prev();
        if prev == Focus::Sidebar && self.layout.rect_of(prev).is_empty() {
            prev = Focus::Transcript;
        }
        self.set_focus(prev);
    }

    /// Apply a view transition; records bounded history, dirties a frame,
    /// and forces [`Freshness::Stale`] for every non-actionable view so
    /// cached rows can never read as live.
    pub fn set_view(&mut self, view: AppView) {
        if self.view != view {
            self.view = view;
            self.history.push(view);
            if self.history.len() > MAX_HISTORY {
                let overflow = self.history.len() - MAX_HISTORY;
                self.history.drain(..overflow);
            }
            if !view.is_actionable() {
                self.freshness = Freshness::Stale;
            }
            self.scheduler.mark_dirty();
        }
    }

    /// Authenticated live daemon payload arrived: actionable + live.
    pub fn mark_live_data(&mut self) {
        self.pending_view = None;
        self.freshness = Freshness::Live;
        self.set_view(AppView::Actionable);
        self.scheduler.mark_dirty();
    }

    /// Transport dropped: offline + stale. Never presents fake live data:
    /// [`shows_live_data`](Self::shows_live_data) flips false here.
    pub fn mark_disconnected(&mut self) {
        self.pending_view = None;
        self.freshness = Freshness::Stale;
        self.set_view(AppView::Offline);
        self.scheduler.mark_dirty();
    }

    /// Reach Empty explicitly (create/open actions). Forces stale via
    /// [`set_view`](Self::set_view).
    pub fn mark_empty(&mut self) {
        self.set_view(AppView::Empty);
    }

    /// Enter Loading explicitly (waiting on daemon; input held).
    pub fn mark_loading(&mut self) {
        self.pending_view = None;
        self.set_view(AppView::Loading);
    }

    /// Enter Error explicitly (retry/dismiss actions, never live transcript).
    pub fn mark_error(&mut self) {
        // Debounce: hold the failure while Loading so an instant Error
        // cannot overwrite the connecting spinner; host applies it via
        // `flush_pending_view`.
        if self.view == AppView::Loading {
            self.pending_view = Some(AppView::Error);
            self.scheduler.mark_dirty();
            return;
        }
        self.set_view(AppView::Error);
    }

    /// Retry from Error back to Loading. True when a transition happened.
    pub fn retry(&mut self) -> bool {
        if self.view == AppView::Error {
            self.pending_view = None;
            self.set_view(AppView::Loading);
            true
        } else {
            false
        }
    }

    /// Dismiss from Error back to Empty. True when a transition happened.
    pub fn dismiss(&mut self) -> bool {
        if self.view == AppView::Error {
            self.set_view(AppView::Empty);
            true
        } else {
            false
        }
    }

    /// View-level input gate: only the actionable view accepts session input.
    pub const fn accepts_input(&self) -> bool {
        self.view.is_actionable()
    }

    /// Composer typing/submit gate: actionable view AND composer focus.
    pub const fn can_edit_composer(&self) -> bool {
        self.view.is_actionable() && matches!(self.focus, Focus::Composer)
    }

    /// Paint flag: true whenever cached rows must be badged stale/offline.
    pub const fn must_badge_stale(&self) -> bool {
        !self.shows_live_data()
    }

    /// Paint label derived from [`shows_live_data`](Self::shows_live_data).
    pub const fn data_badge(&self) -> &'static str {
        if self.shows_live_data() {
            "live"
        } else {
            "stale"
        }
    }

    /// False only when focus sits on a zero-area (compact-collapsed) region.
    pub const fn is_focus_visible(&self) -> bool {
        !self.layout.rect_of(self.focus).is_empty()
    }

    /// Host hook: true when the coalesced scheduler owes a frame.
    pub const fn needs_render(&self) -> bool {
        self.scheduler.pending > 0
    }

    /// Host hook: coalesced pending count for the render loop.
    pub const fn pending_frames(&self) -> u32 {
        self.scheduler.pending
    }

    /// Debounced failure view held while Loading (None when no debounce).
    pub const fn pending_view(&self) -> Option<AppView> {
        self.pending_view
    }

    /// Apply a debounced failure view held while Loading. True on transition.
    pub fn flush_pending_view(&mut self) -> bool {
        if let Some(view) = self.pending_view {
            self.pending_view = None;
            self.set_view(view);
            true
        } else {
            false
        }
    }

    pub fn mark_dirty(&mut self) {
        self.scheduler.mark_dirty();
    }

    /// Consume pending dirty marks as one frame.
    pub fn take_frame(&mut self) -> bool {
        self.scheduler.take_frame()
    }

    /// Resize viewport; recomputes compact/no-overlap layout, redirects a
    /// focus stranded on a collapsed region, dirties frame.
    pub fn resize(&mut self, width: u16, height: u16, sidebar_visible: bool) {
        self.layout = ShellLayout::compute(width, height, sidebar_visible);
        self.focus = self.layout.resolve_focus(self.focus);
        self.scheduler.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_is_distinguishable_and_not_actionable() {
        let views = [
            AppView::Empty,
            AppView::Loading,
            AppView::Offline,
            AppView::Error,
            AppView::Actionable,
        ];
        let mut labels = views.iter().map(|v| v.label()).collect::<Vec<_>>();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), views.len(), "each view needs a distinct label");
        assert!(!AppView::Offline.is_actionable());
        assert!(!AppView::Empty.is_actionable());
        assert!(!AppView::Loading.is_actionable());
        assert!(!AppView::Error.is_actionable());
        assert!(AppView::Actionable.is_actionable());
        assert_eq!(AppView::Offline.action_hint(), Some("reconnect to daemon"));
        assert_eq!(AppView::Actionable.action_hint(), None);
    }

    #[test]
    fn tiny_terminal_sets_compact_flag_without_overlap() {
        for (w, h) in [(40, 10), (10, 5), (79, 30), (100, 10), (0, 0), (1, 1)] {
            let layout = ShellLayout::compute(w, h, true);
            assert!(layout.compact, "tiny viewport {w}x{h} must force compact");
            assert_eq!(layout.sidebar.area(), 0, "compact hides sidebar {w}x{h}");
            assert!(!layout.has_overlap(), "overlap at {w}x{h}");
        }
        let full = ShellLayout::compute(120, 40, true);
        assert!(!full.compact);
        assert!(!full.has_overlap());
        let no_sidebar = ShellLayout::compute(120, 40, false);
        assert!(!no_sidebar.has_overlap());
    }

    #[test]
    fn disconnect_marks_stale_and_never_shows_live() {
        let mut app = NativeApp::new(120, 40);
        app.mark_live_data();
        assert!(app.shows_live_data());
        app.mark_disconnected();
        assert_eq!(app.view(), AppView::Offline);
        assert_eq!(app.freshness(), Freshness::Stale);
        assert!(!app.shows_live_data(), "offline must never read as live");
    }

    #[test]
    fn frame_requests_coalesce_and_idle_takes_nothing() {
        let mut sched = FrameScheduler::new();
        assert!(!sched.needs_render());
        assert!(!sched.take_frame(), "idle scheduler owes no frame");
        for _ in 0..5 {
            sched.mark_dirty();
        }
        assert_eq!(sched.pending(), 5);
        assert!(sched.take_frame(), "coalesced marks render once");
        assert!(!sched.needs_render());
        assert!(!sched.take_frame());
        for _ in 0..(MAX_PENDING_FRAMES + 10) {
            sched.mark_dirty();
        }
        assert_eq!(sched.pending(), MAX_PENDING_FRAMES, "pending stays bounded");
    }

    #[test]
    fn focus_cycles_all_regions_and_dirties_frame() {
        let mut app = NativeApp::new(120, 40);
        assert_eq!(app.focus(), Focus::Composer);
        let _ = app.take_frame();
        for expected in [
            Focus::Transcript,
            Focus::Sidebar,
            Focus::Status,
            Focus::Composer,
        ] {
            app.cycle_focus();
            assert_eq!(app.focus(), expected);
        }
        assert!(app.take_frame(), "focus moves must request a frame");
        assert_eq!(Focus::Composer.next().prev(), Focus::Composer);
    }

    #[test]
    fn non_actionable_views_force_stale_and_bound_history() {
        let mut app = NativeApp::new(120, 40);
        app.mark_live_data();
        app.set_view(AppView::Error);
        assert_eq!(app.freshness(), Freshness::Stale);
        assert!(!app.shows_live_data());
        for _ in 0..(MAX_HISTORY + 10) {
            app.set_view(AppView::Loading);
            app.set_view(AppView::Error);
        }
        assert!(app.history().len() <= MAX_HISTORY, "history stays bounded");
    }

    // TUI-003 RED (frozen before GREEN): compile on HEAD API, fail on the
    // missing remainder behavior. T1 input->focus + frames, T2 state
    // actionability, T3 tiny focus, T3/T5 no-overlap guard.
    #[test]
    fn zero_area_rects_never_overlap() {
        let point = Rect {
            x: 5,
            y: 5,
            w: 0,
            h: 0,
        };
        let big = Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        assert!(!point.overlaps(big), "zero-area rect must never overlap");
        assert!(!big.overlaps(point), "empty overlap must be symmetric");
    }

    #[test]
    fn resize_redirects_stranded_sidebar_focus() {
        let mut app = NativeApp::new(120, 40);
        app.set_focus(Focus::Sidebar);
        assert_eq!(app.focus(), Focus::Sidebar);
        app.resize(40, 10, true);
        assert!(app.layout().compact);
        assert_eq!(
            app.focus(),
            Focus::Transcript,
            "resize must redirect stranded sidebar focus"
        );
    }

    #[test]
    fn compact_tab_cycle_skips_hidden_sidebar() {
        let mut app = NativeApp::new(120, 40);
        app.resize(40, 10, true);
        app.set_focus(Focus::Transcript);
        app.cycle_focus();
        assert_eq!(
            app.focus(),
            Focus::Status,
            "compact Tab must skip Sidebar"
        );
    }

    #[test]
    fn non_live_views_gate_session_input() {
        let mut app = NativeApp::new(120, 40);
        for view in [
            AppView::Empty,
            AppView::Loading,
            AppView::Offline,
            AppView::Error,
        ] {
            app.set_view(view);
            assert!(!app.shows_live_data(), "{view:?} must never read as live");
        }
    }

    #[test]
    fn focused_control_never_stranded_on_tiny() {
        let layout = ShellLayout::compute(40, 10, true);
        assert!(layout.compact);
        let transcript = layout.transcript;
        assert!(transcript.w > 0 && transcript.h > 0);
        assert_eq!(layout.sidebar.area(), 0);
    }
}
