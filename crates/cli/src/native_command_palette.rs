#![forbid(unsafe_code)]
//! Pure native command-palette model/rendering for the Chat modal.
//!
//! This module owns only query, stable row selection, bounded rendering, and
//! typed route intents. It performs no IO and never executes a route. The
//! caller must dispatch an intent through the existing native TUI path; in
//! particular `/sessions`, `/models`, `/connect`, and `/new` keep their real
//! fetch/setup/session-create semantics and acknowledgements.

use crate::native_view_layout::fit;

pub const MAX_QUERY_CHARS: usize = 256;
const MAX_ENTRIES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteAction {
    /// Hand the exact existing local command to the native command dispatcher.
    /// This is an intent, not an execution or acknowledgement.
    Route(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaletteEntry {
    /// Unique display-row identity. Suggested duplicates have their own id.
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    /// Exact pinned upstream registration name (not a locally invented
    /// alias); row id is separate because Suggested duplicates an entry.
    pub command_name: &'static str,
    pub route: &'static str,
    /// None means the native client has no truthful direct shortcut for it.
    pub key_label: Option<&'static str>,
    pub action: PaletteAction,
}

#[derive(Clone, Debug, Default)]
pub struct CommandPalette {
    open: bool,
    has_session: bool,
    query: String,
    selected_id: Option<&'static str>,
}

impl CommandPalette {
    pub fn open(&mut self, has_session: bool) {
        self.open = true;
        self.has_session = has_session;
        self.query.clear();
        self.selected_id = self.visible().first().map(|entry| entry.id);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected_id = None;
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Search is bounded by scalar count and only touches this component's
    /// query, never the composer, credential buffers, or PageState.
    pub fn push_query(&mut self, ch: char) -> bool {
        if !self.open || ch.is_control() || self.query.chars().count() >= MAX_QUERY_CHARS {
            return false;
        }
        self.query.push(ch);
        self.retain_or_select_first(None);
        true
    }

    pub fn backspace(&mut self) -> bool {
        if !self.open || self.query.pop().is_none() {
            return false;
        }
        self.retain_or_select_first(None);
        true
    }

    #[must_use]
    pub fn visible(&self) -> Vec<PaletteEntry> {
        let mut regular = Vec::with_capacity(MAX_ENTRIES);
        // Retain source categories and exact source command names/targets.
        regular.push(entry(
            "agent:model.list",
            "model.list",
            "Switch model",
            "Agent",
            "/models",
        ));
        if self.has_session {
            regular.push(entry(
                "session:session.list",
                "session.list",
                "Switch session",
                "Session",
                "/sessions",
            ));
            regular.push(entry(
                "session:session.new",
                "session.new",
                "New session",
                "Session",
                "/new",
            ));
        }
        regular.push(entry(
            "provider:provider.connect",
            "provider.connect",
            "Connect provider",
            "Provider",
            "/connect",
        ));
        regular.push(entry(
            "system:opencode.status",
            "opencode.status",
            "View status",
            "System",
            "/status",
        ));
        regular.push(entry(
            "system:theme.switch",
            "theme.switch",
            "Switch theme",
            "System",
            "/theme",
        ));
        regular.push(entry(
            "system:appearance.edit",
            "appearance.edit",
            "Box appearance",
            "System",
            "/appearance",
        ));
        regular.push(entry(
            "system:help.show",
            "help.show",
            "Help",
            "System",
            "/help",
        ));
        regular.push(entry(
            "system:app.exit",
            "app.exit",
            "Exit the app",
            "System",
            "/exit",
        ));
        let needle = self.query.to_lowercase();
        let mut entries = Vec::with_capacity(MAX_ENTRIES);
        if needle.is_empty() {
            // Pinned upstream `dialog-select.tsx` duplicates suggested
            // entries only before filtering; query results use their real
            // category and each command remains a single selectable entry.
            entries.push(entry(
                "suggested:model.list",
                "model.list",
                "Switch model",
                "Suggested",
                "/models",
            ));
            if self.has_session {
                entries.push(entry(
                    "suggested:session.list",
                    "session.list",
                    "Switch session",
                    "Suggested",
                    "/sessions",
                ));
                entries.push(entry(
                    "suggested:session.new",
                    "session.new",
                    "New session",
                    "Suggested",
                    "/new",
                ));
            }
        }
        entries.extend(regular);
        if needle.is_empty() {
            return entries.into_iter().take(MAX_ENTRIES).collect();
        }
        let mut filtered: Vec<_> = entries
            .into_iter()
            .filter_map(|item| {
                let title = fuzzy_score(&needle, item.title);
                let category = fuzzy_score(&needle, item.category);
                if title.is_none() && category.is_none() {
                    return None;
                }
                Some((title.unwrap_or(0) * 2 + category.unwrap_or(0), item))
            })
            .collect();
        filtered.sort_by(|a, b| b.0.cmp(&a.0));
        filtered
            .into_iter()
            .take(MAX_ENTRIES)
            .map(|(_, item)| item)
            .collect()
    }

    pub fn next(&mut self) {
        self.step(true);
    }

    pub fn prev(&mut self) {
        self.step(false);
    }

    #[must_use]
    pub fn selected(&self) -> Option<PaletteEntry> {
        let visible = self.visible();
        let selected = self.selected_id?;
        visible.into_iter().find(|item| item.id == selected)
    }

    /// Returns a typed intent; the event loop must route it through the same
    /// authenticated/session-aware command path as keyboard-entered commands.
    pub fn activate(&self) -> Option<PaletteAction> {
        self.selected().map(|item| item.action)
    }

    /// Content rows suitable for the existing 60-cell, viewport-centered
    /// modal painter. Width/height are clipped safely; the footer documents
    /// only bindings this native component actually handles.
    #[must_use]
    pub fn render(&self, width: u16, height: usize) -> Vec<String> {
        if !self.open || width == 0 || height == 0 {
            return Vec::new();
        }
        let mut lines = vec!["Commands".to_owned(), format!("Search: {}", self.query)];
        let visible = self.visible();
        let mut body = Vec::new();
        let mut selected_row = 0;
        let mut last_category = None;
        let selected = self.selected_id;
        for item in visible {
            if last_category != Some(item.category) {
                body.push(item.category.to_owned());
                last_category = Some(item.category);
            }
            let marker = if selected == Some(item.id) {
                "> "
            } else {
                "  "
            };
            if selected == Some(item.id) {
                selected_row = body.len();
            }
            let key = item
                .key_label
                .map_or(String::new(), |label| format!("  {label}"));
            body.push(format!("{marker}{}{key}", item.title));
        }
        if self.visible().is_empty() {
            body.push("No results found".to_owned());
        }
        let capacity = height.saturating_sub(3);
        let start = selected_row.saturating_sub(capacity.saturating_sub(1));
        lines.extend(body.into_iter().skip(start).take(capacity));
        lines.push("↑/↓ move · Enter select · Esc close".to_owned());
        lines
            .into_iter()
            .take(height)
            .map(|line| fit(&line, width))
            .collect()
    }

    fn step(&mut self, down: bool) {
        if !self.open {
            return;
        }
        let visible = self.visible();
        if visible.is_empty() {
            self.selected_id = None;
            return;
        }
        let current = self
            .selected_id
            .and_then(|id| visible.iter().position(|item| item.id == id));
        let index = match (current, down) {
            (Some(index), true) => (index + 1) % visible.len(),
            (Some(index), false) => (index + visible.len() - 1) % visible.len(),
            (None, _) => 0,
        };
        self.selected_id = Some(visible[index].id);
    }

    fn retain_or_select_first(&mut self, previous: Option<&'static str>) {
        let visible = self.visible();
        self.selected_id = previous
            .filter(|id| visible.iter().any(|item| item.id == *id))
            .or_else(|| visible.first().map(|item| item.id));
    }
}

fn fuzzy_score(needle: &str, value: &str) -> Option<usize> {
    let haystack: Vec<_> = value.to_lowercase().chars().collect();
    let query: Vec<_> = needle.chars().collect();
    let first = *query.first()?;
    haystack
        .iter()
        .enumerate()
        .filter(|(_, ch)| **ch == first)
        .filter_map(|(start, _)| {
            let mut previous = start;
            let mut score = 4 + if start == 0 || !haystack[start - 1].is_alphanumeric() {
                8
            } else {
                0
            };
            for &character in &query[1..] {
                let cursor = previous + 1;
                let offset = haystack
                    .get(cursor..)?
                    .iter()
                    .position(|&item| item == character)?;
                let position = cursor + offset;
                score += if position == cursor { 16 } else { 4 };
                if !haystack[position - 1].is_alphanumeric() {
                    score += 8;
                }
                previous = position;
            }
            Some(score)
        })
        .max()
}

fn entry(
    id: &'static str,
    command_name: &'static str,
    title: &'static str,
    category: &'static str,
    route: &'static str,
) -> PaletteEntry {
    PaletteEntry {
        id,
        title,
        category,
        command_name,
        route,
        // Native currently implements no direct model/session key bindings
        // matching the pinned upstream leader bindings; do not invent them.
        key_label: None,
        action: PaletteAction::Route(route),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_source_commands_and_preserves_source_identity() {
        let mut palette = CommandPalette::default();
        palette.open(true);
        let entries = palette.visible();
        assert_eq!(entries[0].category, "Suggested");
        assert_eq!(entries[0].command_name, "model.list");
        assert_eq!(entries[1].command_name, "session.list");
        assert_eq!(entries[2].command_name, "session.new");
        assert!(entries.iter().any(|entry| entry.category == "Agent"));
        assert!(entries.iter().any(|entry| entry.category == "Session"));
        assert_eq!(palette.activate(), Some(PaletteAction::Route("/models")));
    }

    #[test]
    fn query_selection_navigation_and_close_are_modal_local() {
        let mut palette = CommandPalette::default();
        palette.open(true);
        palette.next();
        palette.push_query('s');
        palette.push_query('e');
        palette.push_query('s');
        palette.push_query('s');
        palette.push_query('i');
        palette.push_query('o');
        palette.push_query('n');
        // Independently superseded against pinned DialogSelect: filtering
        // removes Suggested duplicates and resets to the first regular row.
        let visible = palette.visible();
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|entry| entry.category == "Session"));
        assert!(
            visible
                .iter()
                .all(|entry| !entry.id.starts_with("suggested:"))
        );
        assert_eq!(palette.selected().unwrap().id, "session:session.list");
        assert_eq!(palette.activate(), Some(PaletteAction::Route("/sessions")));
        palette.prev();
        assert_eq!(palette.selected().unwrap().id, "session:session.new");
        palette.next();
        assert_eq!(palette.activate(), Some(PaletteAction::Route("/sessions")));
        palette.close();
        assert!(!palette.is_open());
        assert!(palette.query().is_empty());
        assert_eq!(palette.activate(), None);
    }

    #[test]
    fn empty_results_and_unicode_query_are_safe_and_bounded() {
        let mut palette = CommandPalette::default();
        palette.open(false);
        assert!(palette.push_query('🦀'));
        assert!(palette.visible().is_empty());
        assert_eq!(palette.activate(), None);
        assert_eq!(palette.render(40, 5).len(), 4);
        for _ in 0..MAX_QUERY_CHARS {
            palette.push_query('x');
        }
        assert!(!palette.push_query('y'));
        assert_eq!(palette.query().chars().count(), MAX_QUERY_CHARS);
    }
}
