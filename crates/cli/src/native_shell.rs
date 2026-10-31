#![forbid(unsafe_code)]
//! Native shell paint state (opentui buffers).
//!
//! Pure state only: capped lines for shell paint buffers. No rendering,
//! no IO, no threads; caller paints `snapshot()` into the real buffer.

use std::collections::VecDeque;

use crate::native_timeline::{TimelineBuilder, TimelinePage};

/// Max chars retained per line.
pub const MAX_LINE: usize = 1024;
/// Max lines retained per buffer.
pub const MAX_LINES: usize = 500;

/// One paintable shell line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellLine {
    pub text: String,
    pub bold: bool,
}

impl ShellLine {
    #[must_use]
    pub fn new(text: impl Into<String>, bold: bool) -> Self {
        let raw: String = text.into();
        let text = if raw.chars().count() > MAX_LINE {
            raw.chars().take(MAX_LINE).collect()
        } else {
            raw
        };
        Self { text, bold }
    }
}

/// Bounded shell buffer with scrollback offset.
#[derive(Clone, Debug, Default)]
pub struct ShellBuffer {
    lines: VecDeque<ShellLine>,
    /// Rows scrolled up from latest (0 = pinned to bottom).
    scroll: usize,
}

impl ShellBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a line; evicts oldest when full.
    pub fn push_line(&mut self, text: impl Into<String>, bold: bool) {
        if self.lines.len() >= MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(ShellLine::new(text, bold));
        self.clamp_scroll();
    }

    /// Visible window: last `n` lines offset by scroll.
    #[must_use]
    pub fn snapshot(&self, n: usize) -> Vec<&ShellLine> {
        let len = self.lines.len();
        let scrolled = self.scroll.min(len);
        let end = len - scrolled;
        let start = end.saturating_sub(n);
        self.lines.iter().skip(start).take(end - start).collect()
    }

    /// Scroll up (away from latest) by `delta` rows.
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_add(delta).min(self.lines.len());
    }

    /// Scroll down (toward latest) by `delta` rows.
    pub fn scroll_down(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
    }

    /// Clear all lines and reset scroll.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    #[must_use]
    pub fn lines(&self) -> &VecDeque<ShellLine> {
        &self.lines
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.min(self.lines.len());
    }
}

/// Page snapshots: one bounded [`ShellBuffer`] per paint region
/// (transcript, composer, sidebar, dock, footer). Caller sizes each window
/// from the region height (`ShellLayout` rect) so pages never overlap.
///
/// `dock` holds the composed auxiliary-composer rows
/// (`routes/session/composer/index.tsx:84-155`); the shell tab's rows are
/// pushed there by [`SessionDock::paint`] so a caller can paint the dock from
/// the same buffer model it uses for every other region.
///
/// `footer` holds the prompt-footer rows (`component/prompt/index.tsx:1899-1967`)
/// and the progress diagnostics that [`ShellFooter::paint`] composes for the
/// turn: the running spinner / `esc interrupt` status row, the background
/// hint (`routes/session/index.tsx:1894-1926`), retry, error and token rows.
#[derive(Clone, Debug, Default)]
pub struct ShellPages {
    transcript: ShellBuffer,
    composer: ShellBuffer,
    sidebar: ShellBuffer,
    dock: ShellBuffer,
    footer: ShellBuffer,
}

impl ShellPages {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_transcript(&mut self, text: impl Into<String>, bold: bool) {
        self.transcript.push_line(text, bold);
    }

    /// Paint the timeline sidebar window into the transcript buffer.
    ///
    /// Calls [`TimelineBuilder::render_page`] with the caller's `page`,
    /// `height` (region rows) and `width` (chars per row), then pushes each
    /// bounded row into the transcript buffer. Rows are already bounded by
    /// `render_page` (`MAX_PAGE` rows, wrapped at `width`); the buffer caps
    /// them again at [`MAX_LINES`] lines / [`MAX_LINE`] chars. Returns the
    /// number of rows painted.
    pub fn paint_timeline(
        &mut self,
        builder: &TimelineBuilder,
        page: &TimelinePage,
        height: usize,
        width: usize,
    ) -> usize {
        let rows = builder.render_page(page, height, width);
        let n = rows.len();
        for row in rows {
            self.push_transcript(row, false);
        }
        n
    }

    pub fn push_composer(&mut self, text: impl Into<String>, bold: bool) {
        self.composer.push_line(text, bold);
    }

    pub fn push_sidebar(&mut self, text: impl Into<String>, bold: bool) {
        self.sidebar.push_line(text, bold);
    }

    #[must_use]
    pub fn transcript_window(&self, height: usize) -> Vec<&ShellLine> {
        self.transcript.snapshot(height)
    }

    #[must_use]
    pub fn composer_window(&self, height: usize) -> Vec<&ShellLine> {
        self.composer.snapshot(height)
    }

    #[must_use]
    pub fn sidebar_window(&self, height: usize) -> Vec<&ShellLine> {
        self.sidebar.snapshot(height)
    }

    pub fn scroll_transcript_up(&mut self, delta: usize) {
        self.transcript.scroll_up(delta);
    }

    pub fn scroll_transcript_down(&mut self, delta: usize) {
        self.transcript.scroll_down(delta);
    }

    pub fn scroll_composer_up(&mut self, delta: usize) {
        self.composer.scroll_up(delta);
    }

    pub fn scroll_composer_down(&mut self, delta: usize) {
        self.composer.scroll_down(delta);
    }

    pub fn scroll_sidebar_up(&mut self, delta: usize) {
        self.sidebar.scroll_up(delta);
    }

    pub fn scroll_sidebar_down(&mut self, delta: usize) {
        self.sidebar.scroll_down(delta);
    }

    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
    }

    pub fn clear_composer(&mut self) {
        self.composer.clear();
    }

    pub fn clear_sidebar(&mut self) {
        self.sidebar.clear();
    }

    #[must_use]
    pub fn transcript_len(&self) -> usize {
        self.transcript.len()
    }

    #[must_use]
    pub fn composer_len(&self) -> usize {
        self.composer.len()
    }

    #[must_use]
    pub fn sidebar_len(&self) -> usize {
        self.sidebar.len()
    }

    /// Append one composed dock row.
    pub fn push_dock(&mut self, text: impl Into<String>, bold: bool) {
        self.dock.push_line(text, bold);
    }

    /// Replace the dock region with exactly these rows (a dock repaint is a
    /// full block, not an append).
    pub fn set_dock(&mut self, rows: &[ShellLine]) -> usize {
        self.dock.clear();
        for row in rows {
            self.dock.push_line(row.text.clone(), row.bold);
        }
        rows.len()
    }

    #[must_use]
    pub fn dock_window(&self, height: usize) -> Vec<&ShellLine> {
        self.dock.snapshot(height)
    }

    #[must_use]
    pub fn dock_len(&self) -> usize {
        self.dock.len()
    }

    pub fn clear_dock(&mut self) {
        self.dock.clear();
    }

    /// Replace the footer region with exactly these rows (the footer is
    /// recomposed as a whole each frame: spinner frame, hint visibility, retry
    /// countdown).
    pub fn set_footer(&mut self, rows: &[ShellLine]) -> usize {
        self.footer.clear();
        for row in rows {
            self.footer.push_line(row.text.clone(), row.bold);
        }
        rows.len()
    }

    /// Append one footer row.
    pub fn push_footer(&mut self, text: impl Into<String>, bold: bool) {
        self.footer.push_line(text, bold);
    }

    #[must_use]
    pub fn footer_window(&self, height: usize) -> Vec<&ShellLine> {
        self.footer.snapshot(height)
    }

    #[must_use]
    pub fn footer_len(&self) -> usize {
        self.footer.len()
    }

    pub fn clear_footer(&mut self) {
        self.footer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_evicts_oldest_when_full() {
        let mut buf = ShellBuffer::new();
        for i in 0..(MAX_LINES + 10) {
            buf.push_line(format!("line{i}"), false);
        }
        assert_eq!(buf.len(), MAX_LINES);
        assert_eq!(buf.lines()[0].text, "line10");
        assert_eq!(
            buf.lines()[MAX_LINES - 1].text,
            format!("line{}", MAX_LINES + 9)
        );
    }

    #[test]
    fn line_text_capped_at_max_line() {
        let long = "x".repeat(MAX_LINE + 100);
        let line = ShellLine::new(long, true);
        assert_eq!(line.text.chars().count(), MAX_LINE);
        assert!(line.bold);
        let mut buf = ShellBuffer::new();
        buf.push_line("y".repeat(MAX_LINE + 1), false);
        assert_eq!(buf.lines()[0].text.chars().count(), MAX_LINE);
    }

    #[test]
    fn empty_snapshot_and_window() {
        let buf = ShellBuffer::new();
        assert!(buf.is_empty());
        assert!(buf.snapshot(10).is_empty());
        assert!(buf.snapshot(0).is_empty());
        let mut buf = ShellBuffer::new();
        buf.push_line("a", false);
        buf.push_line("b", true);
        let snap: Vec<&str> = buf.snapshot(10).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(snap, vec!["a", "b"]);
        let last: Vec<&str> = buf.snapshot(1).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(last, vec!["b"]);
        assert!(buf.snapshot(1)[0].bold);
    }

    #[test]
    fn scroll_offsets_snapshot_window() {
        let mut buf = ShellBuffer::new();
        for i in 0..5 {
            buf.push_line(format!("l{i}"), false);
        }
        buf.scroll_up(2);
        assert_eq!(buf.scroll(), 2);
        let snap: Vec<&str> = buf.snapshot(2).iter().map(|l| l.text.as_str()).collect();
        assert_eq!(snap, vec!["l1", "l2"]);
        buf.scroll_down(10);
        assert_eq!(buf.scroll(), 0);
    }

    #[test]
    fn pages_route_to_own_region() {
        let mut pages = ShellPages::new();
        pages.push_transcript("t1", false);
        pages.push_composer("c1", true);
        pages.push_sidebar("s1", false);
        let t: Vec<&str> = pages
            .transcript_window(10)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        let c: Vec<&str> = pages
            .composer_window(10)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        let s: Vec<&str> = pages
            .sidebar_window(10)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(t, vec!["t1"]);
        assert_eq!(c, vec!["c1"]);
        assert_eq!(s, vec!["s1"]);
    }

    #[test]
    fn page_windows_clamp_to_height() {
        let mut pages = ShellPages::new();
        for i in 0..5 {
            pages.push_transcript(format!("l{i}"), false);
        }
        let win: Vec<&str> = pages
            .transcript_window(2)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(win, vec!["l3", "l4"]);
        assert!(pages.transcript_window(0).is_empty());
        assert!(pages.sidebar_window(10).is_empty());
    }

    #[test]
    fn page_scroll_is_per_region() {
        let mut pages = ShellPages::new();
        for i in 0..5 {
            pages.push_transcript(format!("t{i}"), false);
            pages.push_sidebar(format!("s{i}"), false);
        }
        pages.scroll_transcript_up(2);
        let t: Vec<&str> = pages
            .transcript_window(2)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        let s: Vec<&str> = pages
            .sidebar_window(2)
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(t, vec!["t1", "t2"]);
        assert_eq!(s, vec!["s3", "s4"]);
    }

    #[test]
    fn page_buffers_respect_bounds() {
        let mut pages = ShellPages::new();
        for i in 0..(MAX_LINES + 10) {
            pages.push_transcript(format!("line{i}"), false);
        }
        assert_eq!(pages.transcript_len(), MAX_LINES);
        assert_eq!(
            pages.transcript_window(1)[0].text,
            format!("line{}", MAX_LINES + 9)
        );
        pages.push_composer("z".repeat(MAX_LINE + 5), false);
        assert_eq!(pages.composer_window(1)[0].text.chars().count(), MAX_LINE);
    }

    #[test]
    fn page_clear_resets_region() {
        let mut pages = ShellPages::new();
        pages.push_transcript("t", false);
        pages.scroll_transcript_up(1);
        pages.clear_transcript();
        assert!(pages.transcript_window(10).is_empty());
        assert_eq!(pages.transcript_len(), 0);
    }
}

