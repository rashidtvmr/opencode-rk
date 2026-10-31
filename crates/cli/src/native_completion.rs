#![forbid(unsafe_code)]
//! Composer-owned, bounded completion state. Acceptance only edits the draft;
//! dispatch and permission checks remain in the ordinary submit path.
//!
//! Geometry and row presentation mirror upstream at pin `450e7b9`
//! (`component/prompt/autocomplete.tsx:560-660,818-960`): the popup is an
//! absolute overlay above the measured prompt anchor, its height is
//! `min(10, options or 1, max(1, anchor.y))`, rows reserve four cells plus a
//! kind label, titles middle-truncate, and empty/loading/error views use
//! distinct messages.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub value: String,
    pub description: String,
    /// Disabled catalogue entries remain visible but cannot be accepted.
    pub disabled: bool,
}

/// Optional right-aligned kind label (`autocomplete.tsx:844-848`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Skill,
    Agent,
    Reference,
}

impl Kind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Agent => "agent",
            Self::Reference => "reference",
        }
    }
}

/// The completion source whose empty state is being rendered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionMode {
    Command,
    Reference,
    Directory,
}

/// Asynchronous search state for reference/directory completion
/// (`autocomplete.tsx:829-852`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchStatus {
    pub loading: bool,
    pub failed: bool,
}

/// Presentation inputs that do not belong to the option list itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PanelState {
    /// `None` derives The mode from the active trigger marker.
    pub mode: Option<CompletionMode>,
    pub status: SearchStatus,
}

#[derive(Default)]
pub struct Completion {
    pub commands: Vec<Item>,
    pub mentions: Vec<Item>,
    selected: usize,
    draft: String,
    dismissed: Option<String>,
    kinds: BTreeMap<String, Kind>,
    destructive: BTreeMap<String, String>,
}

impl Completion {
    pub fn triggered(&self) -> bool { self.trigger().is_some() }
    pub fn replace_mentions(&mut self, items: Vec<Item>) {
        let active = self.trigger().is_some_and(|(_, marker, _)| marker == '@');
        let previous = if active { self.visible().get(self.selected()).map(|item| item.value.clone()) } else { None };
        self.mentions = items;
        if active {
            self.selected = previous.and_then(|value| self.visible().iter().position(|item| item.value == value)).unwrap_or(0);
        }
    }
    pub fn sync(&mut self, draft: &str) {
        if self.draft != draft {
            self.draft = draft.to_owned();
            self.selected = 0;
            self.dismissed = None;
        }
    }

    fn trigger(&self) -> Option<(usize, char, &str)> {
        if self.dismissed.as_deref() == Some(&self.draft) || self.draft.starts_with('!') { return None; }
        if let Some(query) = self.draft.strip_prefix('/') {
            return (!query.chars().any(char::is_whitespace)).then_some((0, '/', query));
        }
        let start = self.draft.rfind(char::is_whitespace).map_or(0, |index| {
            index + self.draft[index..].chars().next().unwrap().len_utf8()
        });
        let query = self.draft[start..].strip_prefix('@')?;
        Some((start, '@', query))
    }

    pub fn visible(&self) -> Vec<&Item> {
        let Some((_, marker, query)) = self.trigger() else { return Vec::new() };
        let query = query.to_lowercase();
        let source = if marker == '/' { &self.commands } else { &self.mentions };
        source.iter().filter(|item| item.value.strip_prefix(marker).is_some_and(|value| !value.is_empty() && value.to_lowercase().contains(&query)))
            .take(128).collect()
    }

    pub fn selected(&self) -> usize { self.selected.min(self.visible().len().saturating_sub(1)) }

    pub fn selected_command(&self) -> Option<&str> {
        let (_, marker, _) = self.trigger()?;
        if marker != '/' { return None; }
        self.visible().get(self.selected()).filter(|item| !item.disabled).map(|item| item.value.as_str())
    }

    pub fn selected_command_at(&self, draft: &str, cursor: usize) -> Option<&str> {
        let items = self.visible_at(draft, cursor);
        items.get(self.selected.min(items.len().saturating_sub(1))).filter(|item| !item.disabled).map(|item| item.value.as_str())
    }

    /// Overlay rows fit strictly above the composer, including a spacer and
    /// keyboard hint. The selected row stays visible as selection scrolls.
    pub fn panel(&self, anchor: (u16, u16, u16, u16)) -> Option<Panel> {
        self.panel_at(&self.draft, self.draft.len(), anchor)
    }

    pub fn panel_at(&self, draft: &str, cursor: usize, anchor: (u16, u16, u16, u16)) -> Option<Panel> {
        self.panel_state(draft, cursor, anchor, PanelState::default(), None)
    }

