#![forbid(unsafe_code)]
//! Pane system: the pane-resize state machine, the drag-handle hit/geometry
//! model, the plugin-panel fullscreen threshold, the narrow-sidebar overlay
//! and the pane focus guard.
//!
//! Upstream reference: OpenCode TUI pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
//!
//! * `packages/tui/src/ui/pane-resize.ts:4-76` — `createPaneResize`:
//!   preferred/requested size split (`:12-28`), clamp on read (`:21`), the
//!   commit-dedup rule (`:22-28`), hover over/out (`:47-48`), left-button-only
//!   drag start and the 300 ms double-click reset (`:49-65`), live clamp while
//!   dragging (`:66-72`) and the shared finish path (`:31-39`).
//! * `packages/tui/src/ui/pane-resize-handle.tsx:4-33` — handle box is
//!   absolute, `zIndex={10}`, `width={2}`, `height="100%"` (`:12-22`); the
//!   1-cell inner highlight is offset by `marginLeft={1}` only for
//!   `highlight="right"` (`:23-30`) and paints `background.raised.high` only
//!   while hovered or resizing (`:27-29`).
//! * `packages/tui/src/component/session-frame.tsx:41-45` — persisted value,
//!   `defaultPaneWidth`, clamp and `fromMouse`/`contains` hit zones;
//!   `:382` — the handle is shown only when not fullscreen, the right pane is
//!   a terminal/panel and `availableWidth() >= 3`; `:384` — `left =
//!   availableWidth() - paneResize.size() - 1`.
//! * `packages/tui/src/component/session-frame.tsx:277-340` — the session
//!   pane keeps `flexGrow=1` unless fullscreen, the right box is
//!   `SESSION_SIDEBAR_WIDTH` for a docked sidebar and `paneResize.size()`
//!   otherwise; fullscreen measures the session pane as
//!   `max(0, panels.width() - paneResize.size())` but hides it.
//! * `packages/tui/src/component/session-frame.tsx:386-398` — a non-wide
//!   sidebar is an absolute right-aligned overlay with
//!   `RGBA.fromInts(0, 0, 0, 70)` backdrop and reserves no transcript width.
//! * `packages/tui/src/routes/session/index.tsx:253-258` — `escape,ctrl+c`
//!   closes that narrow sidebar through its own layer, gated on
//!   `width - verticalTabsWidth <= 120`.
//! * `packages/tui/src/context/panel.tsx:15-16,38-41` — a panel can split only
//!   at `SESSION_PANEL_SPLIT_MIN_WIDTH` (108) and fullscreen is forced below
//!   it; `toggleFullscreen` is a no-op when it cannot split.
//! * `packages/tui/src/app.tsx:615-619` — tabs are visible only when available
//!   and the active panel is not fullscreen.
//! * `packages/tui/src/component/session-frame.tsx:287-318` — the pane focus
//!   guard overlay consumes Enter/click, forwards wheel events that land in
//!   the transcript viewport, and consumes the mouse release before revealing
//!   permission buttons; `:132-150` — `focusSession` is refused while
//!   fullscreen and focus within the right node activates the right pane.
//!
//! Geometry scalars are consumed from [`crate::native_view_layout`] (P03), not
//! re-derived here, so there is one source of truth for the clamp, the split
//! threshold and the 120-cell wide test.
//!
//! LAY-13 wiring: the handle is PLACED from the single frame descriptor in
//! [`crate::native_shell_frame::FrameDescriptor`] — its `available` box, docked
//! `right` width and terminal `cols` — so painting, hover and hit testing share
//! the upstream `left = availableWidth() - paneResize.size() - 1` arithmetic
//! (`component/session-frame.tsx:381-384`). [`PaneResize::sync_frame`] adopts
//! `panels.width()` and the terminal width from the same descriptor
//! (`component/session-frame.tsx:41-45`); the dock width stays a projection of
//! `size()`, so a clamped frame never rewrites the persisted size.

use crate::native_shell_frame::ShellFrame;
use crate::native_view_layout::{
    clamp_session_pane_width, default_pane_width, session_wide, RightPane,
    SESSION_PANEL_SPLIT_MIN_WIDTH, SESSION_SIDEBAR_WIDTH, SESSION_WIDE_THRESHOLD,
};

/// Double-click window: `now - lastBoundaryClick < 300`
/// (`ui/pane-resize.ts:52`). Strictly less than, in milliseconds.
pub const PANE_DOUBLE_CLICK_MS: u64 = 300;
/// Handle box width (`ui/pane-resize-handle.tsx:17`).
pub const PANE_HANDLE_WIDTH: u16 = 2;
/// Handle highlight width (`ui/pane-resize-handle.tsx:24`).
pub const PANE_HANDLE_HIGHLIGHT_WIDTH: u16 = 1;
/// Handle stack order (`ui/pane-resize-handle.tsx:16`).
pub const PANE_HANDLE_Z_INDEX: u16 = 10;
/// The handle is hidden below this available width
/// (`component/session-frame.tsx:382`).
pub const PANE_HANDLE_MIN_AVAILABLE: u16 = 3;
/// Narrow-sidebar backdrop `RGBA.fromInts(0, 0, 0, 70)`
/// (`component/session-frame.tsx:394`).
pub const SIDEBAR_OVERLAY_BACKDROP_ALPHA: u8 = 70;
/// Upstream paints the narrow sidebar as an overlay at or below this
/// available width (`routes/session/index.tsx:257`,
/// `component/session-frame.tsx:107`).
pub const SIDEBAR_OVERLAY_MAX_AVAILABLE: u16 = SESSION_WIDE_THRESHOLD;

/// Mouse button as delivered to the pane handlers. Only the left button
/// starts a drag (`ui/pane-resize.ts:50`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PaneMouseButton {
    Left,
    Middle,
    Right,
}

/// Which side of the 2-cell handle owns the visible 1-cell highlight
/// (`ui/pane-resize-handle.tsx:26`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum HandleHighlight {
    Left,
    Right,
}

/// Session column vs. right column (`component/session-frame.tsx:86-91`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PaneSide {
    Session,
    Right,
}

/// One cell rectangle in frame coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl CellRect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// True when `(x, y)` is inside the half-open cell range.
    #[must_use]
    pub const fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x
            && x < self.x.saturating_add(self.width)
            && y >= self.y
            && y < self.y.saturating_add(self.height)
    }
}

/// The 2-cell hit target and its 1-cell highlight
/// (`ui/pane-resize-handle.tsx:12-30`, `component/session-frame.tsx:382-384`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneResizeHandle {
    /// Absolute left cell. `i32` because upstream computes
    /// `available - size - 1` without clamping (`session-frame.tsx:384`);
    /// callers clip at paint time.
    pub left: i32,
    pub top: u16,
    pub width: u16,
    pub z_index: u16,
    /// Left cell of the 1-cell highlight inside the 2-cell target
    /// (`pane-resize-handle.tsx:26`).
    pub highlight_left: i32,
    pub highlight_width: u16,
}

impl PaneResizeHandle {
    /// The highlight paints `background.raised.high` only while hovered or
    /// resizing (`ui/pane-resize-handle.tsx:27-29`).
    #[must_use]
    pub const fn paints_highlight(self, hovered: bool, resizing: bool) -> bool {
        hovered || resizing
    }

    /// Cells covered by the target, clipped at the left frame edge.
    #[must_use]
    pub fn hit_rect(self, height: u16) -> CellRect {
        let clipped = if self.left < 0 { 0 } else { self.left as u16 };
        CellRect::new(clipped, self.top, self.width, height)
    }

    /// Point test for the 2-cell target (`ui/pane-resize-handle.tsx:12-22`):
    /// the box is `width={2}` and the full available height, so a pointer one
    /// cell outside either edge is not a resize grab.
    #[must_use]
    pub fn hit(self, x: u16, y: u16, height: u16) -> bool {
        self.hit_rect(height).contains(x, y)
    }
}

/// The resize handle placed on a composed frame: the 2-cell target, the
/// 1-cell highlight and the stack order, all in terminal coordinates.
///
/// Upstream mounts `PaneResizeHandle` inside the session-frame root box (the
/// box that starts at the rail's right edge) with `top={0}` and
/// `height="100%"`, and passes `left={availableWidth() - paneResize.size() - 1}`
/// (`component/session-frame.tsx:381-384`). Because the frame's right edge is
/// the terminal's right edge, that left column is also the absolute column the
/// pointer handlers use (`component/session-frame.tsx:43-44`), so one rect
/// serves painting, hover and hit testing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneHandleTarget {
    /// The 2-cell grab target.
    pub target: CellRect,
    /// The 1-cell highlight inside [`PaneHandleTarget::target`].
    pub highlight: CellRect,
    /// Stack order (`zIndex={10}`).
    pub z_index: u16,
}