/// Shell tab of the session composer auxiliary dock
/// (`packages/tui/src/routes/session/composer/shell-tab.tsx:12-149`).
///
/// Upstream facts encoded here:
/// - only `status === "running"` shells are listed (`:20-22`);
/// - the selection clamps to the last row when the list shrinks (`:34-36`);
/// - the scroll effect keeps the selected row inside the viewport by centring
///   it when it falls outside (`:38-47`);
/// - `up` on the first row closes the composer, `down` wraps and is inert on an
///   empty list (`:70-90`);
/// - `select` opens the shell-output dialog for the selected entry (`:29-32`),
///   `kill` removes it (`:98-109`);
/// - the only hint pair is `output`/`kill`, resolved from the
///   `composer.shell.select`/`composer.shell.kill` keybinds, and absent when
///   nothing is selected (`:52-60`). Commands are addressed by their keybind
///   id; the component has no key fallbacks of its own (`test/cli/tui/
///   composer-keymap.test.tsx` "disabled shell bindings have no component
///   fallbacks");
/// - each row renders `command.split("\n", 1)[0]`, i.e. a multiline command is
///   one row (`:139`), and the empty list renders ` No shell commands`
///   (`:116`) inside a `maxHeight={5}` scrollbox (`:115`);
/// - row paint: active uses `background.action.primary.focused` +
///   `text.action.primary.focused` + BOLD, otherwise the `.base` pair
///   (`:125-137`); rows pad 1 left and right (`:123-124`);
/// - pointer move selects, pointer up selects and opens (`:128-132`).
#[cfg(feature = "native")]
pub use composer_shell::{
    ShellAction, ShellEntry, ShellRow, ShellShortcuts, ShellTab, SHELL_EMPTY_TEXT,
};

#[cfg(feature = "native")]
mod composer_shell {
    use crate::native_dock::{DockHint, DockStyle, MAX_DOCK_ROWS, SHELL_TAB_ID, SHELL_TAB_LABEL};

    /// Exact upstream empty-state text, including its leading space
    /// (`shell-tab.tsx:116`).
    pub const SHELL_EMPTY_TEXT: &str = " No shell commands";
    /// `composer.shell.select` / `composer.shell.kill` command ids
    /// (`shell-tab.tsx:56-57,92,98`).
    pub const SHELL_SELECT_COMMAND: &str = "composer.shell.select";
    pub const SHELL_KILL_COMMAND: &str = "composer.shell.kill";
    pub const SHELL_UP_COMMAND: &str = "composer.shell.up";
    pub const SHELL_DOWN_COMMAND: &str = "composer.shell.down";

