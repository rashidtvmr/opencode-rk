#![forbid(unsafe_code)]
//! Transient overlay controllers and layer order.
//!
//! Upstream authority (read-only checkout, pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! `packages/tui/src/component/startup-loading.tsx`,
//! `packages/tui/src/component/reconnecting.tsx`,
//! `packages/tui/src/component/migration-overlay.tsx`,
//! `packages/tui/src/app.tsx:1377-1400,1470-1480` and
//! `packages/tui/src/ui/toast.tsx`.
//!
//! Timing contracts copied exactly:
//! - startup loading: hidden for the first 500 ms, then held visible for at
//!   least 3000 ms; a `ready` transition before the grace expires cancels the
//!   pending hint entirely; the label switches from "Loading plugins…" to
//!   "Finishing startup…";
//! - reconnecting: 5000 ms grace for the initial connection and 1000 ms for a
//!   retry; any `connected` status hides the overlay immediately;
//! - migration: first status query after 1000 ms, then once per second; a
//!   failure becomes a 10000 ms error toast titled "Data migration failed";
//! - layer order: OpenTUI paints by z-index first and insertion order second.
//!   Startup has z=5000, reconnecting and migration z=10000, and the toast
//!   surface declares no z-index (0), so a toast is painted *below* these
//!   overlays; this reflects the pinned source instead of assuming toasts are
//!   topmost.
//!
//! Pure state and cell projection only: no timers, threads, IO or new
//! dependencies. Colours resolve from the shared runtime palette, and spinner
//! glyphs use the pinned `spinner-frames.ts` first frame / animations-off
//! fallback.

use crate::native_theme_runtime::{Palette, Slot, ThemeMode};
use crate::native_toast::{
    info_color, take_cells, text_cells, CellRow, CellRun, ToastOptions, ToastVariant,
};

/// `setTimeout(..., 500)` before the startup hint may appear.
pub const STARTUP_GRACE_MS: u64 = 500;
/// Once visible the startup hint remains for at least three seconds.
pub const STARTUP_MIN_VISIBLE_MS: u64 = 3_000;
/// Initial connection grace in `app.tsx`.
pub const RECONNECT_INITIAL_GRACE_MS: u64 = 5_000;
/// Retry grace in `app.tsx`.
pub const RECONNECT_RETRY_GRACE_MS: u64 = 1_000;
/// `await Bun.sleep(1_000)` before the first migration status query.
pub const MIGRATION_FIRST_POLL_MS: u64 = 1_000;
/// Migration poll interval.
pub const MIGRATION_POLL_MS: u64 = 1_000;
/// Failed migration toast duration.
pub const MIGRATION_ERROR_TOAST_MS: u64 = 10_000;
/// Toast error title from `migration-overlay.tsx`.
pub const MIGRATION_ERROR_TITLE: &str = "Data migration failed";
/// Full-screen overlay backdrop alpha (black 150/255).
pub const OVERLAY_BACKDROP_ALPHA: u8 = 150;
/// Reconnecting box width before the 90% viewport clamp.
pub const RECONNECT_WIDTH: u16 = 48;
/// Startup overlay z-index.
pub const Z_STARTUP: u16 = 5_000;
/// Reconnecting/migration overlay z-index.
pub const Z_OVERLAY: u16 = 10_000;
/// Toast surface declares no z-index in upstream source.
pub const Z_TOAST: u16 = 0;
/// First frame of `spinner-frames.ts`.
pub const SPINNER_FIRST_FRAME: &str = "⠋";
/// Animations-off fallback from `spinner.tsx`.
pub const SPINNER_ANIMATIONS_OFF: &str = "⋯";

/// Minimal overlay colours resolved from the runtime palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayPalette {
    /// `background.raised.base`.
    pub raised_base: [u8; 4],
    /// `background.raised.high`.
    pub raised_high: [u8; 4],
    /// `text.base`.
    pub text: [u8; 4],
    /// `text.muted`.
    pub muted: [u8; 4],
    /// `text.feedback.info.base` (no cyan palette slot; exact upstream value).
    pub info: [u8; 4],
}

impl OverlayPalette {
    /// Resolve from the existing painter palette and runtime mode.
    #[must_use]
    pub fn from_palette(palette: &Palette, mode: ThemeMode) -> Self {
        Self {
            raised_base: palette.get(Slot::BackgroundPanel),
            raised_high: palette.get(Slot::BackgroundElement),
            text: palette.get(Slot::Text),
            muted: palette.get(Slot::TextMuted),
            info: info_color(mode),
        }
    }

    /// Builtin upstream dark theme.
    #[must_use]
    pub fn opencode_dark() -> Self {
        Self::from_palette(&Palette::opencode_dark(), ThemeMode::Dark)
    }

    /// Builtin upstream light theme.
    #[must_use]
    pub fn opencode_light() -> Self {
        Self::from_palette(&Palette::opencode_light(), ThemeMode::Light)
    }
}

/// Viewport size in terminal cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalSize {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
}

