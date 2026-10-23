#![forbid(unsafe_code)]
//! Composer-owned, bounded completion state. Acceptance only edits the draft;
//! dispatch and permission checks remain in the ordinary submit path.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub value: String,
    pub description: String,
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
        self.visible().get(self.selected()).map(|item| item.value.as_str())
    }

    /// Overlay rows fit strictly above the composer, including a spacer and
    /// keyboard hint. The selected row stays visible as selection scrolls.
    pub fn panel(&self, anchor: (u16, u16, u16, u16)) -> Option<Panel> {
        let (x, y, width, _) = anchor;
        let items = self.visible();
        if items.is_empty() || y < 3 || width < 12 { return None; }
        let capacity = usize::from(y.saturating_sub(2)).min(6);
        let selected = self.selected();
        let start = selected.saturating_sub(capacity - 1);
        let command_mode = self.trigger()?.1 == '/';
        let mut rows = vec![(if command_mode { " Commands".into() } else { " Mentions".into() }, false)];
        let mut command_values = Vec::new();
        for (index, item) in items.iter().enumerate().skip(start).take(capacity) {
            rows.push((format!(" {} {}  {}", if index == selected { "›" } else { " " }, item.value, item.description), index == selected));
            if command_mode { command_values.push(item.value.clone()); }
        }
        rows.push((" ↑/↓ select · Tab/Enter complete · Esc close".into(), false));
        Some(Panel { x, y: y - rows.len() as u16, width, rows, command_values })
    }

    pub fn navigate(&mut self, down: bool) {
        let count = self.visible().len();
        if count > 0 {
            self.selected = if down { (self.selected + 1) % count } else { (self.selected + count - 1) % count };
        }
    }

    pub fn dismiss(&mut self) { self.dismissed = Some(self.draft.clone()); }

    pub fn accept(&mut self, draft: &mut String, limit: usize) -> bool {
        if draft != &self.draft { return false; }
        let Some((start, _, _)) = self.trigger() else { return false; };
        let Some(item) = self.visible().get(self.selected()).copied() else { return false; };
        if start + item.value.len() + 1 > limit { return false; }
        let replacement = format!("{} ", item.value);
        draft.replace_range(start.., &replacement);
        self.sync(draft);
        true
    }

    pub fn accept_value(&mut self, draft: &mut String, value: &str, limit: usize) -> bool {
        if draft != &self.draft || !value.starts_with('/') { return false; }
        let Some((start, marker, _)) = self.trigger() else { return false };
        if marker != '/' || !self.visible().iter().any(|item| item.value == value) { return false; }
        if start + value.len() + 1 > limit { return false; }
        draft.replace_range(start.., &format!("{value} "));
        self.sync(draft);
        true
    }

    /// False means the ordinary composer key path retains ownership, including
    /// Enter when the bounded replacement cannot be inserted.
    pub fn complete_key(&mut self, key: u8, draft: &mut String, limit: usize) -> bool {
        matches!(key, b'\t' | b'\r' | b'\n') && self.accept(draft, limit)
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
    command_values: Vec<String>,
}

impl Panel {
    /// Return the command under a left-button press. Coordinates are derived
    /// from the actual rendered panel rectangle and rows, not a guessed popup
    /// offset. Non-command panels never produce an executable value.
    pub fn command_at(&self, x: u16, y: u16) -> Option<&str> {
        if x < self.x || x >= self.x.saturating_add(self.width) || y < self.y { return None; }
        let row = usize::from(y - self.y);
        if row == 0 || row + 1 >= self.rows.len() { return None; }
        self.command_values.get(row - 1).map(String::as_str)
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
                items.push(Item { value: format!("@{relative}"), description: "file".into() });
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
            commands: vec![Item { value: "/models".into(), description: "Select model".into() }, Item { value: "/mcp".into(), description: "MCP status".into() }],
            mentions: vec![Item { value: "@explore".into(), description: "agent".into() }, Item { value: "@src/café.rs".into(), description: "file".into() }],
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
        c.commands.push(Item { value: String::new(), description: String::new() });
        c.commands.push(Item { value: "é".into(), description: String::new() });
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
        c.replace_mentions(vec![Item {value:"@added".into(),description:String::new()},
            Item {value:"@extra".into(),description:String::new()},
            Item {value:"@src/café.rs".into(),description:"file".into()}]);
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
        assert_eq!((panel.x, panel.y, panel.width), (4, 0, 40));
        assert_eq!(panel.rows.len(), 3);
        assert!(panel.rows[1].0.contains("/mcp"));
        assert!(panel.rows[1].1);
        assert!(c.panel((0, 2, 40, 4)).is_none());
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