impl PaneHandleTarget {
    /// True when the pointer is on the 2-cell grab target.
    #[must_use]
    pub const fn hit(&self, x: u16, y: u16) -> bool {
        self.target.contains(x, y)
    }

    /// The highlight paints only while hovered or resizing
    /// (`ui/pane-resize-handle.tsx:27-29`).
    #[must_use]
    pub const fn paints_highlight(&self, hovered: bool, resizing: bool) -> bool {
        hovered || resizing
    }

    /// Highlight cells: always one, the other target cell stays unpainted.
    #[must_use]
    pub const fn highlight_cells(&self) -> u16 {
        self.highlight.width
    }
}

/// Place the resize handle on a composed frame
/// (`component/session-frame.tsx:381-384`).
///
/// The frame's [`FrameDescriptor`] is the single geometry source: the target
/// spans the frame box's full height, its two columns straddle the boundary
/// between the last session cell and the docked right pane, and it only exists
/// for a docked terminal/panel when the frame is not fullscreen and at least
/// [`PANE_HANDLE_MIN_AVAILABLE`] columns remain.
#[must_use]
pub fn frame_resize_handle(
    frame: &ShellFrame,
    fullscreen: bool,
    highlight: HandleHighlight,
) -> Option<PaneHandleTarget> {
    let descriptor = &frame.descriptor;
    let handle = pane_resize_handle(
        descriptor.pane,
        fullscreen,
        descriptor.available.w,
        descriptor.right.w,
        highlight,
    )?;
    // `left` is frame-local upstream; shift by the rail reservation and clip
    // to the terminal so a degenerate frame cannot paint outside.
    let absolute = |local: i32| {
        u16::try_from(i32::from(descriptor.available.x).saturating_add(local).max(0))
            .unwrap_or(u16::MAX)
            .min(descriptor.cols)
    };
    let left = absolute(handle.left);
    let target = CellRect::new(
        left,
        descriptor.available.y,
        handle.width.min(descriptor.cols.saturating_sub(left)),
        descriptor.available.h,
    );
    let highlight_left = absolute(handle.highlight_left);
    let highlight_rect = CellRect::new(
        highlight_left,
        descriptor.available.y,
        handle
            .highlight_width
            .min(descriptor.cols.saturating_sub(highlight_left)),
        descriptor.available.h,
    );
    Some(PaneHandleTarget {
        target,
        highlight: highlight_rect,
        z_index: handle.z_index,
    })
}

/// The handle exists only for a docked terminal/panel when `available >= 3`
/// and the session is not fullscreen (`component/session-frame.tsx:382`).
#[must_use]
pub fn pane_resize_handle(
    right_pane: RightPane,
    fullscreen: bool,
    available_width: u16,
    size: u16,
    highlight: HandleHighlight,
) -> Option<PaneResizeHandle> {
    if fullscreen || available_width < PANE_HANDLE_MIN_AVAILABLE {
        return None;
    }
    if !matches!(right_pane, RightPane::Panel | RightPane::Terminal) {
        return None;
    }
    let left = i32::from(available_width) - i32::from(size) - 1;
    let highlight_left = match highlight {
        HandleHighlight::Left => left,
        HandleHighlight::Right => left + i32::from(PANE_HANDLE_HIGHLIGHT_WIDTH),
    };
    Some(PaneResizeHandle {
        left,
        top: 0,
        width: PANE_HANDLE_WIDTH,
        z_index: PANE_HANDLE_Z_INDEX,
        highlight_left,
        highlight_width: PANE_HANDLE_HIGHLIGHT_WIDTH,
    })
}

/// Resizable right-pane state machine, a direct port of
/// `createPaneResize` (`ui/pane-resize.ts:4-76`) with an injected clock so
/// the 300 ms window is deterministic in tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneResize {
    /// `panels.width()` — the clamp basis (`session-frame.tsx:42`).
    pane_width: u16,
    /// `dimensions().width` — the `fromMouse`/`contains` basis
    /// (`session-frame.tsx:43-44`).
    terminal_width: u16,
    preferred: u16,
    requested: Option<u16>,
    hovered: bool,
    resizing: bool,
    moved: bool,
    last_boundary_click_ms: Option<u64>,
    last_commit: Option<u16>,
    commits: u32,
}

impl PaneResize {
    /// `value: () => layout.paneWidth ?? layout.terminalWidth ??
    /// defaultPaneWidth()` (`component/session-frame.tsx:41-43`).
    #[must_use]
    pub fn new(pane_width: u16, terminal_width: u16, persisted: Option<u16>) -> Self {
        let default_value = default_pane_width(pane_width);
        let value = persisted.unwrap_or(default_value);
        Self {
            pane_width,
            terminal_width,
            preferred: value,
            requested: Some(value),
            hovered: false,
            resizing: false,
            moved: false,
            last_boundary_click_ms: None,
            last_commit: None,
            commits: 0,
        }
    }

    /// Track a terminal/session resize. The preferred size is kept; only the
    /// clamp basis changes (`session-frame.tsx:42-43`).
    pub fn set_geometry(&mut self, pane_width: u16, terminal_width: u16) {
        self.pane_width = pane_width;
        self.terminal_width = terminal_width;
    }

    /// Adopt a composed frame's geometry: `panels.width()` as the clamp basis
    /// and the terminal width for the pointer math
    /// (`component/session-frame.tsx:42-44`). The docked width is a projection
    /// of [`PaneResize::size`], never a new persisted value: upstream flows
    /// `layout.paneWidth -> preferredSize -> size() -> dock`, and a shrunken
    /// frame must not overwrite the stored size it only clamped for display
    /// (`ui/pane-resize.ts:5,16-21`, `component/session-frame.tsx:41-45`).
    pub fn sync_frame(&mut self, frame: &ShellFrame) {
        let descriptor = frame.descriptor;
        self.set_geometry(descriptor.available.w, descriptor.cols);
    }

    /// `defaultValue: defaultPaneWidth()` (`component/session-frame.tsx:45`):
    /// a live accessor over the CURRENT panel width, so the double-click reset
    /// follows a terminal resize instead of the width at construction.
    #[must_use]
    pub fn default_value(&self) -> u16 {
        default_pane_width(self.pane_width)
    }

    /// The reactive `createEffect` that copies an external value into the
    /// preferred size unless a drag is active (`ui/pane-resize.ts:16-20`).
    pub fn sync_value(&mut self, value: u16) {
        if self.resizing {
            return;
        }
        self.requested = Some(value);
        self.preferred = value;
    }

    #[must_use]
    pub const fn preferred(&self) -> u16 {
        self.preferred
    }

    /// `size() = clamp(preferredSize())` (`ui/pane-resize.ts:21`).
    #[must_use]
    pub fn size(&self) -> u16 {
        self.clamp(self.preferred)
    }

    /// `clampSessionPaneWidth` against the panel width
    /// (`component/session-frame.tsx:42`, `ui/layout.ts:22-26`).
    #[must_use]
    pub fn clamp(&self, width: u16) -> u16 {
        clamp_session_pane_width(width, self.pane_width)
    }

    #[must_use]
    pub const fn hovered(&self) -> bool {
        self.hovered
    }

    #[must_use]
    pub const fn resizing(&self) -> bool {
        self.resizing
    }

    #[must_use]
    pub const fn moved(&self) -> bool {
        self.moved
    }

    /// Commit notifications emitted so far (`onCommit`, `ui/pane-resize.ts:22-28`).
    #[must_use]
    pub const fn commits(&self) -> u32 {
        self.commits
    }

    #[must_use]
    pub const fn last_commit(&self) -> Option<u16> {
        self.last_commit
    }

    /// `fromMouse = dimensions().width - event.x - 1`
    /// (`component/session-frame.tsx:43`), saturating at zero rather than
    /// wrapping for off-screen drag coordinates.
    #[must_use]
    pub fn from_mouse(&self, x: u16) -> u16 {
        self.terminal_width.saturating_sub(x.saturating_add(1))
    }