    /// Popup geometry and rows for a specific search state.
    ///
    /// While a trigger is active the popup is visible even with zero options:
    /// upstream paints one message row for searching/empty/error. Height is
    /// `min(10, max(1, count), max(1, anchor.y))`, positioned entirely above
    /// the anchor (`autocomplete.tsx:829-869`).
    pub fn panel_state(
        &self,
        draft: &str,
        cursor: usize,
        anchor: (u16, u16, u16, u16),
        state: PanelState,
        confirming: Option<&str>,
    ) -> Option<Panel> {
        let (x, y, width, _) = anchor;
        if y == 0 || width == 0 {
            return None;
        }
        let (_, marker, _) = self.trigger_at(draft, cursor)?;
        let items = self.visible_at(draft, cursor);
        let mode = state.mode.unwrap_or(if marker == '/' {
            CompletionMode::Command
        } else {
            CompletionMode::Reference
        });
        let count = items.len();
        let height = count.max(1).min(10).min(usize::from(y).max(1));
        if count == 0 {
            let message = empty_message(mode, state.status);
            return Some(Panel {
                x,
                y: y.saturating_sub(1),
                width,
                rows: vec![(format!(" {message}"), false)],
                values: Vec::new(),
                disabled: Vec::new(),
                height: 1,
                empty: Some(message.to_owned()),
                empty_is_error: matches!(mode, CompletionMode::Reference)
                    && state.status.failed
                    && !state.status.loading,
                kind_labels: vec![None],
                confirming: vec![false],
            });
        }
        let selected = self.selected.min(count - 1);
        let start = selected.saturating_sub(height - 1);
        let mut rows = Vec::new();
        let mut values = Vec::new();
        let mut disabled = Vec::new();
        let mut kind_labels = Vec::new();
        let mut confirming_rows = Vec::new();
        for (index, item) in items.iter().enumerate().skip(start).take(height) {
            let is_confirming =
                confirming.is_some_and(|id| id == item.value);
            let kind = self.kinds.get(&item.value).copied();
            let row = styled_row(
                item,
                kind,
                confirming.and_then(|id| {
                    (id == item.value)
                        .then(|| self.destructive.get(&item.value).map(String::as_str))
                        .flatten()
                }),
                width,
            );
            rows.push((row, index == selected));
            values.push(item.value.clone());
            disabled.push(item.disabled);
            kind_labels.push(kind);
            confirming_rows.push(is_confirming);
        }
        let height = rows.len() as u16;
        Some(Panel {
            x,
            y: y - height,
            width,
            rows,
            values,
            disabled,
            height,
            empty: None,
            empty_is_error: false,
            kind_labels,
            confirming: confirming_rows,
        })
    }

    /// Record the optional kind label for a value (`autocomplete.tsx:844-848`).
    pub fn set_kind(&mut self, value: &str, kind: Kind) {
        self.kinds.insert(value.to_owned(), kind);
    }

    /// Record the destructive confirmation text for a value
    /// (`autocomplete.tsx:640-660`).
    pub fn set_destructive(&mut self, value: &str, confirm: &str) {
        self.destructive
            .insert(value.to_owned(), confirm.to_owned());
    }

    /// Hover/mouse-move selection. Changing the row clears any pending
    /// destructive confirmation (`autocomplete.tsx:590-600`).
    pub fn hover_select(&mut self, index: usize) -> bool {
        let count = self.visible().len();
        if count == 0 {
            return false;
        }
        let next = index.min(count - 1);
        if next != self.selected {
            self.selected = next;
            return true;
        }
        false
    }

    pub fn mouse_down(&mut self, index: usize) -> bool {
        self.hover_select(index)
    }

    /// Mouse up accepts the hovered row (`autocomplete.tsx:920-924`).
    pub fn mouse_up(&mut self, draft: &mut String, limit: usize) -> bool {
        self.accept(draft, limit)
    }

    /// First press arms the destructive confirmation, second press runs it
    /// (`autocomplete.tsx:640-657`). `Run` is returned to the caller, which
    /// owns the action; selection clamps to the shortened list.
    pub fn trigger_destructive(
        &mut self,
        value: &str,
        confirming: &mut Option<String>,
    ) -> DestructiveOutcome {
        if !self.destructive.contains_key(value) {
            return DestructiveOutcome::None;
        }
        if confirming.as_deref() != Some(value) {
            *confirming = Some(value.to_owned());
            return DestructiveOutcome::Confirm;
        }
        *confirming = None;
        let count = self.visible().len();
        self.selected = self.selected.min(count.saturating_sub(2));
        DestructiveOutcome::Run
    }

    /// Move to a concrete row (used by keyboard navigation).
    pub fn move_to(&mut self, index: usize) -> bool {
        self.hover_select(index)
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected()
    }

    pub fn navigate(&mut self, down: bool) {
        let count = self.visible().len();
        self.navigate_count(count, down);
    }

    fn navigate_count(&mut self, count: usize, down: bool) {
        if count > 0 {
            self.selected = if down { (self.selected + 1) % count } else { (self.selected + count - 1) % count };
        }
    }

    /// Navigate the completion set belonging to the token at `cursor`, not
    /// the full draft's end token. This is the native equivalent of the
    /// prompt's cursor-relative completion model.
    pub fn navigate_at(&mut self, draft: &str, cursor: usize, down: bool) {
        let count = self.visible_at(draft, cursor).len();
        self.navigate_count(count, down);
    }

