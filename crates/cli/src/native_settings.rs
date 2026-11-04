#![forbid(unsafe_code)]
//! Native settings surface. Only settings with a live producer/consumer are shown.
//!
//! Parity evidence (pinned upstream `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! search weighting/title-first ranking and the 0.7 threshold mirror
//! `component/dialog-config.tsx:410-458`; numeric step/min/max and the save
//! lock mirror `dialog-config.tsx:228-235,289-297,321-330,427-447`; the
//! experiments empty view and toggle lock mirror
//! `component/dialog-experiments.tsx:30-96`.

use crate::native_navigation::TabOrientation;

pub const MAX_ROWS: usize = 6;
pub const MAX_QUERY_CHARS: usize = 128;
const HEADER_ROWS: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsAction {
    ApplySidebar(bool),
    ApplyTabs(TabOrientation),
    /// Numeric rows step/saturate (`dialog-config.tsx:428-435`); the live
    /// consumer persists and echoes the value.
    ApplyNumeric {
        id: &'static str,
        value: f64,
    },
    /// Activating the Experiments row opens the picker
    /// (`dialog-experiments.tsx`).
    OpenExperiments,
    /// A picker toggle acquired the save lock; the consumer applies it and
    /// releases the lock through [`SettingsDialog::finish_experiments_toggle`]
    /// (`dialog-experiments.tsx:50-56`).
    ToggleExperiments,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingsRow {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    /// Enumerated `values` for this setting (`dialog-config.tsx` `values`).
    pub values: &'static [&'static str],
    /// Optional `labels` shown instead of the raw value
    /// (`dialog-config.tsx` `labels`).
    pub labels: &'static [&'static str],
    /// Numeric formatter; `Some` when the row is a numeric setting
    /// (`dialog-config.tsx:237,298,330`).
    pub format: Option<ValueFormat>,
    /// Step/min/max for numeric rows (`dialog-config.tsx:228-235,289-297,
    /// 321-330`); `None` for enumerated and launcher rows.
    pub limit: Option<NumericLimit>,
}

/// Exact numeric formatter selected by a numeric `dialog-config.tsx` row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueFormat {
    /// Scroll speed: `Number(value).toFixed(2)` (`dialog-config.tsx:237`).
    Fixed2,
    /// Leader timeout: `` `${value} ms` `` (`dialog-config.tsx:298`).
    Milliseconds,
    /// Alert volume: `` `${Math.round(Number(value) * 100)}%` ``
    /// (`dialog-config.tsx:330`).
    Percent,
}

