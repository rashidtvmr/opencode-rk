#![forbid(unsafe_code)]
//! Native settings surface. Only settings with a live producer/consumer are shown.

use crate::native_navigation::TabOrientation;

pub const MAX_ROWS: usize = 2;
pub const MAX_QUERY_CHARS: usize = 128;

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
    pub key: &'static str,
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
                key: "session.sidebar",
            },
            SettingsRow {
                id: "tabs.layout",
                category: "Tabs",
                title: "Layout",
                description: "Whether tabs run along the top or down the side",
                key: "tabs.layout",
            },
        ]
    }

    fn visible_indices(&self) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        Self::rows()
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                query.is_empty()
                    || row.title.to_lowercase().contains(&query)
                    || row.category.to_lowercase().contains(&query)
                    || row.description.to_lowercase().contains(&query)
                    || row.key.contains(&query)
                    || match row.id {
                        "session.sidebar" => ["side panel", "sidebar"]
                            .iter()
                            .any(|word| word.contains(&query)),
                        "tabs.layout" => ["orientation", "left"]
                            .iter()
                            .any(|word| word.contains(&query)),
                        _ => false,
                    }
            })
            .map(|(i, _)| i)
            .collect()
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
        };
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
        let n = self.visible_indices().len();
        if n > 0 {
            self.selected = (self.selected + 1) % n;
        }
    }
    pub fn previous(&mut self) {
        let n = self.visible_indices().len();
        if n > 0 {
            self.selected = (self.selected + n - 1) % n;
        }
    }
    pub fn click(&mut self, id: &str) -> Option<SettingsAction> {
        let index = Self::rows().iter().position(|row| row.id == id)?;
        let visible = self.visible_indices();
        let position = visible.iter().position(|i| *i == index)?;
        self.selected = position;
        self.cycle(1)
    }
    pub fn left(&mut self) -> Option<SettingsAction> {
        self.cycle(-1)
    }
    pub fn right(&mut self) -> Option<SettingsAction> {
        self.cycle(1)
    }
    fn cycle(&mut self, direction: i8) -> Option<SettingsAction> {
        if !self.open {
            return None;
        }
        let visible = self.visible_indices();
        let index = *visible.get(self.selected)?;
        match index {
            0 => Some(SettingsAction::ApplySidebar(!self.sidebar_hidden)),
            1 => Some(SettingsAction::ApplyTabs(if direction < 0 {
                if self.tabs == TabOrientation::Horizontal {
                    TabOrientation::Vertical
                } else {
                    TabOrientation::Horizontal
                }
            } else {
                if self.tabs == TabOrientation::Horizontal {
                    TabOrientation::Vertical
                } else {
                    TabOrientation::Horizontal
                }
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
        self.open.then(|| {
            let visible = self.visible_indices();
            match visible.get(self.selected).copied().unwrap_or(1) {
                0 => SettingsAction::ApplySidebar(!self.sidebar_hidden),
                _ => SettingsAction::ApplyTabs(self.tabs),
            }
        })
    }

    fn row_line(&self, visible_position: usize) -> usize {
        3 + visible_position * 2
    }
    pub fn hit_row(&self, x: u16, y: usize, width: u16, height: usize) -> Option<&'static str> {
        if !self.open || usize::from(x) >= usize::from(width) {
            return None;
        }
        let visible = self.visible_indices();
        let capacity = height.saturating_sub(3).div_ceil(2);
        if capacity == 0 {
            return None;
        }
        let end = self
            .viewport_start
            .saturating_add(capacity)
            .min(visible.len());
        let position = (self.viewport_start..end).find(|position| {
            self.row_line(position.saturating_sub(self.viewport_start)) == usize::from(y)
        })?;
        Some(Self::rows()[visible[position]].id)
    }
    pub fn hit_layout(&self, x: u16, y: usize, width: u16, height: usize) -> bool {
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
                "No matching settings".to_owned(),
            ]
            .into_iter()
            .take(height)
            .map(|line| crate::native_view_layout::fit(&line, width))
            .collect();
        }
        let capacity = height.saturating_sub(3).div_ceil(2);
        if capacity == 0 {
            return (0..height)
                .map(|_| crate::native_view_layout::fit("", width))
                .collect();
        }
        if self.selected < self.viewport_start {
            self.viewport_start = self.selected;
        }
        if self.selected >= self.viewport_start.saturating_add(capacity) {
            self.viewport_start = self.selected + 1 - capacity;
        }
        let mut out = vec![
            "Settings".into(),
            format!("Search settings: {}", self.query),
            String::new(),
        ];
        let rows = Self::rows();
        for (position, index) in visible
            .iter()
            .enumerate()
            .skip(self.viewport_start)
            .take(capacity)
        {
            let row = rows[*index];
            let value = match row.id {
                "session.sidebar" => {
                    if self.sidebar_hidden {
                        "hide"
                    } else {
                        "auto"
                    }
                }
                _ => {
                    if self.tabs == TabOrientation::Horizontal {
                        "horizontal"
                    } else {
                        "vertical"
                    }
                }
            };
            out.push(format!(
                "{} {}  {}",
                if position == self.selected { ">" } else { " " },
                row.title,
                value
            ));
            if out.len() < height {
                out.push(row.description.into());
            }
        }
        out.into_iter()
            .take(height)
            .map(|line| crate::native_view_layout::fit(&line, width))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_live_settings_are_exposed() {
        assert_eq!(SettingsDialog::rows().len(), 2);
        assert!(SettingsDialog::rows()
            .iter()
            .all(|r| ["session.sidebar", "tabs.layout"].contains(&r.id)));
    }
    #[test]
    fn first_and_last_rows_have_stable_hits() {
        let mut d = SettingsDialog::default();
        d.open(TabOrientation::Horizontal);
        assert_eq!(d.hit_row(0, 3, 40, 7), Some("session.sidebar"));
        assert_eq!(d.hit_row(0, 5, 40, 7), Some("tabs.layout"));
        assert_eq!(d.hit_row(0, 5, 40, 5), None);
        assert_eq!(d.hit_row(0, 0, 40, 0), None);
        assert_eq!(d.hit_row(0, 3, 40, 3), None);
        assert_eq!(d.hit_row(0, 3, 40, 4), Some("session.sidebar"));
        assert_eq!(d.hit_row(0, 3, 0, 7), None);
        assert_eq!(d.hit_row(0, 7, 40, 7), None);
        d.select(1);
        let frame = d.render(40, 5);
        assert!(frame.iter().any(|line| line.starts_with("> Layout")));
        assert_eq!(d.hit_row(0, 3, 40, 5), Some("tabs.layout"));
        assert_eq!(d.hit_row(0, 2, 40, 7), None);
        assert_eq!(d.hit_row(40, 3, 40, 7), None);
        assert_eq!(
            d.click("session.sidebar"),
            Some(SettingsAction::ApplySidebar(true))
        );
    }
    #[test]
    fn event_navigation_down_enter_tabs_then_up_sidebar() {
        let mut d = SettingsDialog::default();
        d.open_with_state(false, TabOrientation::Horizontal);
        // Settings event arms map ArrowDown -> next, Enter/Right -> cycle.
        d.next();
        assert_eq!(
            d.right(),
            Some(SettingsAction::ApplyTabs(TabOrientation::Vertical))
        );
        // A failed persistence leaves the dialog model unchanged until applied.
        assert_eq!(d.current(), TabOrientation::Horizontal);
        // ArrowUp returns focus to the sidebar row; its next value is the
        // action the caller persists through state_path.
        d.previous();
        assert_eq!(d.right(), Some(SettingsAction::ApplySidebar(true)));
    }
    #[test]
    fn search_aliases_belong_to_their_row() {
        let mut d = SettingsDialog::default();
        d.open(TabOrientation::Horizontal);
        d.push_query('l');
        d.push_query('e');
        d.push_query('f');
        d.push_query('t');
        assert_eq!(d.hit_row(0, 3, 40, 7), Some("tabs.layout"));
        assert_eq!(d.hit_row(0, 5, 40, 7), None);
    }
    #[test]
    fn search_and_viewport_are_bounded() {
        let mut d = SettingsDialog::default();
        d.open_with_state(false, TabOrientation::Horizontal);
        assert!(d.render(20, 5).iter().all(|s| s.chars().count() <= 20));
        d.push_query('z');
        assert!(d.render(20, 5).iter().any(|s| s.contains("No matching")));
    }
    #[test]
    fn failed_persist_can_leave_model_unchanged() {
        let mut d = SettingsDialog::default();
        d.open_with_state(false, TabOrientation::Horizontal);
        let action = d.right().expect("tabs action");
        assert_eq!(action, SettingsAction::ApplySidebar(true));
        assert!(!d.is_open() || d.current() == TabOrientation::Horizontal);
        assert_eq!(d.current(), TabOrientation::Horizontal);
        assert_eq!(d.hit_row(0, 3, 40, 4), Some("session.sidebar"));
    }
}
