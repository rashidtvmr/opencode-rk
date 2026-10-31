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
/// (transcript, composer, sidebar). Caller sizes each window from the
/// region height (`ShellLayout` rect) so pages never overlap.
#[derive(Clone, Debug, Default)]
pub struct ShellPages {
    transcript: ShellBuffer,
    composer: ShellBuffer,
    sidebar: ShellBuffer,
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