impl TerminalSize {
    /// Build a viewport size.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }
}

/// Absolute terminal rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Left column.
    pub x: u16,
    /// Top row.
    pub y: u16,
    /// Width in cells.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl Rect {
    /// Build a rectangle.
    #[must_use]
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Exclusive right edge.
    #[must_use]
    pub fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// Exclusive bottom edge.
    #[must_use]
    pub fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    /// Whether a cell lies inside the rectangle.
    #[must_use]
    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// Overlay layer kinds, in upstream mount order.
///
/// The first four are the app-lifecycle overlays mounted by `app.tsx`. The
/// last two are the session bottom-stack surfaces that share the toast layer's
/// z-index:
/// `component/prompt/autocomplete.tsx:865` is the only bottom-stack surface
/// that declares a z-index (100) and it is not modelled here; the auxiliary
/// composer dock (`routes/session/composer/index.tsx:84-155`) and the prompt
/// footer progress row are ordinary flow children with no `zIndex` prop, so
/// they paint at 0 like `ui/toast.tsx:47-63`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OverlayKind {
    Startup,
    Reconnecting,
    Migration,
    Toast,
    /// Shell-surface progress row (`prompt.footer.status`, no z-index).
    Progress,
    /// Auxiliary composer dock (`composer/index.tsx:84-155`, no z-index).
    Dock,
}

impl OverlayKind {
    /// Upstream mount order from `app.tsx`.
    pub const ALL: [Self; 4] = [
        Self::Startup,
        Self::Reconnecting,
        Self::Migration,
        Self::Toast,
    ];

    /// Bottom-stack surfaces in upstream document order: the dock sits above
    /// the prompt, its progress row is inside the prompt footer, and the toast
    /// surface is a root-level overlay mounted last (`app.tsx:1470-1480`).
    pub const SHELL_SURFACE: [Self; 3] = [Self::Dock, Self::Progress, Self::Toast];

    /// Effective z-index (toast, progress and dock declare none).
    #[must_use]
    pub fn z_index(self) -> u16 {
        match self {
            Self::Startup => Z_STARTUP,
            Self::Reconnecting | Self::Migration => Z_OVERLAY,
            Self::Toast | Self::Progress | Self::Dock => Z_TOAST,
        }
    }

    /// Whether this kind belongs to the session bottom stack.
    #[must_use]
    pub fn is_shell_surface(self) -> bool {
        Self::SHELL_SURFACE.contains(&self)
    }
}

/// One mounted overlay layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayLayer {
    /// Layer kind.
    pub kind: OverlayKind,
    /// Insertion sequence (mount order).
    pub order: u64,
    /// Effective z-index.
    pub z_index: u16,
    /// Painted rectangle.
    pub rect: Rect,
    /// Visibility flag.
    pub visible: bool,
}

/// Ordered overlay registry topped by the last painted layer.
#[derive(Clone, Debug, Default)]
pub struct OverlayStack {
    next_order: u64,
    layers: Vec<OverlayLayer>,
}

impl OverlayStack {
    /// Empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount or update a layer. Existing layers keep their original order.
    pub fn show(&mut self, kind: OverlayKind, rect: Rect) {
        if let Some(layer) = self.layers.iter_mut().find(|layer| layer.kind == kind) {
            layer.rect = rect;
            layer.visible = true;
            layer.z_index = kind.z_index();
            return;
        }
        self.layers.push(OverlayLayer {
            kind,
            order: self.next_order,
            z_index: kind.z_index(),
            rect,
            visible: true,
        });
        self.next_order = self.next_order.saturating_add(1);
    }

    /// Hide a layer without unmounting it.
    pub fn hide(&mut self, kind: OverlayKind) {
        if let Some(layer) = self.layers.iter_mut().find(|layer| layer.kind == kind) {
            layer.visible = false;
        }
    }

    /// Unmount a layer.
    pub fn remove(&mut self, kind: OverlayKind) -> bool {
        let before = self.layers.len();
        self.layers.retain(|layer| layer.kind != kind);
        self.layers.len() != before
    }

    /// Current layer, if mounted.
    #[must_use]
    pub fn layer(&self, kind: OverlayKind) -> Option<&OverlayLayer> {
        self.layers.iter().find(|layer| layer.kind == kind)
    }

    /// Mount the layer when `rect` is present and unmount it when it is not.
    ///
    /// Bottom-stack callers drive every surface from their own state each
    /// frame; the stack stays the single ordering authority.
    pub fn sync(&mut self, kind: OverlayKind, rect: Option<Rect>) {
        match rect {
            Some(rect) => self.show(kind, rect),
            None => {
                self.remove(kind);
            }
        }
    }

    /// Visible layers in paint order, bottom first.
    ///
    /// This is the ordered paint list callers blit in sequence: OpenTUI paints
    /// by z-index first and insertion order second, so equal z-index surfaces
    /// keep the order in which they were first mounted.
    #[must_use]
    pub fn paint_list(&self) -> Vec<PaintItem> {
        let mut items: Vec<PaintItem> = self
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .map(|layer| PaintItem {
                kind: layer.kind,
                z_index: layer.z_index,
                order: layer.order,
                rect: layer.rect,
            })
            .collect();
        items.sort_by_key(|item| (item.z_index, item.order));
        items
    }