    /// One shell record from the session shell listing. `status` stays a
    /// string: only the exact value `"running"` is listed (`:21`), and the
    /// model never invents terminal states.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ShellEntry {
        pub id: String,
        pub command: String,
        pub status: String,
    }

    impl ShellEntry {
        #[must_use]
        pub fn new(
            id: impl Into<String>,
            command: impl Into<String>,
            status: impl Into<String>,
        ) -> Self {
            Self {
                id: id.into(),
                command: command.into(),
                status: status.into(),
            }
        }

        #[must_use]
        pub fn is_running(&self) -> bool {
            self.status == "running"
        }

        /// `shell.command.split("\n", 1)[0]` (`:139`).
        #[must_use]
        pub fn row_label(&self) -> &str {
            self.command.split('\n').next().unwrap_or("")
        }
    }

    /// One painted row.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ShellRow {
        pub id: String,
        pub label: String,
        pub active: bool,
    }

    /// Resolved `composer.shell.*` shortcut labels. `None` is a disabled
    /// keybind: the hint renders the label with an empty shortcut, and the
    /// command itself is only reachable through the keymap.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct ShellShortcuts {
        pub select: Option<String>,
        pub kill: Option<String>,
    }

    impl ShellShortcuts {
        /// Upstream defaults (`config/keybind.ts:255-256`: `return`,
        /// `ctrl+d`), formatted the way `useShortcuts().get` returns them.
        #[must_use]
        pub fn upstream_defaults() -> Self {
            Self {
                select: Some("return".into()),
                kill: Some("ctrl+d".into()),
            }
        }

        #[must_use]
        pub fn disabled() -> Self {
            Self::default()
        }
    }

    /// What the caller must do after a shell-tab command.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ShellAction {
        /// No-op (no selection, empty list, or an out-of-range index).
        None,
        /// Selection moved inside the list.
        Selected(usize),
        /// `composer.close()` (`shell-tab.tsx:74-76`).
        CloseComposer,
        /// Open the shell-output dialog for this shell (`shell-tab.tsx:29-32`).
        Output(String),
        /// `client.api.shell.remove({ id, location })` (`shell-tab.tsx:98-109`).
        Kill(String),
    }

    /// State of the shell tab.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct ShellTab {
        entries: Vec<ShellEntry>,
        selected: usize,
        /// `scroll.y` of the dock scrollbox (`shell-tab.tsx:38-47`).
        scroll_top: usize,
    }

    impl ShellTab {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        #[must_use]
        pub const fn tab_id() -> &'static str {
            SHELL_TAB_ID
        }

        #[must_use]
        pub const fn tab_label() -> &'static str {
            SHELL_TAB_LABEL
        }

        /// Replace the session's shells. Only running rows are kept
        /// (`shell-tab.tsx:20-22`) and the selection clamps to the new last row
        /// (`shell-tab.tsx:34-36`).
        pub fn set_entries(&mut self, entries: Vec<ShellEntry>) {
            self.entries = entries.into_iter().filter(ShellEntry::is_running).collect();
            if self.selected >= self.entries.len() {
                self.selected = self.entries.len().saturating_sub(1);
            }
            self.scroll_top = self.scroll_top.min(self.max_scroll());
        }

        #[must_use]
        pub fn entries(&self) -> &[ShellEntry] {
            &self.entries
        }

        #[must_use]
        pub fn selected(&self) -> usize {
            self.selected
        }

        #[must_use]
        pub fn scroll_top(&self) -> usize {
            self.scroll_top
        }

        #[must_use]
        pub fn selected_entry(&self) -> Option<&ShellEntry> {
            self.entries.get(self.selected)
        }

        #[must_use]
        pub fn rows(&self) -> Vec<ShellRow> {
            self.entries
                .iter()
                .enumerate()
                .map(|(index, entry)| ShellRow {
                    id: entry.id.clone(),
                    label: entry.row_label().to_owned(),
                    active: index == self.selected,
                })
                .collect()
        }

        /// Row paint (`shell-tab.tsx:125-137`).
        #[must_use]
        pub fn row_style(row: &ShellRow) -> (DockStyle, bool) {
            if row.active {
                (DockStyle::ShellActive, true)
            } else {
                (DockStyle::ShellBase, false)
            }
        }

        /// `hints` (`shell-tab.tsx:52-60`): `output`/`kill` only while an entry
        /// is selected. The pair order is upstream's.
        #[must_use]
        pub fn hints(&self, shortcuts: &ShellShortcuts) -> Vec<DockHint> {
            if self.selected_entry().is_none() {
                return Vec::new();
            }
            vec![
                DockHint::new("output", shortcuts.select.clone()),
                DockHint::new("kill", shortcuts.kill.clone()),
            ]
        }

        /// Dispatch one upstream command id (`shell-tab.tsx:70-109`). Unknown or
        /// disabled ids are inert: the component owns no key fallbacks.
        pub fn command(&mut self, id: &str) -> ShellAction {
            match id {
                SHELL_UP_COMMAND => self.up(),
                SHELL_DOWN_COMMAND => self.down(),
                SHELL_SELECT_COMMAND => self.open(),
                SHELL_KILL_COMMAND => self.kill(),
                _ => ShellAction::None,
            }
        }

        /// `composer.shell.up` (`shell-tab.tsx:70-80`).
        pub fn up(&mut self) -> ShellAction {
            if self.selected == 0 {
                return ShellAction::CloseComposer;
            }
            self.selected -= 1;
            ShellAction::Selected(self.selected)
        }

        /// `composer.shell.down` (`shell-tab.tsx:81-90`).
        pub fn down(&mut self) -> ShellAction {
            if self.entries.is_empty() {
                return ShellAction::None;
            }
            self.selected = (self.selected + 1) % self.entries.len();
            ShellAction::Selected(self.selected)
        }

        /// `composer.shell.select` (`shell-tab.tsx:29-32,91-96`).
        pub fn open(&mut self) -> ShellAction {
            self.selected_entry()
                .map(|entry| ShellAction::Output(entry.id.clone()))
                .unwrap_or(ShellAction::None)
        }

        /// `composer.shell.kill` (`shell-tab.tsx:97-109`). The entry is removed
        /// from the listing only when the caller's refresh lands; the model
        /// emits the request and never mutates daemon state itself.
        pub fn kill(&mut self) -> ShellAction {
            self.selected_entry()
                .map(|entry| ShellAction::Kill(entry.id.clone()))
                .unwrap_or(ShellAction::None)
        }

        /// Pointer move selects the hovered row (`shell-tab.tsx:128`).
        pub fn hover(&mut self, index: usize) -> ShellAction {
            if index < self.entries.len() && index != self.selected {
                self.selected = index;
                ShellAction::Selected(index)
            } else {
                ShellAction::None
            }
        }

        /// Pointer up: select then open (`shell-tab.tsx:129-132`).
        pub fn click(&mut self, index: usize) -> ShellAction {
            if index < self.entries.len() {
                self.selected = index;
                return self.open();
            }
            ShellAction::None
        }

        fn max_scroll(&self) -> usize {
            self.entries.len().saturating_sub(1)
        }

        /// Scroll effect (`shell-tab.tsx:38-47`): with `y = selected -
        /// scrollTop`, an off-viewport row scrolls by `y - floor(height / 2)`.
        /// `viewport` is the scrollbox height, capped at the dock maximum.
        pub fn scroll_selected_into_view(&mut self, viewport: usize) {
            let height = viewport.min(MAX_DOCK_ROWS);
            if height == 0 || self.entries.is_empty() {
                return;
            }
            let y = self.selected as i64 - self.scroll_top as i64;
            if y >= height as i64 || y < 0 {
                let center = (height / 2) as i64;
                let target = self.scroll_top as i64 + (y - center);
                let max_scroll = self.entries.len().saturating_sub(height) as i64;
                self.scroll_top = target.clamp(0, max_scroll) as usize;
            }
        }

        /// Rows the scrollbox shows: `min(viewport, maxHeight 5)` starting at
        /// `scroll_top` (`shell-tab.tsx:115`).
        #[must_use]
        pub fn visible_rows(&self, viewport: usize) -> &[ShellEntry] {
            let height = viewport.min(MAX_DOCK_ROWS);
            let start = self.scroll_top.min(self.entries.len());
            let end = start.saturating_add(height).min(self.entries.len());
            &self.entries[start..end]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn shells() -> Vec<ShellEntry> {
            vec![
                ShellEntry::new("sh-a", "bun test", "running"),
                ShellEntry::new("sh-b", "bun dev", "running"),
                ShellEntry::new("sh-c", "python3 - <<'PY'\nimport json\nPY", "running"),
                ShellEntry::new("sh-d", "already exited", "completed"),
            ]
        }

        // CP-26: multiline command renders one row; running-only filter.
        #[test]
        fn cp26_multiline_command_renders_single_row_and_running_only() {
            let mut tab = ShellTab::new();
            tab.set_entries(shells());
            let rows = tab.rows();
            assert_eq!(rows.len(), 3, "only status == running is listed");
            assert_eq!(rows[2].label, "python3 - <<'PY'");
            assert!(!tab
                .rows()
                .iter()
                .any(|row| row.label.contains("import json")));
            assert!(!tab
                .rows()
                .iter()
                .any(|row| row.label.contains("already exited")));
            assert!(ShellEntry::new("x", "cmd", "running").is_running());
            assert!(!ShellEntry::new("x", "cmd", "exited").is_running());
        }

        // CP-26: selection lifecycle — clamp on shrink, up closes at the first
        // row, down wraps.
        #[test]
        fn cp26_selection_clamps_and_wraps() {
            let mut tab = ShellTab::new();
            tab.set_entries(shells());
            assert_eq!(tab.selected(), 0);
            assert_eq!(tab.command(SHELL_UP_COMMAND), ShellAction::CloseComposer);
            assert_eq!(tab.selected(), 0, "closing does not move the selection");
            assert_eq!(tab.command(SHELL_DOWN_COMMAND), ShellAction::Selected(1));
            assert_eq!(tab.command(SHELL_DOWN_COMMAND), ShellAction::Selected(2));
            assert_eq!(tab.command(SHELL_DOWN_COMMAND), ShellAction::Selected(0));
            // A shrinking list clamps to the last remaining row.
            tab.set_entries(vec![ShellEntry::new("only", "solo", "running")]);
            assert_eq!(tab.selected(), 0);
            tab.set_entries(Vec::new());
            assert_eq!(tab.selected(), 0);
            assert_eq!(tab.command(SHELL_DOWN_COMMAND), ShellAction::None);
            assert_eq!(tab.command(SHELL_SELECT_COMMAND), ShellAction::None);
            assert_eq!(tab.command(SHELL_KILL_COMMAND), ShellAction::None);
        }

        // CP-26: selected removal emits the selected id; unknown commands are
        // inert (no component fallbacks).
        #[test]
        fn cp26_selected_removal_and_open_commands() {
            let mut tab = ShellTab::new();
            tab.set_entries(shells());
            tab.down();
            assert_eq!(
                tab.command(SHELL_SELECT_COMMAND),
                ShellAction::Output("sh-b".into())
            );
            assert_eq!(
                tab.command(SHELL_KILL_COMMAND),
                ShellAction::Kill("sh-b".into())
            );
            assert_eq!(tab.command("none"), ShellAction::None);
            assert_eq!(tab.command("composer.subagent.up"), ShellAction::None);
            // The refresh after removal drops the killed row; the selection
            // clamps to the new last row instead of pointing past the list.
            tab.set_entries(vec![
                ShellEntry::new("sh-a", "bun test", "running"),
                ShellEntry::new("sh-c", "python3 - <<'PY'", "running"),
            ]);
            assert_eq!(tab.selected(), 1);
            assert_eq!(tab.selected_entry().unwrap().id, "sh-c");
        }

        // CP-26: hints follow the selection; configured and disabled keybinds
        // both render as upstream does.
        #[test]
        fn cp26_hints_follow_selection_and_bindings() {
            let mut tab = ShellTab::new();
            assert!(tab.hints(&ShellShortcuts::upstream_defaults()).is_empty());
            tab.set_entries(shells());
            let hints = tab.hints(&ShellShortcuts::upstream_defaults());
            assert_eq!(
                hints,
                vec![
                    DockHint::new("output", Some("return".into())),
                    DockHint::new("kill", Some("ctrl+d".into())),
                ]
            );
            let configured = ShellShortcuts {
                select: Some("ctrl+o".into()),
                kill: Some("ctrl+u".into()),
            };
            assert_eq!(
                tab.hints(&configured),
                vec![
                    DockHint::new("output", Some("ctrl+o".into())),
                    DockHint::new("kill", Some("ctrl+u".into())),
                ]
            );
            let disabled = tab.hints(&ShellShortcuts::disabled());
            assert_eq!(
                disabled,
                vec![DockHint::new("output", None), DockHint::new("kill", None)],
                "disabled keybinds keep the label and drop the shortcut"
            );
        }

        // CP-26: pointer actions and the empty list surface.
        #[test]
        fn cp26_pointer_rows_and_empty_state() {
            let mut tab = ShellTab::new();
            tab.set_entries(shells());
            assert_eq!(tab.hover(2), ShellAction::Selected(2));
            assert_eq!(tab.hover(9), ShellAction::None);
            assert_eq!(tab.click(2), ShellAction::Output("sh-c".into()));
            assert_eq!(tab.click(9), ShellAction::None);
            let empty = ShellTab::new();
            assert_eq!(SHELL_EMPTY_TEXT, " No shell commands");
            assert!(empty.rows().is_empty());
        }

        // CP-26: the scroll effect centres an off-viewport selection and the
        // painted window never exceeds maxHeight 5.
        #[test]
        fn cp26_scroll_keeps_selection_visible() {
            let mut tab = ShellTab::new();
            let entries: Vec<ShellEntry> = (0..8)
                .map(|index| {
                    ShellEntry::new(format!("id{index}"), format!("cmd{index}"), "running")
                })
                .collect();
            tab.set_entries(entries);
            // Selection moved past the viewport: y = 5 - 0 = 5 >= 5, so the
            // scrollbox scrolls by 5 - 2 = 3.
            for _ in 0..5 {
                tab.down();
            }
            assert_eq!(tab.selected(), 5);
            tab.scroll_selected_into_view(5);
            assert_eq!(tab.scroll_top(), 3);
            let visible: Vec<&str> = tab
                .visible_rows(5)
                .iter()
                .map(|entry| entry.id.as_str())
                .collect();
            assert_eq!(visible, vec!["id3", "id4", "id5", "id6", "id7"]);
            // A scrollbox taller than the dock cap still shows at most 5 rows.
            assert_eq!(tab.visible_rows(20).len(), MAX_DOCK_ROWS);
            // Scrolling back to the top row keeps it visible.
            for _ in 0..5 {
                tab.up();
            }
            tab.scroll_selected_into_view(5);
            assert!(tab.scroll_top() <= tab.selected());
            assert!(tab.visible_rows(5).iter().any(|entry| entry.id == "id0"));
        }
    }
}

/// Shell-surface wiring for the auxiliary composer dock, the prompt footer and
/// the bottom overlay stack.
///
/// The three models this module consumes were previously unreferenced by any
/// paint path:
///
/// * [`crate::native_dock`] composes the auxiliary-composer block
///   (`packages/tui/src/routes/session/composer/index.tsx:84-155`) and the shell
///   tab rows (`.../composer/shell-tab.tsx:114-149`);
/// * [`crate::native_progress`] owns the footer clock: the prompt-footer
///   spinner and interrupt affordance (`component/prompt/index.tsx:1899-1967`),
///   retry / error / free-limit rows (`routes/session/index.tsx:2035-2070,
///   2529-2570`), the token diagnostics label, and the delayed background
///   hint (`routes/session/index.tsx:1894-1926`);
/// * [`crate::native_overlay_stack`] owns paint ordering: dock, progress and
///   toast all declare no `zIndex` upstream, so they share z 0 and paint in
///   mount order (`ui/toast.tsx:47-63`, `composer/index.tsx:84-155`).
///
/// Everything here is pure state: rows are produced for the caller's painter,
/// no IO and no wall clock. `now_ms` is injected so timer behavior is testable.
#[cfg(feature = "native")]
pub use dock_surface::{
    BackgroundShell, FooterError, FooterRow, FooterSpan, FooterStyle, SessionDock, ShellChunk,
    ShellFooter, ShellLoad, ShellSurface, SurfacePaint,
};

#[cfg(feature = "native")]
mod dock_surface {
    use super::composer_shell::{ShellAction, ShellEntry, ShellRow, ShellShortcuts, ShellTab, SHELL_EMPTY_TEXT};
    use super::{ShellLine, ShellPages};
    use crate::native_dock::{
        ComposerDock, DockBlock, DockBody, DockBodyRow, DockKey, DockKeyOutcome, DockTabId,
        MAX_DOCK_ROWS,
    };
    use crate::native_overlay_stack::{OverlayKind, OverlayStack, Rect};
    use crate::native_progress::{
        background_candidate, error_line, free_limit_notice, retry_label, token_diagnostics_label,
        BackgroundHint, ErrorLine, RetryState, RunningTool, Spinner, TokenSummary,
    };
    use crate::native_view_layout::visit_unpadded_cells;