    /// `contains = event.x >= width - size - 1 && event.x <= width - size`
    /// (`component/session-frame.tsx:44`) — a two-cell boundary band.
    #[must_use]
    pub fn contains(&self, x: u16, width: u16) -> bool {
        let upper = self.terminal_width.saturating_sub(width);
        let lower = upper.saturating_sub(1);
        x >= lower && x <= upper
    }

    pub fn on_mouse_over(&mut self) {
        self.hovered = true;
    }

    pub fn on_mouse_out(&mut self) {
        self.hovered = false;
    }

    /// `now - lastBoundaryClick < 300` (`ui/pane-resize.ts:52`): whether a
    /// release armed the double-click latch and the window is still open. The
    /// latch is consulted only on the next press, so a stale latch must never
    /// turn a later single click into a reset.
    #[must_use]
    pub fn double_click_armed(&self, now_ms: u64) -> bool {
        self.last_boundary_click_ms
            .is_some_and(|last| now_ms.saturating_sub(last) < PANE_DOUBLE_CLICK_MS)
    }

    /// Mouse-down handler (`ui/pane-resize.ts:49-65`). Returns whether the
    /// event was handled (left button only). A second left click within
    /// [`PANE_DOUBLE_CLICK_MS`] of a drag-less release resets to the default
    /// width.
    pub fn on_mouse_down(&mut self, button: PaneMouseButton, now_ms: u64) -> bool {
        if button != PaneMouseButton::Left {
            return false;
        }
        if self.double_click_armed(now_ms) {
            self.last_boundary_click_ms = None;
            self.resizing = false;
            self.hovered = false;
            self.commit(self.default_value());
            return true;
        }
        self.moved = false;
        self.resizing = true;
        true
    }

    /// Drag handler (`ui/pane-resize.ts:66-72`): any movement clears the
    /// double-click latch and clamps the live size.
    pub fn on_mouse_drag(&mut self, x: u16) -> bool {
        if !self.resizing {
            return false;
        }
        self.moved = true;
        self.last_boundary_click_ms = None;
        self.preferred = self.clamp(self.from_mouse(x));
        true
    }

    /// `onMouseDragEnd` (`ui/pane-resize.ts:73`).
    pub fn on_mouse_drag_end(&mut self, x: u16, now_ms: u64) -> bool {
        self.finish(x, now_ms)
    }

    /// `onMouseUp` (`ui/pane-resize.ts:74`) and the session-frame release
    /// wrapper (`component/session-frame.tsx:56-65`).
    pub fn on_mouse_up(&mut self, x: u16, now_ms: u64) -> bool {
        self.finish(x, now_ms)
    }

    /// Shared release path (`ui/pane-resize.ts:31-39`): a drag commits the
    /// cursor-derived width, a plain click commits the current size and arms
    /// the 300 ms double-click latch; hover is re-evaluated against the final
    /// boundary band.
    fn finish(&mut self, x: u16, now_ms: u64) -> bool {
        if !self.resizing {
            return false;
        }
        let next = if self.moved {
            self.from_mouse(x)
        } else {
            self.size()
        };
        self.resizing = false;
        self.last_boundary_click_ms = if self.moved { None } else { Some(now_ms) };
        self.commit(next);
        self.hovered = self.contains(x, self.clamp(next));
        true
    }

    /// `commit` (`ui/pane-resize.ts:22-28`): clamp, adopt as preferred, skip
    /// `onCommit` when the requested value is unchanged, then persist.
    fn commit(&mut self, value: u16) {
        let next = self.clamp(value);
        self.preferred = next;
        if self.requested == Some(next) {
            return;
        }
        self.requested = Some(next);
        self.last_commit = Some(next);
        self.commits = self.commits.saturating_add(1);
    }
}

/// Plugin-panel presentation (`context/panel.tsx:13-16`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PanelPresentation {
    /// Docked beside the session at its requested split width.
    Panel,
    /// Takes over the frame; the session pane is hidden (`fullscreen`).
    Fullscreen,
}

/// `canSplit = width() >= SESSION_PANEL_SPLIT_MIN_WIDTH`
/// (`context/panel.tsx:15`, `ui/layout.ts:9`). At 107 cells this is false, at
/// 108 it is true.
#[must_use]
pub const fn panel_can_split(panel_width: u16) -> bool {
    panel_width >= SESSION_PANEL_SPLIT_MIN_WIDTH
}

/// `presentation = canSplit ? requested : "fullscreen"`
/// (`context/panel.tsx:16`).
#[must_use]
pub fn panel_presentation(requested: PanelPresentation, panel_width: u16) -> PanelPresentation {
    if panel_can_split(panel_width) {
        requested
    } else {
        PanelPresentation::Fullscreen
    }
}

/// `toggleFullscreen` (`context/panel.tsx:38-41`): no-op when the panel
/// cannot split, otherwise flips the requested presentation.
#[must_use]
pub fn panel_toggle_fullscreen(
    requested: PanelPresentation,
    panel_width: u16,
) -> PanelPresentation {
    if !panel_can_split(panel_width) {
        return PanelPresentation::Fullscreen;
    }
    match requested {
        PanelPresentation::Panel => PanelPresentation::Fullscreen,
        PanelPresentation::Fullscreen => PanelPresentation::Panel,
    }
}

/// `tabsVisible = tabsAvailable && !fullscreenPanel`
/// (`app.tsx:615-619`). Tabs hide only while the *active* panel is
/// fullscreen; a docked panel keeps them.
#[must_use]
pub const fn tabs_visible(tabs_available: bool, panel_fullscreen: bool) -> bool {
    tabs_available && !panel_fullscreen
}

/// Measured split geometry (`component/session-frame.tsx:277-340`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneSplitGeometry {
    /// Width the session column reports (`Session`'s `width` prop). Under
    /// fullscreen the session node is hidden but still measured as
    /// `max(0, panels.width() - paneResize.size())`.
    pub session_width: u16,
    /// `visible={!fullscreen()}` (`:280`).
    pub session_visible: bool,
    /// Width reserved by the docked right column; 0 for no pane or for a
    /// narrow sidebar that is painted as an overlay.
    pub right_width: u16,
    /// The resize handle, present only for a docked terminal/panel.
    pub handle: Option<PaneResizeHandle>,
}

/// Derive the split geometry for the current pane combination.
///
/// `available_width` is `dimensions().width - verticalTabsWidth`;
/// `wide` is `dimensions().width - verticalTabsWidth > 120`
/// (`component/session-frame.tsx:107`).
#[must_use]
pub fn pane_split_geometry(
    right_pane: RightPane,
    fullscreen: bool,
    available_width: u16,
    wide: bool,
    size: u16,
) -> PaneSplitGeometry {
    let right_width = if fullscreen {
        available_width
    } else {
        match right_pane {
            RightPane::Panel | RightPane::Terminal => size,
            RightPane::Sidebar if wide => SESSION_SIDEBAR_WIDTH,
            RightPane::Sidebar | RightPane::None => 0,
        }
    };
    let session_width = if fullscreen {
        available_width.saturating_sub(size)
    } else {
        available_width.saturating_sub(right_width)
    };
    PaneSplitGeometry {
        session_width,
        session_visible: !fullscreen,
        right_width,
        handle: pane_resize_handle(
            right_pane,
            fullscreen,
            available_width,
            size,
            HandleHighlight::Right,
        ),
    }
}

/// Width test that routes the sidebar between docked and overlay:
/// `wide = dimensions().width - verticalTabsWidth > 120`
/// (`component/session-frame.tsx:107`); at or below 120 it overlays.
#[must_use]
pub fn sidebar_narrow(terminal_width: u16, vertical_tabs_width: u16) -> bool {
    !session_wide(terminal_width, vertical_tabs_width)
}

/// Geometry of the narrow-sidebar overlay
/// (`component/session-frame.tsx:386-398`). The overlay is right-aligned,
/// covers the full frame height, paints a 70/255 black backdrop behind it and
/// reserves **zero** width from the session column — the transcript does not
/// shrink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarOverlayLayout {
    pub backdrop: CellRect,
    pub sidebar: CellRect,
    /// Width the session column keeps while the overlay is shown.
    pub transcript_width: u16,
    pub backdrop_alpha: u8,
}

/// Build the overlay geometry for a narrow viewport. Returns `None` when the
/// viewport is wide enough for the sidebar to dock instead.
#[must_use]
pub fn narrow_sidebar_overlay(
    available_width: u16,
    height: u16,
    wide: bool,
) -> Option<SidebarOverlayLayout> {
    if wide {
        return None;
    }
    let width = SESSION_SIDEBAR_WIDTH.min(available_width);
    Some(SidebarOverlayLayout {
        backdrop: CellRect::new(0, 0, available_width, height),
        sidebar: CellRect::new(available_width.saturating_sub(width), 0, width, height),
        transcript_width: available_width,
        backdrop_alpha: SIDEBAR_OVERLAY_BACKDROP_ALPHA,
    })
}