    /// Visible kinds in paint order, bottom first.
    #[must_use]
    pub fn paint_order(&self) -> Vec<OverlayKind> {
        self.paint_list().into_iter().map(|item| item.kind).collect()
    }

    /// Topmost visible layer containing a cell.
    #[must_use]
    pub fn top_at(&self, x: u16, y: u16) -> Option<OverlayKind> {
        self.paint_list()
            .into_iter()
            .rev()
            .find(|item| item.rect.contains(x, y))
            .map(|item| item.kind)
    }
}

/// One entry of the ordered paint list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintItem {
    /// Layer kind.
    pub kind: OverlayKind,
    /// Effective z-index.
    pub z_index: u16,
    /// Insertion sequence (mount order).
    pub order: u64,
    /// Painted rectangle.
    pub rect: Rect,
}

/// `StartupLoading` timer state.
#[derive(Clone, Debug, Default)]
pub struct StartupLoading {
    ready: bool,
    shown: bool,
    wait_at: Option<u64>,
    shown_at: Option<u64>,
    hide_at: Option<u64>,
}

impl StartupLoading {
    /// Hidden initial state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a logical timestamp and the current `plugins.ready` value.
    pub fn update(&mut self, now: u64, ready: bool) {
        self.tick(now);
        self.ready = ready;
        if ready {
            // Clear a pending show; a ready transition before 500 ms means the
            // hint is never shown.
            self.wait_at = None;
            if !self.shown || self.hide_at.is_some() {
                return;
            }
            let shown_at = self.shown_at.unwrap_or(now);
            let left = STARTUP_MIN_VISIBLE_MS.saturating_sub(now.saturating_sub(shown_at));
            if left == 0 {
                self.shown = false;
                self.shown_at = None;
            } else {
                self.hide_at = Some(now.saturating_add(left));
            }
            return;
        }
        self.hide_at = None;
        if self.shown || self.wait_at.is_some() {
            return;
        }
        self.wait_at = Some(now.saturating_add(STARTUP_GRACE_MS));
    }

    /// Advance logical time without changing readiness.
    pub fn tick(&mut self, now: u64) {
        if let Some(deadline) = self.wait_at {
            if now >= deadline {
                self.wait_at = None;
                self.shown = true;
                self.shown_at = Some(deadline);
            }
        }
        if let Some(deadline) = self.hide_at {
            if now >= deadline {
                self.hide_at = None;
                self.shown = false;
                self.shown_at = None;
            }
        }
    }

    /// Whether the hint is painted.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.shown
    }

    /// Current spinner label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.ready {
            "Finishing startup…"
        } else {
            "Loading plugins…"
        }
    }

    /// Spinner text including the frame glyph.
    #[must_use]
    pub fn text(&self, animations: bool) -> String {
        spinner_text(animations, self.label())
    }
}

/// Connection phase driving the reconnecting overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPhase {
    /// Initial connection, longer grace.
    Connecting,
    /// Retry after a lost connection, short grace.
    Reconnecting,
    /// Connected; hide immediately.
    Connected,
}

/// `showReconnecting` timer state from `app.tsx`.
#[derive(Clone, Debug, Default)]
pub struct Reconnecting {
    visible: bool,
    deadline: Option<u64>,
}

impl Reconnecting {
    /// Hidden initial state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a connection status change at `now`.
    pub fn update(&mut self, now: u64, phase: ConnectionPhase) {
        match phase {
            ConnectionPhase::Connected => {
                self.visible = false;
                self.deadline = None;
            }
            ConnectionPhase::Connecting | ConnectionPhase::Reconnecting => {
                if self.visible {
                    self.deadline = None;
                    return;
                }
                let grace = if phase == ConnectionPhase::Reconnecting {
                    RECONNECT_RETRY_GRACE_MS
                } else {
                    RECONNECT_INITIAL_GRACE_MS
                };
                self.deadline = Some(now.saturating_add(grace));
            }
        }
    }

    /// Fire a pending grace deadline.
    pub fn tick(&mut self, now: u64) {
        if let Some(deadline) = self.deadline {
            if now >= deadline {
                self.deadline = None;
                self.visible = true;
            }
        }
    }

    /// Whether the full-screen overlay is painted.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// `managed` copy pair `(spinner label, detail line)`.
    #[must_use]
    pub fn copy(&self, managed: bool) -> (&'static str, &'static str) {
        if managed {
            (
                "Restarting service…",
                "Your session will resume automatically.",
            )
        } else {
            (
                "Connection lost…",
                "Reconnecting to the server automatically.",
            )
        }
    }
}

/// Migration progress payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationProgress {
    /// Progress label.
    pub label: String,
    /// Optional completed numerator.
    pub numerator: Option<u64>,
    /// Optional denominator.
    pub denominator: Option<u64>,
}