    /// Interval of the prompt-footer scanner (`component/prompt/index.tsx:1916`:
    /// `<spinner ... interval={40} />`).
    pub const FOOTER_SPINNER_INTERVAL_MS: u64 = 40;
    /// Scanner width used by the footer (`ui/spinner.ts:274`
    /// `const width = options.width ?? 8`).
    pub const FOOTER_SPINNER_WIDTH: usize = 8;
    /// `holdStart` default (`ui/spinner.ts:276`).
    const FOOTER_HOLD_START: usize = 30;
    /// `holdEnd` default (`ui/spinner.ts:277`).
    const FOOTER_HOLD_END: usize = 9;
    /// `deriveTrailColors` default step count (`ui/spinner.ts:262`).
    const FOOTER_TRAIL: usize = 6;
    /// `marginLeft={1}` on the spinner box (`component/prompt/index.tsx:1914`).
    pub const FOOTER_SPINNER_MARGIN_LEFT: u16 = 1;
    /// Animations-off fallback glyph, brackets included
    /// (`component/prompt/index.tsx:1915` `<text>[⋯]</text>`).
    pub const FOOTER_ANIMATIONS_OFF: &str = "[⋯]";
    /// Base-colored interrupt prefix (`component/prompt/index.tsx:130-132`).
    pub const FOOTER_INTERRUPT_PREFIX: &str = "esc ";
    /// Muted interrupt label (`component/prompt/index.tsx:134`).
    pub const FOOTER_INTERRUPT: &str = "interrupt";
    /// Muted armed label (`component/prompt/index.tsx:134`).
    pub const FOOTER_INTERRUPT_ARMED: &str = "again to interrupt";
    /// Left inset of the assistant-footer rows (`index.tsx:2038,2050,2535,2561`
    /// all use `paddingLeft={3}`).
    pub const FOOTER_TEXT_X: u16 = 3;
    /// `SHELL_DISPLAY_LIMIT = 1024 * 1024` (`index.tsx:2982`).
    pub const SHELL_DISPLAY_LIMIT: usize = 1024 * 1024;
    /// `setInterval(..., 1_000)` for background shells (`index.tsx:3081`).
    pub const SHELL_POLL_INTERVAL_MS: u64 = 1_000;
    /// Head label once background output was truncated (`index.tsx:3090`).
    pub const SHELL_EARLIER_OUTPUT: &str = "[earlier output omitted]";

    /// Display cells of `text`, using the same segmentation as the painter.
    fn cells(text: &str) -> u16 {
        u16::try_from(visit_unpadded_cells(text, usize::from(u16::MAX), |_, _| {}))
            .unwrap_or(u16::MAX)
    }

    /// Truncate to `budget` display cells (`wrapMode="none"` semantics).
    fn take_cells(text: &str, budget: u16) -> String {
        let mut out = String::new();
        visit_unpadded_cells(text, usize::from(budget), |glyph, _| {
            out.push_str(glyph);
        });
        out
    }