#[derive(Clone, Debug)]
pub struct SettingsDialog {
    open: bool,
    selected: usize,
    viewport_start: usize,
    sidebar_hidden: bool,
    tabs: TabOrientation,
    query: String,
    /// Mirrors `saving()` in `dialog-config.tsx:427-447`: while a config
    /// update is in flight every further value change is ignored.
    saving: bool,
    /// `scroll.speed` current value (`dialog-config.tsx:228-236`, default 3).
    scroll_speed: f64,
    /// `leader.timeout` current value (`dialog-config.tsx:289-297`, default 2000).
    leader_timeout: f64,
    /// `attention.volume` current value (`dialog-config.tsx:321-330`, default 0.4).
    alert_volume: f64,
    /// Experiments picker sub-dialog (`dialog-experiments.tsx:30-96`).
    experiments: ExperimentsDialog,
    experiments_open: bool,
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
            saving: false,
            scroll_speed: 3.0,
            leader_timeout: 2000.0,
            alert_volume: 0.4,
            experiments: ExperimentsDialog::default(),
            experiments_open: false,
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
        self.saving = false;
        self.experiments_open = false;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.experiments_open = false;
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
                values: &["hide", "auto"],
                labels: &[],
                format: None,
                limit: None,
            },
            SettingsRow {
                id: "tabs.layout",
                category: "Tabs",
                title: "Layout",
                description: "Whether tabs run along the top or down the side",
                keywords: &["orientation", "left", "layout", "tabs"],
                values: &["horizontal", "vertical"],
                labels: &[],
                format: None,
                limit: None,
            },
            SettingsRow {
                id: "scroll.speed",
                category: "Input",
                title: "Scroll speed",
                description: "How far each scroll step moves",
                keywords: &["scrolling"],
                values: &[],
                labels: &[],
                format: Some(ValueFormat::Fixed2),
                limit: Some(SCROLL_SPEED),
            },
            SettingsRow {
                id: "leader.timeout",
                category: "Input",
                title: "Leader timeout",
                description: "How long the leader key waits for the next key",
                keywords: &["leader key", "shortcut timeout"],
                values: &[],
                labels: &[],
                format: Some(ValueFormat::Milliseconds),
                limit: Some(LEADER_TIMEOUT),
            },
            SettingsRow {
                id: "alert.volume",
                category: "Alerts",
                title: "Volume",
                description: "Volume of alert sounds",
                keywords: &["sound volume", "audio volume"],
                values: &[],
                labels: &[],
                format: Some(ValueFormat::Percent),
                limit: Some(ALERT_VOLUME),
            },
            SettingsRow {
                id: "advanced.experiments",
                category: "Advanced",
                title: "Experiments",
                description: "Open the in-flight experiments picker",
                keywords: &["experimental", "previews"],
                values: &["open"],
                labels: &[],
                format: None,
                limit: None,
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
                0 if self.sidebar_hidden => "hide".to_owned(),
                0 => "auto".to_owned(),
                1 => {
                    if self.tabs == TabOrientation::Horizontal {
                        "horizontal"
                    } else {
                        "vertical"
                    }
                    .to_owned()
                }
                5 => "open".to_owned(),
                index => {
                    let row = &rows[index];
                    row.display(&self.numeric_string(row.id))
                }
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
        // The picker owns the keyboard when open (`dialog-experiments.tsx`).
        if !self.open || self.experiments_open || ch.is_control() || self.query.chars().count() >= MAX_QUERY_CHARS {
            return false;
        }
        self.query.push(ch);
        self.selected = 0;
        self.viewport_start = 0;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if !self.open || self.experiments_open {
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

    /// Focus a live row selected by the command palette, without applying it.
    pub fn select_id(&mut self, id: &str) -> bool {
        if !self.open {
            return false;
        }
        let Some(index) = Self::rows().iter().position(|row| row.id == id) else {
            return false;
        };
        let visible = self.visible_indices();
        let Some(position) = visible.iter().position(|item| *item == index) else {
            return false;
        };
        self.selected = position;
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

    fn cycle(&mut self, direction: i8) -> Option<SettingsAction> {
        // `if (saving()) return` (`dialog-config.tsx:427-428`).
        if !self.open || self.saving {
            return None;
        }
        // While the picker is open, left/right/enter toggle the focused
        // experiment (`dialog-experiments.tsx:50-56,86-97`); `begin_toggle`
        // denies the change while a save is in flight or when empty.
        if self.experiments_open {
            return if self.experiments.begin_toggle() {
                Some(SettingsAction::ToggleExperiments)
            } else {
                None
            };
        }
        let visible = self.visible_indices();
        let index = *visible.get(self.selected)?;
        match index {
            0 => Some(SettingsAction::ApplySidebar(!self.sidebar_hidden)),
            1 => Some(SettingsAction::ApplyTabs(if self.tabs == TabOrientation::Horizontal {
                TabOrientation::Vertical
            } else {
                TabOrientation::Horizontal
            })),
            _ => {
                let row = Self::rows()[index];
                if row.id == "advanced.experiments" {
                    return Some(SettingsAction::OpenExperiments);
                }
                let Some(limit) = row.limit else { return None };
                let current = self.numeric_value(row.id);
                // `Math.min(max, Math.max(min, current + direction * step))`
                // (`dialog-config.tsx:428-435`); a saturated value is a no-op
                // (`if (next === current) return`, `dialog-config.tsx:430`).
                let next = limit.next(current, direction);
                if next == current {
                    return None;
                }
                Some(SettingsAction::ApplyNumeric { id: row.id, value: next })
            }
        }
    }

    pub fn applied_sidebar(&mut self, value: bool) {
        self.sidebar_hidden = value;
    }

    pub fn applied(&mut self, value: TabOrientation) {
        self.tabs = value;
    }

    /// Current numeric string fed through [`SettingsRow::display`].
    fn numeric_string(&self, id: &str) -> String {
        match id {
            "scroll.speed" => format!("{}", self.scroll_speed),
            "leader.timeout" => format!("{}", self.leader_timeout),
            "alert.volume" => format!("{}", self.alert_volume),
            _ => String::new(),
        }
    }

    fn numeric_value(&self, id: &str) -> f64 {
        match id {
            "scroll.speed" => self.scroll_speed,
            "leader.timeout" => self.leader_timeout,
            "alert.volume" => self.alert_volume,
            _ => 0.0,
        }
    }

    /// Echo a successful [`SettingsAction::ApplyNumeric`] back into the
    /// dialog's own state (`tui_entry.rs` live consumer).
    pub fn applied_numeric(&mut self, id: &str, value: f64) {
        match id {
            "scroll.speed" => self.scroll_speed = value,
            "leader.timeout" => self.leader_timeout = value,
            "alert.volume" => self.alert_volume = value,
            _ => {}
        }
    }

    /// Live experiment count; the CLI wires the picker to the config
    /// `experimental` map at startup.
    pub fn set_experiments(&mut self, count: usize) {
        self.experiments = ExperimentsDialog::new(count);
    }

    /// Picker visibility; Escape dismisses it before the dialog
    /// (`dialog-experiments.tsx`).
    pub const fn experiments_open(&self) -> bool {
        self.experiments_open
    }

    pub const fn experiments_count(&self) -> usize {
        self.experiments.count()
    }

    pub fn show_experiments(&mut self) {
        self.experiments_open = true;
    }

    pub fn close_experiments(&mut self) {
        self.experiments_open = false;
    }

    /// Release the picker's save lock after the consumer applied a
    /// [`SettingsAction::ToggleExperiments`].
    pub fn finish_experiments_toggle(&mut self) {
        self.experiments.finish_toggle();
    }

    /// `setSaving(true)` before the config write; concurrent changes are
    /// ignored until [`Self::finish_save`] (`dialog-config.tsx:427-447`).
    #[must_use]
    pub const fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn begin_save(&mut self) -> bool {
        if !self.open || self.saving {
            return false;
        }
        self.saving = true;
        true
    }

    pub fn finish_save(&mut self) {
        self.saving = false;
    }

    pub fn activate(&mut self) -> Option<SettingsAction> {
        self.cycle(1)
    }    pub fn hit_row(
        &self,
        x: u16,
        y: usize,
        width: u16,
        height: usize,
    ) -> Option<&'static str> {
        if !self.open || self.experiments_open || usize::from(x) >= usize::from(width) || y >= height {
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
        // Picker overlay: `DialogExperiments` is its own dialog
        // (`dialog-experiments.tsx:58-97`) whose filter row renders only when
        // experiments exist and whose empty view replaces the list otherwise.
        if self.experiments_open {
            let mut out = vec!["Experiments".to_owned()];
            if let Some(message) = self.experiments.empty_message() {
                out.push(message.to_owned());
            } else {
                out.push("Search experiments:".to_owned());
                if out.len() < height {
                    out.push("change  ←/→".to_owned());
                }
            }
            return out
                .into_iter()
                .take(height)
                .map(|line| crate::native_view_layout::fit(&line, width))
                .collect();
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

impl SettingsRow {
    /// Mirrors `display(setting)` in `dialog-config.tsx:398-402`: a formatted
    /// numeric row wins, otherwise the enumerated value maps through `labels`
    /// when present and falls back to `String(current)`.
    #[must_use]
    pub fn display(&self, current: &str) -> String {
        if let Some(format) = self.format {
            if let Ok(number) = current.parse::<f64>() {
                return match format {
                    ValueFormat::Fixed2 => format_fixed2(number),
                    ValueFormat::Milliseconds => format_milliseconds(number),
                    ValueFormat::Percent => format_percent(number),
                };
            }
            return current.to_owned();
        }
        match self.values.iter().position(|value| *value == current) {
            Some(index) => self
                .labels
                .get(index)
                .map_or_else(|| current.to_owned(), |label| (*label).to_owned()),
            None => current.to_owned(),
        }
    }
}

/// `` `${value} ms` `` for a finite number: an integral value drops the
/// fraction so `2000.0` prints `2000 ms`, matching JS number stringification
/// (`dialog-config.tsx:298`).
#[must_use]
pub fn format_milliseconds(value: f64) -> String {
    if !value.is_finite() {
        return format!("{value} ms");
    }
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{value:.0} ms")
    } else {
        format!("{value} ms")
    }
}

/// `Number(value).toFixed(2)` (`dialog-config.tsx:237`): always two fraction
/// digits, e.g. `3.00`, `0.25`.
#[must_use]
pub fn format_fixed2(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.2}")
    } else {
        format!("{value}")
    }
}

/// `` `${Math.round(Number(value) * 100)}%` `` (`dialog-config.tsx:330`):
/// e.g. `0.4 -> "40%"`, `1.0 -> "100%"`.
#[must_use]
pub fn format_percent(value: f64) -> String {
    if value.is_finite() {
        format!("{}%", value.mul_add(100.0, 0.0).round() as i64)
    } else {
        format!("{value}%")
    }
}

/// Numeric settings bounds declared by `dialog-config.tsx:228-235,289-297,321-330`.
/// A numeric setting steps by `step` and saturates at `min`/`max`; there is no
/// wraparound (only enumerated `values` wrap).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericLimit {
    pub step: f64,
    pub min: f64,
    pub max: f64,
}

/// Scroll speed: `step: 0.25, min: 0.25, max: 10`
/// (`dialog-config.tsx:228-236`).
pub const SCROLL_SPEED: NumericLimit = NumericLimit {
    step: 0.25,
    min: 0.25,
    max: 10.0,
};

/// Leader timeout (ms): `step: 250, min: 250, max: 10000`
/// (`dialog-config.tsx:289-297`).
pub const LEADER_TIMEOUT: NumericLimit = NumericLimit {
    step: 250.0,
    min: 250.0,
    max: 10_000.0,
};

/// Alert volume: `step: 0.1, min: 0, max: 1`
/// (`dialog-config.tsx:321-330`).
pub const ALERT_VOLUME: NumericLimit = NumericLimit {
    step: 0.1,
    min: 0.0,
    max: 1.0,
};

impl NumericLimit {
    /// `Math.min(max, Math.max(min, current + direction * step))`
    /// (`dialog-config.tsx:428-435`). A malformed range leaves the value
    /// untouched instead of panicking.
    #[must_use]
    pub fn next(self, current: f64, direction: i8) -> f64 {
        if self.min > self.max || !current.is_finite() {
            return current;
        }
        let next = current + f64::from(direction) * self.step;
        next.max(self.min).min(self.max)
    }
}

/// Empty-view copy for the experiments picker
/// (`dialog-experiments.tsx:68-75`).
pub const EXPERIMENTS_EMPTY_MESSAGE: &str = "No experiments available";

/// Experiments picker state (`dialog-experiments.tsx:30-96`): the filter row
/// is only rendered when experiments exist, the empty view replaces the list
/// otherwise, and toggling is locked while a config write is in flight.
#[derive(Clone, Debug, Default)]
pub struct ExperimentsDialog {
    experiments: usize,
    saving: bool,
}

impl ExperimentsDialog {
    #[must_use]
    pub const fn new(experiments: usize) -> Self {
        Self {
            experiments,
            saving: false,
        }
    }

    #[must_use]
    pub const fn count(&self) -> usize {
        self.experiments
    }

    /// `renderFilter={experiments.length > 0}`
    /// (`dialog-experiments.tsx:68`).
    #[must_use]
    pub const fn render_filter(&self) -> bool {
        self.experiments > 0
    }

    /// `emptyView` is only shown once the list is empty
    /// (`dialog-experiments.tsx:71-75`).
    #[must_use]
    pub const fn empty_message(&self) -> Option<&'static str> {
        if self.experiments == 0 {
            Some(EXPERIMENTS_EMPTY_MESSAGE)
        } else {
            None
        }
    }

    /// `if (saving()) return; if (!experiment) return`
    /// (`dialog-experiments.tsx:50-53`).
    pub fn begin_toggle(&mut self) -> bool {
        if self.saving || self.experiments == 0 {
            return false;
        }
        self.saving = true;
        true
    }

    pub fn finish_toggle(&mut self) {
        self.saving = false;
    }

    #[must_use]
    pub const fn is_saving(&self) -> bool {
        self.saving
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
        let ids = [
            "session.sidebar",
            "tabs.layout",
            "scroll.speed",
            "leader.timeout",
            "alert.volume",
            "advanced.experiments",
        ];
        assert_eq!(SettingsDialog::rows().len(), ids.len());
        let exposed: Vec<_> = SettingsDialog::rows().iter().map(|row| row.id).collect();
        for id in ids {
            assert!(exposed.contains(&id));
        }
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
        let experiments = frame.iter().position(|line| line.trim_end().contains("Experiments")).unwrap();
        assert_eq!(dialog.hit_row(0, experiments, 40, 7), Some("advanced.experiments"));
        dialog.next();
        let frame = dialog.render(40, 7);
        let sidebar = frame.iter().position(|line| line.trim_end().contains("Sidebar")).unwrap();
        assert_eq!(dialog.hit_row(0, sidebar, 40, 7), Some("session.sidebar"));
        dialog.previous();
        let frame = dialog.render(40, 7);
        let experiments = frame.iter().position(|line| line.trim_end().contains("Experiments")).unwrap();
        assert_eq!(dialog.hit_row(0, experiments, 40, 7), Some("advanced.experiments"));
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
    fn numeric_formatters_match_upstream_display_strings() {
        // dialog-config.tsx:237 `Number(value).toFixed(2)`.
        assert_eq!(format_fixed2(3.0), "3.00");
        assert_eq!(format_fixed2(0.25), "0.25");
        assert_eq!(format_fixed2(10.0), "10.00");
        assert_eq!(format_fixed2(3.25), "3.25");
        // dialog-config.tsx:298 `` `${value} ms` ``.
        assert_eq!(format_milliseconds(2000.0), "2000 ms");
        assert_eq!(format_milliseconds(2250.0), "2250 ms");
        assert_eq!(format_milliseconds(250.0), "250 ms");
        // dialog-config.tsx:330 `` `${Math.round(value * 100)}%` ``.
        assert_eq!(format_percent(0.4), "40%");
        assert_eq!(format_percent(0.0), "0%");
        assert_eq!(format_percent(1.0), "100%");
    }

    #[test]
    fn rows_display_matches_upstream_labels_and_values() {
        let rows = SettingsDialog::rows();
        // session.sidebar values ["hide","auto"], no labels.
        assert_eq!(rows[0].display("auto"), "auto");
        assert_eq!(rows[0].display("hide"), "hide");
        // Unknown value falls back to String(current) (dialog-config.tsx:401).
        assert_eq!(rows[0].display("legacy"), "legacy");
        // tabs.layout values ["horizontal","vertical"].
        assert_eq!(rows[1].display("horizontal"), "horizontal");
        assert_eq!(rows[1].display("vertical"), "vertical");
    }

    #[test]
    fn numeric_row_format_selection_projects_through_display() {
        let scroll = SettingsRow {
            id: "scroll.speed",
            category: "Input",
            title: "Scroll speed",
            description: "How far each scroll step moves",
            keywords: &["scrolling"],
            values: &[],
            labels: &[],
            format: Some(ValueFormat::Fixed2),
            limit: Some(SCROLL_SPEED),
        };
        assert_eq!(scroll.display("3"), "3.00");
        let leader = SettingsRow {
            format: Some(ValueFormat::Milliseconds),
            values: &[],
            labels: &[],
            ..scroll.clone()
        };
        assert_eq!(leader.display("2000"), "2000 ms");
        let volume = SettingsRow {
            format: Some(ValueFormat::Percent),
            values: &[],
            labels: &[],
            ..leader.clone()
        };
        assert_eq!(volume.display("0.4"), "40%");
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
                        ("Scroll speed", "scroll.speed"),
                        ("Leader timeout", "leader.timeout"),
                        ("Volume", "alert.volume"),
                        ("Experiments", "advanced.experiments"),
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

    // DIALOG-16: numeric settings clamp to their declared min/max instead of
    // wrapping, and a value change is ignored while a save is in flight
    // (dialog-config.tsx:228-235,289-297,321-330,427-447).
    #[test]
    fn numeric_limits_clamp_and_saves_lock_further_changes() {
        assert_eq!(SCROLL_SPEED.next(3.0, 1), 3.25);
        assert_eq!(SCROLL_SPEED.next(10.0, 1), 10.0);
        assert_eq!(SCROLL_SPEED.next(0.25, -1), 0.25);
        assert_eq!(LEADER_TIMEOUT.next(2_000.0, 1), 2_250.0);
        assert_eq!(LEADER_TIMEOUT.next(10_000.0, 1), 10_000.0);
        assert_eq!(LEADER_TIMEOUT.next(250.0, -1), 250.0);
        assert!((ALERT_VOLUME.next(0.4, 1) - 0.5).abs() < 1e-9);
        assert_eq!(ALERT_VOLUME.next(1.0, 1), 1.0);
        assert_eq!(ALERT_VOLUME.next(0.0, -1), 0.0);
        let malformed = NumericLimit { step: 1.0, min: 5.0, max: 1.0 };
        assert_eq!(malformed.next(3.0, 1), 3.0);

        let mut dialog = open();
        assert!(dialog.begin_save());
        assert!(dialog.is_saving());
        assert_eq!(dialog.right(), None);
        assert_eq!(dialog.activate(), None);
        dialog.finish_save();
        assert_eq!(
            dialog.right(),
            Some(SettingsAction::ApplySidebar(true))
        );
    }

    // DIALOG-16: the experiments picker hides its filter when empty and shows
    // the empty view instead; toggles are save-locked
    // (dialog-experiments.tsx:50-76).
    #[test]
    fn experiments_empty_state_and_save_lock_match() {
        let empty = ExperimentsDialog::new(0);
        assert!(!empty.render_filter());
        assert_eq!(empty.empty_message(), Some("No experiments available"));
        let mut populated = ExperimentsDialog::new(2);
        assert!(populated.render_filter());
        assert_eq!(populated.empty_message(), None);
        assert!(populated.begin_toggle());
        assert!(!populated.begin_toggle(), "a save in flight ignores toggles");
        populated.finish_toggle();
        assert!(populated.begin_toggle());
    }

    // DIALOG-16: the numeric settings rows step and saturate through the LIVE
    // dialog (`right`/`left`/`activate` -> `ApplyNumeric`), and the value is
    // echoed back with `applied_numeric`
    // (dialog-config.tsx:228-235,289-297,321-330,428-435).
    #[test]
    fn dialog16_numeric_rows_step_and_saturate_live() {
        fn numeric(action: Option<SettingsAction>) -> Option<(&'static str, f64)> {
            match action {
                Some(SettingsAction::ApplyNumeric { id, value }) => Some((id, value)),
                _ => None,
            }
        }

        let mut dialog = open();
        // scroll.speed: default 3.0, step 0.25, min 0.25, max 10.
        assert!(dialog.select_id("scroll.speed"));
        let (id, value) = numeric(dialog.right()).expect("step right");
        assert_eq!(id, "scroll.speed");
        assert!((value - 3.25).abs() < 1e-9, "{value}");
        dialog.applied_numeric("scroll.speed", 3.25);
        let (_, value) = numeric(dialog.left()).expect("step left");
        assert!((value - 3.0).abs() < 1e-9, "{value}");
        // Saturates at max (10) and min (0.25): a no-op returns None.
        dialog.applied_numeric("scroll.speed", 10.0);
        assert_eq!(dialog.right(), None);
        dialog.applied_numeric("scroll.speed", 0.25);
        assert_eq!(dialog.left(), None);
        // `activate` is the Enter/right path.
        dialog.applied_numeric("scroll.speed", 3.0);
        assert!(numeric(dialog.activate()).is_some());

        // leader.timeout: default 2000, step 250, min 250, max 10000.
        assert!(dialog.select_id("leader.timeout"));
        let (id, value) = numeric(dialog.right()).expect("leader step");
        assert_eq!(id, "leader.timeout");
        assert!((value - 2250.0).abs() < 1e-9, "{value}");
        dialog.applied_numeric("leader.timeout", 10_000.0);
        assert_eq!(dialog.right(), None);
        dialog.applied_numeric("leader.timeout", 250.0);
        assert_eq!(dialog.left(), None);

        // alert.volume: default 0.4, step 0.1, min 0, max 1.
        assert!(dialog.select_id("alert.volume"));
        let (id, value) = numeric(dialog.right()).expect("volume step");
        assert_eq!(id, "alert.volume");
        assert!((value - 0.5).abs() < 1e-9, "{value}");
        dialog.applied_numeric("alert.volume", 1.0);
        assert_eq!(dialog.right(), None);
        dialog.applied_numeric("alert.volume", 0.0);
        assert_eq!(dialog.left(), None);

        // The rendered row reflects the accepted value (Percent format).
        dialog.applied_numeric("alert.volume", 0.4);
        let frame = dialog.render(48, 20);
        assert!(frame.iter().any(|line| line.contains("40%")), "{frame:?}");
        assert!(frame.iter().any(|line| line.contains("Scroll speed")), "{frame:?}");
    }

    // DIALOG-16: the experiments launcher opens the picker; the picker hides
    // its filter when empty, shows the empty view otherwise, and gates toggles
    // on the save lock (dialog-experiments.tsx:50-76).
    #[test]
    fn dialog16_experiments_picker_opens_and_toggles_live() {
        let mut dialog = open();
        // The launcher row opens the picker and does not step a value.
        assert!(dialog.select_id("advanced.experiments"));
        assert_eq!(dialog.right(), Some(SettingsAction::OpenExperiments));
        assert!(!dialog.experiments_open());
        dialog.show_experiments();
        assert!(dialog.experiments_open());

        // Empty picker: no filter row, the empty view replaces the list.
        dialog.set_experiments(0);
        let frame = dialog.render(48, 12);
        assert_eq!(frame[0].trim_end(), "Experiments");
        assert!(
            frame.iter().any(|line| line.contains("No experiments available")),
            "{frame:?}"
        );
        assert!(
            !frame.iter().any(|line| line.contains("Search experiments")),
            "{frame:?}"
        );
        // An empty picker denies the toggle (no lock acquired).
        assert_eq!(dialog.right(), None);

        // Populated picker: the filter row renders; a toggle acquires the save
        // lock and the live consumer releases it.
        dialog.set_experiments(2);
        let frame = dialog.render(48, 12);
        assert!(
            frame.iter().any(|line| line.contains("Search experiments")),
            "{frame:?}"
        );
        assert_eq!(dialog.right(), Some(SettingsAction::ToggleExperiments));
        assert_eq!(dialog.right(), None, "save lock blocks a second toggle");
        dialog.finish_experiments_toggle();
        assert_eq!(dialog.right(), Some(SettingsAction::ToggleExperiments));

        // Escape closes the picker but keeps the settings dialog open.
        dialog.close_experiments();
        assert!(!dialog.experiments_open());
        assert!(dialog.is_open());
    }
}