impl MigrationProgress {
    /// ` ""` when no numerator, `" N"` with only a numerator, else `" N/D"`.
    #[must_use]
    pub fn count(&self) -> String {
        match (self.numerator, self.denominator) {
            (None, _) => String::new(),
            (Some(numerator), None) => format!(" {numerator}"),
            (Some(numerator), Some(denominator)) => format!(" {numerator}/{denominator}"),
        }
    }
}

/// Migration status returned by the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationStatus {
    /// No migration running.
    Idle,
    /// Migration running with optional progress.
    Running(Option<MigrationProgress>),
    /// Migration completed.
    Completed,
    /// Migration failed with a message.
    Error(String),
}

/// `MigrationOverlay` state: first query after 1 s, then once per second.
#[derive(Clone, Debug, Default)]
pub struct MigrationOverlay {
    started: bool,
    next_poll: Option<u64>,
    progress: Option<MigrationProgress>,
    completed: bool,
    error: Option<String>,
    error_toast_emitted: bool,
}

impl MigrationOverlay {
    /// Not started.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mount at `now`; schedules the first poll one second later.
    pub fn begin(&mut self, now: u64) {
        self.started = true;
        self.next_poll = Some(now.saturating_add(MIGRATION_FIRST_POLL_MS));
    }

    /// Whether a status query is due, advancing the poll schedule.
    pub fn poll_due(&mut self, now: u64) -> bool {
        if !self.started || self.completed || self.error.is_some() {
            return false;
        }
        match self.next_poll {
            Some(deadline) if now >= deadline => {
                self.next_poll = Some(deadline.saturating_add(MIGRATION_POLL_MS));
                true
            }
            _ => false,
        }
    }

    /// Apply a status response.
    pub fn apply(&mut self, status: MigrationStatus) {
        match status {
            MigrationStatus::Idle => self.progress = None,
            MigrationStatus::Running(progress) => self.progress = progress,
            MigrationStatus::Completed => {
                self.completed = true;
                self.progress = None;
                self.next_poll = None;
            }
            MigrationStatus::Error(message) => {
                self.progress = None;
                self.error = Some(message);
                self.next_poll = None;
            }
        }
    }

    /// Running badge is shown only while progress exists.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.progress.is_some() && !self.completed && self.error.is_none()
    }

    /// Migration finished.
    #[must_use]
    pub fn completed(&self) -> bool {
        self.completed
    }

    /// Current progress.
    #[must_use]
    pub fn progress(&self) -> Option<&MigrationProgress> {
        self.progress.as_ref()
    }

    /// Badge text including the spinner glyph.
    #[must_use]
    pub fn badge_text(&self, animations: bool) -> Option<String> {
        let progress = self.progress.as_ref()?;
        if self.completed || self.error.is_some() {
            return None;
        }
        Some(spinner_text(
            animations,
            &format!("{}{}", progress.label, progress.count()),
        ))
    }

    /// Build the one 10-second error toast, once.
    pub fn take_error_toast(&mut self) -> Option<ToastOptions> {
        if self.error_toast_emitted {
            return None;
        }
        let message = self.error.clone()?;
        self.error_toast_emitted = true;
        Some(
            ToastOptions::new(ToastVariant::Error, message)
                .with_title(MIGRATION_ERROR_TITLE)
                .with_duration(MIGRATION_ERROR_TOAST_MS),
        )
    }
}

/// Painted overlay frame. `backdrop` fills the whole viewport first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayFrame {
    /// Outer box rectangle.
    pub rect: Rect,
    /// Optional full-screen dim rectangle and its RGBA colour.
    pub backdrop: Option<(Rect, [u8; 4])>,
    /// Painted rows inside the box.
    pub rows: Vec<CellRow>,
}

/// Startup hint: centered bottom rail at `bottom = 1`, raised-base surface,
/// one cell of horizontal padding.
#[must_use]
pub fn layout_startup(
    text: &str,
    size: TerminalSize,
    palette: &OverlayPalette,
) -> Option<OverlayFrame> {
    if size.cols == 0 || size.rows < 2 {
        return None;
    }
    let available = usize::from(size.cols).saturating_sub(2);
    let text = take_cells(text, available);
    let width = u16::try_from(text_cells(&text).saturating_add(2)).ok()?;
    let x = size.cols.saturating_sub(width) / 2;
    let y = size.rows.saturating_sub(2);
    let row = padded_row(x, y, width, &text, palette.muted, palette.raised_base);
    Some(OverlayFrame {
        rect: Rect::new(x, y, width, 1),
        backdrop: None,
        rows: vec![row],
    })
}