    fn command_items_at(&self, draft: &str, cursor: usize) -> Option<Vec<&Item>> {
        if draft != self.draft || self.dismissed.as_deref() == Some(&self.draft) || draft.starts_with('!') || cursor > draft.len() || !draft.is_char_boundary(cursor) { return None; }
        let prefix = &draft[..cursor];
        let start = prefix.rfind(char::is_whitespace).map_or(0, |index| {
            index + prefix[index..].chars().next().unwrap().len_utf8()
        });
        let token = &prefix[start..];
        let marker = token.chars().next()?;
        if marker != '/' && marker != '@' { return None; }
        let query = token.get(marker.len_utf8()..)?;
        if query.chars().any(char::is_whitespace) { return None; }
        let query = query.to_lowercase();
        let source = if marker == '/' { &self.commands } else { &self.mentions };
        Some(source.iter().filter(|item| {
            item.value.strip_prefix(marker).is_some_and(|value| !value.is_empty() && value.to_lowercase().contains(&query))
        }).take(128).collect())
    }

    fn trigger_at<'a>(&self, draft: &'a str, cursor: usize) -> Option<(usize, char, &'a str)> {
        if draft != self.draft || self.dismissed.as_deref() == Some(&self.draft) || draft.starts_with('!') || cursor > draft.len() || !draft.is_char_boundary(cursor) { return None; }
        let prefix = &draft[..cursor];
        let start = match prefix.rfind(char::is_whitespace) {
            Some(index) => index + prefix[index..].chars().next()?.len_utf8(),
            None => 0,
        };
        let token = &prefix[start..];
        let marker = token.chars().next()?;
        if marker != '/' && marker != '@' { return None; }
        Some((start, marker, token.get(marker.len_utf8()..)?))
    }

    fn visible_at(&self, draft: &str, cursor: usize) -> Vec<&Item> {
        self.command_items_at(draft, cursor).unwrap_or_default()
    }

    pub fn dismiss(&mut self) { self.dismissed = Some(self.draft.clone()); }

    pub fn accept(&mut self, draft: &mut String, limit: usize) -> bool {
        if draft != &self.draft { return false; }
        let Some((start, _, _)) = self.trigger() else { return false; };
        let Some(item) = self.visible().get(self.selected()).copied() else { return false; };
        if item.disabled { return false; }
        // `accept` is the end-of-draft path; cursor-relative suffixes use
        // `accept_value_at`, so no completed-value-length slice is needed here.
        let replacement = format!("{} ", item.value);
        if start + replacement.len() > limit { return false; }
        draft.replace_range(start.., &replacement);
        self.sync(draft);
        true
    }

    pub fn accept_value(&mut self, draft: &mut String, value: &str, limit: usize) -> bool {
        self.accept_value_at(draft, value, draft.len(), limit).is_some()
    }

    /// Accept a command at the cursor without consuming the suffix.  The
    /// returned byte offset is the cursor immediately after the inserted
    /// token (including its separating space), allowing the native composer
    /// to restore it after replacing its draft.
    pub fn accept_value_at(
        &mut self,
        draft: &mut String,
        value: &str,
        cursor: usize,
        limit: usize,
    ) -> Option<usize> {
        let (start, marker, query) = self.trigger_at(draft, cursor)?;
        let items = self.visible_at(draft, cursor);
        let item = items.iter().find(|item| item.value == value)?;
        if item.disabled || !value.starts_with(marker) || query.chars().any(char::is_whitespace) { return None; }
        let suffix = &draft[cursor..];
        let replacement = format!("{value}{}", (!suffix.starts_with(' ')).then_some(" ").unwrap_or(""));
        let new_len = draft.len() - (cursor - start) + replacement.len();
        if new_len > limit { return None; }
        draft.replace_range(start..cursor, &replacement);
        let next = start + replacement.len();
        self.sync(draft);
        Some(next)
    }

    /// False means the ordinary composer key path retains ownership, including
    /// Enter when the bounded replacement cannot be inserted.
    pub fn complete_key(&mut self, key: u8, draft: &mut String, limit: usize) -> bool {
        matches!(key, b'\t' | b'\r' | b'\n') && self.accept(draft, limit)
    }

    /// Cursor-aware counterpart for native dispatch. Returns the inserted
    /// cursor position when completion was accepted.
    pub fn complete_key_at(&mut self, key: u8, draft: &mut String, cursor: usize, limit: usize) -> Option<usize> {
        if !matches!(key, b'\t' | b'\r' | b'\n') { return None; }
        if draft != &self.draft || cursor > draft.len() || !draft.is_char_boundary(cursor) { return None; }
        let prefix = &draft[..cursor];
        let start = prefix.rfind(char::is_whitespace).map_or(0, |index| {
            index + prefix[index..].chars().next().unwrap().len_utf8()
        });
        let items = self.command_items_at(draft, cursor)?;
        let item = items.get(self.selected.min(items.len().saturating_sub(1)))?;
        let item = item.value.clone();
        self.accept_value_at(draft, &item, cursor, limit)
    }
}

pub fn description(text: &str) -> String {
    crate::native_view_layout::fit(text, 256).trim_end().to_owned()
}

pub struct Panel {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    /// Painted height, `min(10, max(1, options), max(1, anchor.y))`.
    pub height: u16,
    pub rows: Vec<(String, bool)>,
    /// Present when the trigger is active but there are no options.
    pub empty: Option<String>,
    /// Error styling for a failed reference search (`autocomplete.tsx:850-852`).
    pub empty_is_error: bool,
    pub kind_labels: Vec<Option<Kind>>,
    pub confirming: Vec<bool>,
    values: Vec<String>,
    disabled: Vec<bool>,
}

