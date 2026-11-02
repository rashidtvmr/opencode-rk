#![forbid(unsafe_code)]
//! Pure native command-palette model/rendering for the Chat modal.
//!
//! This module owns only query, stable row selection, bounded rendering, and
//! typed route intents. It performs no IO and never executes a route. The
//! caller must dispatch an intent through the existing native TUI path; in
//! particular `/sessions`, `/models`, `/connect`, and `/new` keep their real
//! fetch/setup/session-create semantics and acknowledgements.
//!
//! DIALOG-11 evidence (pinned upstream
//! `450e7b92412c780a7b5a75226bb8d49aaca24e22`,
//! `packages/tui/src/component/command-palette.tsx:7-74`): suggested rows are
//! duplicated only while the filter is empty, filtering uses
//! `title*2 + category + searchText`, the threshold is `0.7`
//! (`command-palette.tsx:71`), and footer hints document only bindings this
//! native component actually handles — no shortcut label is invented when the
//! native client has no matching binding.


use crate::native_view_layout::fit;
use crate::native_view_layout::visit_unpadded_cells;

pub const MAX_QUERY_CHARS: usize = 256;
const MAX_ENTRIES: usize = 16;
/// `filterThreshold={0.7}` on the pinned palette
/// (`command-palette.tsx:71`). The threshold is applied to the same weighted
/// score as `DialogSelect` (title ×2, category + searchText ×1).
pub const FILTER_THRESHOLD: f32 = 0.7;
/// Body rows reserve the shared two-cell right inset (`DIALOG_LIST_INSET - 1`,
/// `dialog-select.tsx:938-941`), which is also where the right-aligned,
/// flattened footer lands.
const ROW_RIGHT_INSET: u16 = 2;

#[cfg(test)]
mod filtered_command_regressions {
    use super::*;

    fn search(query: &str) -> CommandPalette {
        let mut palette = CommandPalette::default();
        palette.open(true);
        for ch in query.chars() {
            assert!(palette.push_query(ch));
        }
        palette
    }

    #[test]
    fn uppercase_query_has_the_same_command_identities() {
        let identities = |palette: CommandPalette| palette.visible().into_iter()
            .map(|row| (row.command_name, row.route)).collect::<Vec<_>>();
        let lower = identities(search("session"));
        assert!(!lower.is_empty());
        assert_eq!(identities(search("SESSION")), lower);
    }

    #[test]
    fn command_id_search_activates_the_registered_route() {
        let palette = search("opencode.status");
        let selected = palette.selected().expect("registered status command");
        assert_eq!(selected.command_name, "opencode.status");
        assert_eq!(selected.category, "System");
        assert_eq!(palette.activate(), Some(PaletteAction::Route("/status")));
    }