/// Reconnecting overlay: black-alpha150 backdrop and a centered 48-wide
/// (max 90% of the viewport) raised-base box with padding 2/1 and gap 1.
#[must_use]
pub fn layout_reconnecting(
    managed: bool,
    size: TerminalSize,
    palette: &OverlayPalette,
) -> Option<OverlayFrame> {
    if size.cols < 5 || size.rows == 0 {
        return None;
    }
    let width = u16::try_from(
        usize::from(size.cols)
            .saturating_mul(9)
            .saturating_div(10)
            .min(usize::from(RECONNECT_WIDTH)),
    )
    .ok()?
    .max(1);
    let height = 5u16.min(size.rows);
    let x = size.cols.saturating_sub(width) / 2;
    let y = size.rows.saturating_sub(height) / 2;
    let (label, detail) = Reconnecting::new().copy(managed);
    let inner = usize::from(width).saturating_sub(4);
    let line1 = spinner_text(true, label);
    let line1 = take_cells(&line1, inner);
    let line2 = take_cells(detail, inner);
    let mut rows = Vec::new();
    if height >= 1 {
        rows.push(blank_row(x, y, width, palette.raised_base));
    }
    if height >= 2 {
        rows.push(padded_row(
            x,
            y.saturating_add(1),
            width,
            &line1,
            palette.text,
            palette.raised_base,
        ));
    }
    if height >= 3 {
        rows.push(blank_row(
            x,
            y.saturating_add(2),
            width,
            palette.raised_base,
        ));
    }
    if height >= 4 {
        rows.push(padded_row(
            x,
            y.saturating_add(3),
            width,
            &line2,
            palette.muted,
            palette.raised_base,
        ));
    }
    if height >= 5 {
        rows.push(blank_row(
            x,
            y.saturating_add(4),
            width,
            palette.raised_base,
        ));
    }
    Some(OverlayFrame {
        rect: Rect::new(x, y, width, height),
        backdrop: Some((
            Rect::new(0, 0, size.cols, size.rows),
            [0, 0, 0, OVERLAY_BACKDROP_ALPHA],
        )),
        rows,
    })
}

/// Migration badge: top 1 / right 2, info-coloured left split border,
/// raised-high surface, padding 2/1.
#[must_use]
pub fn layout_migration(
    text: &str,
    size: TerminalSize,
    palette: &OverlayPalette,
) -> Option<OverlayFrame> {
    if size.cols < 7 || size.rows <= 1 {
        return None;
    }
    let max_width = usize::from(size.cols).saturating_sub(2);
    let inner = max_width.saturating_sub(5);
    let text = take_cells(text, inner);
    let width = u16::try_from(text_cells(&text).saturating_add(5)).ok()?;
    let x = size.cols.saturating_sub(2).saturating_sub(width);
    let y = 1u16;
    let height = 3u16.min(size.rows.saturating_sub(y));
    let mut rows = vec![bordered_row(
        x,
        y,
        width,
        "",
        palette.info,
        palette.raised_high,
    )];
    if height >= 2 {
        rows.push(bordered_row(
            x,
            y.saturating_add(1),
            width,
            &text,
            palette.info,
            palette.raised_high,
        ));
    }
    if height >= 3 {
        rows.push(bordered_row(
            x,
            y.saturating_add(2),
            width,
            "",
            palette.info,
            palette.raised_high,
        ));
    }
    Some(OverlayFrame {
        rect: Rect::new(x, y, width, height),
        backdrop: None,
        rows,
    })
}

fn spinner_text(animations: bool, label: &str) -> String {
    let glyph = if animations {
        SPINNER_FIRST_FRAME
    } else {
        SPINNER_ANIMATIONS_OFF
    };
    if label.is_empty() {
        glyph.to_owned()
    } else {
        format!("{glyph} {label}")
    }
}

fn blank_row(x: u16, y: u16, width: u16, bg: [u8; 4]) -> CellRow {
    CellRow {
        y,
        runs: vec![CellRun {
            x,
            text: " ".repeat(usize::from(width)),
            fg: bg,
            bg,
            attributes: 0,
        }],
    }
}

/// One text row with one cell of padding on each side.
fn padded_row(x: u16, y: u16, width: u16, text: &str, fg: [u8; 4], bg: [u8; 4]) -> CellRow {
    let inner = width.saturating_sub(2);
    let text = take_cells(text, usize::from(inner));
    let mut used = u16::try_from(text_cells(&text)).unwrap_or(inner);
    if used > inner {
        used = inner;
    }
    let mut runs = vec![CellRun {
        x,
        text: " ".to_owned(),
        fg,
        bg,
        attributes: 0,
    }];
    if used > 0 {
        runs.push(CellRun {
            x: x.saturating_add(1),
            text,
            fg,
            bg,
            attributes: 0,
        });
    }
    let trailing = inner.saturating_sub(used);
    if trailing > 0 {
        runs.push(CellRun {
            x: x.saturating_add(1).saturating_add(used),
            text: " ".repeat(usize::from(trailing)),
            fg,
            bg,
            attributes: 0,
        });
    }
    runs.push(CellRun {
        x: x.saturating_add(width).saturating_sub(1),
        text: " ".to_owned(),
        fg,
        bg,
        attributes: 0,
    });
    CellRow { y, runs }
}