/// Result of a destructive-row activation (`autocomplete.tsx:640-657`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DestructiveOutcome {
    None,
    Confirm,
    Run,
}

/// Upstream empty-view message (`autocomplete.tsx:840-850`).
#[must_use]
pub fn empty_message(mode: CompletionMode, status: SearchStatus) -> &'static str {
    match mode {
        CompletionMode::Command => "No matching commands",
        CompletionMode::Directory => {
            if status.loading {
                "Searching…"
            } else if status.failed {
                "Could not search directories. Keep typing to try again."
            } else {
                "No matching directories"
            }
        }
        CompletionMode::Reference => {
            if status.loading {
                "Searching…"
            } else if status.failed {
                "Could not search files. Keep typing to try again."
            } else {
                "No matching files, agents, or references"
            }
        }
    }
}

/// Cells occupied by `text` under the shared renderer segmentation.
#[must_use]
pub fn display_width(text: &str) -> usize {
    crate::native_view_layout::visit_unpadded_cells(text, 4096, |_, _| {})
}

/// Take at most `budget` display cells without splitting a cluster.
fn take_cells(text: &str, budget: usize) -> String {
    let mut out = String::new();
    crate::native_view_layout::visit_unpadded_cells(text, budget, |glyph, _| out.push_str(glyph));
    out
}

/// `Locale.truncateMiddle` (`util/locale.ts:89-97`): UTF-16 units, but never
/// splitting a Rust scalar/display cluster.
#[must_use]
pub fn truncate_middle(text: &str, max_units: usize) -> String {
    if text.encode_utf16().count() <= max_units {
        return text.to_owned();
    }
    if max_units <= 1 {
        return "…".to_owned();
    }
    let ellipsis = 1;
    let keep_start = (max_units - ellipsis).div_ceil(2);
    let keep_end = (max_units - ellipsis) / 2;
    let mut start_units = 0;
    let mut start = String::new();
    for ch in text.chars() {
        let units = ch.len_utf16();
        if start_units + units > keep_start {
            break;
        }
        start_units += units;
        start.push(ch);
    }
    let mut end_units = 0;
    let mut end_reversed = Vec::new();
    for ch in text.chars().rev() {
        let units = ch.len_utf16();
        if end_units + units > keep_end {
            break;
        }
        end_units += units;
        end_reversed.push(ch);
    }
    let end: String = end_reversed.into_iter().rev().collect();
    format!("{start}…{end}")
}

/// One row with upstream presentation rules: one cell of padding, title
/// middle-truncated to `width - 4 - (label + 2)`, normalized description, and
/// a right-aligned kind label. Total painted width never exceeds `width`.
fn styled_row(
    item: &Item,
    kind: Option<Kind>,
    confirm: Option<&str>,
    width: u16,
) -> String {
    let width = usize::from(width);
    let label = kind.map(Kind::label);
    let label_cells = label.map_or(0, |label| 2 + display_width(label));
    let content_width = width.saturating_sub(4 + label_cells).max(1);
    let title = truncate_middle(confirm.unwrap_or(&item.value), content_width);
    let mut row = String::from(" ");
    row.push_str(&title);
    if confirm.is_none() && !item.description.is_empty() {
        let desc = item
            .description
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let fixed = 2 + display_width(&title) + label_cells + 1;
        let budget = width.saturating_sub(fixed);
        if budget > 0 {
            row.push(' ');
            row.push_str(&take_cells(&desc, budget));
        }
    }
    if let Some(label) = label {
        row.push_str("  ");
        row.push_str(label);
    }
    row.push(' ');
    row
}

impl Panel {
    /// Return the command under a left-button press. Coordinates are derived
    /// from the actual rendered panel rectangle and rows, not a guessed popup
    /// offset. Non-command panels never produce an executable value.
    pub fn value_at(&self, x: u16, y: u16) -> Option<&str> {
        if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return None; }
        self.values.get(usize::from(y - self.y)).map(String::as_str)
    }

    /// Disabled catalogue entries may be painted and hit-tested, but never accepted.
    pub fn is_disabled_at(&self, x: u16, y: u16) -> bool {
        if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return true; }
        self.disabled.get(usize::from(y - self.y)).copied().unwrap_or(true)
    }

    pub fn command_at(&self, x: u16, y: u16) -> Option<&str> {
        self.value_at(x, y).filter(|value| value.starts_with('/'))
    }
}