    /// `createFrames({ style: "blocks" })` — the prompt-footer scanner
    /// (`component/prompt/index.tsx:1655-1673` builds frames with the default
    /// width, `"blocks"` style, `holdStart` 30 and `holdEnd` 9;
    /// `ui/spinner.ts:274-329` generates them).
    ///
    /// Upstream paints glyphs from `calculateColorIndex`; only whether the
    /// index is a valid color slot decides between `■` and `⬝`
    /// (`ui/spinner.ts:318-324`). This reproduces that predicate for the
    /// default options, which is the entire glyph stream.
    #[must_use]
    pub fn footer_spinner_frames() -> Vec<String> {
        let width = FOOTER_SPINNER_WIDTH;
        let total = width + FOOTER_HOLD_END + (width - 1) + FOOTER_HOLD_START;
        (0..total)
            .map(|frame| {
                (0..width)
                    .map(|cell| {
                        if footer_cell_active(frame, cell, width) {
                            '■'
                        } else {
                            '⬝'
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// `getScannerState` + `calculateColorIndex` (`ui/spinner.ts:26-146`) for
    /// the default bidirectional blocks scanner.
    fn footer_cell_active(frame: usize, cell: usize, width: usize) -> bool {
        let forward = width;
        let backward = width - 1;
        let active = if frame < forward {
            // Forward scan: `activePosition = frameIndex`.
            let distance = frame as isize - cell as isize;
            distance >= 0 && distance < FOOTER_TRAIL as isize
        } else if frame < forward + FOOTER_HOLD_END {
            // Hold at the end: `activePosition = width - 1`.
            let index = cell as isize - (width as isize - 1) + (frame - forward) as isize;
            index >= 0 && index < FOOTER_TRAIL as isize
        } else if frame < forward + FOOTER_HOLD_END + backward {
            // Backward scan: `activePosition = width - 2 - backwardIndex`.
            let index = frame - forward - FOOTER_HOLD_END;
            let position = width - 2 - index;
            let distance = cell as isize - position as isize;
            distance >= 0 && distance < FOOTER_TRAIL as isize
        } else {
            // Hold at the start: `activePosition = 0`.
            let index = cell as isize + (frame - forward - FOOTER_HOLD_END - backward) as isize;
            index >= 0 && index < FOOTER_TRAIL as isize
        };
        active
    }

    /// Paint class of one footer span. Token names mirror the upstream theme
    /// lookups without owning palette values.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FooterStyle {
        /// `text.muted`.
        Muted,
        /// `text.base` (the interrupt `esc` prefix, free-limit commands).
        Base,
        /// `text.feedback.warning.base` (`AssistantRetry`, `index.tsx:2562`).
        Warning,
        /// `text.feedback.error.base` (`index.tsx:2039`).
        Error,
    }

    /// One positioned footer span.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FooterSpan {
        pub text: String,
        pub style: FooterStyle,
        pub x: u16,
    }

    /// One composed footer row. [`Self::text`] is the concatenation of the
    /// spans; a completely empty row is the layout gap upstream emits with
    /// `marginTop`/`Show` (`index.tsx:1918`).
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct FooterRow {
        pub text: String,
        pub spans: Vec<FooterSpan>,
    }

    impl FooterRow {
        /// Start a row with `indent` blank cells.
        fn indented(indent: u16) -> Self {
            let mut row = Self::default();
            if indent > 0 {
                row.push(" ".repeat(usize::from(indent)), FooterStyle::Muted);
            }
            row
        }

        /// Start a row whose first span is placed at absolute `x`.
        fn at(x: u16) -> Self {
            let mut row = Self::default();
            row.push(" ".repeat(usize::from(x)), FooterStyle::Muted);
            row
        }

        fn push(&mut self, text: impl Into<String>, style: FooterStyle) {
            let text = text.into();
            let x = cells(&self.text);
            self.text.push_str(&text);
            self.spans.push(FooterSpan { text, style, x });
        }

        /// Clip every span to `width` display cells.
        fn clip(&mut self, width: u16) {
            let mut used = 0u16;
            let mut text = String::new();
            let mut spans = Vec::new();
            for span in self.spans.drain(..) {
                if used >= width {
                    break;
                }
                let clipped = take_cells(&span.text, width - used);
                let empty = clipped.is_empty();
                let span = FooterSpan {
                    text: clipped,
                    style: span.style,
                    x: used,
                };
                used = used.saturating_add(cells(&span.text));
                text.push_str(&span.text);
                spans.push(span);
                if empty {
                    break;
                }
            }
            self.text = text;
            self.spans = spans;
        }
    }

    /// The auxiliary composer dock plus the shell tab it renders.
    ///
    /// Upstream always mounts two tabs in this order (`index.tsx:127-128`:
    /// `SubagentsTab` then `ShellTab`), so a session dock always shows the
    /// Shell tab in its tab row. Only the *active* tab's body renders
    /// (`shell-tab.tsx:114`), and shell commands are only live while the shell
    /// tab is active (`shell-tab.tsx:63` `enabled: () => composer.active("shell")`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SessionDock {
        dock: ComposerDock,
        shells: ShellTab,
        shortcuts: ShellShortcuts,
        viewport: usize,
    }

    impl Default for SessionDock {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SessionDock {
        /// A closed session composer with the Subagents and Shell tabs mounted.
        #[must_use]
        pub fn new() -> Self {
            Self {
                dock: ComposerDock::session(),
                shells: ShellTab::new(),
                shortcuts: ShellShortcuts::upstream_defaults(),
                viewport: MAX_DOCK_ROWS,
            }
        }

        #[must_use]
        pub fn dock(&self) -> &ComposerDock {
            &self.dock
        }

        pub fn dock_mut(&mut self) -> &mut ComposerDock {
            &mut self.dock
        }

        #[must_use]
        pub fn shells(&self) -> &ShellTab {
            &self.shells
        }

        #[must_use]
        pub fn shortcuts(&self) -> &ShellShortcuts {
            &self.shortcuts
        }

        /// Replace the resolved `composer.shell.select`/`composer.shell.kill`
        /// bindings; hints are re-resolved lazily like upstream's
        /// `hints: () => ...` (`shell-tab.tsx:52-60`).
        pub fn set_shortcuts(&mut self, shortcuts: ShellShortcuts) {
            self.shortcuts = shortcuts;
            self.refresh_hints();
        }

        /// Scrollbox height. Upstream caps it with `maxHeight={5}`
        /// (`shell-tab.tsx:115`); the body model clamps again.
        pub fn set_viewport(&mut self, viewport: usize) {
            self.viewport = viewport;
            self.shells.scroll_selected_into_view(self.viewport);
        }

        #[must_use]
        pub fn viewport(&self) -> usize {
            self.viewport
        }

        /// Open the composer with the route's default tab
        /// (`index.tsx:34-40,1597`). `None` keeps the current tab (the first
        /// registered one, i.e. Subagents) per the same effect.
        pub fn open(&mut self, default_tab: Option<DockTabId>) {
            self.dock.open(default_tab);
        }

        pub fn close(&mut self) {
            self.dock.close();
        }

        #[must_use]
        pub fn is_open(&self) -> bool {
            self.dock.visible()
        }

        #[must_use]
        pub fn active(&self) -> Option<DockTabId> {
            self.dock.active()
        }

        pub fn switch_tab(&mut self, dir: i32) -> DockKeyOutcome {
            let outcome = self.dock.switch_tab(dir);
            self.refresh_hints();
            outcome
        }

        pub fn key(&mut self, key: DockKey) -> DockKeyOutcome {
            let outcome = self.dock.key(key);
            self.refresh_hints();
            outcome
        }

        /// Replace the session's shell listing. The running-only filter, the
        /// selection clamp and the scroll-into-view effect are all applied
        /// (`shell-tab.tsx:20-22,34-47`).
        pub fn set_shells(&mut self, entries: Vec<ShellEntry>) {
            self.shells.set_entries(entries);
            self.shells.scroll_selected_into_view(self.viewport);
            self.refresh_hints();
        }

        /// Dispatch one shell-tab command. Inert while the shell tab is not
        /// active (`shell-tab.tsx:63`), mirroring the layer gate.
        pub fn command(&mut self, id: &str) -> ShellAction {
            if !self.dock.is_active(DockTabId::Shell) {
                return ShellAction::None;
            }
            let action = self.shells.command(id);
            self.shells.scroll_selected_into_view(self.viewport);
            self.refresh_hints();
            action
        }

        /// Pointer move over the visible list (`shell-tab.tsx:128`).
        pub fn hover(&mut self, index: usize) -> ShellAction {
            let action = self.shells.hover(index);
            self.shells.scroll_selected_into_view(self.viewport);
            self.refresh_hints();
            action
        }

        /// Pointer up over the visible list (`shell-tab.tsx:129-132`).
        pub fn click(&mut self, index: usize) -> ShellAction {
            let action = self.shells.click(index);
            self.refresh_hints();
            action
        }

        /// The active tab's body when it is the Shell tab
        /// (`shell-tab.tsx:114-149`): visible running rows clipped to the
        /// scrollbox height, or the muted empty state.
        #[must_use]
        pub fn shell_body(&self) -> DockBody {
            if self.shells.entries().is_empty() {
                return DockBody::Empty(SHELL_EMPTY_TEXT.to_owned());
            }
            let start = self.shells.scroll_top();
            let selected = self.shells.selected();
            let rows = self
                .shells
                .visible_rows(self.viewport)
                .iter()
                .enumerate()
                .map(|(offset, entry)| {
                    let row = ShellRow {
                        id: entry.id.clone(),
                        label: entry.row_label().to_owned(),
                        active: start + offset == selected,
                    };
                    let (style, bold) = ShellTab::row_style(&row);
                    DockBodyRow::new(row.label, style, bold)
                })
                .collect();
            DockBody::Rows(rows)
        }

        /// Compose the dock block for this frame.
        ///
        /// `other` supplies the active *non-shell* tab's body (the subagent
        /// tab is owned elsewhere); it is ignored while the shell tab is
        /// active. `None` composes an empty body, which is what Solid's
        /// `gap` does for a tab that renders nothing.
        #[must_use]
        pub fn block(&self, width: u16, other: Option<DockBody>) -> Option<DockBlock> {
            let body = if self.active() == Some(DockTabId::Shell) {
                self.shell_body()
            } else {
                other.unwrap_or_default()
            };
            self.dock.block(width, self.viewport, &body)
        }

        /// Write the dock block into the shell page buffer, replacing the
        /// previous frame's rows. Returns the number of rows painted.
        ///
        /// [`ShellLine`] carries one bold flag; a row is flagged bold when any
        /// of its spans is bold (tab label, hint label), which is all a
        /// text-only painter can represent. [`Self::block`] remains the
        /// authoritative span source for styled painters.
        pub fn paint(&mut self, pages: &mut ShellPages, width: u16, other: Option<DockBody>) -> usize {
            let Some(block) = self.block(width, other) else {
                pages.clear_dock();
                return 0;
            };
            let rows: Vec<ShellLine> = block
                .rows
                .iter()
                .map(|row| ShellLine::new(row.text.clone(), row.spans.iter().any(|span| span.bold)))
                .collect();
            pages.set_dock(&rows)
        }

        fn refresh_hints(&mut self) {
            let hints = self.shells.hints(&self.shortcuts);
            self.dock.set_hints(DockTabId::Shell, hints);
        }
    }

    /// Error fact of the current assistant turn (`AssistantFooter`,
    /// `routes/session/index.tsx:2030-2048`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct FooterError {
        pub message: String,
        /// `error.message === "Step interrupted"` (`index.tsx:2030`).
        pub interrupted: bool,
        /// Formatted reset instant for `provider.free-limit`
        /// (`index.tsx:2031,2531-2532`); `None` keeps the generic first line.
        pub free_limit_reset: Option<String>,
    }

    /// Shell footer state: the prompt-footer status row
    /// (`component/prompt/index.tsx:1899-1967`), the delayed background hint
    /// (`routes/session/index.tsx:1894-1926`) and the progress diagnostics
    /// owned by [`crate::native_progress`].
    ///
    /// The diagnostics are composed here because the shell is their only
    /// consumer: upstream draws them at the assistant footer
    /// (`index.tsx:2035-2070,2529-2570`) and as the turn-usage row
    /// (`rows.ts:53`), all of which are flow siblings of the prompt footer in
    /// the session column. `native_footer` owns the agent/model metadata line;
    /// this type owns the retry, error/free-limit and token rows that
    /// `native_progress` computes, so a running turn reaches output instead of
    /// leaving the loader model unreferenced.
    #[derive(Clone, Debug)]
    pub struct ShellFooter {
        running: bool,
        animations: bool,
        interrupt_armed: bool,
        location: Option<String>,
        retry: Option<RetryState>,
        error: Option<FooterError>,
        tokens: Option<TokenSummary>,
        tokens_expanded: bool,
        hint: BackgroundHint,
        hint_shortcut: String,
        spinner: Spinner,
    }

    impl Default for ShellFooter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ShellFooter {
        #[must_use]
        pub fn new() -> Self {
            Self {
                running: false,
                animations: true,
                interrupt_armed: false,
                location: None,
                retry: None,
                error: None,
                tokens: None,
                tokens_expanded: false,
                hint: BackgroundHint::new(),
                hint_shortcut: String::new(),
                spinner: footer_spinner(),
            }
        }

        /// Turn status (`data.session.status(id) === "running"`,
        /// `component/prompt/index.tsx:1912`). A fresh running turn remounts
        /// the scanner at its first frame.
        pub fn set_running(&mut self, running: bool) {
            if running && !self.running {
                self.spinner = footer_spinner();
            }
            self.running = running;
        }

        #[must_use]
        pub fn is_running(&self) -> bool {
            self.running
        }

        /// `config.animations ?? true` (`component/prompt/index.tsx:1915`).
        pub fn set_animations(&mut self, animations: bool) {
            self.animations = animations;
            let _ = self.spinner.set_autoplay(animations);
        }

        #[must_use]
        pub fn animations(&self) -> bool {
            self.animations
        }

        /// `store.interrupt > 0` (`component/prompt/index.tsx:1920`).
        pub fn set_interrupt_armed(&mut self, armed: bool) {
            self.interrupt_armed = armed;
        }

        /// Idle status label (`index.tsx:1947-1962`), muted and truncating.
        pub fn set_location(&mut self, location: Option<String>) {
            self.location = location;
        }

        pub fn set_retry(&mut self, retry: Option<RetryState>) {
            self.retry = retry;
        }

        pub fn set_error(&mut self, error: Option<FooterError>) {
            self.error = error;
        }

        /// `debug.turn_tokens` diagnostics (`native_progress::token_diagnostics`).
        pub fn set_tokens(&mut self, tokens: Option<TokenSummary>) {
            self.tokens = tokens;
        }

        pub fn set_tokens_expanded(&mut self, expanded: bool) {
            self.tokens_expanded = expanded;
        }

        /// Resolved `session.background` shortcut. An unbound shortcut hides
        /// the hint (`index.tsx:1916`).
        pub fn set_background_shortcut(&mut self, shortcut: impl Into<String>) {
            self.hint_shortcut = shortcut.into();
        }

        /// Feed the current assistant message's parts. Only the first eligible
        /// running `shell`/`subagent` part qualifies (`index.tsx:1899-1906`).
        /// Returns true when the hint's subject changed.
        pub fn set_background_work(&mut self, now_ms: u64, parts: &[RunningTool]) -> bool {
            let candidate = background_candidate(parts);
            self.hint.update(now_ms, candidate)
        }

        #[must_use]
        pub fn background_hint_visible(&self) -> bool {
            self.hint.visible()
        }

        /// Advance the visible clock by one renderer frame. Returns true when a
        /// painted row changed (scanner frame or hint appearing).
        pub fn advance(&mut self, frame_id: u64, delta_ms: u64, now_ms: u64) -> bool {
            let mut changed = self.spinner.advance_visible(
                self.running && self.animations,
                frame_id,
                delta_ms,
            );
            let before = self.hint.visible();
            self.hint.tick(now_ms);
            changed |= self.hint.visible() != before;
            changed
        }

        /// Delay until the next visible change: the scanner interval, the
        /// hint's remaining delay, or the retry countdown's next second
        /// (`index.tsx:2548-2557`). `None` means fully static.
        #[must_use]
        pub fn repaint_delay_ms(&self, now_ms: u64) -> Option<u64> {
            let mut delay = if self.running && self.animations {
                self.spinner.repaint_delay_ms()
            } else {
                None
            };
            if let Some(deadline) = self.hint.deadline_ms() {
                let remaining = deadline.saturating_sub(now_ms);
                delay = Some(delay.map_or(remaining, |value| value.min(remaining)));
            }
            if let Some(retry) = &self.retry {
                if retry.at_ms > now_ms {
                    let remaining = retry.at_ms - now_ms;
                    let next_tick = ((remaining - 1) % 1_000) + 1;
                    delay = Some(delay.map_or(next_tick, |value| value.min(next_tick)));
                }
            }
            delay
        }

        /// Status slot rows: the running scanner + interrupt affordance, or
        /// the location label (`component/prompt/index.tsx:1911-1965`).
        #[must_use]
        pub fn status_rows(&self, width: u16) -> Vec<FooterRow> {
            let mut rows = Vec::new();
            // `BackgroundToolHint` renders above the prompt with
            // `marginTop={1}` (`index.tsx:1918`).
            if let Some(hint) = self.hint_row(width) {
                rows.push(FooterRow::default());
                rows.push(hint);
            }
            rows.push(self.status_row(width));
            rows
        }

        /// Progress diagnostics rows owned by `native_progress`: the error /
        /// free-limit block (`index.tsx:2035-2048`), retry
        /// (`index.tsx:2545-2570`) and the optional token label
        /// (`routes/session/rows.ts:53`).
        #[must_use]
        pub fn diagnostic_rows(&self, now_ms: u64, width: u16) -> Vec<FooterRow> {
            let mut rows = Vec::new();
            if let Some(error) = &self.error {
                match error_line(
                    &error.message,
                    error.interrupted,
                    self.retry.is_some(),
                    error.free_limit_reset.is_some(),
                ) {
                    ErrorLine::None => {}
                    ErrorLine::FreeLimit => {
                        let [first, _] = free_limit_notice(error.free_limit_reset.as_deref());
                        let mut row = FooterRow::indented(FOOTER_TEXT_X);
                        row.push(first, FooterStyle::Muted);
                        rows.push(row);
                        // Second line: `/connect` and `/models` are
                        // `text.base` spans (`index.tsx:2537-2540`).
                        let [_, second] = free_limit_notice(error.free_limit_reset.as_deref());
                        let mut row = FooterRow::indented(FOOTER_TEXT_X);
                        let mut rest = second.as_str();
                        for command in ["/connect", "/models"] {
                            if let Some(index) = rest.find(command) {
                                let (before, after) = rest.split_at(index);
                                row.push(before, FooterStyle::Muted);
                                row.push(command, FooterStyle::Base);
                                rest = &after[command.len()..];
                            }
                        }
                        row.push(rest, FooterStyle::Muted);
                        rows.push(row);
                    }
                    ErrorLine::Ordinary(text) => {
                        let mut row = FooterRow::indented(FOOTER_TEXT_X);
                        row.push(text, FooterStyle::Error);
                        rows.push(row);
                    }
                }
            }
            if let Some(retry) = &self.retry {
                let mut row = FooterRow::indented(FOOTER_TEXT_X);
                row.push(retry_label(retry, now_ms), FooterStyle::Warning);
                rows.push(row);
            }
            if let Some(summary) = &self.tokens {
                let mut row = FooterRow::indented(FOOTER_TEXT_X);
                row.push(
                    token_diagnostics_label(summary, self.tokens_expanded),
                    FooterStyle::Muted,
                );
                rows.push(row);
            }
            for row in &mut rows {
                row.clip(width);
            }
            rows
        }

        /// Every footer row for this frame, top to bottom: diagnostics, then
        /// the background hint, then the status row.
        #[must_use]
        pub fn rows(&self, now_ms: u64, width: u16) -> Vec<FooterRow> {
            let mut rows = self.diagnostic_rows(now_ms, width);
            rows.extend(self.status_rows(width));
            rows
        }

        /// Recompose the footer page region. Returns the row count.
        pub fn paint(&self, pages: &mut ShellPages, now_ms: u64, width: u16) -> usize {
            let rows: Vec<ShellLine> = self
                .rows(now_ms, width)
                .into_iter()
                .map(|row| ShellLine::new(row.text, false))
                .collect();
            pages.set_footer(&rows)
        }

        fn hint_row(&self, width: u16) -> Option<FooterRow> {
            let spans = self.hint.spans(&self.hint_shortcut)?;
            let mut row = FooterRow::at(FOOTER_TEXT_X);
            for (text, muted) in spans {
                row.push(
                    text,
                    if muted {
                        FooterStyle::Muted
                    } else {
                        FooterStyle::Base
                    },
                );
            }
            row.clip(width);
            Some(row)
        }

        fn status_row(&self, width: u16) -> FooterRow {
            let mut row = if self.running {
                // `<box marginLeft={1}>` inside a `gap={1}` row
                // (`index.tsx:1913-1914`).
                let mut row = FooterRow::at(FOOTER_SPINNER_MARGIN_LEFT);
                let glyph = if self.animations {
                    self.spinner.current().unwrap_or(FOOTER_ANIMATIONS_OFF)
                } else {
                    FOOTER_ANIMATIONS_OFF
                };
                row.push(glyph, FooterStyle::Muted);
                row.push(" ", FooterStyle::Muted);
                row.push(FOOTER_INTERRUPT_PREFIX, FooterStyle::Base);
                row.push(
                    if self.interrupt_armed {
                        FOOTER_INTERRUPT_ARMED
                    } else {
                        FOOTER_INTERRUPT
                    },
                    FooterStyle::Muted,
                );
                row
            } else {
                // `text fg={muted} wrapMode="none" truncate flexGrow={1}`
                // (`index.tsx:1948-1961`).
                let mut row = FooterRow::default();
                if let Some(location) = &self.location {
                    row.push(location.clone(), FooterStyle::Muted);
                }
                row
            };
            row.clip(width);
            row
        }
    }

    fn footer_spinner() -> Spinner {
        Spinner::with_frames(
            footer_spinner_frames(),
            FOOTER_SPINNER_INTERVAL_MS,
            true,
        )
    }

    /// One ordered surface in [`ShellSurface::paint_list`].
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SurfacePaint {
        pub kind: OverlayKind,
        pub z_index: u16,
        pub order: u64,
        pub rect: Rect,
        pub rows: Vec<ShellLine>,
    }

    /// Bottom-stack surfaces (dock, prompt-footer progress, toast) ordered by
    /// [`OverlayStack`] — the single ordering authority.
    ///
    /// Callers mount each surface with its rect and rows every frame; the stack
    /// decides paint order (z-index first, then first-mount order, the OpenTUI
    /// rule), and [`Self::paint_list`] is the list to blit bottom-first.
    #[derive(Clone, Debug, Default)]
    pub struct ShellSurface {
        stack: OverlayStack,
        rows: Vec<(OverlayKind, Vec<ShellLine>)>,
    }

    impl ShellSurface {
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }

        /// Mount (`Some(rect)`) or unmount (`None`) a shell-surface layer and
        /// attach its rows. Returns false for a non-shell-surface kind; those
        /// are mounted on the stack directly by their owners.
        pub fn set(&mut self, kind: OverlayKind, rect: Option<Rect>, rows: Vec<ShellLine>) -> bool {
            if !kind.is_shell_surface() {
                return false;
            }
            self.stack.sync(kind, rect);
            if let Some(slot) = self.rows.iter_mut().find(|(slot, _)| *slot == kind) {
                slot.1 = rows;
            } else {
                self.rows.push((kind, rows));
            }
            true
        }

        pub fn set_dock(&mut self, rect: Option<Rect>, rows: Vec<ShellLine>) -> bool {
            self.set(OverlayKind::Dock, rect, rows)
        }

        pub fn set_progress(&mut self, rect: Option<Rect>, rows: Vec<ShellLine>) -> bool {
            self.set(OverlayKind::Progress, rect, rows)
        }

        pub fn set_toast(&mut self, rect: Option<Rect>, rows: Vec<ShellLine>) -> bool {
            self.set(OverlayKind::Toast, rect, rows)
        }

        /// Visible surfaces in paint order, bottom first.
        #[must_use]
        pub fn paint_list(&self) -> Vec<SurfacePaint> {
            self.stack
                .paint_list()
                .into_iter()
                .map(|item| SurfacePaint {
                    kind: item.kind,
                    z_index: item.z_index,
                    order: item.order,
                    rect: item.rect,
                    rows: self.rows_of(item.kind).to_vec(),
                })
                .collect()
        }

        #[must_use]
        pub fn paint_order(&self) -> Vec<OverlayKind> {
            self.stack.paint_order()
        }

        #[must_use]
        pub fn top_at(&self, x: u16, y: u16) -> Option<OverlayKind> {
            self.stack.top_at(x, y)
        }

        /// All visible rows in paint order. Overlapping rows are concatenated,
        /// not composited: a rect-aware painter uses [`Self::paint_list`]; this
        /// is the line-buffer view for callers that paint sequentially.
        #[must_use]
        pub fn flatten(&self) -> Vec<ShellLine> {
            self.paint_list()
                .into_iter()
                .flat_map(|surface| surface.rows)
                .collect()
        }

        #[must_use]
        pub fn stack(&self) -> &OverlayStack {
            &self.stack
        }

        fn rows_of(&self, kind: OverlayKind) -> &[ShellLine] {
            self.rows
                .iter()
                .find(|(slot, _)| *slot == kind)
                .map_or(&[], |(_, rows)| rows.as_slice())
        }
    }

    /// One `client.api.shell.output` request the caller must perform
    /// (`routes/session/index.tsx:3039-3047`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ShellLoad {
        pub shell_id: String,
        /// Byte/char cursor to read from (`index.tsx:3047`).
        pub cursor: u64,
        /// `SHELL_DISPLAY_LIMIT` (`index.tsx:3044`).
        pub limit: usize,
        /// The shell's own location, which is where the process actually runs
        /// (`index.tsx:3014,3045`).
        pub location: Option<String>,
        /// `loadBackgroundOutput(drain = true)`: keep reading until the tail.
        pub drain: bool,
    }

    /// One response of `client.api.shell.output`
    /// (`index.tsx:3040-3057`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ShellChunk {
        pub output: String,
        pub cursor: u64,
        pub size: u64,
    }

