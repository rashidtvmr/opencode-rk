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

/// Backdrop stack order (`ui/dialog.tsx:66`).
pub const MODAL_Z_INDEX: u16 = 3000;
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
        closed
    }

    /// `clear()` (`ui/dialog.tsx:389-399`): close every entry, reset the
    /// presentation flags and request a refocus.
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

    fn pop_top(&mut self) {
        if self.stack.pop().is_some() {
            self.closed = self.closed.saturating_add(1);
            if self.stack.is_empty() {
                self.request_refocus();
            }
        }
    }

    fn close_all(&mut self) -> u32 {
        let closed = self.stack.len() as u32;
        self.stack.clear();
        self.closed = self.closed.saturating_add(closed);
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
}