/// The `escape,ctrl+c` close layer is enabled only for a narrow sidebar
/// (`routes/session/index.tsx:253-258`): a docked sidebar uses the normal
/// toggle shortcut instead.
#[must_use]
pub const fn narrow_sidebar_closeable(sidebar_visible: bool, available_width: u16) -> bool {
    sidebar_visible && available_width <= SIDEBAR_OVERLAY_MAX_AVAILABLE
}

/// LAY-11: resolved `/diff` panel geometry — the only builtin `session.panel`
/// claim (`opencode.diffs`,
/// `feature-plugins/system/diff-viewer.tsx:1363-1379` at pin
/// `450e7b92412c780a7b5a75226bb8d49aaca24e22`).
///
/// Composition of the two upstream rules:
/// - presentation (`context/panel.tsx:15-16`):
///   [`panel_presentation`] forces fullscreen below
///   [`SESSION_PANEL_SPLIT_MIN_WIDTH`] (108) or when
///   `config.diffs.fullscreen` requests it;
/// - split geometry (`component/session-frame.tsx:277-340`):
///   [`pane_split_geometry`] docks the panel as a non-shrinking flex sibling
///   at the pane width, or takes the full available width and hides the
///   session pane (`visible={!fullscreen()}`, `:280`) under fullscreen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffPanelLayout {
    /// Effective presentation after the split threshold.
    pub presentation: PanelPresentation,
    /// Pane rectangle in absolute cells while the panel is open; zeroed when
    /// closed. The pane spans the full frame height (the right box is
    /// `height="100%"`, `session-frame.tsx:326`).
    pub pane: CellRect,
    /// Session-column width that keeps painting. Under fullscreen the session
    /// pane is HIDDEN, so nothing of the main column paints and this is 0
    /// (upstream still measures the hidden node; the paint contract here is
    /// the visible one).
    pub session_width: u16,
    /// `visible={!fullscreen()}` (`:280`).
    pub session_visible: bool,
}

/// Resolve the `/diff` panel geometry. `available` is
/// `dimensions().width - verticalTabsWidth` (`app.tsx:623`); `pane_resize` is
/// the pane width — the `/diff` slice has no resize state yet, so callers
/// pass [`default_pane_width`]`(available)` (upstream's `defaultPaneWidth`,
/// `session-frame.tsx:44-45`).
#[must_use]
pub fn diff_panel_layout(
    cols: u16,
    tab_offset: u16,
    rows: u16,
    open: bool,
    fullscreen_requested: bool,
    pane_resize: u16,
) -> DiffPanelLayout {
    if !open {
        return DiffPanelLayout {
            presentation: PanelPresentation::Panel,
            pane: CellRect::new(0, 0, 0, 0),
            session_width: cols.saturating_sub(tab_offset),
            session_visible: true,
        };
    }
    let available = cols.saturating_sub(tab_offset);
    let requested = if fullscreen_requested {
        PanelPresentation::Fullscreen
    } else {
        PanelPresentation::Panel
    };
    let presentation = panel_presentation(requested, available);
    let fullscreen = presentation == PanelPresentation::Fullscreen;
    let geometry = pane_split_geometry(RightPane::Panel, fullscreen, available, true, pane_resize);
    // Docked: the pane starts after the session column. Fullscreen: the
    // session pane is hidden and the right box takes the whole available
    // width from the rail offset (`session-frame.tsx:330`).
    let pane_x = if fullscreen {
        tab_offset
    } else {
        tab_offset.saturating_add(geometry.session_width)
    };
    DiffPanelLayout {
        presentation,
        pane: CellRect::new(pane_x, 0, geometry.right_width, rows),
        session_width: if fullscreen { 0 } else { geometry.session_width },
        session_visible: geometry.session_visible,
    }
}

/// Outcome of routing one input through the pane focus guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GuardedAction {
    /// The overlay ate the event; nothing below it may react.
    Consumed,
    /// The event belongs to the session pane as usual.
    Forwarded,
    /// The overlay ate the release and returned focus to the session pane
    /// (`component/session-frame.tsx:309-314`).
    FocusSession,
}

/// Pane focus guard (`component/session-frame.tsx:86-91,132-150,287-318`).
///
/// When the right pane is active an absolute overlay covers the session pane.
/// It must not let Enter or a click reach an obscured permission prompt, but
/// it must still forward wheel events that land in the transcript viewport
/// (the transcript remains scrollable).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneFocusGuard {
    active: PaneSide,
    fullscreen: bool,
    resizing: bool,
    resize_release: bool,
    right_pane_is_sidebar: bool,
    transcript: CellRect,
}

impl PaneFocusGuard {
    #[must_use]
    pub fn new(transcript: CellRect) -> Self {
        Self {
            active: PaneSide::Session,
            fullscreen: false,
            resizing: false,
            resize_release: false,
            right_pane_is_sidebar: false,
            transcript,
        }
    }

    #[must_use]
    pub const fn active(&self) -> PaneSide {
        self.active
    }

    /// `InteractivityProvider enabled={activePane() === "session" &&
    /// !fullscreen()}` (`component/session-frame.tsx:287`). A permission
    /// prompt inside the session pane is reachable only while this is true.
    #[must_use]
    pub const fn session_interactive(&self) -> bool {
        matches!(self.active, PaneSide::Session) && !self.fullscreen
    }

    /// Enter/click may reach a permission prompt only through the interactive
    /// session provider.
    #[must_use]
    pub const fn permission_prompt_reachable(&self) -> bool {
        self.session_interactive()
    }

    pub fn set_transcript_viewport(&mut self, transcript: CellRect) {
        self.transcript = transcript;
    }