    #[test]
    fn filtered_commands_are_unique_and_navigation_keeps_route_identity() {
        assert!(search("").visible().iter().any(|row| row.category == "Suggested"));
        let mut palette = search("session");
        let rows = palette.visible();
        assert!(rows.len() >= 2);
        let mut ids = std::collections::HashSet::new();
        let mut routes = std::collections::HashSet::new();
        for row in &rows {
            assert_ne!(row.category, "Suggested");
            assert!(ids.insert(row.command_name));
            assert!(routes.insert(row.route));
        }
        for expected in rows.iter().cycle().take(rows.len() + 1) {
            assert_eq!(palette.selected().map(|row| row.id), Some(expected.id));
            assert_eq!(palette.activate(), Some(expected.action));
            palette.next();
        }
        assert!(palette.push_query('.'));
        let selected = palette.selected().expect("session command ID remains searchable");
        assert_eq!(palette.activate(), Some(PaletteAction::Route(selected.route)));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteAction {
    /// Hand the exact existing local command to the native command dispatcher.
    /// This is an intent, not an execution or acknowledgement.
    Route(&'static str),
    /// Focus a live native settings row without applying a change.
    Settings(&'static str),
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
    /// Upstream `searchText`: supplemental searchable text such as setting keywords.
    pub search_text: &'static str,
    pub route: &'static str,
    /// None means the native client has no truthful direct shortcut for it.
    pub key_label: Option<&'static str>,
    /// `searchFooter` (`command-palette.tsx:26,41`): `"<group> · <keys>"` for
    /// command rows, `"Settings · <category>"` for setting rows.
    pub search_footer: Option<&'static str>,
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
                "agent:subagents.list",
                "subagents.list",
                "View subagents",
                "Agent",
                "/subagents",
            ));
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
            "system:opencode.settings",
            "opencode.settings",
            "Open settings",
            "System",
            "/settings",
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
        if !needle.is_empty() {
            // Search only settings the native settings dialog can edit.
            regular.extend(crate::native_settings::SettingsDialog::rows().iter().map(setting_entry));
        }
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
            entries.push(entry(
                "suggested:opencode.settings",
                "opencode.settings",
                "Open settings",
                "Suggested",
                "/settings",
            ));
        }
        entries.extend(regular.clone());
        if needle.is_empty() {
            return entries.into_iter().take(MAX_ENTRIES).collect();
        }
        // V2 `list()` duplicates suggested rows only while the filter is empty;
        // filtered search uses the real command options once, avoiding duplicate
        // selectable values (command-palette.tsx:48-60).
        let mut filtered: Vec<_> = regular
            .into_iter()
            .filter_map(|item| {
                let quality = search_quality(&needle, &item)?;
                if quality < FILTER_THRESHOLD {
                    return None;
                }
                let title = fuzzy_score(&needle, item.title);
                let category = fuzzy_score(&needle, item.category);
                let search = fuzzy_score(&needle, item.search_text);
                Some((title.unwrap_or(0) * 2 + category.unwrap_or(0) + search.unwrap_or(0), item))
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
        self.render_styled(width, height).into_iter().map(|line| line.text).collect()
    }

    pub fn render_styled(&self, width: u16, height: usize) -> Vec<crate::native_ui_style::Line> {
        use crate::native_ui_style::{Line, Role};
        if !self.open || width == 0 || height == 0 {
            return Vec::new();
        }
        let roomy = height >= 9;
        let mut lines = vec![("Commands".to_owned(), Role::Heading, 2u16, 2u16)];
        if roomy { lines.push((String::new(), Role::Description, 0, 0)); }
        lines.push((format!("Search: {}", self.query), Role::Description, 2, 2));
        if roomy { lines.push((String::new(), Role::Description, 0, 0)); }
        let visible = self.visible();
        let mut body = Vec::new();
        let mut selected_row = 0;
        let mut last_category = None;
        let selected = self.selected_id;
        // `flatten` (`dialog-select.tsx:233`) for this palette: a non-empty
        // query drops the category groups, so no heading is painted and the
        // row footer becomes `searchFooter ?? category ?? footer`.
        let flattened = !self.query.is_empty();
        for item in visible {
            if !flattened && last_category != Some(item.category) {
                if roomy && last_category.is_some() {
                    body.push((String::new(), Role::Description, 0, 0));
                }
                body.push((item.category.to_owned(), Role::Heading, 2, 0));
                last_category = Some(item.category);
            }
            let marker = if selected == Some(item.id) {
                ">    "
            } else {
                "     "
            };
            if selected == Some(item.id) {
                selected_row = body.len();
            }
            // Upstream row (`dialog-select.tsx:795-820`): the title flex-grows,
            // an optional `" " + description` span follows it, and the footer is
            // the last right-aligned child; while flattened the footer is
            // `searchFooter ?? category ?? footer`.
            let key = item
                .key_label
                .map_or(String::new(), |label| format!("  {label}"));
            let mut row = format!("{marker}{}{key}", item.title);
            if flattened {
                let footer = item.search_footer.unwrap_or(item.category).trim();
                // Body rows reserve the shared two-cell right inset
                // (`DIALOG_LIST_INSET - 1`), so the footer right-aligns inside
                // the content width, not the terminal width.
                let inner = usize::from(width).saturating_sub(usize::from(ROW_RIGHT_INSET));
                let title_cells = visit_unpadded_cells(&row, inner, |_, _| {});
                let footer_cells = visit_unpadded_cells(footer, inner, |_, _| {});
                if !footer.is_empty() && title_cells + footer_cells < inner {
                    let gap = inner - title_cells - footer_cells;
                    row.push_str(&" ".repeat(gap));
                    row.push_str(footer);
                }
            }
            body.push((
                row,
                if selected == Some(item.id) { Role::Selected } else { Role::Item },
                0,
                ROW_RIGHT_INSET,
            ));
        }
        if self.visible().is_empty() {
            body.push(("No results found".to_owned(), Role::Description, 2, 2));
        }
        let capacity = height.saturating_sub(lines.len() + 1 + usize::from(roomy));
        let start = selected_row.saturating_sub(capacity.saturating_sub(1));
        lines.extend(body.into_iter().skip(start).take(capacity));
        if roomy { lines.push((String::new(), Role::Description, 0, 0)); }
        lines.push(("↑/↓ move · Enter select · Esc close".to_owned(), Role::Hint, 2, 0));
        lines
            .into_iter()
            .take(height)
            .map(|(text, role, left, right)| {
                let left = left.min(width);
                let right = right.min(width.saturating_sub(left));
                let inner = width.saturating_sub(left + right);
                let content = fit(&text, inner);
                let text = fit(
                    &format!(
                        "{}{}{}",
                        " ".repeat(usize::from(left)),
                        content,
                        " ".repeat(usize::from(right)),
                    ),
                    width,
                );
                Line { text, role, detail_from: None }
            })
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

/// Weighted fuzzy quality used for the 0.7 palette threshold. Title weighs
/// twice, category and the command id (`searchText`) once each, mirroring
/// `DialogSelect`'s `scoreFn` (`dialog-select.tsx:220-227`). `None` means no
/// field matched at all.
#[must_use]
pub fn search_quality(query: &str, item: &PaletteEntry) -> Option<f32> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Some(1.0);
    }
    let query: Vec<char> = needle.chars().collect();
    let title = character_quality(&query, item.title);
    let category = character_quality(&query, item.category);
    let search = character_quality(&query, item.search_text);
    if title.is_none() && category.is_none() && search.is_none() {
        return None;
    }
    Some(title.unwrap_or(0.0) * 2.0 + category.unwrap_or(0.0) + search.unwrap_or(0.0))
}

/// Greedy case-insensitive subsequence quality in `0.0..=1.0`; adjacent and
/// word-boundary characters score higher.
fn character_quality(query: &[char], value: &str) -> Option<f32> {
    if query.is_empty() {
        return Some(1.0);
    }
    let haystack: Vec<char> = value.to_lowercase().chars().collect();
    let mut cursor = 0usize;
    let mut previous: Option<usize> = None;
    let mut score = 0.0f32;
    for &needle in query {
        let position = haystack.get(cursor..)?.iter().position(|c| *c == needle)? + cursor;
        score += 1.0;
        if position == 0 || !haystack[position - 1].is_alphanumeric() {
            score += 0.5;
        }
        if previous.is_some_and(|index| index + 1 == position) {
            score += 0.5;
        }
        previous = Some(position);
        cursor = position + 1;
    }
    Some((score / (query.len() as f32 * 2.0)).clamp(0.0, 1.0))
}

fn fuzzy_score(needle: &str, value: &str) -> Option<usize> {    let haystack: Vec<_> = value.to_lowercase().chars().collect();
    let query: Vec<_> = needle.to_lowercase().chars().collect();
    let first = *query.first()?;
    haystack
        .iter()
        .enumerate()
        .filter(|(_, ch)| **ch == first)
        .filter_map(|(start, _)| {
            let mut previous = start;
            let mut score = 4 + usize::from(start == 0 || !haystack[start - 1].is_alphanumeric()) * 8;
            for &character in &query[1..] {
                let cursor = previous + 1;
                let offset = haystack.get(cursor..)?.iter().position(|&item| item == character)?;
                let position = cursor + offset;
                score += if position == cursor { 16 } else { 4 };
                if !haystack[position - 1].is_alphanumeric() { score += 8; }
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
        search_text: command_name,
        route,
        // Native currently implements no direct model/session key bindings
        // matching pinned upstream leader bindings; do not invent labels.
        key_label: None,
        // Upstream `searchFooter = [group, footer].join(" · ")`; with no
        // native shortcut labels that reduces to the group, so the row render
        // falls back to the category (`searchFooter ?? category`).
        search_footer: None,
        action: PaletteAction::Route(route),
    }
}

fn setting_entry(row: &crate::native_settings::SettingsRow) -> PaletteEntry {
    let search_text = match row.id {
        "session.sidebar" => "side panel",
        "tabs.layout" => "orientation left layout tabs",
        _ => "",
    };
    PaletteEntry {
        id: row.id,
        title: row.title,
        category: row.category,
        command_name: row.id,
        search_text,
        route: "/settings",
        key_label: None,
        search_footer: None,
        action: PaletteAction::Settings(row.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_palette::select::SelectOption;

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
        // The live `session.sidebar` setting also matches on its `Session`
        // category (`command-palette.tsx:36-46`), so the filtered set is the
        // three Session-categoried rows.
        let visible = palette.visible();
        assert_eq!(visible.len(), 3);
        assert!(visible.iter().all(|entry| entry.category == "Session"));
        assert!(
            visible
                .iter()
                .all(|entry| !entry.id.starts_with("suggested:"))
        );
        assert!(visible.iter().any(|entry| entry.id == "session.sidebar"));
        assert_eq!(palette.selected().unwrap().id, "session:session.list");
        assert_eq!(palette.activate(), Some(PaletteAction::Route("/sessions")));
        // The two Session commands outrank the setting row (which matches only
        // on its category), so `prev` wraps past them to the appended setting.
        palette.prev();
        assert_eq!(palette.selected().unwrap().id, "session.sidebar");
        assert_eq!(palette.activate(), Some(PaletteAction::Settings("session.sidebar")));
        palette.next();
        assert_eq!(palette.selected().unwrap().id, "session:session.list");
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

    #[test]
    fn styled_rows_use_insets_and_keep_selected_rows_bounded() {        let mut palette = CommandPalette::default();
        palette.open(true);

        let rows = palette.render_styled(12, 30);
        assert!(rows.iter().all(|line| line.text.chars().count() <= 12));
        assert_eq!(rows[0].text, "  Commands  ");
        assert!(rows
            .iter()
            .any(|line| line.role == crate::native_ui_style::Role::Selected));

        for _ in 0..12 {
            palette.next();
        }
        let clipped = palette.render_styled(7, 7);
        assert!(clipped.len() <= 7);
        assert!(clipped.iter().all(|line| line.text.chars().count() <= 7));
        assert!(clipped
            .iter()
            .any(|line| line.role == crate::native_ui_style::Role::Selected));

        let narrow = palette.render_styled(1, 7);
        assert!(narrow.iter().all(|line| line.text.chars().count() <= 1));
    }

    #[test]
    fn settings_search_uses_live_rows_in_source_order_without_fake_shortcuts() {
        let mut palette = CommandPalette::default();
        palette.open(true);
        // Unfiltered: upstream omits `settingOptions` from the non-filtered
        // list (`command-palette.tsx:36-46`, `ref?.filter` gate).
        assert!(palette.visible().iter().all(|row| !matches!(row.action, PaletteAction::Settings(_))));

        // "layout" surfaces the live Tabs row by its own title.
        for ch in "layout".chars() { assert!(palette.push_query(ch)); }
        let layout: Vec<_> = palette.visible().into_iter()
            .filter(|row| matches!(row.action, PaletteAction::Settings(_)))
            .collect();
        assert_eq!(layout.iter().map(|row| row.id).collect::<Vec<_>>(), ["tabs.layout"]);
        assert!(layout[0].key_label.is_none());

        // "side panel" reaches the Session row only through its upstream
        // `searchText` (`setting.keywords.join(" ")`, `command-palette.tsx:38-39`);
        // neither its title nor category contains that phrase.
        let mut palette = CommandPalette::default();
        palette.open(true);
        for ch in "side panel".chars() { assert!(palette.push_query(ch)); }
        let side: Vec<_> = palette.visible().into_iter()
            .filter(|row| matches!(row.action, PaletteAction::Settings(_)))
            .collect();
        assert_eq!(side.iter().map(|row| row.id).collect::<Vec<_>>(), ["session.sidebar"]);
        assert_eq!(side[0].action, PaletteAction::Settings("session.sidebar"));
        // No shortcut hint is invented for a setting the native client cannot
        // trigger directly (`dialog-select.tsx:193-208`).
        assert!(side[0].key_label.is_none() && side[0].search_footer.is_none());
    }

    // DIALOG-11: suggestions are duplicated only before filtering, the 0.7
    // threshold gates weak matches, and no shortcut hint is invented for a
    // command the native client cannot trigger directly
    // (command-palette.tsx:48-71).
    #[test]
    fn filter_threshold_and_hint_coherence_match() {
        assert_eq!(FILTER_THRESHOLD, 0.7);
        let weak = PaletteEntry {
            id: "weak",
            title: "Zoo",
            category: "Zoo",
            command_name: "mode",
            search_text: "mode",
            route: "/weak",
            key_label: None,
            search_footer: None,
            action: PaletteAction::Route("/weak"),
        };
        assert!(search_quality("e", &weak).expect("searchText matches") < FILTER_THRESHOLD);
        let strong = PaletteEntry { title: "Echo", ..weak };
        assert!(search_quality("e", &strong).expect("title matches") >= FILTER_THRESHOLD);

        let mut palette = CommandPalette::default();
        palette.open(true);
        let all = palette.visible();
        let suggested: Vec<_> = all
            .iter()
            .filter(|row| row.id.starts_with("suggested:"))
            .collect();
        assert!(!suggested.is_empty());
        assert!(suggested.iter().all(|row| all
            .iter()
            .any(|regular| !regular.id.starts_with("suggested:")
                && regular.command_name == row.command_name)));
        palette.push_query('s');
        let filtered = palette.visible();
        assert!(!filtered.is_empty());
        assert!(filtered
            .iter()
            .all(|row| !row.id.starts_with("suggested:")));
        assert!(filtered
            .iter()
            .all(|row| search_quality("s", row).is_some_and(|q| q >= FILTER_THRESHOLD)));
        assert!(filtered.iter().all(|row| row.key_label.is_none()));
        let rendered = palette.render(80, 20);
        assert_eq!(
            rendered.last().map(|line| line.trim().to_owned()),
            Some("↑/↓ move · Enter select · Esc close".to_owned())
        );
    }

    // DIALOG-11 / `dialog-select.tsx:807`: while the list is flattened (a
    // search is active) the row's right-aligned footer is
    // `searchFooter ?? category ?? footer`; a grouped list shows no footer and
    // keeps its category heading.
    #[test]
    fn flattened_rows_paint_search_footer_and_drop_category_headings() {
        let mut palette = CommandPalette::default();
        palette.open(true);

        // Grouped (empty query): category headings, no right-aligned footer.
        let grouped = palette.render(40, 20);
        assert!(grouped.iter().any(|line| line.trim() == "Agent"));
        assert!(
            grouped.iter().any(|line| line.trim() == "Switch model"),
            "grouped row keeps its bare title: {grouped:?}"
        );
        assert!(
            !grouped.iter().any(|line| line.contains("Switch model  Agent")),
            "grouped rows carry no footer: {grouped:?}"
        );

        // Flattened (`session` query): no headings, footer = category, and the
        // footer overrides the category when the entry supplies one.
        for ch in "session".chars() {
            assert!(palette.push_query(ch));
        }
        let flattened = palette.render(40, 20);
        assert!(
            !flattened.iter().any(|line| line.trim() == "Session"),
            "flattened rows paint no category heading: {flattened:?}"
        );
        let row = flattened
            .iter()
            .find(|line| line.contains("Switch session"))
            .expect("session row");
        // Body rows reserve the shared two-cell right inset, so the footer
        // right-aligns at `width - 2`.
        assert_eq!(row.trim_end().len(), 38, "footer is right-aligned: {row:?}");
        assert_eq!(row.len(), 40, "row fills the content width: {row:?}");
        assert!(
            row.trim_end().ends_with("Session"),
            "flattened footer falls back to the category: {row:?}"
        );
        assert!(row.contains("Switch session  "), "title then gap then footer: {row:?}");

        // An explicit `searchFooter` wins over the category, exactly as the
        // option-level `searchFooter ?? category` chain.
        let option = crate::native_palette::select::SelectOption::new("s", "Switch session")
            .with_category("Session")
            .with_footer("ctrl+x s")
            .with_search_footer("Session · ctrl+x s");
        assert_eq!(option.row_footer(true), Some("Session · ctrl+x s"));
        assert_eq!(option.row_footer(false), Some("ctrl+x s"));
        assert_eq!(
            SelectOption::new("s", "T").with_category("Group").row_footer(true),
            Some("Group")
        );
        assert_eq!(SelectOption::new("s", "T").with_footer("f").row_footer(true), Some("f"));
        assert_eq!(SelectOption::new("s", "T").row_footer(true), None);
    }
}