/// Migration badge row: one left split border, two-cell padding, text.
fn bordered_row(
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    border_fg: [u8; 4],
    bg: [u8; 4],
) -> CellRow {
    let inner = width.saturating_sub(5);
    let text = take_cells(text, usize::from(inner));
    let used = u16::try_from(text_cells(&text)).unwrap_or(inner).min(inner);
    let mut runs = vec![CellRun {
        x,
        text: "┃".to_owned(),
        fg: border_fg,
        bg,
        attributes: 0,
    }];
    let left_pad = 2u16.min(width.saturating_sub(1));
    if left_pad > 0 {
        runs.push(CellRun {
            x: x.saturating_add(1),
            text: " ".repeat(usize::from(left_pad)),
            fg: border_fg,
            bg,
            attributes: 0,
        });
    }
    let mut col = 1u16.saturating_add(left_pad);
    if used > 0 && col < width {
        runs.push(CellRun {
            x: x.saturating_add(col),
            text,
            fg: border_fg,
            bg,
            attributes: 0,
        });
        col = col.saturating_add(used).min(width);
    }
    if col < width {
        runs.push(CellRun {
            x: x.saturating_add(col),
            text: " ".repeat(usize::from(width.saturating_sub(col))),
            fg: border_fg,
            bg,
            attributes: 0,
        });
    }
    CellRow { y, runs }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> OverlayPalette {
        OverlayPalette::opencode_dark()
    }

    fn row_width(row: &CellRow) -> u16 {
        row.runs
            .iter()
            .map(|run| u16::try_from(text_cells(&run.text)).unwrap())
            .sum()
    }

    #[test]
    fn startup_is_absent_at_499_and_visible_at_500() {
        let mut startup = StartupLoading::new();
        startup.update(0, false);
        startup.update(499, false);
        assert!(!startup.visible());
        startup.update(500, false);
        assert!(startup.visible());
    }

    #[test]
    fn startup_holds_three_seconds_after_becoming_visible() {
        let mut startup = StartupLoading::new();
        startup.update(0, false);
        startup.update(500, false);
        assert!(startup.visible());
        startup.update(1_000, true);
        startup.update(3_499, true);
        assert!(startup.visible());
        startup.update(3_500, true);
        assert!(!startup.visible());
    }

    #[test]
    fn startup_ready_before_grace_cancels_the_hint() {
        let mut startup = StartupLoading::new();
        startup.update(0, false);
        startup.update(100, true);
        startup.update(700, true);
        assert!(!startup.visible());
        startup.update(5_000, true);
        assert!(!startup.visible());
    }

    #[test]
    fn startup_ready_while_hidden_after_cancel_never_shows() {
        let mut startup = StartupLoading::new();
        startup.update(0, false);
        startup.update(499, true);
        startup.update(600, false);
        assert!(!startup.visible());
        startup.update(1_100, false);
        assert!(startup.visible());
    }

    #[test]
    fn startup_text_switches_between_loading_and_finishing() {
        let mut startup = StartupLoading::new();
        startup.update(0, false);
        assert_eq!(startup.text(true), "⠋ Loading plugins…");
        startup.update(10, true);
        assert_eq!(startup.text(false), "⋯ Finishing startup…");
    }

    #[test]
    fn reconnect_initial_grace_is_five_seconds() {
        let mut reconnect = Reconnecting::new();
        reconnect.update(0, ConnectionPhase::Connecting);
        reconnect.tick(4_999);
        assert!(!reconnect.visible());
        reconnect.tick(5_000);
        assert!(reconnect.visible());
    }

    #[test]
    fn reconnect_retry_grace_is_one_second() {
        let mut reconnect = Reconnecting::new();
        reconnect.update(10_000, ConnectionPhase::Reconnecting);
        reconnect.tick(10_999);
        assert!(!reconnect.visible());
        reconnect.tick(11_000);
        assert!(reconnect.visible());
    }

    #[test]
    fn connected_hides_immediately() {
        let mut reconnect = Reconnecting::new();
        reconnect.update(0, ConnectionPhase::Connecting);
        reconnect.tick(5_000);
        assert!(reconnect.visible());
        reconnect.update(5_001, ConnectionPhase::Connected);
        assert!(!reconnect.visible());
        reconnect.tick(60_000);
        assert!(!reconnect.visible());
    }

    #[test]
    fn managed_and_remote_copy_match_upstream() {
        let reconnect = Reconnecting::new();
        assert_eq!(
            reconnect.copy(true),
            (
                "Restarting service…",
                "Your session will resume automatically."
            )
        );
        assert_eq!(
            reconnect.copy(false),
            (
                "Connection lost…",
                "Reconnecting to the server automatically."
            )
        );
    }

    #[test]
    fn migration_first_poll_is_after_one_second_then_each_second() {
        let mut migration = MigrationOverlay::new();
        migration.begin(0);
        assert!(!migration.poll_due(999));
        assert!(migration.poll_due(1_000));
        assert!(!migration.poll_due(1_001));
        assert!(migration.poll_due(2_000));
        assert!(!migration.poll_due(2_500));
        assert!(!migration.poll_due(2_999));
        assert!(migration.poll_due(3_000));
    }

    #[test]
    fn migration_progress_count_matches_upstream_format() {
        let mut progress = MigrationProgress {
            label: "Migrating".into(),
            numerator: None,
            denominator: None,
        };
        assert_eq!(progress.count(), "");
        progress.numerator = Some(3);
        assert_eq!(progress.count(), " 3");
        progress.denominator = Some(10);
        assert_eq!(progress.count(), " 3/10");
        let mut migration = MigrationOverlay::new();
        migration.apply(MigrationStatus::Running(Some(progress.clone())));
        assert!(migration.visible());
        assert_eq!(
            migration.badge_text(true).as_deref(),
            Some("⠋ Migrating 3/10")
        );
        migration.apply(MigrationStatus::Completed);
        assert!(!migration.visible());
        assert!(migration.completed());
    }

    #[test]
    fn migration_failure_emits_one_ten_second_error_toast() {
        let mut migration = MigrationOverlay::new();
        migration.begin(0);
        migration.apply(MigrationStatus::Error("socket closed".into()));
        let toast = migration.take_error_toast().unwrap();
        assert_eq!(toast.variant, ToastVariant::Error);
        assert_eq!(toast.title.as_deref(), Some(MIGRATION_ERROR_TITLE));
        assert_eq!(toast.duration, MIGRATION_ERROR_TOAST_MS);
        assert_eq!(toast.message, "socket closed");
        assert!(migration.take_error_toast().is_none());
    }

    #[test]
    fn overlay_kind_z_indexes_match_source() {
        let z: Vec<u16> = OverlayKind::ALL.iter().map(|kind| kind.z_index()).collect();
        assert_eq!(z, vec![5_000, 10_000, 10_000, 0]);
        // The bottom stack declares no z-index anywhere in upstream source.
        for kind in OverlayKind::SHELL_SURFACE {
            assert_eq!(kind.z_index(), Z_TOAST);
            assert!(kind.is_shell_surface());
        }
        assert!(!OverlayKind::Startup.is_shell_surface());
    }

    #[test]
    fn shell_surface_paint_list_is_the_ordering_authority() {
        let mut stack = OverlayStack::new();
        // Mount order differs from document order: the toast arrives first,
        // the dock and progress row later.
        stack.show(OverlayKind::Toast, Rect::new(10, 1, 20, 3));
        stack.show(OverlayKind::Progress, Rect::new(2, 20, 60, 1));
        stack.show(OverlayKind::Dock, Rect::new(0, 12, 80, 8));
        let order: Vec<OverlayKind> = stack.paint_list().into_iter().map(|i| i.kind).collect();
        assert_eq!(
            order,
            vec![
                OverlayKind::Toast,
                OverlayKind::Progress,
                OverlayKind::Dock
            ],
            "equal z-index surfaces paint in insertion order (OpenTUI)"
        );
        // The paint list carries the geometry a caller blits.
        let list = stack.paint_list();
        assert_eq!(list[2].rect, Rect::new(0, 12, 80, 8));
        assert_eq!(list[2].order, 2);
        // A lifecycle overlay outranks the whole bottom stack.
        stack.show(OverlayKind::Startup, Rect::new(30, 22, 20, 1));
        assert_eq!(stack.paint_order().last(), Some(&OverlayKind::Startup));
        assert_eq!(
            stack.top_at(0, 12),
            Some(OverlayKind::Dock),
            "bottom-stack hit testing follows the paint list"
        );
    }

    #[test]
    fn sync_mounts_and_unmounts_bottom_stack_surfaces() {
        let mut stack = OverlayStack::new();
        stack.sync(OverlayKind::Dock, Some(Rect::new(0, 12, 80, 8)));
        stack.sync(OverlayKind::Progress, Some(Rect::new(2, 20, 60, 1)));
        assert_eq!(
            stack.paint_order(),
            vec![OverlayKind::Dock, OverlayKind::Progress],
            "the dock mounted first, so it paints first at equal z-index"
        );
        // A surface that stops applying is unmounted, not merely hidden, so a
        // later remount takes a fresh insertion order.
        stack.sync(OverlayKind::Dock, None);
        assert_eq!(stack.paint_order(), vec![OverlayKind::Progress]);
        assert!(stack.layer(OverlayKind::Dock).is_none());
        stack.sync(OverlayKind::Dock, Some(Rect::new(0, 12, 80, 8)));
        assert_eq!(
            stack.paint_order(),
            vec![OverlayKind::Progress, OverlayKind::Dock],
            "remounting appends to the insertion order"
        );
    }

    #[test]
    fn overlay_order_uses_z_index_then_mount_order() {
        let mut stack = OverlayStack::new();
        stack.show(OverlayKind::Startup, Rect::new(0, 20, 80, 1));
        stack.show(OverlayKind::Reconnecting, Rect::new(0, 0, 80, 24));
        stack.show(OverlayKind::Migration, Rect::new(70, 1, 8, 3));
        stack.show(OverlayKind::Toast, Rect::new(50, 1, 20, 5));
        assert_eq!(
            stack.paint_order(),
            vec![
                OverlayKind::Toast,
                OverlayKind::Startup,
                OverlayKind::Reconnecting,
                OverlayKind::Migration
            ]
        );
    }

    #[test]
    fn hit_test_prefers_topmost_layer() {
        let mut stack = OverlayStack::new();
        stack.show(OverlayKind::Reconnecting, Rect::new(0, 0, 80, 24));
        stack.show(OverlayKind::Migration, Rect::new(70, 1, 8, 3));
        assert_eq!(stack.top_at(72, 2), Some(OverlayKind::Migration));
        assert_eq!(stack.top_at(10, 10), Some(OverlayKind::Reconnecting));
        stack.hide(OverlayKind::Migration);
        assert_eq!(stack.top_at(72, 2), Some(OverlayKind::Reconnecting));
        assert!(stack.remove(OverlayKind::Reconnecting));
        assert_eq!(stack.top_at(10, 10), None);
    }

    #[test]
    fn startup_renders_centered_bottom_rail() {
        let p = palette();
        let frame = layout_startup("⠋ Loading plugins…", TerminalSize::new(80, 24), &p).unwrap();
        assert_eq!(frame.rect.y, 22);
        assert_eq!(frame.rect.width, 20);
        assert_eq!(frame.rect.x, 30);
        assert_eq!(frame.backdrop, None);
        assert_eq!(frame.rows.len(), 1);
        assert_eq!(row_width(&frame.rows[0]), frame.rect.width);
        assert!(frame.rows[0].text().contains("Loading plugins…"));
    }

    #[test]
    fn reconnect_renders_backdrop_and_centered_48_wide_box() {
        let p = palette();
        let frame = layout_reconnecting(false, TerminalSize::new(120, 40), &p).unwrap();
        assert_eq!(frame.rect.width, 48);
        assert_eq!(frame.rect.x, 36);
        assert_eq!(frame.rect.height, 5);
        assert_eq!(frame.rect.y, 17);
        let (backdrop, color) = frame.backdrop.unwrap();
        assert_eq!(backdrop, Rect::new(0, 0, 120, 40));
        assert_eq!(color, [0, 0, 0, 150]);
        for row in &frame.rows {
            assert_eq!(row_width(row), frame.rect.width);
        }
        assert!(frame.rows[1].text().contains("Connection lost…"));
        assert!(frame.rows[3].text().contains("Reconnecting to the server"));
    }

    #[test]
    fn reconnect_box_clamps_to_ninety_percent_and_clips_rows() {
        let p = palette();
        let frame = layout_reconnecting(true, TerminalSize::new(20, 4), &p).unwrap();
        assert_eq!(frame.rect.width, 18);
        assert_eq!(frame.rect.height, 4);
        assert_eq!(frame.rows.len(), 4);
        assert!(frame.rows[1].text().contains("Restarting"));
    }

    #[test]
    fn migration_badge_sits_top_right_with_info_border() {
        let p = palette();
        let frame = layout_migration("⠋ Migrating 3/10", TerminalSize::new(120, 40), &p).unwrap();
        assert_eq!(frame.rect.y, 1);
        assert_eq!(frame.rect.x, 120 - 2 - frame.rect.width);
        assert_eq!(frame.rect.height, 3);
        assert_eq!(frame.rows[1].runs[0].text, "┃");
        assert_eq!(frame.rows[1].runs[0].fg, p.info);
        assert!(frame.rows[1].text().contains("Migrating 3/10"));
        for row in &frame.rows {
            assert_eq!(row_width(row), frame.rect.width);
        }
    }

    #[test]
    fn narrow_overlays_stay_inside_the_viewport() {
        let p = palette();
        let startup = layout_startup("⠋ Loading plugins…", TerminalSize::new(8, 2), &p).unwrap();
        assert!(startup.rect.right() <= 8);
        assert_eq!(startup.rect.y, 0);
        let migration =
            layout_migration("⠋ Migrating 300/1000", TerminalSize::new(10, 4), &p).unwrap();
        assert!(migration.rect.right() <= 10);
        assert!(migration
            .rows
            .iter()
            .all(|row| row_width(row) == migration.rect.width));
        assert!(layout_startup("x", TerminalSize::new(0, 0), &p).is_none());
        assert!(layout_migration("x", TerminalSize::new(10, 1), &p).is_none());
    }

    #[test]
    fn palette_uses_raised_slots_and_exact_info_color() {
        let p = OverlayPalette::opencode_dark();
        let base = Palette::opencode_dark();
        assert_eq!(p.raised_base, base.get(Slot::BackgroundPanel));
        assert_eq!(p.raised_high, base.get(Slot::BackgroundElement));
        assert_eq!(p.info, [0x56, 0xb6, 0xc2, 0xff]);
        assert_eq!(
            OverlayPalette::opencode_light().info,
            [0x31, 0x87, 0x95, 0xff]
        );
    }
}
