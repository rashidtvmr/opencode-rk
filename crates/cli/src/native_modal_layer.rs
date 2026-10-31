#![forbid(unsafe_code)]
//! Modal overlay layer: the dialog stack, its paint geometry, the backdrop
//! dismiss/selection guard, and the focus guard that keeps a modal from
//! leaking keys or focus back into the shell.
//!
//! Upstream reference: OpenCode TUI pin
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`, `packages/tui/src/ui/dialog.tsx`.
//!
//! * `:24-34` — `DIALOG_INSET = 3`, `DIALOG_LIST_INSET = 4` and the fixed
//!   dialog widths (medium 60, large 88, xlarge 116).
//! * `:48-90` — backdrop is absolute, full terminal, `zIndex={3000}`,
//!   `backgroundColor={RGBA.fromInts(0, 0, 0, 150)}`, horizontally centered,
//!   `paddingTop={centered ? 0 : height / 4}`, and `justifyContent="center"`
//!   only when centered; the panel is `dialogWidth(size)` wide, capped at
//!   `dimensions().width - 2`, with `paddingTop={1}`.
//! * `:50-60,72-79` — backdrop mouse-down records `dismiss = !!selection`;
//!   backdrop mouse-up with `dismiss` set keeps the dialog open once; the
//!   panel's mouse-up lets a selection release bubble for copy-on-select and
//!   otherwise stops propagation and clears `dismiss`.
//! * `:209-261` — footer actions switch to a column below 60 cells
//!   (`:214`).
//! * `:306-343` — the stack saves the previously focused renderable and blurs
//!   it on the first open; `refocus()` restores it after a 1 ms timeout only
//!   when the stack is empty, the target is not destroyed and it is still
//!   attached to the root.
//! * `:320-324,345-386` — a `modal` keymap mode is active exactly while the
//!   stack is non-empty; Escape clears a selection first and otherwise closes
//!   the top entry; Ctrl+C clears a selection first, then the focused editor
//!   text, and only closes when the editor is empty.
//! * `:389-418` — `clear()` closes every entry, resets size/centered and
//!   refocuses; `replace()` closes the previous entries, resets
//!   size/centered, and installs exactly one entry.
//! * `:472-479` — only the top stack entry is rendered, and it is rendered
//!   once (keyed) so a reactive root change cannot remount/double-paint it.
//!
//! This module is pure state. The caller owns the renderer, keymap and
//! timers; [`ModalLayer::take_refocus`] models the 1 ms refocus timeout with
//! an injected clock.
//!
//! Layer composition (DIALOG-19, DIALOG-20). Every transient surface in the
//! pinned TUI is independently positioned with a z-index, and the ones that
//! can overlap must paint and dismiss in that order rather than in the order
//! the caller happens to call them:
//!
//! * dialog backdrop `zIndex={3000}` (`ui/dialog.tsx:66`);
//! * diff file menu portal `zIndex = 2600`
//!   (`feature-plugins/system/diff-viewer-file-menu.tsx:40,50`), i.e.
//!   painted below an open dialog but above the diff cards;
//! * startup hint `zIndex={5000}` (`component/startup-loading.tsx:56`);
//! * reconnecting and migration `zIndex={10_000}`
//!   (`component/reconnecting.tsx:11`, `component/migration-overlay.tsx:52`);
//! * the toast surface declares no z-index (`ui/toast.tsx:47-63`), so it
//!   paints below all of the above — the audit's D22 note that toasts are not
//!   guaranteed topmost.
//!
//! OpenTUI paints equal z-index layers in insertion order
//! (`native_overlay_stack` documents the same rule), so [`ChromeModel`]
//! records first-activation order and sorts by `(z-index, order)`.
//!
//! Keyboard dismissal follows *activation* order, not paint order: the diff
//! file menu pushes its own `menu` keymap layer
//! (`diff-viewer-file-menu.tsx:23-30`) and the dialog stack pushes `modal`
//! (`ui/dialog.tsx:320-324`). The most recently activated interactive layer
//! therefore consumes Escape/Ctrl+C first. Non-interactive overlays
//! (startup/reconnecting/migration) never take keys — they have no keymap
//! layer upstream.
//!
//! All timing is driven by an injected [`Clock`]; this module never reads an
//! ambient wall clock, so every timer decision is reproducible from the
//! caller's event-loop timestamp alone.

use crate::native_overlay_stack::{
    ConnectionPhase, MigrationOverlay, MigrationStatus, Reconnecting, StartupLoading, Z_OVERLAY,
    Z_STARTUP, Z_TOAST,
};
use crate::native_toast::{Toast, ToastOptions};

/// Backdrop stack order (`ui/dialog.tsx:66`).
pub const MODAL_Z_INDEX: u16 = 3000;
/// Diff file-menu portal (`diff-viewer-file-menu.tsx:40,50`).
pub const FILE_MENU_Z_INDEX: u16 = 2600;
/// Shared side padding (`ui/dialog.tsx:26`).
pub const DIALOG_INSET: u16 = 3;
/// List rows reserve one gutter for the `▌` bar / `●` marker
/// (`ui/dialog.tsx:28`).
pub const DIALOG_LIST_INSET: u16 = DIALOG_INSET + 1;
/// Full-frame backdrop alpha (`ui/dialog.tsx:70`).
pub const MODAL_BACKDROP_ALPHA: u8 = 150;
/// The panel's top padding (`ui/dialog.tsx:83`).
pub const DIALOG_PANEL_PADDING_TOP: u16 = 1;
/// Footer actions stack vertically below this terminal width
/// (`ui/dialog.tsx:214`).
pub const DIALOG_FOOTER_NARROW_WIDTH: u16 = 60;
/// Refocus timeout (`ui/dialog.tsx:328,342`).
pub const REFOCUS_DELAY_MS: u64 = 1;

/// Dialog size classes (`ui/dialog.tsx:24`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum DialogSize {
    #[default]
    Medium,
    Large,
    XLarge,
}

impl DialogSize {
    /// `dialogWidth(size)` (`ui/dialog.tsx:30-34`).
    #[must_use]
    pub const fn width(self) -> u16 {
        match self {
            DialogSize::Medium => 60,
            DialogSize::Large => 88,
            DialogSize::XLarge => 116,
        }
    }
}

/// Rectangle in frame cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// One frame's paint instructions for the single visible dialog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogPaint {
    pub backdrop: DialogRect,
    pub panel: DialogRect,
    pub z_index: u16,
    pub backdrop_alpha: u8,
    pub size: DialogSize,
    pub centered: bool,
}

/// Saved focus target of the shell when the first dialog opened
/// (`ui/dialog.tsx:400-404`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModalFocus {
    pub id: u64,
    /// `focus.isDestroyed` (`ui/dialog.tsx:331`).
    pub destroyed: bool,
    /// Still reachable from `renderer.root` (`ui/dialog.tsx:332-341`).
    pub attached: bool,
}

impl ModalFocus {
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self {
            id,
            destroyed: false,
            attached: true,
        }
    }
}

/// Key handled by the modal layer (`ui/dialog.tsx:349-384`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ModalKey {
    /// A selection existed, so the key cleared it and closed nothing
    /// (`ui/dialog.tsx:354-357,369-372`).
    SelectionCleared,
    /// The focused editor had text, so Ctrl+C emptied it and closed nothing
    /// (`ui/dialog.tsx:373-378`).
    EditorCleared,
    /// The top entry was closed (its `onClose` ran).
    Closed,
    /// No dialog was open.
    Ignored,
}

/// Backdrop release outcome (`ui/dialog.tsx:54-60`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum BackdropRelease {
    /// `dismiss` was armed by mouse-down: the release is swallowed once so a
    /// drag-select does not close the dialog.
    Suppressed,
    /// The backdrop was clicked without a selection: the dialog closes.
    Closed,
}

/// Panel release outcome (`ui/dialog.tsx:73-79`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PanelRelease {
    /// A selection release bubbles to the copy-on-select handler and must not
    /// close the dialog.
    Bubbled,
    /// Release consumed; a pending backdrop dismiss is cancelled.
    Stopped,
}

/// Entry on the dialog stack. The stack is a `Vec` because the upstream
/// context exposes it as one (`ui/dialog.tsx:307-314,420-422`), but the public
/// `replace` operation keeps it at exactly one entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogEntry {
    pub id: u64,
    pub key: Option<u64>,
}

/// A source of monotonically non-decreasing milliseconds.
///
/// Every timer in this module takes its reading through this trait so the
/// decision path is deterministic and testable. The production caller passes
/// a clock backed by its own event-loop timestamp; tests pass
/// [`ManualClock`]. No implementation here consults an ambient clock.
pub trait Clock {
    fn now_ms(&self) -> u64;
}

/// Deterministic clock for tests and injectable production clocks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManualClock {
    now_ms: u64,
}

impl ManualClock {
    /// Clock reading `start_ms`.
    #[must_use]
    pub const fn new(start_ms: u64) -> Self {
        Self { now_ms: start_ms }
    }

    /// Move forward by `ms`, saturating.
    pub fn advance(&mut self, ms: u64) {
        self.now_ms = self.now_ms.saturating_add(ms);
    }

    /// Move to an absolute reading; a backwards target is ignored so the
    /// clock can never run backwards and re-fire a passed deadline.
    pub fn advance_to(&mut self, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

impl Clock for u64 {
    fn now_ms(&self) -> u64 {
        *self
    }
}

/// Every surface that can overlap the session in one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ChromeLayer {
    /// Toast surface; no z-index upstream, so painted first.
    Toast,
    /// Diff viewer file menu (`diff-viewer-file-menu.tsx`).
    DiffFileMenu,
    /// Modal dialog backdrop (`ui/dialog.tsx:66`).
    Dialog,
    /// Startup loading hint (`startup-loading.tsx:56`).
    Startup,
    /// Reconnecting backdrop (`reconnecting.tsx:11`).
    Reconnecting,
    /// Migration progress badge (`migration-overlay.tsx:52`).
    Migration,
}

impl ChromeLayer {
    /// Paint z-index for this layer.
    #[must_use]
    pub const fn z_index(self) -> u16 {
        match self {
            ChromeLayer::Toast => Z_TOAST,
            ChromeLayer::DiffFileMenu => FILE_MENU_Z_INDEX,
            ChromeLayer::Dialog => MODAL_Z_INDEX,
            ChromeLayer::Startup => Z_STARTUP,
            ChromeLayer::Reconnecting | ChromeLayer::Migration => Z_OVERLAY,
        }
    }

    /// Only layers that push a keymap layer upstream can consume keys.
    #[must_use]
    pub const fn interactive(self) -> bool {
        matches!(self, ChromeLayer::Dialog | ChromeLayer::DiffFileMenu)
    }

    /// Stable machine-readable name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            ChromeLayer::Toast => "toast",
            ChromeLayer::DiffFileMenu => "diff_file_menu",
            ChromeLayer::Dialog => "dialog",
            ChromeLayer::Startup => "startup",
            ChromeLayer::Reconnecting => "reconnecting",
            ChromeLayer::Migration => "migration",
        }
    }
}

/// Which layer consumed a routed key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModalRoute {
    /// Layer that handled the key, or `None` when no interactive layer was
    /// active (the key belongs to the shell).
    pub layer: Option<ChromeLayer>,
    /// What the layer did with it.
    pub key: ModalKey,
}

/// Dialog stack and focus guard (`ui/dialog.tsx:306-439`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModalLayer {
    stack: Vec<DialogEntry>,
    size: DialogSize,
    centered: bool,
    focus: Option<ModalFocus>,
    focused_blurred: bool,
    dismiss_armed: bool,
    closed: u32,
    selection_clears: u32,
    editor_clears: u32,
    refocus_due_ms: Option<u64>,
    refocus_requested: u32,
    /// Activation order of the interactive chrome layers. Later entries are
    /// dismissed first, whatever their z-index (`diff-viewer-file-menu.tsx`
    /// pushes `menu` on top of the dialog stack's `modal`).
    activated: Vec<ChromeLayer>,
}

impl ModalLayer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn stack(&self) -> &[DialogEntry] {
        &self.stack
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    #[must_use]
    pub const fn size(&self) -> DialogSize {
        self.size
    }

    #[must_use]
    pub const fn centered(&self) -> bool {
        self.centered
    }

    #[must_use]
    pub fn key(&self) -> Option<u64> {
        self.stack.last().and_then(|entry| entry.key)
    }

    /// `onClose` invocations observed so far.
    #[must_use]
    pub const fn closed(&self) -> u32 {
        self.closed
    }

    #[must_use]
    pub const fn selection_clears(&self) -> u32 {
        self.selection_clears
    }

    #[must_use]
    pub const fn editor_clears(&self) -> u32 {
        self.editor_clears
    }

    #[must_use]
    pub const fn refocus_requests(&self) -> u32 {
        self.refocus_requested
    }

    /// The saved shell focus, while a dialog owns focus.
    #[must_use]
    pub const fn saved_focus(&self) -> Option<ModalFocus> {
        self.focus
    }

    /// True while the previous focus target was blurred
    /// (`ui/dialog.tsx:402-403`).
    #[must_use]
    pub const fn focused_blurred(&self) -> bool {
        self.focused_blurred
    }

    /// The modal keymap mode is enabled exactly while the stack is non-empty
    /// (`ui/dialog.tsx:320-324,345-347`), so shell keys cannot reach the
    /// session underneath.
    #[must_use]
    pub fn shell_keys_blocked(&self) -> bool {
        self.is_open()
    }

    /// `replace()` (`ui/dialog.tsx:400-419`): on the first open the current
    /// focus is saved and blurred; every existing entry is closed; size and
    /// centered reset to the supplied values; exactly one entry remains.
    /// Returns the number of `onClose` callbacks run.
    pub fn replace(
        &mut self,
        id: u64,
        size: DialogSize,
        centered: bool,
        key: Option<u64>,
        focused: ModalFocus,
    ) -> u32 {
        if self.stack.is_empty() {
            self.focus = Some(focused);
            self.focused_blurred = true;
        }
        let closed = self.close_all();
        self.size = size;
        self.centered = centered;
        self.stack.push(DialogEntry { id, key });
        self.activate(ChromeLayer::Dialog);
        closed
    }

    /// Record that `layer` became interactive. Re-activating an already open
    /// layer moves it to the top of the dismissal order, matching a keymap
    /// layer that is pushed again on top.
    pub fn activate(&mut self, layer: ChromeLayer) {
        self.activated.retain(|active| *active != layer);
        self.activated.push(layer);
    }

    /// Record that `layer` stopped being interactive.
    pub fn deactivate(&mut self, layer: ChromeLayer) {
        self.activated.retain(|active| *active != layer);
    }

    /// Interactive layers in activation order (oldest first).
    #[must_use]
    pub fn activations(&self) -> &[ChromeLayer] {
        &self.activated
    }

    /// Most recently activated interactive layer — the one that owns keys.
    /// Non-interactive overlays are skipped: they declare no keymap layer
    /// upstream (`startup-loading.tsx`, `reconnecting.tsx`,
    /// `migration-overlay.tsx`).
    #[must_use]
    pub fn top_interactive(&self) -> Option<ChromeLayer> {
        self.activated.iter().rev().copied().find(|layer| layer.interactive())
    }

    /// Escape routed through the activation order. The diff file menu is
    /// dismissed first when it was opened last
    /// (`diff-viewer-file-menu.tsx:27,51-55`); otherwise the dialog's own
    /// selection-then-close rules apply (`ui/dialog.tsx:349-363`).
    pub fn escape_route(&mut self, has_selection: bool) -> ModalRoute {
        match self.top_interactive() {
            Some(ChromeLayer::DiffFileMenu) => {
                self.deactivate(ChromeLayer::DiffFileMenu);
                ModalRoute {
                    layer: Some(ChromeLayer::DiffFileMenu),
                    key: ModalKey::Closed,
                }
            }
            Some(ChromeLayer::Dialog) => ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: self.escape(has_selection),
            },
            _ => ModalRoute {
                layer: None,
                key: ModalKey::Ignored,
            },
        }
    }

    /// Ctrl+C routed through the activation order, with the same precedence:
    /// the file menu closes before the dialog reads the key.
    pub fn ctrl_c_route(&mut self, has_selection: bool, editor_has_text: bool) -> ModalRoute {
        match self.top_interactive() {
            Some(ChromeLayer::DiffFileMenu) => {
                self.deactivate(ChromeLayer::DiffFileMenu);
                ModalRoute {
                    layer: Some(ChromeLayer::DiffFileMenu),
                    key: ModalKey::Closed,
                }
            }
            Some(ChromeLayer::Dialog) => ModalRoute {
                layer: Some(ChromeLayer::Dialog),
                key: self.ctrl_c(has_selection, editor_has_text),
            },
            _ => ModalRoute {
                layer: None,
                key: ModalKey::Ignored,
            },
        }
    }

    /// Close every entry, reset the presentation flags and request a refocus.
    pub fn clear(&mut self) -> u32 {
        let closed = self.close_all();
        self.size = DialogSize::Medium;
        self.centered = false;
        if closed > 0 {
            self.request_refocus();
        }
        closed
    }

    /// Escape handler (`ui/dialog.tsx:349-363`). A selection is cleared
    /// instead of closing. When the last entry closes, refocus is requested.
    pub fn escape(&mut self, has_selection: bool) -> ModalKey {
        if !self.is_open() {
            return ModalKey::Ignored;
        }
        if has_selection {
            self.selection_clears = self.selection_clears.saturating_add(1);
            return ModalKey::SelectionCleared;
        }
        self.pop_top();
        ModalKey::Closed
    }

    /// Ctrl+C handler (`ui/dialog.tsx:364-384`). A selection wins over the
    /// editor, and a non-empty editor wins over closing.
    pub fn ctrl_c(&mut self, has_selection: bool, editor_has_text: bool) -> ModalKey {
        if !self.is_open() {
            return ModalKey::Ignored;
        }
        if has_selection {
            self.selection_clears = self.selection_clears.saturating_add(1);
            return ModalKey::SelectionCleared;
        }
        if editor_has_text {
            self.editor_clears = self.editor_clears.saturating_add(1);
            return ModalKey::EditorCleared;
        }
        self.pop_top();
        ModalKey::Closed
    }

    /// Backdrop mouse-down (`ui/dialog.tsx:51-53`): a selection arms dismiss
    /// so the following release does not close the dialog.
    pub fn backdrop_mouse_down(&mut self, has_selection: bool) {
        self.dismiss_armed = has_selection;
    }

    /// Backdrop mouse-up (`ui/dialog.tsx:54-60`).
    pub fn backdrop_mouse_up(&mut self) -> BackdropRelease {
        if self.dismiss_armed {
            self.dismiss_armed = false;
            return BackdropRelease::Suppressed;
        }
        self.clear();
        BackdropRelease::Closed
    }

    /// Panel mouse-up (`ui/dialog.tsx:73-79`). A selection release must bubble
    /// so copy-on-select sees it; otherwise the release stops here.
    pub fn panel_mouse_up(&mut self, has_selected_text: bool) -> PanelRelease {
        if has_selected_text {
            return PanelRelease::Bubbled;
        }
        self.dismiss_armed = false;
        PanelRelease::Stopped
    }

    /// Number of dialogs painted this frame. Only the top entry renders
    /// (`ui/dialog.tsx:472-479`), so this is 0 or 1 — never two overlapping
    /// panels, even if entries were ever stacked.
    #[must_use]
    pub fn painted_dialogs(&self) -> u8 {
        if self.is_open() {
            1
        } else {
            0
        }
    }

    /// Paint geometry for the current top entry
    /// (`ui/dialog.tsx:48-90,472-479`). `content_height` is the caller-side
    /// intrinsic height of the dialog body; the panel adds one padding row.
    #[must_use]
    pub fn paint(
        &self,
        terminal_width: u16,
        terminal_height: u16,
        content_height: u16,
    ) -> Option<DialogPaint> {
        self.stack.last().map(|_top| {
            let panel_width = dialog_panel_width(self.size, terminal_width);
            let panel_height = content_height.saturating_add(DIALOG_PANEL_PADDING_TOP);
            let panel_x = terminal_width.saturating_sub(panel_width) / 2;
            let panel_y = dialog_panel_y(self.centered, terminal_height, panel_height);
            DialogPaint {
                backdrop: DialogRect {
                    x: 0,
                    y: 0,
                    width: terminal_width,
                    height: terminal_height,
                },
                panel: DialogRect {
                    x: panel_x,
                    y: panel_y,
                    width: panel_width,
                    height: panel_height,
                },
                z_index: MODAL_Z_INDEX,
                backdrop_alpha: MODAL_BACKDROP_ALPHA,
                size: self.size,
                centered: self.centered,
            }
        })
    }

    /// The 1 ms refocus timer (`ui/dialog.tsx:326-343`): only fires once the
    /// stack is empty and the saved target is alive and still attached.
    pub fn take_refocus(&mut self, now_ms: u64) -> Option<ModalFocus> {
        let due = self.refocus_due_ms?;
        if now_ms < due || self.is_open() {
            return None;
        }
        self.refocus_due_ms = None;
        let focus = self.focus.take()?;
        self.focused_blurred = false;
        if focus.destroyed || !focus.attached {
            return None;
        }
        Some(focus)
    }

    /// [`ModalLayer::take_refocus`] reading the deadline from an injected
    /// clock, so the caller never has to thread a raw timestamp through.
    pub fn take_refocus_at(&mut self, clock: &impl Clock) -> Option<ModalFocus> {
        self.take_refocus(clock.now_ms())
    }

    fn pop_top(&mut self) {
        if self.stack.pop().is_some() {
            self.closed = self.closed.saturating_add(1);
            if self.stack.is_empty() {
                self.deactivate(ChromeLayer::Dialog);
                self.request_refocus();
            }
        }
    }

    fn close_all(&mut self) -> u32 {
        let closed = self.stack.len() as u32;
        self.stack.clear();
        self.closed = self.closed.saturating_add(closed);
        if closed > 0 {
            self.deactivate(ChromeLayer::Dialog);
        }
        closed
    }

    fn request_refocus(&mut self) {
        if self.focus.is_none() {
            return;
        }
        self.refocus_requested = self.refocus_requested.saturating_add(1);
        // `setTimeout(..., 1)`: the deadline is 1 ms after the close. The
        // caller supplies the clock in `take_refocus`.
        self.refocus_due_ms = Some(REFOCUS_DELAY_MS);
    }
}

/// `dialogWidth(size)` capped by `maxWidth={dimensions().width - 2}`
/// (`ui/dialog.tsx:30-34,80-81`). A terminal narrower than 2 paints nothing.
#[must_use]
pub fn dialog_panel_width(size: DialogSize, terminal_width: u16) -> u16 {
    let max = terminal_width.saturating_sub(2);
    size.width().min(max)
}

/// Horizontal origin: the backdrop uses `alignItems="center"`
/// (`ui/dialog.tsx:63`).
#[must_use]
pub fn dialog_panel_x(size: DialogSize, terminal_width: u16) -> u16 {
    terminal_width.saturating_sub(dialog_panel_width(size, terminal_width)) / 2
}

/// Vertical origin: `paddingTop = centered ? 0 : height / 4` on the backdrop
/// (`ui/dialog.tsx:67`) plus the panel's own one-row top padding
/// (`ui/dialog.tsx:83`). Centered dialogs are vertically centered by the
/// backdrop's `justifyContent="center"` (`ui/dialog.tsx:64`).
#[must_use]
pub fn dialog_panel_y(centered: bool, terminal_height: u16, panel_height: u16) -> u16 {
    let outer_top = if centered { 0 } else { terminal_height / 4 };
    if centered {
        outer_top.saturating_add(terminal_height.saturating_sub(panel_height) / 2)
    } else {
        outer_top.saturating_add(DIALOG_PANEL_PADDING_TOP)
    }
}

/// Footer action direction (`ui/dialog.tsx:214,245-246`).
#[must_use]
pub const fn dialog_footer_narrow(terminal_width: u16) -> bool {
    terminal_width < DIALOG_FOOTER_NARROW_WIDTH
}

/// One frame's decided chrome: paint order, who owns keys, and the dim
/// backdrop that actually applies.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChromeFrame {
    /// Active layers, bottom first: sorted by `(z-index, mount order)`.
    pub paint: Vec<ChromeLayer>,
    /// Interactive layer that owns Escape/Ctrl+C, if any.
    pub input_owner: Option<ChromeLayer>,
    /// Whether a toast is on screen.
    pub toast_visible: bool,
    /// Queued toasts behind the visible one (`+N more`).
    pub pending: usize,
    /// Topmost active full-screen dim, with its alpha.
    pub backdrop: Option<(ChromeLayer, u8)>,
}

impl ChromeFrame {
    /// True when the session shell underneath must not receive keys.
    #[must_use]
    pub fn shell_keys_blocked(&self) -> bool {
        self.input_owner.is_some()
    }

    /// Whether `layer` is painted this frame.
    #[must_use]
    pub fn paints(&self, layer: ChromeLayer) -> bool {
        self.paint.contains(&layer)
    }
}

/// Composition root for every overlapping chrome layer: the dialog stack,
/// the diff file menu, the startup hint, the reconnecting backdrop, the
/// migration badge and the toast surface.
///
/// Timers advance only through [`ChromeModel::tick`], whose only time source
/// is the injected [`Clock`]. Status changes arrive as explicit events
/// (`set_plugins_ready`, `set_connection`, `apply_migration`) exactly like
/// the upstream effects, which reschedule their timeouts on a change rather
/// than on every frame.
pub struct ChromeModel {
    /// Dialog stack and activation order.
    pub modal: ModalLayer,
    startup: StartupLoading,
    reconnecting: Reconnecting,
    migration: MigrationOverlay,
    toast: Toast,
    ready: bool,
    phase: ConnectionPhase,
    last_now: u64,
    /// Mount order of every layer that has ever been active; OpenTUI paints
    /// equal z-index layers in insertion order.
    order: Vec<ChromeLayer>,
    file_menu: bool,
}

impl Default for ChromeModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromeModel {
    /// All timers at logical time zero, nothing visible.
    #[must_use]
    pub fn new() -> Self {
        Self {
            modal: ModalLayer::new(),
            startup: StartupLoading::new(),
            reconnecting: Reconnecting::new(),
            migration: MigrationOverlay::new(),
            toast: Toast::new(),
            ready: false,
            phase: ConnectionPhase::Connecting,
            last_now: 0,
            order: Vec::new(),
            file_menu: false,
        }
    }

    /// Toast controller (read-only view).
    #[must_use]
    pub fn toast(&self) -> &Toast {
        &self.toast
    }

    /// Startup hint controller (read-only view).
    #[must_use]
    pub fn startup(&self) -> &StartupLoading {
        &self.startup
    }

    /// Reconnecting controller (read-only view).
    #[must_use]
    pub fn reconnecting(&self) -> &Reconnecting {
        &self.reconnecting
    }

    /// Migration controller (read-only view).
    #[must_use]
    pub fn migration(&self) -> &MigrationOverlay {
        &self.migration
    }

    /// Mount the transient layers at `now`: the startup hint starts its
    /// 500 ms grace and the migration overlay schedules its first poll one
    /// second later (`startup-loading.tsx:42-46`,
    /// `migration-overlay.tsx:18-27`).
    pub fn begin(&mut self, now: u64) {
        self.last_now = self.last_now.max(now);
        self.startup.update(now, self.ready);
        self.migration.begin(now);
    }

    /// `plugins.ready` changed. Only transitions reschedule the hint, so a
    /// steady `false` never restarts the 500 ms grace.
    pub fn set_plugins_ready(&mut self, now: u64, ready: bool) {
        if self.ready == ready {
            return;
        }
        self.ready = ready;
        self.startup.update(now, ready);
    }

    /// Connection status changed (`app.tsx:1378-1402`): `connected` hides the
    /// overlay immediately, otherwise the 5000 ms initial or 1000 ms retry
    /// grace starts.
    pub fn set_connection(&mut self, now: u64, phase: ConnectionPhase) {
        self.phase = phase;
        self.reconnecting.update(now, phase);
    }

    /// Current connection phase last reported to the model.
    #[must_use]
    pub const fn connection_phase(&self) -> ConnectionPhase {
        self.phase
    }

    /// Whether a migration status query is due; the caller performs the IO
    /// and reports the answer through [`ChromeModel::apply_migration`].
    pub fn migration_poll_due(&mut self, now: u64) -> bool {
        self.migration.poll_due(now)
    }

    /// Apply a migration status. Returns true when this failed run produced
    /// the single 10-second error toast (`migration-overlay.tsx:28-37`).
    pub fn apply_migration(&mut self, status: MigrationStatus) -> bool {
        self.migration.apply(status);
        match self.migration.take_error_toast() {
            Some(toast) => {
                self.toast.show(toast);
                true
            }
            None => false,
        }
    }

    /// Open the diff file menu as an interactive layer.
    pub fn open_file_menu(&mut self) {
        self.file_menu = true;
        self.modal.activate(ChromeLayer::DiffFileMenu);
        self.note_mount(ChromeLayer::DiffFileMenu);
    }

    /// Close the diff file menu (`Escape`, `Ctrl+C`, outside press or a run).
    pub fn close_file_menu(&mut self) {
        self.file_menu = false;
        self.modal.deactivate(ChromeLayer::DiffFileMenu);
    }

    /// Whether the diff file menu is open.
    #[must_use]
    pub const fn file_menu_open(&self) -> bool {
        self.file_menu
    }

    /// Escape routed across the whole chrome stack: the file menu first when
    /// it was opened last, then the dialog's selection/close rules
    /// (`diff-viewer-file-menu.tsx:27,51-55`, `ui/dialog.tsx:349-371`).
    pub fn escape_route(&mut self, has_selection: bool) -> ModalRoute {
        if self.file_menu && self.modal.top_interactive() == Some(ChromeLayer::DiffFileMenu) {
            self.close_file_menu();
            return ModalRoute {
                layer: Some(ChromeLayer::DiffFileMenu),
                key: ModalKey::Closed,
            };
        }
        self.modal.escape_route(has_selection)
    }

    /// Ctrl+C routed across the whole chrome stack with the same precedence.
    pub fn ctrl_c_route(&mut self, has_selection: bool, editor_has_text: bool) -> ModalRoute {
        if self.file_menu && self.modal.top_interactive() == Some(ChromeLayer::DiffFileMenu) {
            self.close_file_menu();
            return ModalRoute {
                layer: Some(ChromeLayer::DiffFileMenu),
                key: ModalKey::Closed,
            };
        }
        self.modal.ctrl_c_route(has_selection, editor_has_text)
    }

    /// Show or queue a toast (`ui/toast.tsx:159-167`).
    pub fn show_toast(&mut self, options: ToastOptions) {
        self.toast.show(options);
    }

    /// Hover in pauses the remaining lifetime, hover out resumes it
    /// (`ui/toast.tsx:127-130,171-181`).
    pub fn hover_toast(&mut self, hovered: bool) {
        if hovered {
            self.toast.pause();
        } else {
            self.toast.resume();
        }
    }

    /// Activate the toast surface. A release that ends a text selection must
    /// not activate it (`ui/toast.tsx:59-62`), so `has_selection` returns
    /// `None` and changes nothing. Otherwise the toast is dismissed *before*
    /// its action runs (`ui/toast.tsx:183-187`).
    pub fn activate_toast(&mut self, has_selection: bool) -> Option<String> {
        if has_selection {
            return None;
        }
        self.toast.activate()
    }

    /// Dismiss the visible toast without running its action.
    pub fn dismiss_toast(&mut self) {
        self.toast.dismiss();
    }

    /// Advance every timer to the clock reading and return the frame.
    pub fn tick(&mut self, clock: &impl Clock) -> ChromeFrame {
        let now = clock.now_ms();
        let delta = now.saturating_sub(self.last_now);
        self.last_now = self.last_now.max(now);
        self.startup.tick(now);
        self.reconnecting.tick(now);
        self.toast.advance(delta);
        for layer in self.active_layers() {
            self.note_mount(layer);
        }
        self.frame()
    }

    /// Current frame without advancing time.
    #[must_use]
    pub fn frame(&self) -> ChromeFrame {
        let active = self.active_layers();
        let mut paint: Vec<ChromeLayer> = active
            .iter()
            .copied()
            .filter(|layer| *layer != ChromeLayer::DiffFileMenu || self.file_menu)
            .collect();
        paint.sort_by_key(|layer| {
            (
                layer.z_index(),
                self.order.iter().position(|mounted| mounted == layer).unwrap_or(usize::MAX),
            )
        });
        let backdrop_layer = paint
            .iter()
            .rev()
            .copied()
            .find(|layer| matches!(layer, ChromeLayer::Dialog | ChromeLayer::Reconnecting));
        let backdrop = backdrop_layer.map(|layer| (layer, MODAL_BACKDROP_ALPHA));
        ChromeFrame {
            input_owner: self.modal.top_interactive(),
            toast_visible: self.toast.current().is_some(),
            pending: self.toast.pending(),
            backdrop,
            paint,
        }
    }

    fn active_layers(&self) -> Vec<ChromeLayer> {
        let mut active = Vec::new();
        if self.toast.current().is_some() {
            active.push(ChromeLayer::Toast);
        }
        if self.file_menu {
            active.push(ChromeLayer::DiffFileMenu);
        }
        if self.modal.is_open() {
            active.push(ChromeLayer::Dialog);
        }
        if self.startup.visible() {
            active.push(ChromeLayer::Startup);
        }
        if self.reconnecting.visible() {
            active.push(ChromeLayer::Reconnecting);
        }
        if self.migration.visible() {
            active.push(ChromeLayer::Migration);
        }
        active
    }

    fn note_mount(&mut self, layer: ChromeLayer) {
        if !self.order.contains(&layer) {
            self.order.push(layer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focus(id: u64) -> ModalFocus {
        ModalFocus::new(id)
    }

    fn open(layer: &mut ModalLayer, id: u64) {
        layer.replace(id, DialogSize::Medium, false, None, focus(7));
    }

    #[test]
    fn dialog_widths_and_insets_match_upstream() {
        assert_eq!(DialogSize::Medium.width(), 60);
        assert_eq!(DialogSize::Large.width(), 88);
        assert_eq!(DialogSize::XLarge.width(), 116);
        assert_eq!(DIALOG_INSET, 3);
        assert_eq!(DIALOG_LIST_INSET, 4);
    }

    #[test]
    fn backdrop_is_full_screen_at_z3000_with_alpha150() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        let paint = layer.paint(100, 30, 10).expect("paint");
        assert_eq!(
            paint.backdrop,
            DialogRect {
                x: 0,
                y: 0,
                width: 100,
                height: 30
            }
        );
        assert_eq!(paint.z_index, 3000);
        assert_eq!(paint.backdrop_alpha, 150);
        assert_eq!(paint.size, DialogSize::Medium);
        assert!(!paint.centered);
    }

    #[test]
    fn panel_width_is_capped_two_cells_below_the_terminal() {
        assert_eq!(dialog_panel_width(DialogSize::XLarge, 100), 98);
        assert_eq!(dialog_panel_width(DialogSize::Medium, 100), 60);
        assert_eq!(dialog_panel_width(DialogSize::Medium, 1), 0);
        assert_eq!(dialog_panel_x(DialogSize::Medium, 100), 20);
    }

    #[test]
    fn non_centered_panel_starts_after_a_quarter_height_plus_padding() {
        // height 40 => paddingTop 10, panel paddingTop 1 => y 11.
        assert_eq!(dialog_panel_y(false, 40, 10), 11);
        // Centered: (40 - 11) / 2 = 14.
        assert_eq!(dialog_panel_y(true, 40, 11), 14);
        let mut layer = ModalLayer::new();
        layer.replace(1, DialogSize::Large, true, None, focus(1));
        let paint = layer.paint(100, 40, 10).expect("centered paint");
        assert!(paint.centered);
        assert_eq!(paint.panel.x, 6);
        assert_eq!(paint.panel.y, 14);
    }

    #[test]
    fn only_the_top_dialog_paints_and_entries_never_overlap() {
        let mut layer = ModalLayer::new();
        assert_eq!(layer.painted_dialogs(), 0);
        assert!(layer.paint(80, 24, 5).is_none());
        open(&mut layer, 1);
        assert_eq!(layer.painted_dialogs(), 1);
        // `replace` closes the previous entry and installs exactly one.
        let closed = layer.replace(2, DialogSize::Large, true, Some(9), focus(8));
        assert_eq!(closed, 1);
        assert_eq!(layer.stack().len(), 1);
        assert_eq!(layer.key(), Some(9));
        assert_eq!(layer.painted_dialogs(), 1);
    }

    #[test]
    fn replace_resets_size_and_centered_instead_of_reusing_stale_flags() {
        let mut layer = ModalLayer::new();
        layer.replace(1, DialogSize::XLarge, true, None, focus(1));
        layer.replace(2, DialogSize::Medium, false, None, focus(2));
        assert_eq!(layer.size(), DialogSize::Medium);
        assert!(!layer.centered());
        let paint = layer.paint(120, 40, 5).expect("paint");
        assert_eq!(paint.size, DialogSize::Medium);
        assert_eq!(paint.panel.width, 60);
    }

    #[test]
    fn modal_mode_blocks_shell_keys_until_the_stack_is_empty() {
        let mut layer = ModalLayer::new();
        assert!(!layer.shell_keys_blocked());
        open(&mut layer, 1);
        assert!(layer.shell_keys_blocked());
        assert_eq!(layer.escape(false), ModalKey::Closed);
        assert!(!layer.shell_keys_blocked());
    }

    #[test]
    fn escape_closes_selection_first_then_the_top_entry() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        assert_eq!(layer.escape(true), ModalKey::SelectionCleared);
        assert_eq!(layer.selection_clears(), 1);
        assert!(layer.is_open());
        assert_eq!(layer.escape(false), ModalKey::Closed);
        assert_eq!(layer.closed(), 1);
        assert!(!layer.is_open());
        assert_eq!(layer.escape(false), ModalKey::Ignored);
    }

    #[test]
    fn ctrl_c_clears_editor_text_before_closing() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        assert_eq!(layer.ctrl_c(false, true), ModalKey::EditorCleared);
        assert_eq!(layer.editor_clears(), 1);
        assert!(layer.is_open());
        assert_eq!(layer.ctrl_c(false, false), ModalKey::Closed);
        assert!(!layer.is_open());
    }

    #[test]
    fn ctrl_c_clears_a_selection_before_touching_the_editor() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        assert_eq!(layer.ctrl_c(true, true), ModalKey::SelectionCleared);
        assert_eq!(layer.selection_clears(), 1);
        assert_eq!(layer.editor_clears(), 0);
        assert!(layer.is_open());
    }

    #[test]
    fn backdrop_release_is_suppressed_once_after_a_selection_drag() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        layer.backdrop_mouse_down(true);
        assert_eq!(layer.backdrop_mouse_up(), BackdropRelease::Suppressed);
        assert!(layer.is_open());
        // A plain backdrop click closes.
        layer.backdrop_mouse_down(false);
        assert_eq!(layer.backdrop_mouse_up(), BackdropRelease::Closed);
        assert!(!layer.is_open());
        assert_eq!(layer.closed(), 1);
    }

    #[test]
    fn panel_release_with_selection_bubbles_and_keeps_the_dialog() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        layer.backdrop_mouse_down(true);
        assert_eq!(layer.panel_mouse_up(true), PanelRelease::Bubbled);
        assert!(layer.is_open());
        // Text-less release stops propagation and cancels a pending dismiss.
        assert_eq!(layer.panel_mouse_up(false), PanelRelease::Stopped);
        assert_eq!(layer.backdrop_mouse_up(), BackdropRelease::Closed);
    }

    #[test]
    fn first_open_saves_and_blurs_focus_and_close_refocuses_after_1ms() {
        let mut layer = ModalLayer::new();
        assert!(!layer.focused_blurred());
        open(&mut layer, 1);
        assert!(layer.focused_blurred());
        assert_eq!(layer.saved_focus(), Some(focus(7)));
        assert_eq!(layer.refocus_requests(), 0);

        assert_eq!(layer.escape(false), ModalKey::Closed);
        assert_eq!(layer.refocus_requests(), 1);
        // Not before the 1 ms timer.
        assert!(layer.take_refocus(0).is_none());
        assert_eq!(layer.take_refocus(1), Some(focus(7)));
        assert!(!layer.focused_blurred());
        // Only one restore per close.
        assert!(layer.take_refocus(2).is_none());
    }

    #[test]
    fn destroyed_or_detached_focus_is_not_restored() {
        let mut layer = ModalLayer::new();
        let mut saved = focus(3);
        saved.destroyed = true;
        layer.replace(1, DialogSize::Medium, false, None, saved);
        layer.clear();
        assert!(layer.take_refocus(1).is_none());

        let mut layer = ModalLayer::new();
        let mut detached = focus(4);
        detached.attached = false;
        layer.replace(1, DialogSize::Medium, false, None, detached);
        layer.clear();
        assert!(layer.take_refocus(1).is_none());
    }

    #[test]
    fn refocus_waits_until_the_stack_is_empty() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        layer.clear();
        // Reopened before the timer fires: nothing to restore.
        layer.replace(2, DialogSize::Medium, false, None, focus(9));
        assert!(layer.take_refocus(5).is_none());
        assert_eq!(layer.saved_focus(), Some(focus(9)));
    }

    #[test]
    fn footer_stacks_below_60_cells() {
        assert!(dialog_footer_narrow(59));
        assert!(!dialog_footer_narrow(60));
    }

    // ---------- deterministic clock / layer composition (DIALOG-19) ----------

    const SOURCE: &str = include_str!("native_modal_layer.rs");

    #[test]
    fn decision_path_never_reads_an_ambient_clock() {
        // Built from fragments so this assertion does not match itself.
        for token in [
            ["System", "Time"].concat(),
            ["In", "stant::now"].concat(),
            ["std", "::time"].concat(),
        ] {
            assert!(
                !SOURCE.contains(&token),
                "decision path must use the injected Clock, found {token}"
            );
        }
    }

    #[test]
    fn manual_clock_never_runs_backwards() {
        let mut clock = ManualClock::new(100);
        clock.advance(50);
        assert_eq!(clock.now_ms(), 150);
        clock.advance_to(120);
        assert_eq!(clock.now_ms(), 150);
        clock.advance_to(400);
        assert_eq!(clock.now_ms(), 400);
        assert_eq!(Clock::now_ms(&7u64), 7);
    }

    #[test]
    fn startup_hint_waits_500ms_and_holds_three_seconds() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.begin(0);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Startup));
        clock.advance_to(499);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Startup));
        clock.advance_to(500);
        assert!(chrome.tick(&clock).paints(ChromeLayer::Startup));
        // Ready at 1000: the hint still holds until 500 + 3000.
        chrome.set_plugins_ready(1000, true);
        clock.advance_to(3_499);
        assert!(chrome.tick(&clock).paints(ChromeLayer::Startup));
        clock.advance_to(3_500);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Startup));
    }

    #[test]
    fn startup_ready_before_the_grace_cancels_the_hint() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.begin(0);
        clock.advance_to(300);
        chrome.set_plugins_ready(300, true);
        clock.advance_to(5_000);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Startup));
    }

    #[test]
    fn reconnecting_uses_5s_then_1s_graces_and_hides_immediately() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.set_connection(0, ConnectionPhase::Connecting);
        clock.advance_to(4_999);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Reconnecting));
        clock.advance_to(5_000);
        assert!(chrome.tick(&clock).paints(ChromeLayer::Reconnecting));
        chrome.set_connection(5_000, ConnectionPhase::Connected);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Reconnecting));
        chrome.set_connection(5_000, ConnectionPhase::Reconnecting);
        clock.advance_to(5_999);
        assert!(!chrome.tick(&clock).paints(ChromeLayer::Reconnecting));
        clock.advance_to(6_000);
        assert!(chrome.tick(&clock).paints(ChromeLayer::Reconnecting));
    }

    #[test]
    fn migration_polls_at_one_second_then_every_second() {
        let mut chrome = ChromeModel::new();
        chrome.begin(0);
        assert!(!chrome.migration_poll_due(999));
        assert!(chrome.migration_poll_due(1_000));
        assert!(!chrome.migration_poll_due(1_999));
        assert!(chrome.migration_poll_due(2_000));
    }

    #[test]
    fn migration_failure_emits_one_ten_second_error_toast() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.begin(0);
        assert!(chrome.apply_migration(MigrationStatus::Error("boom".to_owned())));
        assert!(!chrome.apply_migration(MigrationStatus::Error("boom".to_owned())));
        let toast = chrome.toast().current().expect("error toast");
        assert_eq!(toast.title.as_deref(), Some("Data migration failed"));
        assert_eq!(toast.duration, 10_000);
        clock.advance_to(9_999);
        assert!(chrome.tick(&clock).toast_visible);
        clock.advance_to(10_000);
        assert!(!chrome.tick(&clock).toast_visible);
    }

    #[test]
    fn toast_replaces_when_idle_and_queues_while_paused() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.show_toast(ToastOptions::new(
            crate::native_toast::ToastVariant::Info,
            "first",
        ));
        chrome.tick(&clock);
        chrome.hover_toast(true);
        chrome.show_toast(ToastOptions::new(
            crate::native_toast::ToastVariant::Info,
            "second",
        ));
        let frame = chrome.tick(&clock);
        assert_eq!(frame.pending, 1);
        assert_eq!(chrome.toast().current().map(|t| t.message.as_str()), Some("first"));
        // The paused toast never expires.
        clock.advance_to(60_000);
        chrome.tick(&clock);
        assert_eq!(chrome.toast().current().map(|t| t.message.as_str()), Some("first"));
        // Resume and dismiss: the queue head is promoted.
        chrome.hover_toast(false);
        chrome.dismiss_toast();
        assert_eq!(chrome.toast().current().map(|t| t.message.as_str()), Some("second"));
    }

    #[test]
    fn toast_activation_dismisses_before_running_the_action() {
        let mut chrome = ChromeModel::new();
        chrome.show_toast(
            ToastOptions::new(crate::native_toast::ToastVariant::Error, "failed")
                .with_action("retry", || {}),
        );
        // A release that ends a selection must not activate.
        assert_eq!(chrome.activate_toast(true), None);
        assert!(chrome.toast().current().is_some());
        assert_eq!(chrome.activate_toast(false).as_deref(), Some("retry"));
        assert!(chrome.toast().current().is_none());
    }

    #[test]
    fn escape_and_ctrl_c_dismiss_the_menu_before_the_dialog() {
        let mut chrome = ChromeModel::new();
        open(&mut chrome.modal, 1);
        chrome.open_file_menu();
        assert_eq!(chrome.modal.top_interactive(), Some(ChromeLayer::DiffFileMenu));
        let route = chrome.escape_route(false);
        assert_eq!(route.layer, Some(ChromeLayer::DiffFileMenu));
        assert!(!chrome.file_menu_open());
        assert!(chrome.modal.is_open());
        let route = chrome.escape_route(false);
        assert_eq!(route.layer, Some(ChromeLayer::Dialog));
        assert_eq!(route.key, ModalKey::Closed);
        assert!(!chrome.modal.is_open());

        open(&mut chrome.modal, 2);
        chrome.open_file_menu();
        let route = chrome.ctrl_c_route(false, false);
        assert_eq!(route.layer, Some(ChromeLayer::DiffFileMenu));
        assert!(!chrome.file_menu_open());
        assert!(chrome.modal.is_open());
    }

    #[test]
    fn overlapping_layers_paint_by_z_index_and_dialog_owns_keys() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.begin(0);
        chrome.set_connection(0, ConnectionPhase::Connecting);
        chrome.show_toast(ToastOptions::new(
            crate::native_toast::ToastVariant::Info,
            "hello",
        )
        .with_duration(60_000));
        open(&mut chrome.modal, 1);
        clock.advance_to(5_000);
        let frame = chrome.tick(&clock);
        // Toast (no z) < dialog 3000 < startup 5000 < overlay 10000.
        assert_eq!(
            frame.paint,
            vec![
                ChromeLayer::Toast,
                ChromeLayer::Dialog,
                ChromeLayer::Startup,
                ChromeLayer::Reconnecting,
            ]
        );
        // The reconnect backdrop is the topmost full-screen dim, but it is not
        // interactive: the dialog keeps the keys.
        assert_eq!(frame.backdrop, Some((ChromeLayer::Reconnecting, 150)));
        assert_eq!(frame.input_owner, Some(ChromeLayer::Dialog));
        assert!(frame.shell_keys_blocked());
        assert_eq!(ChromeLayer::DiffFileMenu.z_index(), 2600);
        assert_eq!(ChromeLayer::Dialog.z_index(), 3000);
    }

    #[test]
    fn migration_and_reconnect_share_z_but_keep_mount_order() {
        let mut chrome = ChromeModel::new();
        let mut clock = ManualClock::new(0);
        chrome.begin(0);
        chrome.set_connection(0, ConnectionPhase::Connecting);
        chrome.apply_migration(MigrationStatus::Running(Some(
            crate::native_overlay_stack::MigrationProgress {
                label: "migrating".to_owned(),
                numerator: Some(1),
                denominator: Some(2),
            },
        )));
        clock.advance_to(5_000);
        let frame = chrome.tick(&clock);
        let reconnect = frame.paint.iter().position(|l| *l == ChromeLayer::Reconnecting);
        let migration = frame.paint.iter().position(|l| *l == ChromeLayer::Migration);
        assert!(reconnect.unwrap() < migration.unwrap(), "{:?}", frame.paint);
        assert_eq!(ChromeLayer::Reconnecting.z_index(), ChromeLayer::Migration.z_index());
    }

    #[test]
    fn refocus_reads_the_injected_clock() {
        let mut layer = ModalLayer::new();
        open(&mut layer, 1);
        assert_eq!(layer.escape(false), ModalKey::Closed);
        let mut clock = ManualClock::new(0);
        assert!(layer.take_refocus_at(&clock).is_none());
        clock.advance(1);
        assert_eq!(layer.take_refocus_at(&clock), Some(focus(7)));
    }
}