    /// `fullscreen` forces the right pane and refuses `focusSession`
    /// (`component/session-frame.tsx:131-134,153-155`).
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if fullscreen {
            self.active = PaneSide::Right;
        }
    }

    pub fn set_right_pane_is_sidebar(&mut self, is_sidebar: bool) {
        self.right_pane_is_sidebar = is_sidebar;
    }

    pub fn set_resizing(&mut self, resizing: bool) {
        self.resizing = resizing;
    }

    /// A captured drag-end can be followed by a mouse-up on the focus
    /// overlay (`component/session-frame.tsx:57-63`); that next release must
    /// be consumed without returning focus.
    pub fn arm_resize_release(&mut self) {
        self.resize_release = true;
    }

    /// The `queueMicrotask` reset of `resizeRelease`.
    pub fn clear_resize_release(&mut self) {
        self.resize_release = false;
    }

    /// `focusRightPane()` (`component/session-frame.tsx:141-149`).
    pub fn focus_right(&mut self) {
        self.active = PaneSide::Right;
    }

    /// `focusSession()` (`component/session-frame.tsx:131-139`). Returns
    /// false when fullscreen refuses the switch.
    pub fn focus_session(&mut self) -> bool {
        if self.fullscreen {
            return false;
        }
        self.active = PaneSide::Session;
        true
    }

    /// `onFocused` (`component/session-frame.tsx:145-150`): focus inside the
    /// right node activates the right pane unless that node is the sidebar;
    /// focus inside the session node activates the session pane only when not
    /// fullscreen.
    pub fn on_focus_within(&mut self, side: PaneSide) {
        match side {
            PaneSide::Right if !self.right_pane_is_sidebar => self.active = PaneSide::Right,
            PaneSide::Session if !self.fullscreen => self.active = PaneSide::Session,
            _ => {}
        }
    }

    /// Enter while the guard is up cannot reach the session pane.
    #[must_use]
    pub const fn guard_enter(&self) -> GuardedAction {
        if matches!(self.active, PaneSide::Right) {
            GuardedAction::Consumed
        } else {
            GuardedAction::Forwarded
        }
    }

    /// Overlay `onMouseUp` (`component/session-frame.tsx:308-314`): consumes
    /// the release before the permission prompt underneath sees it, except
    /// while a resize release is in flight; otherwise returns focus to the
    /// session pane.
    pub fn guard_click_release(&mut self) -> GuardedAction {
        if !matches!(self.active, PaneSide::Right) {
            return GuardedAction::Forwarded;
        }
        if self.resizing || self.resize_release {
            return GuardedAction::Consumed;
        }
        self.active = PaneSide::Session;
        GuardedAction::FocusSession
    }

    /// Overlay wheel handling (`component/session-frame.tsx:292-306`): events
    /// inside the transcript viewport are forwarded to the transcript's own
    /// scrollbox, all other wheel events stay consumed by the guard.
    #[must_use]
    pub fn guard_wheel(&self, x: u16, y: u16) -> GuardedAction {
        if !matches!(self.active, PaneSide::Right) {
            return GuardedAction::Forwarded;
        }
        if self.transcript.contains(x, y) {
            GuardedAction::Forwarded
        } else {
            GuardedAction::Consumed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_shell_frame::{ShellFrame, ShellFrameInput, compose_shell};

    // LAY-11: the /diff panel geometry (`opencode.diffs`, the only builtin
    // `session.panel` claim, `feature-plugins/system/diff-viewer.tsx:1363-1379`).
    #[test]
    fn diff_panel_docks_at_the_default_split_above_108() {
        // 140 cols, no vertical tabs: available 140 >= 108, so the requested
        // `panel` presentation docks the pane as a flex sibling at
        // `default_pane_width(140) = 70` (`session-frame.tsx:44-45,329-336`).
        let layout = diff_panel_layout(140, 0, 40, true, false, default_pane_width(140));
        assert_eq!(layout.presentation, PanelPresentation::Panel);
        assert_eq!(layout.pane, CellRect::new(70, 0, 70, 40));
        assert_eq!(layout.session_width, 70);
        assert!(layout.session_visible);
        // The 108 threshold is strict: 107 available cells force fullscreen.
        let narrow = diff_panel_layout(107, 0, 40, true, false, default_pane_width(107));
        assert_eq!(narrow.presentation, PanelPresentation::Fullscreen);
        assert_eq!(narrow.pane, CellRect::new(0, 0, 107, 40));
        assert_eq!(narrow.session_width, 0);
        assert!(!narrow.session_visible);
        // 108 docks again.
        let exact = diff_panel_layout(108, 0, 40, true, false, default_pane_width(108));
        assert_eq!(exact.presentation, PanelPresentation::Panel);
        assert_eq!(exact.session_width, 108 - default_pane_width(108));
    }

    #[test]
    fn diff_panel_fullscreen_takes_the_available_width_and_hides_the_session() {
        // `config.diffs.fullscreen === true` requests fullscreen
        // (`diff-viewer.tsx:1331-1336`); the pane takes the whole available
        // width from the rail offset and the session pane is hidden
        // (`session-frame.tsx:279-281,330`).
        let layout = diff_panel_layout(140, 0, 40, true, true, 70);
        assert_eq!(layout.presentation, PanelPresentation::Fullscreen);
        assert_eq!(layout.pane, CellRect::new(0, 0, 140, 40));
        assert_eq!(layout.session_width, 0);
        assert!(!layout.session_visible);
        // Vertical tabs shrink the available width below the 108 threshold,
        // which forces fullscreen too (`canSplit = width() >= 108`).
        let tabbed = diff_panel_layout(140, 42, 40, true, false, default_pane_width(98));
        assert_eq!(tabbed.presentation, PanelPresentation::Fullscreen);
        assert_eq!(tabbed.pane, CellRect::new(42, 0, 98, 40));
        assert_eq!(tabbed.session_width, 0);
    }

    #[test]
    fn diff_panel_closed_zeroes_the_pane_and_keeps_the_session() {
        let layout = diff_panel_layout(140, 0, 40, false, false, 70);
        assert_eq!(layout.presentation, PanelPresentation::Panel);
        assert_eq!(layout.pane, CellRect::new(0, 0, 0, 0));
        assert_eq!(layout.session_width, 140);
        assert!(layout.session_visible);
    }

    /// Rendered pin at the acceptance geometry (140x40, panel docked): the
    /// diff content lives inside the pane's columns and the transcript stays
    /// inside the shrunk session column — the same `compose_frame` the live
    /// renderer uses.
    #[test]
    fn diff_panel_composed_frame_separates_pane_and_transcript_at_140x40() {
        use crate::native_view_layout::{compose_frame, fit, Layout};
        let patch = "--- a/main.rs\n+++ b/main.rs\n@@ -1,2 +1,3 @@\n fn main() {\n-    old();\n+    new();\n+    extra();\n }";
        let mut state = crate::native_diff_view::DiffPanelState::default();
        state.open = true;
        state.patch = Some(patch.to_owned());
        let layout = diff_panel_layout(140, 0, 40, true, false, default_pane_width(140));
        assert_eq!(layout.pane.x, 70);
        let pane_rows = crate::native_diff_view::panel_rows(&state, usize::from(layout.pane.width), 38);
        assert!(pane_rows.iter().any(|row| row.contains("+    new();")));
        // The transcript shrinks to the session column; the pane carries the
        // diff rows.
        let transcript = vec!["  assistant reply token".to_owned(); 38];
        let frame_layout = Layout {
            cols: 140,
            rows: 40,
            left_width: 0,
            center_width: layout.session_width,
            right_width: layout.pane.width,
            compact: false,
        };
        let frame = compose_frame(&frame_layout, &[], &transcript, &pane_rows);
        assert_eq!(frame.len(), 40);
        for row in &frame {
            let session_region: String = row.chars().take(usize::from(layout.session_width)).collect();
            let pane_region: String = row
                .chars()
                .skip(usize::from(layout.pane.x))
                .collect();
            assert!(
                !session_region.contains("+    new();"),
                "diff content under the transcript region: {session_region:?}"
            );
            assert!(
                !pane_region.contains("assistant reply token"),
                "transcript paints under the pane: {pane_region:?}"
            );
        }
        // The pane's own content starts inside its columns.
        let title_row = frame
            .iter()
            .position(|row| row.contains("Diff viewer"))
            .expect("panel title renders");
        let title_col = frame[title_row].find("Diff viewer").unwrap();
        assert!(title_col >= usize::from(layout.pane.x), "title left of the pane");
    }

    fn resize() -> PaneResize {
        // 120-wide panel, 160-wide terminal, persisted 60.
        PaneResize::new(120, 160, Some(60))
    }

    /// A composed frame with a docked panel of `pane` columns.
    fn frame_with_panel(cols: u16, rows: u16, rail: u16, pane: u16) -> ShellFrame {
        let mut input = ShellFrameInput::new(cols, rows);
        input.tabs_width = rail;
        input.panel_open = true;
        input.pane_width = pane;
        compose_shell(&input)
    }

    // ---- LAY-13: handle target and highlight ----------------------------

    #[test]
    fn lay13_handle_is_two_cell_target_with_one_cell_highlight() {
        let handle = pane_resize_handle(RightPane::Panel, false, 100, 30, HandleHighlight::Right)
            .expect("panel handle");
        assert_eq!(handle.width, PANE_HANDLE_WIDTH);
        assert_eq!(handle.width, 2);
        assert_eq!(handle.highlight_width, PANE_HANDLE_HIGHLIGHT_WIDTH);
        assert_eq!(handle.highlight_width, 1);
        assert_eq!(handle.z_index, 10);
        // left = available - size - 1 = 100 - 30 - 1.
        assert_eq!(handle.left, 69);
        // `highlight="right"` puts the highlight in the right half.
        assert_eq!(handle.highlight_left, 70);
        assert!(handle.paints_highlight(false, true));
        assert!(handle.paints_highlight(true, false));
        assert!(!handle.paints_highlight(false, false));

        let left_highlight =
            pane_resize_handle(RightPane::Terminal, false, 100, 30, HandleHighlight::Left)
                .expect("terminal handle");
        assert_eq!(left_highlight.highlight_left, 69);
    }

    #[test]
    fn lay13_handle_hidden_below_three_cells_or_fullscreen_or_sidebar() {
        assert!(
            pane_resize_handle(RightPane::Panel, false, 2, 1, HandleHighlight::Right).is_none()
        );
        assert!(
            pane_resize_handle(RightPane::Panel, true, 100, 30, HandleHighlight::Right).is_none()
        );
        assert!(
            pane_resize_handle(RightPane::Sidebar, false, 100, 30, HandleHighlight::Right)
                .is_none()
        );
        assert!(
            pane_resize_handle(RightPane::None, false, 100, 30, HandleHighlight::Right).is_none()
        );
    }

    // ---- LAY-13: drag beyond bounds ------------------------------------

    #[test]
    fn lay13_drag_beyond_either_bound_clamps_and_commits_once() {
        let mut pane = resize();
        assert!(pane.on_mouse_down(PaneMouseButton::Left, 1_000));
        // Drag far left: from_mouse(0) = 159, clamp upper = max(60, 120-44) = 76.
        assert!(pane.on_mouse_drag(0));
        assert_eq!(pane.preferred(), 76);
        assert_eq!(pane.size(), 76);
        // Release at x=0: moved drag commits the clamped width.
        assert!(pane.on_mouse_up(0, 1_100));
        assert_eq!(pane.commits(), 1);
        assert_eq!(pane.last_commit(), Some(76));
        assert!(!pane.resizing());

        // Drag far right: from_mouse(u16::MAX) saturates to 0, clamp lower = 24.
        assert!(pane.on_mouse_down(PaneMouseButton::Left, 2_000));
        assert!(pane.on_mouse_drag(u16::MAX));
        assert_eq!(pane.preferred(), 24);
        assert!(pane.on_mouse_drag_end(u16::MAX, 2_100));
        assert_eq!(pane.commits(), 2);
        assert_eq!(pane.last_commit(), Some(24));
    }

    #[test]
    fn lay13_drag_within_bounds_maps_from_the_right_edge() {
        let mut pane = resize();
        pane.on_mouse_down(PaneMouseButton::Left, 0);
        // from_mouse(100) with terminal 160 => 59.
        pane.on_mouse_drag(100);
        assert_eq!(pane.preferred(), 59);
        pane.on_mouse_up(100, 10);
        assert_eq!(pane.last_commit(), Some(59));
    }

    // ---- LAY-13: release persistence -----------------------------------

    #[test]
    fn lay13_release_commits_only_when_value_changes() {
        let mut pane = resize();
        // A plain click commits the current size; requested is already 60, so
        // the dedup rule suppresses `onCommit` (`ui/pane-resize.ts:25`).
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        pane.on_mouse_up(95, 1_050);
        assert_eq!(pane.commits(), 0);
        assert_eq!(pane.last_commit(), None);

        // Drag then release: exactly one commit.
        pane.on_mouse_down(PaneMouseButton::Left, 2_000);
        pane.on_mouse_drag(80);
        pane.on_mouse_up(80, 2_100);
        assert_eq!(pane.commits(), 1);
        // from_mouse(80) = 79, clamped to the 76 upper bound.
        assert_eq!(pane.last_commit(), Some(76));

        // A second release with the same size (fresh press) is deduped.
        pane.on_mouse_down(PaneMouseButton::Left, 3_000);
        pane.on_mouse_up(80, 3_050);
        assert_eq!(pane.commits(), 1);
        // The persisted value survives for the next frame's sync.
        pane.sync_value(79);
        assert_eq!(pane.preferred(), 79);
        // `size()` clamps the external value on read (`ui/pane-resize.ts:21`).
        assert_eq!(pane.size(), 76);
    }

    // ---- LAY-13: double-click reset ------------------------------------

    #[test]
    fn lay13_double_click_resets_to_default_within_300ms() {
        let mut pane = resize();
        // First click (no drag) only arms the latch.
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        assert!(pane.on_mouse_up(80, 1_100));
        assert_eq!(pane.commits(), 0);

        // Second click 250 ms later resets to defaultPaneWidth(120) = 60.
        assert!(pane.on_mouse_down(PaneMouseButton::Left, 1_350));
        assert!(!pane.resizing());
        assert!(!pane.hovered());
        assert_eq!(pane.preferred(), 60);
        // requested was 60 initially, so the reset is deduped.
        assert_eq!(pane.commits(), 0);
    }

    #[test]
    fn lay13_double_click_window_is_strictly_less_than_300ms() {
        let mut pane = PaneResize::new(120, 160, Some(90));
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        pane.on_mouse_up(70, 1_100);
        // Exactly 300 ms later: not a double click, so a new drag begins.
        assert!(pane.on_mouse_down(PaneMouseButton::Left, 1_400));
        assert!(pane.resizing());
        // The click release committed clamp(90) = 76.
        assert_eq!(pane.preferred(), 76);
    }

    #[test]
    fn lay13_dragging_clears_the_double_click_latch() {
        let mut pane = resize();
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        pane.on_mouse_drag(90);
        pane.on_mouse_up(90, 1_050);
        // A release after a drag never arms the latch.
        let committed = pane.last_commit();
        pane.on_mouse_down(PaneMouseButton::Left, 1_100);
        assert!(pane.resizing());
        assert_eq!(pane.last_commit(), committed);
    }

    // ---- LAY-13: hover and button gating -------------------------------

    #[test]
    fn lay13_hover_follows_pointer_and_final_boundary_band() {
        let mut pane = resize();
        pane.on_mouse_over();
        assert!(pane.hovered());
        pane.on_mouse_out();
        assert!(!pane.hovered());

        // A click-release inside the 2-cell band re-hovers the handle
        // (`ui/pane-resize.ts:37`). size 60, terminal 160 => band 99..=100.
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        pane.on_mouse_up(99, 1_050);
        assert!(pane.hovered());
        pane.on_mouse_down(PaneMouseButton::Left, 2_000);
        pane.on_mouse_up(20, 2_050);
        assert!(!pane.hovered());
    }

    #[test]
    fn lay13_only_left_button_starts_a_drag() {
        let mut pane = resize();
        assert!(!pane.on_mouse_down(PaneMouseButton::Right, 0));
        assert!(!pane.resizing());
        assert!(!pane.on_mouse_drag(40));
        assert_eq!(pane.preferred(), 60);
    }

    #[test]
    fn lay13_sync_value_is_ignored_while_resizing() {
        let mut pane = resize();
        pane.on_mouse_down(PaneMouseButton::Left, 0);
        pane.on_mouse_drag(80);
        let live = pane.preferred();
        pane.sync_value(40);
        assert_eq!(pane.preferred(), live);
        pane.on_mouse_up(80, 10);
        pane.sync_value(40);
        assert_eq!(pane.preferred(), 40);
    }

    #[test]
    fn lay13_from_mouse_and_contains_match_upstream_hit_math() {
        // terminal 160, size 60: from_mouse(100) = 59; band 99..=100.
        let pane = resize();
        assert_eq!(pane.from_mouse(100), 59);
        assert_eq!(pane.from_mouse(0), 159);
        assert_eq!(pane.from_mouse(u16::MAX), 0);
        assert!(pane.contains(99, 60));
        assert!(pane.contains(100, 60));
        assert!(!pane.contains(98, 60));
        assert!(!pane.contains(101, 60));
    }

    // ---- LAY-13: frame placement, hit target and live default ----------

    /// LAY-13. `component/session-frame.tsx:381-384` +
    /// `ui/pane-resize-handle.tsx:12-30`: the handle is a 2-cell target that
    /// straddles the session/dock boundary, spans the frame box height, keeps
    /// exactly one highlighted cell, and its hit band is the same 2 columns
    /// `PaneResize::contains` uses.
    #[test]
    fn lay13_frame_handle_target_is_two_cells_on_the_dock_boundary() {
        let frame = frame_with_panel(160, 24, 0, 50);
        let handle =
            frame_resize_handle(&frame, false, HandleHighlight::Right).expect("panel handle");
        assert_eq!(handle.z_index, 10, "zIndex 10");
        assert_eq!(handle.target.width, 2, "2-cell grab target");
        assert_eq!(handle.highlight_cells(), 1, "1-cell highlight");
        assert_eq!(handle.target.height, 24, "height=100% of the frame box");
        assert_eq!(handle.target.y, 0);
        assert_eq!(handle.target.x, 109, "160 - 50 - 1");
        assert_eq!(frame.descriptor.right.x, 110, "the dock starts on cell 110");
        assert_eq!(
            handle.target.x + handle.target.width,
            frame.descriptor.right.x + 1,
            "the target covers the last session cell and the pane's first cell"
        );
        assert_eq!(handle.highlight.x, 110, "highlight=\"right\"");
        assert!(handle.hit(109, 0));
        assert!(handle.hit(110, 23));
        assert!(!handle.hit(108, 0), "one cell left is outside the target");
        assert!(!handle.hit(111, 0), "the pane's second cell is outside");
        assert!(!handle.hit(109, 24), "the row below the frame is outside");
        assert!(!handle.paints_highlight(false, false));
        assert!(handle.paints_highlight(true, false));
        assert!(handle.paints_highlight(false, true));
        // The painted target and the pointer band cannot disagree.
        let pane = PaneResize::new(frame.descriptor.available.w, 160, Some(50));
        assert_eq!(pane.size(), 50);
        for x in 104..=116 {
            assert_eq!(
                handle.hit(x, 0),
                pane.contains(x, pane.size()),
                "{x}: hit band must equal contains()"
            );
        }
        // A left highlight paints the session-side cell instead.
        let left = frame_resize_handle(&frame, false, HandleHighlight::Left).unwrap();
        assert_eq!(left.highlight.x, 109);
        assert_eq!(left.target, handle.target, "same grab target");
    }

    /// LAY-13. `app.tsx:1422-1434` puts the rail beside the frame box, so the
    /// handle's left column shifts by the rail while its height spans the frame
    /// box below a horizontal tab strip.
    #[test]
    fn lay13_frame_handle_follows_the_rail_and_the_strip() {
        let frame = frame_with_panel(200, 30, 42, 50);
        let handle = frame_resize_handle(&frame, false, HandleHighlight::Right).unwrap();
        let d = frame.descriptor;
        assert_eq!(d.available.x, 42, "the rail reservation");
        assert_eq!(d.right.x, 150);
        assert_eq!(handle.target.x, 149, "the last session cell");
        assert_eq!(handle.target.x + 1, d.right.x);
        assert_eq!(handle.target.y, d.available.y);
        assert_eq!(handle.target.height, d.available.h);

        // Horizontal tabs: the frame box starts below the strip.
        let mut input = ShellFrameInput::new(200, 30);
        input.tabs_height = 2;
        input.panel_open = true;
        input.pane_width = 50;
        let stripped = compose_shell(&input);
        let handle = frame_resize_handle(&stripped, false, HandleHighlight::Right).unwrap();
        assert_eq!(handle.target.y, 2, "below the tab strip");
        assert_eq!(handle.target.height, 28);
    }

    /// LAY-13. `component/session-frame.tsx:381-382`: no handle while
    /// fullscreen, for a sidebar right pane, or below 3 available columns.
    #[test]
    fn lay13_frame_handle_hidden_fullscreen_sidebar_or_too_narrow() {
        let frame = frame_with_panel(160, 24, 0, 50);
        assert!(
            frame_resize_handle(&frame, true, HandleHighlight::Right).is_none(),
            "fullscreen hides the handle"
        );
        let mut input = ShellFrameInput::new(160, 24);
        input.sidebar.open = true;
        let sidebar = compose_shell(&input);
        assert_eq!(sidebar.descriptor.pane, RightPane::Sidebar);
        assert!(frame_resize_handle(&sidebar, false, HandleHighlight::Right).is_none());
        let narrow = frame_with_panel(160, 24, 158, 50);
        assert_eq!(narrow.descriptor.available.w, 2);
        assert!(
            frame_resize_handle(&narrow, false, HandleHighlight::Right).is_none(),
            "availableWidth() < 3"
        );
    }

    /// LAY-13. Release persistence (`ui/pane-resize.ts:31-39`,
    /// `component/session-frame.tsx:50-55`): a drag commits the clamped width,
    /// the committed value survives a frame reflow/remount and is clamped by
    /// the new available width, and the reflowed frame places the handle from
    /// that restored width.
    #[test]
    fn lay13_release_persists_the_committed_size_across_a_frame_reflow() {
        let wide = frame_with_panel(160, 24, 0, 50);
        let mut pane = PaneResize::new(
            wide.descriptor.available.w,
            wide.descriptor.cols,
            Some(50),
        );
        pane.sync_frame(&wide);
        assert_eq!(pane.size(), 50);
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        assert!(pane.on_mouse_drag(60)); // from_mouse(60) = 99
        assert!(pane.on_mouse_up(60, 1_100));
        let committed = pane.last_commit().expect("a drag commits");
        assert_eq!(committed, 99);

        // Remount at a narrower frame with the persisted value. The dock is
        // composed from `paneResize.size()` (`component/session-frame.tsx:330`),
        // so the frame carries the value as the remounted shell clamps it.
        let persisted = clamp_session_pane_width(committed, 140);
        let mut input = ShellFrameInput::new(140, 20);
        input.panel_open = true;
        input.pane_width = persisted;
        let narrow = compose_shell(&input);
        let mut restored = PaneResize::new(0, 0, Some(committed));
        restored.sync_frame(&narrow);
        assert_eq!(
            restored.size(),
            persisted,
            "the release persisted and the narrower frame clamps it"
        );
        assert_eq!(restored.size(), 96);
        let handle = frame_resize_handle(&narrow, false, HandleHighlight::Right).unwrap();
        assert_eq!(
            handle.target.x + 1,
            narrow.descriptor.right.x,
            "the handle is placed from the restored dock width"
        );
        assert_eq!(narrow.descriptor.right.w, 96);

        // The clamp is display-only: the stored size survives the reflow, so a
        // later wider frame restores the committed width instead of the shrunken
        // one (`ui/pane-resize.ts:5,21`).
        assert_eq!(restored.preferred(), committed);
        restored.set_geometry(200, 200);
        assert_eq!(restored.size(), committed);
    }

    /// LAY-13. The double-click reset uses the LIVE `defaultPaneWidth()`
    /// (`component/session-frame.tsx:45`), so a terminal resize between the two
    /// clicks still resets to half of the CURRENT panels width, and the 300 ms
    /// window closes and is consumed exactly once.
    #[test]
    fn lay13_double_click_resets_to_the_live_frame_default() {
        let mut pane = PaneResize::new(120, 160, Some(60));
        assert_eq!(pane.default_value(), 60);
        assert!(!pane.double_click_armed(0), "no latch before a release");

        // First click inside the boundary band: no drag, no value change.
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        assert!(!pane.double_click_armed(1_000), "a press does not arm it");
        assert!(pane.on_mouse_up(100, 1_050));
        assert_eq!(pane.commits(), 0);
        assert!(pane.double_click_armed(1_050), "the release arms the latch");
        assert!(pane.double_click_armed(1_349), "349 - 50 < 300");
        assert!(!pane.double_click_armed(1_350), "300 ms closes the window");
        assert_eq!(pane.preferred(), 60);

        // A reflow between the clicks moves the default to 40; the reset must
        // adopt the live value, not the width captured at construction.
        pane.set_geometry(80, 160);
        assert_eq!(pane.default_value(), 40, "defaultPaneWidth(80)");
        assert_eq!(pane.preferred(), 60, "the stored size is unchanged so far");

        assert!(pane.on_mouse_down(PaneMouseButton::Left, 1_300));
        assert!(!pane.resizing(), "the reset never starts a drag");
        assert!(!pane.hovered());
        assert_eq!(pane.preferred(), 40, "reset to the CURRENT default");
        assert_eq!(pane.size(), 40);
        assert!(!pane.double_click_armed(1_301), "the latch is consumed");
        // A third press starts an ordinary drag instead of chaining resets.
        assert!(pane.on_mouse_down(PaneMouseButton::Left, 1_320));
        assert!(pane.resizing());
    }

    /// LAY-13. Dragging past either frame edge clamps to the split limits and
    /// the handle then sits on the new boundary
    /// (`ui/pane-resize.ts:66-72`, `component/session-frame.tsx:384`).
    #[test]
    fn lay13_drag_beyond_frame_bounds_clamps_and_the_handle_follows() {
        let frame = frame_with_panel(200, 30, 42, 50);
        let mut pane = PaneResize::new(
            frame.descriptor.available.w,
            frame.descriptor.cols,
            Some(50),
        );
        pane.sync_frame(&frame);
        // Left past the rail: from_mouse(42) = 157, clamped to the upper bound.
        pane.on_mouse_down(PaneMouseButton::Left, 1_000);
        assert!(pane.on_mouse_drag(42));
        assert_eq!(pane.preferred(), 114, "total - SESSION_CONTENT_MIN_WIDTH");
        assert!(pane.on_mouse_up(42, 1_100));
        assert_eq!(pane.last_commit(), Some(114));
        let mut input = ShellFrameInput::new(200, 30);
        input.tabs_width = 42;
        input.panel_open = true;
        input.pane_width = pane.size();
        let after = compose_shell(&input);
        assert_eq!(after.descriptor.right.w, 114);
        let handle = frame_resize_handle(&after, false, HandleHighlight::Right).unwrap();
        assert_eq!(handle.target.x + 1, after.descriptor.right.x);
        assert_eq!(handle.target.x, 85);

        // Right past the terminal edge: from_mouse saturates to 0.
        pane.on_mouse_down(PaneMouseButton::Left, 2_000);
        assert!(pane.on_mouse_drag(u16::MAX));
        assert_eq!(pane.preferred(), 24, "the 24-column lower bound");
        assert!(pane.on_mouse_drag_end(u16::MAX, 2_100));
        assert_eq!(pane.last_commit(), Some(24));
        assert_eq!(pane.commits(), 2, "one commit per drag");
    }

    // ---- LAY-10: panel 107/108 threshold and tab visibility ------------

    #[test]
    fn lay10_107_forces_fullscreen_and_108_can_dock() {
        assert!(!panel_can_split(107));
        assert!(panel_can_split(108));
        assert_eq!(
            panel_presentation(PanelPresentation::Panel, 107),
            PanelPresentation::Fullscreen
        );
        assert_eq!(
            panel_presentation(PanelPresentation::Panel, 108),
            PanelPresentation::Panel
        );
        assert_eq!(
            panel_presentation(PanelPresentation::Fullscreen, 108),
            PanelPresentation::Fullscreen
        );
    }

    #[test]
    fn lay10_toggle_is_noop_below_split_width() {
        assert_eq!(
            panel_toggle_fullscreen(PanelPresentation::Panel, 107),
            PanelPresentation::Fullscreen
        );
        assert_eq!(
            panel_toggle_fullscreen(PanelPresentation::Panel, 108),
            PanelPresentation::Fullscreen
        );
        assert_eq!(
            panel_toggle_fullscreen(PanelPresentation::Fullscreen, 108),
            PanelPresentation::Panel
        );
    }

    #[test]
    fn lay10_tabs_hide_only_while_panel_fullscreen() {
        assert!(tabs_visible(true, false));
        assert!(!tabs_visible(true, true));
        assert!(!tabs_visible(false, false));

        let below = panel_presentation(PanelPresentation::Panel, 107);
        assert!(!tabs_visible(true, below == PanelPresentation::Fullscreen));
        let at = panel_presentation(PanelPresentation::Panel, 108);
        assert!(tabs_visible(true, at == PanelPresentation::Fullscreen));
    }

    #[test]
    fn lay10_split_geometry_measures_fullscreen_session_but_hides_it() {
        // Docked panel at 108 of 130: session 108-wide minus the panel width.
        let docked = pane_split_geometry(RightPane::Panel, false, 130, true, 54);
        assert!(docked.session_visible);
        assert_eq!(docked.right_width, 54);
        assert_eq!(docked.session_width, 76);
        assert!(docked.handle.is_some());

        // Fullscreen: the right pane owns the full width and the session node
        // is hidden yet still measured as available - pane size.
        let full = pane_split_geometry(RightPane::Panel, true, 130, true, 54);
        assert!(!full.session_visible);
        assert_eq!(full.right_width, 130);
        assert_eq!(full.session_width, 76);
        assert!(full.handle.is_none());
    }

    #[test]
    fn lay10_docked_sidebar_reserves_42_or_overlays() {
        let wide = pane_split_geometry(RightPane::Sidebar, false, 160, true, 54);
        assert_eq!(wide.right_width, SESSION_SIDEBAR_WIDTH);
        assert_eq!(wide.session_width, 160 - SESSION_SIDEBAR_WIDTH);
        let narrow = pane_split_geometry(RightPane::Sidebar, false, 100, false, 54);
        assert_eq!(narrow.right_width, 0);
        assert_eq!(narrow.session_width, 100);
        assert!(narrow.handle.is_none());
    }

    // ---- LAY-08: narrow sidebar overlay --------------------------------

    #[test]
    fn lay08_narrow_sidebar_overlays_without_shrinking_transcript() {
        let overlay = narrow_sidebar_overlay(100, 30, false).expect("narrow overlay");
        assert_eq!(overlay.sidebar.width, SESSION_SIDEBAR_WIDTH);
        assert_eq!(overlay.sidebar.x, 58);
        assert_eq!(overlay.sidebar.y, 0);
        assert_eq!(overlay.sidebar.height, 30);
        // Full-frame backdrop at alpha 70.
        assert_eq!(overlay.backdrop, CellRect::new(0, 0, 100, 30));
        assert_eq!(overlay.backdrop_alpha, 70);
        // No transcript shrink: the overlay reserves zero columns.
        assert_eq!(overlay.transcript_width, 100);
        assert_eq!(overlay.backdrop.width, overlay.transcript_width);

        // A wide viewport docks instead of overlaying.
        assert!(narrow_sidebar_overlay(160, 40, true).is_none());
    }

    #[test]
    fn lay08_overlay_closes_through_escape_ctrl_c_scope_only_when_narrow() {
        // `width - verticalTabs <= 120` (`routes/session/index.tsx:257`).
        assert!(narrow_sidebar_closeable(true, 120));
        assert!(!narrow_sidebar_closeable(true, 121));
        assert!(!narrow_sidebar_closeable(false, 100));
        assert!(sidebar_narrow(125, 5));
        assert!(!sidebar_narrow(126, 5));
    }

    // ---- LAY-12: pane focus guard --------------------------------------

    fn guard() -> PaneFocusGuard {
        // Transcript viewport inside the session pane: x 4..=64, y 2..=20.
        PaneFocusGuard::new(CellRect::new(4, 2, 60, 18))
    }

    #[test]
    fn lay12_guard_blocks_enter_and_click_into_permission_prompt() {
        let mut guard = guard();
        guard.focus_right();
        assert!(!guard.session_interactive());
        assert!(!guard.permission_prompt_reachable());
        assert_eq!(guard.guard_enter(), GuardedAction::Consumed);
        // The release is consumed but returns focus; it never reaches the
        // permission prompt underneath (`session-frame.tsx:309-314`).
        assert_eq!(guard.guard_click_release(), GuardedAction::FocusSession);
        assert_eq!(guard.active(), PaneSide::Session);
        assert!(guard.permission_prompt_reachable());
    }

    #[test]
    fn lay12_guard_keeps_transcript_wheel_usable() {
        let mut guard = guard();
        guard.focus_right();
        // Inside the transcript viewport: forwarded to the transcript
        // scrollbox (`session-frame.tsx:292-306`).
        assert_eq!(guard.guard_wheel(10, 5), GuardedAction::Forwarded);
        assert_eq!(guard.active(), PaneSide::Right);
        // Outside it the guard still eats the event.
        assert_eq!(guard.guard_wheel(0, 0), GuardedAction::Consumed);
        assert_eq!(guard.guard_wheel(90, 5), GuardedAction::Consumed);
    }

    #[test]
    fn lay12_resize_release_is_consumed_without_stealing_focus() {
        let mut guard = guard();
        guard.focus_right();
        guard.set_resizing(true);
        assert_eq!(guard.guard_click_release(), GuardedAction::Consumed);
        assert_eq!(guard.active(), PaneSide::Right);

        guard.set_resizing(false);
        guard.arm_resize_release();
        assert_eq!(guard.guard_click_release(), GuardedAction::Consumed);
        assert_eq!(guard.active(), PaneSide::Right);
        guard.clear_resize_release();
        assert_eq!(guard.guard_click_release(), GuardedAction::FocusSession);
    }

    #[test]
    fn lay12_fullscreen_refuses_session_focus_and_forces_right() {
        let mut guard = guard();
        guard.set_fullscreen(true);
        assert_eq!(guard.active(), PaneSide::Right);
        assert!(!guard.focus_session());
        assert_eq!(guard.active(), PaneSide::Right);
        assert!(!guard.session_interactive());
        // Focus events inside the hidden session node do not steal focus.
        guard.on_focus_within(PaneSide::Session);
        assert_eq!(guard.active(), PaneSide::Right);
        guard.set_fullscreen(false);
        assert!(guard.focus_session());
    }

    #[test]
    fn lay12_sidebar_focus_does_not_activate_the_right_pane() {
        let mut guard = guard();
        guard.set_right_pane_is_sidebar(true);
        guard.on_focus_within(PaneSide::Right);
        assert_eq!(guard.active(), PaneSide::Session);
        guard.set_right_pane_is_sidebar(false);
        guard.on_focus_within(PaneSide::Right);
        assert_eq!(guard.active(), PaneSide::Right);
        guard.on_focus_within(PaneSide::Session);
        assert_eq!(guard.active(), PaneSide::Session);
    }
}