/// Only relative file names are indexed; this never reads file contents.
pub fn workspace_files(root: &std::path::Path) -> Vec<Item> {
    let Ok(root) = root.canonicalize() else { return Vec::new() };
    let mut pending = std::collections::VecDeque::from([(root.clone(), 0)]);
    let mut items = Vec::new();
    let mut visited = 0;
    let started = std::time::Instant::now();
    while let Some((directory, depth)) = pending.pop_front() {
        if visited >= 8192 || items.len() >= 2048 || started.elapsed() >= std::time::Duration::from_secs(2) { break; }
        if directory.canonicalize().map_or(true, |path| !path.starts_with(&root)) { continue; }
        let Ok(entries) = std::fs::read_dir(&directory) else { continue; };
        let mut paths = Vec::new();
        for entry in entries {
            visited += 1;
            if visited > 8192 { break; }
            if let Ok(entry) = entry { paths.push(entry.path()); }
        }
        paths.sort();
        for path in paths {
            if items.len() >= 2048 { break; }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else { continue; };
            if name.starts_with('.') || matches!(name, "target" | "node_modules" | "vendor" | "dist" | "build") { continue; }
            let Ok(metadata) = path.symlink_metadata() else { continue; };
            if metadata.file_type().is_symlink() { continue; }
            if metadata.is_dir() && depth < 8 { pending.push_back((path, depth + 1)); }
            else if metadata.is_file() {
                let Ok(relative) = path.strip_prefix(&root) else { continue; };
                let Some(relative) = relative.to_str() else { continue; };
                if relative.len() > 512 || relative.chars().any(|ch| ch.is_control() || ch.is_whitespace()) { continue; }
                items.push(Item { value: format!("@{relative}"), description: "file".into(), disabled: false });
            }
        }
    }
    items.sort_by(|a, b| a.value.cmp(&b.value));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Completion {
        Completion {
            commands: vec![Item { value: "/models".into(), description: "Select model".into(), disabled: false }, Item { value: "/mcp".into(), description: "MCP status".into(), disabled: false }],
            mentions: vec![Item { value: "@explore".into(), description: "agent".into(), disabled: false }, Item { value: "@src/café.rs".into(), description: "file".into(), disabled: false }],
            ..Completion::default()
        }
    }
    #[test]
    fn slash_filters_and_arrows_choose_without_submitting() {
        let mut c = fixture();
        let mut draft = "/m".to_owned();
        c.sync(&draft);
        assert_eq!(c.visible().len(), 2);
        c.navigate(true);
        assert!(c.accept(&mut draft, 4096));
        assert_eq!(draft, "/mcp ");
    }
    #[test]
    fn mention_replaces_only_active_unicode_token() {
        let mut c = fixture();
        let mut draft = "Please inspect café @src/caf".to_owned();
        c.sync(&draft);
        assert_eq!(c.visible().len(), 1);
        assert!(c.accept(&mut draft, 4096));
        assert_eq!(draft, "Please inspect café @src/café.rs ");
    }

    #[test]
    fn command_acceptance_replaces_token_at_cursor_and_preserves_suffix() {
        let mut c = fixture();
        let mut draft = "prefix /mo suffix 世界".to_owned();
        let cursor = "prefix /mo".len();
        c.sync(&draft);
        assert_eq!(c.accept_value_at(&mut draft, "/models", cursor, 4096), Some("prefix /models".len()));
        assert_eq!(draft, "prefix /models suffix 世界");
    }

    #[test]
    fn complete_key_at_uses_middle_cursor_and_keeps_unicode_suffix() {
        let mut c = fixture();
        let mut draft = "prefix /mo suffix 世界".to_owned();
        let cursor = "prefix /mo".len();
        c.sync(&draft);
        let next = c.complete_key_at(b'\t', &mut draft, cursor, 4096);
        assert_eq!(next, Some("prefix /models".len()));
        assert_eq!(draft, "prefix /models suffix 世界");
    }

    #[test]
    fn mention_panel_highlight_and_keyboard_acceptance_share_local_selection() {
        let mut c = fixture();
        c.mentions.extend([
            Item { value: "@src/café.txt".into(), description: "text".into(), disabled: false },
            Item { value: "@src/café.md".into(), description: "markdown".into(), disabled: false },
        ]);
        let mut draft = "prefix @src/caf suffix 世界".to_owned();
        let cursor = "prefix @src/caf".len();
        c.sync(&draft);
        c.navigate_at(&draft, cursor, false);
        let panel = c.panel_at(&draft, cursor, (0, 10, 80, 1)).unwrap();
        let selected_row = panel.rows.iter().find(|(_, selected)| *selected).unwrap().0.clone();
        let items = c.visible_at(&draft, cursor);
        let selected = items[items.len() - 1].value.clone();
        assert!(selected_row.contains(&selected));
        let expected = selected.clone();
        let expected_cursor = "prefix ".len() + expected.len();
        assert_eq!(c.complete_key_at(b'\t', &mut draft, cursor, 4096), Some(expected_cursor));
        assert_eq!(draft, format!("prefix {expected} suffix 世界"));
    }

    #[test]
    fn dismissed_and_shell_prefixed_middle_tokens_are_not_visible_or_acceptable() {
        let mut c = fixture();
        let mut draft = "prefix /mo suffix".to_owned();
        let cursor = "prefix /mo".len();
        c.sync(&draft);
        c.dismiss();
        assert!(c.panel_at(&draft, cursor, (0, 10, 80, 1)).is_none());
        assert!(c.complete_key_at(b'\t', &mut draft, cursor, 4096).is_none());
        c.sync("! prefix /mo suffix");
        assert!(c.panel_at("! prefix /mo suffix", "! prefix /mo".len(), (0, 10, 80, 1)).is_none());
    }

    #[test]
    fn panel_navigation_and_acceptance_share_middle_cursor_view_for_mentions() {
        let mut c = fixture();
        c.mentions.extend([
            Item { value: "@src/café.txt".into(), description: "text".into(), disabled: false },
            Item { value: "@src/café.md".into(), description: "markdown".into(), disabled: false },
        ]);
        let mut draft = "prefix @src/caf suffix 世界".to_owned();
        let cursor = "prefix @src/caf".len();
        c.sync(&draft);
        c.navigate_at(&draft, cursor, true);
        let panel = c.panel_at(&draft, cursor, (0, 10, 80, 1)).unwrap();
        let selected = c.visible_at(&draft, cursor)[1].value.clone();
        let highlighted = panel.rows.iter().find(|(_, selected)| *selected).unwrap().0.clone();
        assert!(highlighted.contains(&selected));
        let next = c.accept_value_at(&mut draft, &selected, cursor, 4096);
        let expected_cursor = "prefix ".len() + selected.len();
        assert_eq!(next, Some(expected_cursor));
        assert_eq!(draft, format!("prefix {selected} suffix 世界"));
    }

    #[test]
    fn complete_key_at_uses_local_selection_for_middle_token() {
        let mut c = fixture();
        c.commands.extend([
            Item { value: "/more".into(), description: String::new(), disabled: false },
            Item { value: "/module".into(), description: String::new(), disabled: false },
        ]);
        let mut draft = "prefix /mo suffix 世界".to_owned();
        let cursor = "prefix /mo".len();
        c.sync(&draft);
        c.selected = 2;
        let next = c.complete_key_at(b'\t', &mut draft, cursor, 4096);
        assert_eq!(next, Some("prefix /module".len()));
        assert_eq!(draft, "prefix /module suffix 世界");
    }
    #[test]
    fn escape_preserves_draft_and_edit_reopens() {
        let mut c = fixture();
        c.sync("/");
        assert_eq!(c.visible().len(), 2);
        c.dismiss();
        assert!(c.visible().is_empty());
        c.sync("/");
        assert!(c.visible().is_empty());
        c.sync("/mod");
        assert_eq!(c.visible().len(), 1);
    }
    #[test]
    fn prose_email_shell_and_arguments_do_not_trigger() {
        let mut c = fixture();
        for draft in ["user@example.com", "! cat /", "explain /m", "/models x", "@explore "] {
            c.sync(draft);
            assert!(c.visible().is_empty(), "{draft}");
        }
    }
    #[test]
    fn completion_limit_preserves_draft() {
        let mut c = fixture();
        let mut draft = "@src".to_owned();
        c.sync(&draft);
        assert_eq!(c.visible().len(), 1);
        assert!(!c.accept(&mut draft, 5));
        assert_eq!(draft, "@src");
    }
    #[test]
    fn failed_completion_does_not_consume_submit_key_and_bad_items_are_ignored() {
        let mut c = fixture();
        c.commands.push(Item { value: String::new(), description: String::new(), disabled: false });
        c.commands.push(Item { value: "é".into(), description: String::new(), disabled: false });
        let mut draft = "/m".to_owned();
        c.sync(&draft);
        assert_eq!(c.visible().len(), 2);
        assert!(!c.complete_key(b'\r', &mut draft, 3));
        assert_eq!(draft, "/m");
        assert!(!c.complete_key(b'x', &mut draft, 4096));
        assert!(c.complete_key(b'\r', &mut draft, 4096));
        assert_eq!(draft, "/models ");
    }

    #[test]
    fn selected_executable_command_is_available_without_consuming_arguments() {
        let mut c = fixture();
        let mut draft = "/m".to_owned();
        c.sync(&draft);
        assert_eq!(c.selected_command(), Some("/models"));
        assert!(c.accept(&mut draft, 4096));
        assert_eq!(draft, "/models ");
    }
    #[test]
    fn metadata_is_sanitized_and_async_refresh_preserves_selected_identity() {
        assert_eq!(description("safe\x1b[2J\x1b]0;title\x07 text\n"), "safe text");
        let mut c = fixture();
        c.sync("@");
        c.navigate(true);
        c.replace_mentions(vec![Item {value:"@added".into(),description:String::new(),disabled:false},
            Item {value:"@extra".into(),description:String::new(),disabled:false},
            Item {value:"@src/café.rs".into(),description:"file".into(),disabled:false}]);
        let mut draft = "@".to_owned();
        assert!(c.accept(&mut draft, 4096));
        assert_eq!(draft, "@src/café.rs ");
    }
    #[test]
    fn narrow_anchor_keeps_upstream_popup_instead_of_suppressing_it() {
        let mut c = fixture();
        c.sync("/");
        assert!(c.panel((0, 2, 0, 1)).is_none(),
            "a zero-width popup cannot own completion input");
        let panel = c.panel((0, 2, 1, 1)).expect("upstream popup has no width floor");
        assert_eq!((panel.x, panel.y, panel.width), (0, 0, 1));
        assert_eq!(panel.rows.len(), 2);
    }
    #[test]
    fn panel_is_above_composer_and_selection_scrolls_on_short_viewport() {
        let mut c = fixture();
        c.sync("/");
        c.navigate(true);
        let panel = c.panel((4, 3, 40, 4)).unwrap();
        assert_eq!((panel.x, panel.y, panel.width), (4, 1, 40));
        assert_eq!(panel.rows.len(), 2);
        assert!(panel.rows[1].0.contains("/mcp"));
        assert!(panel.rows[1].1);
        assert!(c.panel((0, 0, 40, 4)).is_none());
    }
    #[test]
    fn first_and_last_visible_rows_hit_exact_values() {
        let mut c = fixture();
        c.commands.clear();
        c.commands.extend((0..12).map(|index| Item { value: format!("/model{index:02}"), description: "command".into(), disabled: false }));
        let draft = "/model".to_owned();
        c.sync(&draft);
        let first = c.panel((2, 12, 60, 1)).unwrap();
        assert_eq!(first.rows.len(), 10);
        assert_eq!(first.value_at(2, first.y), Some("/model00"));
        assert_eq!(first.value_at(2, first.y + 9), Some("/model09"));
        c.selected = c.visible().len() - 1;
        let last = c.panel((2, 12, 60, 1)).unwrap();
        assert_eq!(last.value_at(2, last.y), Some("/model02"));
        assert_eq!(last.value_at(2, last.y + 9), Some("/model11"));
        assert_eq!(last.value_at(2, last.y + 10), None);
    }

    #[test]
    fn disabled_entries_are_visible_but_inert() {
        let mut c = fixture();
        c.commands[0].disabled = true;
        let mut draft = "/m".to_owned();
        c.sync(&draft);
        assert!(c.visible().iter().any(|item| item.disabled));
        assert!(!c.accept(&mut draft, 4096));
        assert_eq!(draft, "/m");
    }

    #[test]
    fn unicode_cursor_and_existing_space_do_not_duplicate_suffix_space() {
        let mut c = fixture();
        let mut draft = "préfix /mo suffix 世界".to_owned();
        let cursor = "préfix /mo".len();
        c.sync(&draft);
        assert_eq!(c.accept_value_at(&mut draft, "/models", cursor, 4096), Some("préfix /models".len()));
        assert_eq!(draft, "préfix /models suffix 世界");
    }
    #[test]
    fn workspace_mentions_are_relative_and_exclude_hidden_build_and_symlink_paths() {
        let root = std::env::temp_dir().join(format!("oc2-completion-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join("src/café.rs"), "fixture").unwrap();
        std::fs::write(root.join(".env"), "fixture").unwrap();
        std::fs::write(root.join("target/build.rs"), "fixture").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("src"), root.join("linked")).unwrap();
        let items = workspace_files(&root);
        assert!(items.iter().any(|item| item.value == "@src/café.rs"));
        assert!(!items.iter().any(|item| item.value.contains(".env") || item.value.contains("target/") || item.value.contains("linked/")));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod popup_geometry_and_style_tests {
    //! CP-39, CP-40, CP-41
    //! (`component/prompt/autocomplete.tsx:560-660,818-960`).

    use super::*;

    fn fixture() -> Completion {
        let mut completion = Completion {
            commands: vec![
                Item { value: "/models".into(), description: "Select model".into(), disabled: false },
                Item { value: "/mcp".into(), description: "MCP status".into(), disabled: false },
            ],
            mentions: vec![
                Item { value: "@explore".into(), description: "agent".into(), disabled: false },
                Item { value: "@src/café.rs".into(), description: "file".into(), disabled: false },
            ],
            ..Completion::default()
        };
        completion.set_kind("@explore", Kind::Agent);
        completion.set_kind("@src/café.rs", Kind::Reference);
        completion
    }

    #[test]
    fn cp39_popup_tracks_anchor_resize_growth_and_short_viewports() {
        let mut completion = fixture();
        completion.commands.extend((0..12).map(|index| Item {
            value: format!("/model{index:02}"),
            description: "command".into(),
            disabled: false,
        }));
        let draft = "/model".to_owned();
        completion.sync(&draft);

        // Wide sidebar-open anchor.
        let wide = completion.panel((10, 12, 80, 2)).unwrap();
        assert_eq!((wide.x, wide.y, wide.width, wide.height), (10, 2, 80, 10));

        // Sidebar toggle/resize moves x and narrows width without reflowing y.
        let narrow = completion.panel((3, 12, 40, 2)).unwrap();
        assert_eq!((narrow.x, narrow.y, narrow.width), (3, 2, 40));

        // Long prompt growth pushes the anchor down; popup follows it.
        let grown = completion.panel((10, 20, 80, 8)).unwrap();
        assert_eq!(grown.y, 10);

        // Near-top prompt: height is capped by available rows above the anchor.
        let near_top = completion.panel((10, 4, 80, 1)).unwrap();
        assert_eq!(near_top.height, 4);
        assert_eq!(near_top.y, 0);

        // Short terminal with one row above the anchor.
        let short = completion.panel((10, 1, 80, 1)).unwrap();
        assert_eq!(short.height, 1);
        assert_eq!(short.y, 0);

        // No room above at all.
        assert!(completion.panel((10, 0, 80, 1)).is_none());
    }

    #[test]
    fn cp39_empty_state_still_paints_one_row_above_anchor() {
        let mut completion = fixture();
        completion.sync("/zzz");
        let panel = completion
            .panel_state(
                "/zzz",
                4,
                (2, 6, 30, 1),
                PanelState::default(),
                None,
            )
            .unwrap();
        assert_eq!(panel.height, 1);
        assert_eq!(panel.y, 5);
        assert_eq!(panel.empty.as_deref(), Some("No matching commands"));
        assert_eq!(panel.rows.len(), 1);
        assert!(panel.value_at(2, 5).is_none());
    }

    #[test]
    fn cp40_keyboard_hover_and_mouse_share_one_selection() {
        let mut completion = fixture();
        let mut draft = "/m".to_owned();
        completion.sync(&draft);
        completion.navigate(true);
        assert_eq!(completion.selected_index(), 1);
        // Hover moves selection back to the first row without accepting.
        assert!(completion.mouse_down(0));
        assert_eq!(completion.selected_index(), 0);
        assert_eq!(draft, "/m");
        // Mouse up accepts the hovered row.
        assert!(completion.mouse_up(&mut draft, 4096));
        assert_eq!(draft, "/models ");
    }

    #[test]
    fn cp40_kind_label_and_destructive_confirmation_replace_description() {
        let mut completion = fixture();
        let mut draft = "@expl".to_owned();
        completion.sync(&draft);
        let panel = completion.panel((0, 8, 40, 1)).unwrap();
        assert_eq!(panel.kind_labels[0], Some(Kind::Agent));
        assert!(panel.rows[0].0.trim_end().ends_with("agent"));

        let mut commands = fixture();
        commands.commands.push(Item {
            value: "/delete".into(),
            description: "Delete session".into(),
            disabled: false,
        });
        commands.set_destructive("/delete", "Press again to delete session");
        let mut draft = "/delete".to_owned();
        commands.sync(&draft);
        let mut confirming = None;
        assert_eq!(
            commands.trigger_destructive("/delete", &mut confirming),
            DestructiveOutcome::Confirm
        );
        let panel = commands
            .panel_state(
                "/delete",
                7,
                (0, 8, 48, 1),
                PanelState::default(),
                confirming.as_deref(),
            )
            .unwrap();
        assert!(panel.confirming[0]);
        assert!(panel.rows[0].0.contains("Press again"));
        assert_eq!(panel.kind_labels[0], None, "label hidden while confirming");
        assert_eq!(
            commands.trigger_destructive("/delete", &mut confirming),
            DestructiveOutcome::Run
        );
        assert_eq!(confirming, None);
    }

    #[test]
    fn cp40_empty_messages_distinguish_command_search_and_failure() {
        assert_eq!(
            empty_message(CompletionMode::Command, SearchStatus::default()),
            "No matching commands"
        );
        assert_eq!(
            empty_message(
                CompletionMode::Reference,
                SearchStatus { loading: true, failed: false }
            ),
            "Searching…"
        );
        assert_eq!(
            empty_message(
                CompletionMode::Reference,
                SearchStatus { loading: false, failed: true }
            ),
            "Could not search files. Keep typing to try again."
        );
        assert_eq!(
            empty_message(
                CompletionMode::Reference,
                SearchStatus { loading: false, failed: false }
            ),
            "No matching files, agents, or references"
        );
        assert_eq!(
            empty_message(
                CompletionMode::Directory,
                SearchStatus { loading: false, failed: true }
            ),
            "Could not search directories. Keep typing to try again."
        );
        assert_eq!(
            empty_message(
                CompletionMode::Directory,
                SearchStatus { loading: false, failed: false }
            ),
            "No matching directories"
        );

        let mut completion = fixture();
        completion.sync("@zzz");
        let failed = completion
            .panel_state(
                "@zzz",
                4,
                (0, 8, 40, 1),
                PanelState {
                    mode: None,
                    status: SearchStatus { loading: false, failed: true },
                },
                None,
            )
            .unwrap();
        assert!(failed.empty_is_error);
        assert_eq!(
            failed.empty.as_deref(),
            Some("Could not search files. Keep typing to try again.")
        );
    }

    #[test]
    fn cp41_titles_truncate_by_utf16_units_without_splitting_clusters() {
        // `"😀"` is two UTF-16 units; JS `length` also reports two.
        let value = "😀😀😀😀😀😀";
        assert_eq!(value.encode_utf16().count(), 12);
        let truncated = truncate_middle(value, 5);
        assert_eq!(truncated, "😀…😀");
        assert_eq!(truncated.encode_utf16().count(), 5);
        assert!(truncated.is_char_boundary(truncated.len()));
        // No truncation at exactly the budget.
        assert_eq!(truncate_middle("café", 4), "café");
        assert_eq!(truncate_middle("café", 1), "…");
    }

    #[test]
    fn cp41_rows_fit_by_cells_for_tabs_cjk_zwj_and_combining_marks() {
        let mut completion = fixture();
        completion.mentions = vec![
            Item { value: "@界\tcafé".into(), description: "a\tb  c".into(), disabled: false },
            Item { value: "@👩‍👩‍👧‍👦".into(), description: "e\u{301}x".into(), disabled: false },
        ];
        completion.set_kind("@👩‍👩‍👧‍👦", Kind::Reference);
        let draft = "@".to_owned();
        completion.sync(&draft);
        let panel = completion.panel((1, 8, 24, 1)).unwrap();
        for row in &panel.rows {
            assert!(display_width(&row.0) <= 24, "row overflows cells: {row:?}");
        }
        assert!(panel.rows.iter().any(|row| row.0.contains("café")));
        assert!(panel.rows.iter().any(|row| row.0.contains("reference")));
    }
}