    /// Background shell output state (`ShellDisplay`,
    /// `routes/session/index.tsx:2982-3106`).
    ///
    /// Encoded upstream rules:
    /// - a collapsed background shell does not poll at all
    ///   (`index.tsx:3079`: `if (props.background && !expanded()) return`);
    /// - an expanded or foreground shell polls once a second
    ///   (`index.tsx:3081`);
    /// - completion drains the remaining output (`index.tsx:3074`);
    /// - the accumulated output is bounded at 1 MiB from the tail, and the
    ///   truncation head label is added (`index.tsx:3052-3055,3090`);
    /// - the shell's own location is used for every request, so a session that
    ///   moved keeps following its original process (`index.tsx:3013-3014`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct BackgroundShell {
        shell_id: Option<String>,
        /// Captured from `data.shell.get(id)?.location` at construction; never
        /// replaced by a later session location.
        location: Option<String>,
        /// Session location, used only when the shell has none
        /// (`index.tsx:3014` fallback).
        session_location: Option<String>,
        inline_output: Option<String>,
        status: String,
        background: bool,
        registered: bool,
        expanded: bool,
        cursor: u64,
        size: u64,
        output: String,
        truncated: bool,
        loading: bool,
        drain_requested: bool,
        current_drain: bool,
        was_running: bool,
        next_poll_ms: Option<u64>,
    }

    impl BackgroundShell {
        #[must_use]
        pub fn new(
            shell_id: Option<String>,
            location: Option<String>,
            inline_output: Option<String>,
            status: impl Into<String>,
            background: bool,
        ) -> Self {
            Self {
                shell_id,
                location,
                session_location: None,
                inline_output,
                status: status.into(),
                background,
                registered: false,
                expanded: false,
                cursor: 0,
                size: 0,
                output: String::new(),
                truncated: false,
                loading: false,
                drain_requested: false,
                current_drain: false,
                was_running: false,
                next_poll_ms: None,
            }
        }

        /// `data.shell.get(id)` presence (`index.tsx:3017-3020`).
        pub fn set_registered(&mut self, registered: bool) {
            self.registered = registered;
        }

        /// Session location fallback; ignored once a shell location is known.
        pub fn set_session_location(&mut self, location: Option<String>) {
            self.session_location = location;
        }

        /// The location every request must carry.
        #[must_use]
        pub fn location(&self) -> Option<&str> {
            self.location
                .as_deref()
                .or(self.session_location.as_deref())
        }

        pub fn set_status(&mut self, status: impl Into<String>) {
            self.status = status.into();
        }

        #[must_use]
        pub fn is_running(&self) -> bool {
            self.status == "running" || self.registered
        }

        #[must_use]
        pub fn background_running(&self) -> bool {
            self.registered
        }

        #[must_use]
        pub fn is_expanded(&self) -> bool {
            self.expanded
        }

        #[must_use]
        pub fn cursor(&self) -> u64 {
            self.cursor
        }

        #[must_use]
        pub fn size(&self) -> u64 {
            self.size
        }

        #[must_use]
        pub fn truncated(&self) -> bool {
            self.truncated
        }

        #[must_use]
        pub fn next_poll_ms(&self) -> Option<u64> {
            self.next_poll_ms
        }

        #[must_use]
        pub fn is_loading(&self) -> bool {
            self.loading
        }

        /// Run the mount effect at `now_ms` (`index.tsx:3071-3083`): start or
        /// stop polling, and drain when the background process just finished.
        /// Call this when `is_running`/`background`/`expanded` change.
        pub fn sync(&mut self, now_ms: u64) -> Option<ShellLoad> {
            if !self.is_running() {
                self.next_poll_ms = None;
                if self.was_running {
                    self.was_running = false;
                    return self.request(true);
                }
                return None;
            }
            self.was_running = true;
            if self.background && !self.expanded {
                self.next_poll_ms = None;
                return None;
            }
            self.next_poll_ms = Some(now_ms.saturating_add(SHELL_POLL_INTERVAL_MS));
            self.request(false)
        }

        /// Interval callback: returns a request when one second elapsed.
        pub fn poll(&mut self, now_ms: u64) -> Option<ShellLoad> {
            if !self.is_running() || (self.background && !self.expanded) {
                return None;
            }
            match self.next_poll_ms {
                Some(at) if now_ms >= at => {
                    self.next_poll_ms =
                        Some(now_ms.saturating_add(SHELL_POLL_INTERVAL_MS));
                    self.request(false)
                }
                Some(_) => None,
                None => {
                    self.next_poll_ms =
                        Some(now_ms.saturating_add(SHELL_POLL_INTERVAL_MS));
                    self.request(false)
                }
            }
        }

        /// The click affordance (`index.tsx:3102-3106`). Expanding loads
        /// immediately and drains whenever the background process is not
        /// running.
        pub fn set_expanded(&mut self, expanded: bool, now_ms: u64) -> Option<ShellLoad> {
            self.expanded = expanded;
            if !self.is_running() {
                return None;
            }
            if expanded {
                self.next_poll_ms =
                    Some(now_ms.saturating_add(SHELL_POLL_INTERVAL_MS));
                self.request(!self.background_running())
            } else {
                self.next_poll_ms = None;
                None
            }
        }

        /// Apply one response. `None` is the `.catch(() => undefined)` path
        /// (`index.tsx:3047`). Returns the next request while a drain loop is
        /// still owed (`index.tsx:3058-3069`).
        pub fn apply(&mut self, response: Option<ShellChunk>) -> Option<ShellLoad> {
            self.loading = false;
            let Some(chunk) = response else {
                return self.finish_load();
            };
            if !chunk.output.is_empty() {
                let mut next = strip_ansi(&format!("{}{}", self.output, chunk.output));
                if next.len() > SHELL_DISPLAY_LIMIT {
                    next = tail_bytes(&next, SHELL_DISPLAY_LIMIT);
                    self.truncated = true;
                }
                self.output = next;
            }
            if chunk.cursor <= self.cursor {
                return self.finish_load();
            }
            self.cursor = chunk.cursor;
            self.size = chunk.size;
            if self.current_drain && self.cursor < chunk.size {
                // Skip straight to the bounded tail of the accumulated output
                // (`index.tsx:3059-3063`).
                let tail = chunk
                    .size
                    .saturating_sub(u64::try_from(SHELL_DISPLAY_LIMIT).unwrap_or(u64::MAX))
                    .max(self.cursor);
                if tail > self.cursor {
                    self.cursor = tail;
                    self.truncated = true;
                }
                return self.request(true);
            }
            self.finish_load()
        }

        /// `output` memo (`index.tsx:3084-3093`): the visible text, including
        /// the `[earlier output omitted]` head label after truncation.
        #[must_use]
        pub fn output_text(&self) -> Option<String> {
            if self.status == "streaming" {
                return Some(String::new());
            }
            let Some(_) = self.shell_id.as_ref() else {
                return Some(strip_ansi(
                    self.inline_output.as_deref().unwrap_or("").trim(),
                ));
            };
            if self.background && !self.expanded {
                return Some(String::new());
            }
            if self.status == "completed" {
                if let Some(output) = &self.inline_output {
                    return Some(strip_ansi(output.trim()));
                }
            }
            let raw = if self.output.is_empty() {
                self.inline_output.as_deref().unwrap_or("")
            } else {
                self.output.as_str()
            };
            let text = strip_ansi(raw.trim());
            Some(if self.truncated {
                format!("{SHELL_EARLIER_OUTPUT}\n{text}")
            } else {
                text
            })
        }

        fn request(&mut self, drain: bool) -> Option<ShellLoad> {
            let shell_id = self.shell_id.clone()?;
            if self.loading {
                if drain {
                    self.drain_requested = true;
                }
                return None;
            }
            self.loading = true;
            self.current_drain = drain;
            Some(ShellLoad {
                shell_id,
                cursor: self.cursor,
                limit: SHELL_DISPLAY_LIMIT,
                location: self.location().map(str::to_owned),
                drain,
            })
        }

        fn finish_load(&mut self) -> Option<ShellLoad> {
            if self.drain_requested {
                self.drain_requested = false;
                return self.request(true);
            }
            None
        }
    }

    fn strip_ansi(text: &str) -> String {
        crate::native_transcript::strip_ansi(text)
    }

    /// Last `limit` bytes of `text` on a char boundary. JS `slice(-limit)`
    /// counts UTF-16 units; a char-boundary byte tail is the conservative
    /// equivalent for a display cap.
    fn tail_bytes(text: &str, limit: usize) -> String {
        if text.len() <= limit {
            return text.to_owned();
        }
        let mut start = text.len() - limit;
        while start < text.len() && !text.is_char_boundary(start) {
            start += 1;
        }
        text[start..].to_owned()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::native_dock::SHELL_TAB_LABEL;
        use crate::native_overlay_stack::{OverlayKind, Rect};

        fn running_shells() -> Vec<ShellEntry> {
            vec![
                ShellEntry::new("sh-a", "bun test", "running"),
                ShellEntry::new("sh-b", "bun dev", "running"),
                ShellEntry::new("sh-c", "exited", "completed"),
            ]
        }

        // P11 remainder: the dock paints its whole block — border column,
        // padding rows, gap rows, tab row, shell body, hint row — into the
        // shell's dock buffer, with the Shell tab mounted.
        #[test]
        fn dock_paints_shell_tab_rows_into_the_shell_buffer() {
            let mut dock = SessionDock::new();
            dock.open(Some(DockTabId::Shell));
            dock.set_shells(running_shells());
            let mut pages = ShellPages::new();
            let painted = dock.paint(&mut pages, 40, None);
            let block = dock.block(40, None).unwrap();
            assert_eq!(painted, block.height() as usize);
            assert_eq!(pages.dock_len(), block.height() as usize);
            let window: Vec<String> = pages
                .dock_window(painted)
                .iter()
                .map(|line| line.text.clone())
                .collect();
            assert_eq!(window.first().map(String::as_str), Some("┃"), "paddingTop row");
            let title = &window[1];
            assert!(title.contains(SHELL_TAB_LABEL), "title: {title}");
            assert!(title.contains("Subagents  Shell"), "gap-2 tab row: {title}");
            // The `esc` affordance is right-aligned as an off-flow span
            // (`index.tsx:123`), two cells in from the right edge.
            let esc = block.rows[1]
                .spans
                .iter()
                .find(|span| span.text == "esc")
                .expect("esc span");
            assert_eq!(esc.x, 40 - 2 - 3);
            // Two running shells: one gap, two body rows, one gap.
            let body: Vec<&String> = window
                .iter()
                .filter(|row| row.contains("bun test") || row.contains("bun dev"))
                .collect();
            assert_eq!(body.len(), 2);
            assert!(window
                .iter()
                .any(|row| row.contains("output return  kill ctrl+d  tabs ←/→")),
                "hint row: {window:?}");
            assert_eq!(window.last().map(String::as_str), Some("┃"), "paddingBottom row");
        }

        // P11 remainder: shell commands only fire while the shell tab is
        // active (`shell-tab.tsx:63`), and hints follow the resolved keybinds.
        #[test]
        fn shell_commands_are_gated_by_the_active_tab() {
            let mut dock = SessionDock::new();
            assert_eq!(dock.command("composer.shell.down"), ShellAction::None);
            dock.open(None);
            assert_eq!(dock.active(), Some(DockTabId::Subagents));
            dock.set_shells(running_shells());
            assert_eq!(dock.command("composer.shell.down"), ShellAction::None);
            dock.switch_tab(1);
            assert_eq!(dock.active(), Some(DockTabId::Shell));
            assert_eq!(
                dock.command("composer.shell.down"),
                ShellAction::Selected(1)
            );
            assert_eq!(
                dock.command("composer.shell.select"),
                ShellAction::Output("sh-b".into())
            );
            dock.set_shortcuts(ShellShortcuts::disabled());
            let hints = dock.dock().footer_hints();
            assert_eq!(hints[0].label, "output");
            assert_eq!(hints[0].shortcut, None);
            assert_eq!(hints[1].label, "kill");
            assert_eq!(hints[2].label, "tabs");
        }

        // P11 remainder: the empty list renders upstream's exact literal, one
        // cell left of ordinary rows (`shell-tab.tsx:116`), and never exceeds
        // the scrollbox cap (`:115`).
        #[test]
        fn dock_empty_state_and_body_cap() {
            let mut dock = SessionDock::new();
            dock.open(Some(DockTabId::Shell));
            let mut pages = ShellPages::new();
            dock.paint(&mut pages, 40, None);
            let rows: Vec<String> = pages
                .dock_window(pages.dock_len())
                .iter()
                .map(|line| line.text.clone())
                .collect();
            assert!(
                rows.iter().any(|row| row == "┃   No shell commands"),
                "empty row: {rows:?}"
            );
            let many: Vec<ShellEntry> = (0..8)
                .map(|index| ShellEntry::new(format!("id{index}"), format!("cmd{index}"), "running"))
                .collect();
            dock.set_shells(many);
            let block = dock.block(40, None).unwrap();
            assert_eq!(block.body_rows, MAX_DOCK_ROWS);
        }

        // STATE-001 + prompt footer: the running turn paints the scanner at
        // 40 ms, then the interrupt affordance; animations off paints `[⋯]`
        // (`component/prompt/index.tsx:1912-1916`).
        #[test]
        fn footer_status_spins_and_offers_interrupt() {
            let frames = footer_spinner_frames();
            assert_eq!(frames.len(), 8 + 9 + 7 + 30);
            assert_eq!(frames[0], "■⬝⬝⬝⬝⬝⬝⬝");
            assert_eq!(frames[5], "■■■■■■⬝⬝");
            assert_eq!(frames[7], "⬝⬝■■■■■■");
            let mut footer = ShellFooter::new();
            footer.set_running(true);
            let row = &footer.status_rows(80)[0];
            assert_eq!(row.text, " ■⬝⬝⬝⬝⬝⬝⬝ esc interrupt");
            assert!(!footer.advance(0, 0, 0));
            assert!(footer.advance(1, 40, 40));
            let row = &footer.status_rows(80)[0];
            assert!(row.text.starts_with(" ■■⬝⬝⬝⬝⬝⬝ esc interrupt"), "{row:?}");
            footer.set_interrupt_armed(true);
            assert!(footer.status_rows(80)[0].text.ends_with("esc again to interrupt"));
            footer.set_animations(false);
            let row = &footer.status_rows(80)[0];
            assert!(row.text.starts_with(" [⋯] esc again to interrupt"), "{row:?}");
            assert!(!footer.advance(2, 40, 80), "animations off does not tick");
        }

        // Prompt footer idle branch: the location label is muted, at x 0, and
        // truncates (`component/prompt/index.tsx:1946-1963`).
        #[test]
        fn footer_idle_status_shows_the_location_label() {
            let mut footer = ShellFooter::new();
            footer.set_location(Some("/Users/mymac/Projects/opencode-rk-recovery".into()));
            let rows = footer.status_rows(20);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].text.chars().count(), 20);
            assert_eq!(rows[0].spans[0].style, FooterStyle::Muted);
            assert_eq!(footer.repaint_delay_ms(0), None, "idle footers are static");
        }

        // STATE-003: retry, ordinary error, interruption and free limit use the
        // progress model's exact lines and paint classes (`index.tsx:2035-2048,
        // 2529-2570`).
        #[test]
        fn footer_diagnostic_rows_cover_retry_error_and_free_limit() {
            let mut footer = ShellFooter::new();
            footer.set_retry(Some(RetryState {
                at_ms: 10_000,
                attempt: 3,
                message: "boom".into(),
            }));
            let rows = footer.diagnostic_rows(7_000, 80);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].text, "   ⚠ Retrying in 3s · attempt 3 · boom");
            assert_eq!(rows[0].spans[1].style, FooterStyle::Warning);
            assert_eq!(footer.repaint_delay_ms(7_000), Some(1_000));

            // An active retry suppresses the error line entirely: the gate is
            // `error && !interrupted() && !retry` (`index.tsx:2036`).
            footer.set_error(Some(FooterError {
                message: "kaboom".into(),
                interrupted: false,
                free_limit_reset: None,
            }));
            let rows = footer.diagnostic_rows(7_000, 80);
            assert_eq!(rows.len(), 1, "retry suppresses the error row");
            assert!(rows[0].text.starts_with("   ⚠ Retrying in 3s"), "{rows:?}");

            // An active retry suppresses the error line (`index.tsx:2035`).
            footer.set_retry(None);
            footer.set_error(Some(FooterError {
                message: "Step interrupted".into(),
                interrupted: true,
                free_limit_reset: None,
            }));
            assert!(footer.diagnostic_rows(7_000, 80).is_empty());

            footer.set_error(Some(FooterError {
                message: "free".into(),
                interrupted: false,
                free_limit_reset: Some("14:00".into()),
            }));
            let rows = footer.diagnostic_rows(7_000, 120);
            assert_eq!(rows.len(), 2);
            assert!(rows[0].text.contains("They reset at 14:00."));
            assert_eq!(
                rows[1].text,
                "   Upgrade to OpenCode Go → /connect · Switch model → /models"
            );
            assert!(rows[1]
                .spans
                .iter()
                .any(|span| span.text == "/connect" && span.style == FooterStyle::Base));
        }

        // STATE-003: the optional token diagnostics label reaches the footer
        // and the collapsed/expanded marker follows the summary.
        #[test]
        fn footer_token_diagnostics_row_uses_the_progress_label() {
            let mut footer = ShellFooter::new();
            let summary = crate::native_progress::token_diagnostics(
                &[crate::native_progress::TokenStep {
                    usage: crate::native_progress::TokenUsage {
                        input: 1_200,
                        output: 30,
                        ..Default::default()
                    },
                    provider: "p".into(),
                    model: "m".into(),
                }],
                true,
            )
            .unwrap();
            footer.set_tokens(Some(summary));
            let rows = footer.diagnostic_rows(0, 120);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].text.starts_with("   + Tokens: 1 step · latest: 1,230 new"));
            footer.set_tokens_expanded(true);
            assert!(footer.diagnostic_rows(0, 120)[0].text.starts_with("   - Tokens:"));
        }

        // STATE-004: the hint is absent before 3 s, survives metadata updates
        // and disappears on completion/replacement
        // (`routes/session/index.tsx:1894-1926`, `util/delayed-presence.ts`).
        #[test]
        fn footer_background_hint_waits_and_clears_with_the_work() {
            let mut footer = ShellFooter::new();
            footer.set_background_shortcut("ctrl+b");
            let tool = RunningTool::new("m1", "p1", "shell", "running", 0);
            assert!(footer.set_background_work(0, std::slice::from_ref(&tool)));
            assert!(!footer.background_hint_visible());
            assert_eq!(footer.repaint_delay_ms(0), Some(3_000));
            footer.advance(0, 0, 2_999);
            assert!(!footer.background_hint_visible());
            footer.advance(1, 16, 3_000);
            assert!(footer.background_hint_visible());
            let rows = footer.rows(3_000, 80);
            assert!(rows[0].spans.is_empty(), "marginTop gap row");
            assert_eq!(
                rows[1].text,
                "   Press ctrl+b to move running work to the background"
            );
            assert_eq!(rows[1].spans[2].style, FooterStyle::Base);

            // A metadata update keeps the same identity: no re-arm.
            let updated = RunningTool::new("m1", "p1", "shell", "running", 0);
            assert!(!footer.set_background_work(3_100, std::slice::from_ref(&updated)));
            assert!(footer.background_hint_visible());

            // Completion removes it at once.
            assert!(footer.set_background_work(3_200, &[]));
            assert!(!footer.background_hint_visible());
            assert!(!footer
                .rows(3_200, 80)
                .iter()
                .any(|row| row.text.contains("move running work")));
        }

        // P21 remainder: the overlay stack is the single ordering authority for
        // dock, progress and toast; the paint list is exposed bottom-first.
        #[test]
        fn shell_surface_paint_list_orders_dock_progress_toast() {
            let mut surface = ShellSurface::new();
            assert!(surface.set_dock(
                Some(Rect::new(0, 10, 40, 3)),
                vec![ShellLine::new("dock", false)]
            ));
            assert!(surface.set_progress(
                Some(Rect::new(0, 13, 40, 1)),
                vec![ShellLine::new("progress", false)]
            ));
            assert!(surface.set_toast(
                Some(Rect::new(30, 0, 10, 1)),
                vec![ShellLine::new("toast", false)]
            ));
            assert_eq!(
                surface.paint_order(),
                vec![OverlayKind::Dock, OverlayKind::Progress, OverlayKind::Toast]
            );
            let flattened: Vec<String> = surface
                .flatten()
                .into_iter()
                .map(|line| line.text)
                .collect();
            assert_eq!(flattened, vec!["dock", "progress", "toast"]);
            assert_eq!(surface.top_at(35, 0), Some(OverlayKind::Toast));
            assert_eq!(surface.top_at(1, 10), Some(OverlayKind::Dock));

            // Unmounting removes the surface from the paint list.
            assert!(surface.set_dock(None, Vec::new()));
            assert_eq!(
                surface.paint_order(),
                vec![OverlayKind::Progress, OverlayKind::Toast]
            );
            // A lifecycle overlay outranks the whole bottom stack by z.
            assert!(!surface.set(OverlayKind::Startup, Some(Rect::new(0, 0, 1, 1)), Vec::new()));
        }

        // TOOL-006: a collapsed background shell never polls; expanding polls
        // immediately and then once a second.
        #[test]
        fn background_shell_collapsed_does_not_poll() {
            let mut shell = BackgroundShell::new(
                Some("sh-1".into()),
                Some("/orig".into()),
                None,
                "completed",
                true,
            );
            shell.set_registered(true);
            assert!(shell.is_running());
            assert_eq!(shell.sync(0), None);
            assert_eq!(shell.poll(10_000), None);
            assert_eq!(shell.next_poll_ms(), None);
            let load = shell.set_expanded(true, 20_000).unwrap();
            assert_eq!(load.cursor, 0);
            assert_eq!(load.limit, SHELL_DISPLAY_LIMIT);
            assert_eq!(load.location.as_deref(), Some("/orig"));
            assert_eq!(shell.apply(None), None);
            assert_eq!(shell.next_poll_ms(), Some(21_000));
            assert_eq!(shell.poll(20_500), None);
            assert!(shell.poll(21_000).is_some());
        }

        // TOOL-006: completion drains the remaining output and jumps to the
        // bounded tail; the head label marks truncation.
        #[test]
        fn background_shell_drains_on_completion_and_bounds_output() {
            let mut shell = BackgroundShell::new(
                Some("sh-2".into()),
                Some("/orig".into()),
                None,
                "running",
                false,
            );
            assert!(shell.sync(0).is_some());
            assert_eq!(shell.apply(None), None);
            shell.set_registered(true);
            shell.set_status("completed");
            assert!(shell.is_running(), "the process still runs in the background");
            shell.set_registered(false);
            let drain = shell.sync(1_000).unwrap();
            assert!(drain.drain);
            assert_eq!(drain.cursor, 0);
            // Skip straight to the tail of a >limit output.
            let size = u64::try_from(SHELL_DISPLAY_LIMIT).unwrap() + 100;
            let next = shell
                .apply(Some(ShellChunk {
                    output: "first".into(),
                    cursor: 10,
                    size,
                }))
                .unwrap();
            assert!(next.drain);
            assert!(shell.truncated());
            // One `apply` mirrors one loop iteration: the cursor jumps to the
            // bounded tail (`max(cursor, size - limit)`), not to `size`
            // (`index.tsx:3060-3064`).
            assert_eq!(shell.cursor(), size - u64::try_from(SHELL_DISPLAY_LIMIT).unwrap());
            // The follow-up response lands exactly on `size` and terminates.
            assert_eq!(
                shell.apply(Some(ShellChunk { output: String::new(), cursor: size, size })),
                None
            );
            assert_eq!(shell.cursor(), size);
            let text = shell.output_text().unwrap();
            assert!(text.starts_with(SHELL_EARLIER_OUTPUT));
            assert!(text.len() <= SHELL_DISPLAY_LIMIT + SHELL_EARLIER_OUTPUT.len() + 1);
        }

        // TOOL-006: a moved session keeps the shell's original location for
        // every request (`index.tsx:3013-3014`).
        #[test]
        fn background_shell_keeps_the_original_location() {
            let mut shell = BackgroundShell::new(
                Some("sh-3".into()),
                Some("/orig".into()),
                None,
                "running",
                false,
            );
            shell.set_session_location(Some("/moved".into()));
            let load = shell.sync(0).unwrap();
            assert_eq!(load.location.as_deref(), Some("/orig"));
            // Without a shell location the session location is the fallback.
            let mut fallback =
                BackgroundShell::new(Some("sh-4".into()), None, None, "running", false);
            fallback.set_session_location(Some("/moved".into()));
            assert_eq!(
                fallback.sync(0).unwrap().location.as_deref(),
                Some("/moved")
            );
        }

        // TOOL-005 companion: completed shells return their inlined output,
        // collapsed background shells return nothing
        // (`index.tsx:3084-3093`).
        #[test]
        fn background_shell_output_text_mirrors_the_memo() {
            let mut shell = BackgroundShell::new(
                Some("sh-5".into()),
                None,
                Some("\u{1b}[31mhello\u{1b}[0m\n".into()),
                "completed",
                true,
            );
            assert_eq!(shell.output_text().as_deref(), Some(""));
            shell.set_expanded(true, 0);
            assert_eq!(shell.output_text().as_deref(), Some("hello"));
            let plain = BackgroundShell::new(None, None, Some("inline\n".into()), "completed", false);
            assert_eq!(plain.output_text().as_deref(), Some("inline"));
        }
    }
}
