#![forbid(unsafe_code)]
//! Native settings surface. Only settings with a live producer/consumer are shown.

use crate::native_navigation::TabOrientation;

pub const MAX_ROWS: usize = 2;
pub const MAX_QUERY_CHARS: usize = 128;
const HEADER_ROWS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsAction {
    ApplySidebar(bool),
    ApplyTabs(TabOrientation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsRow {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
}

#[derive(Clone, Debug)]
pub struct SettingsDialog {
    open: bool,
    selected: usize,
    viewport_start: usize,
    sidebar_hidden: bool,
    tabs: TabOrientation,
    query: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum VisibleLine {
    Category(String),
    Separator,
    Setting { position: usize, index: usize, text: String },
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            selected: 0,
            viewport_start: 0,
            sidebar_hidden: false,
            tabs: TabOrientation::Horizontal,
            query: String::new(),
        }
    }
}

impl SettingsDialog {
    pub fn open(&mut self, tabs: TabOrientation) {
        self.open_with_state(false, tabs);
    }

    pub fn open_with_state(&mut self, sidebar_hidden: bool, tabs: TabOrientation) {
        self.open = true;
        self.selected = 0;
        self.viewport_start = 0;
        self.sidebar_hidden = sidebar_hidden;
        self.tabs = tabs;
        self.query.clear();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
    }

    pub const fn is_open(&self) -> bool {
        self.open
    }

    pub const fn current(&self) -> TabOrientation {
        self.tabs
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub const fn rows() -> [SettingsRow; MAX_ROWS] {
        [
            SettingsRow {
                id: "session.sidebar",
                category: "Session",
                title: "Sidebar",
                description: "Whether the sidebar shows when there is room",
                keywords: &["side panel"],
            },
            SettingsRow {
                id: "tabs.layout",
                category: "Tabs",
                title: "Layout",
                description: "Whether tabs run along the top or down the side",
                keywords: &["orientation", "left", "layout", "tabs"],
            },
        ]
    }

    /// Returns source-ordered options when empty and upstream-style fuzzy-ranked
    /// results when searching. Search fields match DialogConfig: title,
    /// category and supplemental keywords, not descriptions or internal IDs.
    fn visible_indices(&self) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        let rows = Self::rows();
        if query.is_empty() {
            return (0..rows.len()).collect();
        }
        let terms: Vec<_> = query.split_whitespace().collect();
        let mut scored = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let mut score = 0usize;
            let mut matches = true;
            for term in &terms {
                let title = fuzzy_score(term, row.title).map(|value| value * 2);
                let category = fuzzy_score(term, row.category);
                let keywords = row
                    .keywords
                    .iter()
                    .filter_map(|keyword| fuzzy_score(term, keyword))
                    .max();
                let Some(best) = title.into_iter().chain(category).chain(keywords).max() else {
                    matches = false;
                    break;
                };
                score = score.saturating_add(best);
            }
            if matches {
                scored.push((index, score));
            }
        }
        scored.sort_by(|left, right| right.1.cmp(&left.1));
        scored.into_iter().map(|(index, _)| index).collect()
    }

    fn body_lines(&self, visible: &[usize]) -> Vec<VisibleLine> {
        let rows = Self::rows();
        let searching = !self.query.trim().is_empty();
        let mut lines = Vec::with_capacity(visible.len().saturating_mul(2));
        let mut previous_category = None;
        for (position, &index) in visible.iter().enumerate() {
            let row = rows[index];
            if !searching && previous_category != Some(row.category) {
                if previous_category.is_some() {
                    lines.push(VisibleLine::Separator);
                }
                lines.push(VisibleLine::Category(row.category.to_owned()));
                previous_category = Some(row.category);
            }
            let selected = if position == self.selected { ">" } else { " " };
            let current = if index == 0 { "●" } else { " " };
            let value = match index {
                0 if self.sidebar_hidden => "hide",
                0 => "auto",
                _ if self.tabs == TabOrientation::Horizontal => "horizontal",
                _ => "vertical",
            };
            lines.push(VisibleLine::Setting {
                position,
                index,
                text: format!("{selected} {current} {:<12} {value}", row.title),
            });
        }
        lines
    }

    fn viewport_start_for(&self, height: usize, lines: &[VisibleLine]) -> usize {
        let capacity = height.saturating_sub(HEADER_ROWS + 1);
        if capacity == 0 {
            return 0;
        }
        let selected_line = lines.iter().position(|line| {
            matches!(line, VisibleLine::Setting { position, .. } if *position == self.selected)
        });
        selected_line
            .map(|line| {
                let start = if line < self.viewport_start {
                    line
                } else if line >= self.viewport_start.saturating_add(capacity) {
                    line.saturating_add(1).saturating_sub(capacity)
                } else {
                    self.viewport_start
                };
                start.min(lines.len().saturating_sub(capacity))
            })
            .unwrap_or(self.viewport_start.min(lines.len().saturating_sub(capacity)))
    }

    fn reveal_selection(&mut self, height: usize, lines: &[VisibleLine]) {
        self.viewport_start = self.viewport_start_for(height, lines);
    }

    pub fn has_visible_row(&self) -> bool {
        !self.visible_indices().is_empty()
    }

    pub fn push_query(&mut self, ch: char) -> bool {
        if !self.open || ch.is_control() || self.query.chars().count() >= MAX_QUERY_CHARS {
            return false;
        }
        self.query.push(ch);
        self.selected = 0;
        self.viewport_start = 0;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if !self.open {
            return false;
        }
        let changed = self.query.pop().is_some();
        if changed {
            self.selected = 0;
            self.viewport_start = 0;
        }
        changed
    }

    pub fn select(&mut self, index: usize) -> bool {
        let visible = self.visible_indices();
        if !self.open || index >= visible.len() {
            return false;
        }
        self.selected = index;
        true
    }

    pub fn next(&mut self) {
        let count = self.visible_indices().len();
        if self.open && count > 0 {
            self.selected = (self.selected + 1) % count;
        }
    }

    pub fn previous(&mut self) {
        let count = self.visible_indices().len();
        if self.open && count > 0 {
            self.selected = (self.selected + count - 1) % count;
        }
    }

    pub fn click(&mut self, id: &str) -> Option<SettingsAction> {
        if !self.open {
            return None;
        }
        let index = Self::rows().iter().position(|row| row.id == id)?;
        let visible = self.visible_indices();
        let position = visible.iter().position(|item| *item == index)?;
        self.selected = position;
        self.cycle(1)
    }

    pub fn left(&mut self) -> Option<SettingsAction> {
        self.cycle(-1)
    }

    pub fn right(&mut self) -> Option<SettingsAction> {
        self.cycle(1)
    }

    fn cycle(&self, _direction: i8) -> Option<SettingsAction> {
        if !self.open {
            return None;
        }
        let visible = self.visible_indices();
        match *visible.get(self.selected)? {
            0 => Some(SettingsAction::ApplySidebar(!self.sidebar_hidden)),
            1 => Some(SettingsAction::ApplyTabs(if self.tabs == TabOrientation::Horizontal {
                TabOrientation::Vertical
            } else {
                TabOrientation::Horizontal
            })),
            _ => None,
        }
    }

    pub fn applied_sidebar(&mut self, value: bool) {
        self.sidebar_hidden = value;
    }

    pub fn applied(&mut self, value: TabOrientation) {
        self.tabs = value;
    }

    pub fn activate(&self) -> Option<SettingsAction> {
        self.cycle(1)
    }

    pub fn hit_row(
        &self,
        x: u16,
        y: usize,
        width: u16,
        height: usize,
    ) -> Option<&'static str> {
        if !self.open || usize::from(x) >= usize::from(width) || y >= height {
            return None;
        }
        let visible = self.visible_indices();
        let lines = self.body_lines(&visible);
        let capacity = height.saturating_sub(HEADER_ROWS + 1);
        if capacity == 0 {
            return None;
        }
        let viewport_start = self.viewport_start_for(height, &lines);
        let line = y.checked_sub(HEADER_ROWS)?;
        if line >= capacity {
            return None;
        }
        match lines.get(viewport_start.saturating_add(line))? {
            VisibleLine::Setting { index, .. } => Some(Self::rows()[*index].id),
            VisibleLine::Category(_) | VisibleLine::Separator => None,
        }
    }

    pub fn hit_layout(&mut self, x: u16, y: usize, width: u16, height: usize) -> bool {
        self.hit_row(x, y, width, height) == Some("tabs.layout")
    }

    pub fn render(&mut self, width: u16, height: usize) -> Vec<String> {
        if !self.open || width == 0 || height == 0 {
            return Vec::new();
        }
        let visible = self.visible_indices();
        if visible.is_empty() {
            return vec![
                "Settings".to_owned(),
                format!("Search settings: {}", self.query),
                "No results found".to_owned(),
            ]
            .into_iter()
            .take(height)
            .map(|line| crate::native_view_layout::fit(&line, width))
            .collect();
        }

        let mut body = self.body_lines(&visible);
        self.reveal_selection(height, &body);
        body = self.body_lines(&visible);
        let mut out = vec![
            "Settings".to_owned(),
            format!("Search settings: {}", self.query),
            String::new(),
        ];
        out.extend(
            body.into_iter()
                .skip(self.viewport_start)
                .take(height.saturating_sub(HEADER_ROWS + 1))
                .map(|line| match line {
                    VisibleLine::Category(text) => text,
                    VisibleLine::Separator => String::new(),
                    VisibleLine::Setting { text, .. } => text,
                }),
        );
        if out.len() < height {
            out.push("change  ←/→".to_owned());
        }
        out.into_iter()
            .take(height)
            .map(|line| crate::native_view_layout::fit(&line, width))
            .collect()
    }
}

