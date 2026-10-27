#![forbid(unsafe_code)]
//! Composer-owned, bounded completion state. Acceptance only edits the draft;
//! dispatch and permission checks remain in the ordinary submit path.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub value: String,
    pub description: String,
    /// Disabled catalogue entries remain visible but cannot be accepted.
    pub disabled: bool,
}

#[derive(Default)]
pub struct Completion {
    pub commands: Vec<Item>,
    pub mentions: Vec<Item>,
    selected: usize,
    draft: String,
    dismissed: Option<String>,
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
        let (x, y, width, _) = anchor;
        let items = self.visible_at(draft, cursor);
        if items.is_empty() || y == 0 || width < 12 { return None; }
        let capacity = usize::from(y).min(10);
        let selected = self.selected.min(items.len().saturating_sub(1));
        let start = selected.saturating_sub(capacity - 1);
        let mut rows = Vec::new();
        let mut values = Vec::new();
        let mut disabled = Vec::new();
        for (index, item) in items.iter().enumerate().skip(start).take(capacity) {
            rows.push((format!(" {} {}  {}", if index == selected { "›" } else { " " }, item.value, item.description), index == selected));
            values.push(item.value.clone());
            disabled.push(item.disabled);
        }
        Some(Panel { x, y: y - rows.len() as u16, width, rows, values, disabled })
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
    pub rows: Vec<(String, bool)>,
    values: Vec<String>,
    disabled: Vec<bool>,
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