/// Small ranked subsequence matcher for the two live settings. It preserves
/// upstream title-first weighting without adding a fuzzy-search dependency.
fn fuzzy_score(query: &str, value: &str) -> Option<usize> {
    let needle: Vec<_> = query.chars().collect();
    let haystack: Vec<_> = value.to_lowercase().chars().collect();
    if needle.is_empty() {
        return Some(0);
    }
    let mut cursor = 0;
    let mut previous = None;
    let mut score = 0usize;
    for &ch in &needle {
        let position = haystack.get(cursor..)?.iter().position(|candidate| *candidate == ch)? + cursor;
        let adjacent = previous.is_some_and(|index| index + 1 == position);
        let boundary = position == 0 || !haystack[position - 1].is_alphanumeric();
        score = score.saturating_add(4 + usize::from(adjacent) * 8 + usize::from(boundary) * 6);
        previous = Some(position);
        cursor = position + 1;
    }
    Some(score.saturating_sub(haystack.len().saturating_sub(needle.len()) / 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> SettingsDialog {
        let mut dialog = SettingsDialog::default();
        dialog.open_with_state(false, TabOrientation::Horizontal);
        dialog
    }

    #[test]
    fn only_live_settings_are_exposed() {
        assert_eq!(SettingsDialog::rows().len(), 2);
        assert!(SettingsDialog::rows()
            .iter()
            .all(|row| ["session.sidebar", "tabs.layout"].contains(&row.id)));
    }

    #[test]
    fn settings_are_grouped_and_mark_current_and_active_rows() {
        let mut dialog = open();
        let frame = dialog.render(48, 10);
        let session = frame.iter().position(|line| line.trim_end() == "Session").unwrap();
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        let tabs = frame.iter().position(|line| line.trim_end() == "Tabs").unwrap();
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert!(session < sidebar && sidebar < tabs && tabs < layout);
        assert!(frame[sidebar].starts_with("> ● Sidebar"));
        assert!(frame[layout].starts_with("    Layout"));
        assert!(frame.iter().all(|line| crate::native_view_layout::visit_unpadded_cells(line, 48, |_, _| {}) == 48));
        dialog.next();
        let frame = dialog.render(48, 10);
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert!(frame[sidebar].starts_with("  ● Sidebar"));
        assert!(frame[layout].starts_with(">   Layout"));
        assert!(frame[layout].contains("horizontal"));
    }

    #[test]
    fn row_hits_follow_category_heading_and_viewport_lines() {
        let mut dialog = open();
        let frame = dialog.render(40, 7);
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        assert_eq!(dialog.hit_row(0, sidebar, 40, 7), Some("session.sidebar"));
        let heading = frame.iter().position(|line| line.trim_end() == "Session").unwrap();
        assert_eq!(dialog.hit_row(0, heading, 40, 7), None);
        assert_eq!(dialog.hit_row(0, 7, 40, 7), None);
        assert_eq!(dialog.hit_row(40, sidebar, 40, 7), None);
        assert_eq!(dialog.hit_row(0, sidebar, 0, 7), None);
        dialog.select(1);
        let frame = dialog.render(40, 5);
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert_eq!(dialog.hit_row(0, layout, 40, 5), Some("tabs.layout"));
        assert!(dialog.hit_layout(0, layout, 40, 5));
        assert_eq!(dialog.hit_row(0, 5, 40, 5), None);
        assert_eq!(dialog.hit_row(0, 0, 40, 0), None);
    }

    #[test]
    fn fuzzy_search_matches_title_category_and_orientation_keywords() {
        let mut dialog = open();
        for ch in "ori".chars() {
            dialog.push_query(ch);
        }
        let frame = dialog.render(40, 6);
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert_eq!(dialog.hit_row(0, layout, 40, 6), Some("tabs.layout"));
        assert!(dialog.render(40, 6)[1].contains("ori"));
        dialog.backspace();
        dialog.backspace();
        dialog.backspace();
        for ch in "sbar".chars() {
            dialog.push_query(ch);
        }
        let frame = dialog.render(40, 6);
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        assert_eq!(dialog.hit_row(0, sidebar, 40, 6), Some("session.sidebar"));
    }

    #[test]
    fn search_flattens_categories_and_unmatched_results_are_clear() {
        let mut dialog = open();
        dialog.push_query('t');
        dialog.push_query('a');
        dialog.push_query('b');
        dialog.push_query('s');
        let frame = dialog.render(40, 6);
        assert!(!frame.iter().any(|line| line == "Session" || line == "Tabs"));
        assert!(frame.iter().any(|line| line.contains("Layout")));
        while dialog.backspace() {}
        dialog.push_query('z');
        assert!(dialog.render(40, 6).iter().any(|line| line.contains("No results")));
        assert!(!dialog.has_visible_row());
    }

    #[test]
    fn value_change_actions_wait_for_successful_application() {
        let mut dialog = open();
        dialog.next();
        assert_eq!(
            dialog.right(),
            Some(SettingsAction::ApplyTabs(TabOrientation::Vertical))
        );
        assert_eq!(dialog.current(), TabOrientation::Horizontal);
        dialog.applied(TabOrientation::Vertical);
        assert_eq!(dialog.current(), TabOrientation::Vertical);
        assert_eq!(
            dialog.left(),
            Some(SettingsAction::ApplyTabs(TabOrientation::Horizontal))
        );
        dialog.previous();
        assert_eq!(dialog.right(), Some(SettingsAction::ApplySidebar(true)));
        dialog.applied_sidebar(true);
        assert!(dialog.render(48, 10)[4].contains("hide"));
    }

    #[test]
    fn click_and_enter_cycle_the_selected_live_setting() {
        let mut dialog = open();
        assert_eq!(
            dialog.click("session.sidebar"),
            Some(SettingsAction::ApplySidebar(true))
        );
        dialog.next();
        assert_eq!(
            dialog.activate(),
            Some(SettingsAction::ApplyTabs(TabOrientation::Vertical))
        );
        assert_eq!(dialog.click("not-a-setting"), None);
    }

    #[test]
    fn navigation_wraps_and_queries_are_bounded() {
        let mut dialog = open();
        dialog.previous();
        let frame = dialog.render(40, 7);
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert_eq!(dialog.hit_row(0, layout, 40, 7), Some("tabs.layout"));
        dialog.next();
        let frame = dialog.render(40, 7);
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        assert_eq!(dialog.hit_row(0, sidebar, 40, 7), Some("session.sidebar"));
        dialog.previous();
        let frame = dialog.render(40, 7);
        let layout = frame.iter().position(|line| line.trim_end().contains("Layout")).unwrap();
        assert_eq!(dialog.hit_row(0, layout, 40, 7), Some("tabs.layout"));
        assert!(!dialog.push_query('\n'));
        for _ in 0..MAX_QUERY_CHARS {
            assert!(dialog.push_query('x'));
        }
        assert!(!dialog.push_query('x'));
        assert!(dialog.render(20, 6).iter().all(|line| line.chars().count() <= 20));
    }

    #[test]
    fn closed_dialog_ignores_input_and_render() {
        let mut dialog = SettingsDialog::default();
        dialog.next();
        assert!(!dialog.push_query('x'));
        assert_eq!(dialog.right(), None);
        assert_eq!(dialog.render(40, 8), Vec::<String>::new());
        dialog.open(TabOrientation::Vertical);
        assert_eq!(dialog.current(), TabOrientation::Vertical);
        dialog.close();
        assert_eq!(dialog.query(), "");
    }

    #[test]
    fn unicode_query_and_tiny_render_remain_bounded() {
        let mut dialog = open();
        for ch in "界".chars() {
            assert!(dialog.push_query(ch));
        }
        for width in 0..=3 {
            for height in 0..=4 {
                let frame = dialog.render(width, height);
                assert!(frame.len() <= height);
                assert!(frame.iter().all(|line| {
                    crate::native_view_layout::visit_unpadded_cells(line, usize::from(width), |_, _| {})
                        == usize::from(width)
                }));
            }
        }
        assert_eq!(dialog.hit_row(0, 0, 3, 0), None);
        assert_eq!(dialog.hit_row(0, 1, 3, 1), None);
    }

    #[test]
    fn every_small_viewport_hit_matches_a_painted_setting_row() {
        for query in ["", "ori"] {
            for selected in 0..2 {
                for height in 0..=16 {
                    let mut dialog = open();
                    for ch in query.chars() {
                        dialog.push_query(ch);
                    }
                    dialog.select(selected);
                    let frame = dialog.render(48, height);
                    let expected: Vec<_> = [
                        ("Sidebar", "session.sidebar"),
                        ("Layout", "tabs.layout"),
                    ]
                    .into_iter()
                    .filter_map(|(title, id)| {
                        frame
                            .iter()
                            .position(|line| line.trim_end().contains(title))
                            .map(|row| (row, id))
                    })
                    .collect();
                    for (row, id) in &expected {
                        assert_eq!(dialog.hit_row(0, *row, 48, height), Some(*id), "height={height} query={query:?}");
                    }
                    for row in 0..height {
                        if !expected.iter().any(|(expected_row, _)| *expected_row == row) {
                            assert_eq!(dialog.hit_row(0, row, 48, height), None, "inert row {row} height={height} query={query:?}");
                        }
                    }
                    assert!(dialog.hit_row(0, height, 48, height).is_none());
                    assert!(dialog.hit_row(0, 0, 0, height).is_none());
                    assert!(frame.iter().all(|line| crate::native_view_layout::visit_unpadded_cells(line, 48, |_, _| {}) == 48));
                }
            }
        }
    }
}
