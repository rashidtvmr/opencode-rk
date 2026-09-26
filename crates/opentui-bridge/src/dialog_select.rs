#![forbid(unsafe_code)]
//! Filterable select state (mirrors `packages/tui/src/ui/dialog-select.tsx:80`
//! `DialogSelect<T>` + ref `DialogSelectRef<T>` (:74-78, :486-498)) with an
//! export-format helper grounded in `ui/dialog-export-options.tsx:24`
//! (`DialogExportOptionsProps` filename + four flags, :8-22).
//!
//! Divergences:
//! - T-erasure: TS is generic (`DialogSelectOption<T>{title,value:T,...}`
//!   :56-72); Rust stores `value: String`. Hosts stringify ids on insert.
//! - Filtering: TS ranks via `fuzzysort.go(needle, {keys:["title","category"],
//!   scoreFn: title*2+category})` (:165-170); Rust keeps input order and uses
//!   case-insensitive substring over `label` + `hint`. Subsequence-only hits
//!   (e.g. `"tml"` vs `"html"`) match in TS but not here.
//! - `hint` covers TS `description` (:60). Category grouping (:186-195),
//!   `disabled` exclusion (:155-159), `locked`/`preserveSelection`/`current`
//!   and scroll/actions are host concerns, not modeled.
//! - TS has no format picker; sole evidenced output is Markdown (default
//!   `session-<id8>.md`, `routes/session/index.tsx:956`; tip "save the
//!   conversation as Markdown", `feature-plugins/home/tips-view.tsx:185`).
//!   `ExportFormat` is single-variant; `pick` is filename/extension sugar.

/// Max chars for `SelectItem.label` (mirrors option `title`, :57).
pub const MAX_LABEL: usize = 256;
/// Max chars for `SelectItem.value` (T-erased id string).
pub const MAX_VALUE: usize = 256;
/// Max chars for `SelectItem.hint` (mirrors option `description`, :60).
pub const MAX_HINT: usize = 128;
/// Max items in one `SelectState` (fail-closed bound).
pub const MAX_ITEMS: usize = 256;
/// Max chars for the filter query (TS `store.filter`, :92).
pub const MAX_QUERY: usize = 128;

/// One selectable row: `label` = TS `title`, `value` = stringified TS `value`,
/// `hint` = TS `description`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
}

impl SelectItem {
    pub fn new(label: &str, value: &str, hint: Option<&str>) -> Result<Self, &'static str> {
        if label.is_empty() || label.chars().count() > MAX_LABEL {
            return Err("bad label");
        }
        if value.is_empty() || value.chars().count() > MAX_VALUE {
            return Err("bad value");
        }
        if let Some(h) = hint {
            if h.is_empty() || h.chars().count() > MAX_HINT {
                return Err("bad hint");
            }
        }
        Ok(Self { label: label.to_string(), value: value.to_string(), hint: hint.map(str::to_string) })
    }
}

/// Filterable select cursor. `selected` indexes the filtered view (TS
/// `flat()[store.selected]`, :215) and is clamped on every mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectState {
    items: Vec<SelectItem>,
    query: String,
    selected: usize,
}

impl SelectState {
    pub fn new(items: Vec<SelectItem>) -> Result<Self, &'static str> {
        if items.len() > MAX_ITEMS {
            return Err("too many items");
        }
        Ok(Self { items, query: String::new(), selected: 0 })
    }

    #[must_use]
    pub fn items(&self) -> &[SelectItem] {
        &self.items
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Cursor position within the filtered view.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Set filter text (TS `setStore("filter", e)` on input, :576); clamps.
    pub fn set_query(&mut self, query: &str) -> Result<(), &'static str> {
        if query.chars().count() > MAX_QUERY {
            return Err("query too long");
        }
        self.query = query.to_string();
        self.clamp_selected();
        Ok(())
    }

    /// Indices of visible items, input order (TS `filtered()` minus ranking).
    #[must_use]
    pub fn filter(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.items.len()).collect();
        }
        let needle = self.query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.label.to_lowercase().contains(&needle)
                    || it.hint.as_ref().is_some_and(|h| h.to_lowercase().contains(&needle))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Advance cursor, wrapping (TS `move(1)`, :290-297). Empty: no-op.
    pub fn select_next(&mut self) {
        let n = self.filter().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
    }

    /// Retreat cursor, wrapping (TS `move(-1)`, :290-297). Empty: no-op.
    pub fn select_prev(&mut self) {
        let n = self.filter().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + n - 1) % n;
    }

    /// Focused option (TS `selected()`, :215). `None` when view is empty.
    #[must_use]
    pub fn selected_item(&self) -> Option<&SelectItem> {
        self.filter().get(self.selected).map(|&i| &self.items[i])
    }

    /// Jump cursor to the filtered row with this value (TS ref `moveTo(value)`
    /// deep-equal find, :493-496). Returns false when absent.
    pub fn move_to_value(&mut self, value: &str) -> bool {
        match self.filter().iter().position(|&i| self.items[i].value == value) {
            Some(pos) => {
                self.selected = pos;
                true
            }
            None => false,
        }
    }

    fn clamp_selected(&mut self) {
        let n = self.filter().len();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }
}

/// Evidenced session-export output format. Single-variant: TS exposes no
/// picker, only Markdown output (see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
}

impl ExportFormat {
    /// Canonical file extension.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => ".md",
        }
    }

    /// Match a format name, extension, or filename (`"md"`, `"markdown"`,
    /// `".md"`, `"session-abc123.md"`). `None` for anything else.
    #[must_use]
    pub fn pick(s: &str) -> Option<Self> {
        let t = s.trim().to_lowercase();
        let base = t.rsplit('.').next().unwrap_or(&t);
        // Bare names without dots fall through as themselves.
        let key = if t.contains('.') { base } else { t.as_str() };
        match key {
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, value: &str) -> SelectItem {
        SelectItem::new(label, value, None).unwrap()
    }

    fn state() -> SelectState {
        SelectState::new(vec![
            item("Export session", "session.export"),
            item("Background subagents", "session.subagents"),
            SelectItem::new("Open HTML file", "open.html", Some("browser preview")).unwrap(),
        ])
        .unwrap()
    }

    #[test]
    fn item_bounds_rejected() {
        assert!(SelectItem::new("", "v", None).is_err());
        assert!(SelectItem::new("l", "", None).is_err());
        assert!(SelectItem::new(&"l".repeat(MAX_LABEL + 1), "v", None).is_err());
        assert!(SelectItem::new("l", &"v".repeat(MAX_VALUE + 1), None).is_err());
        assert!(SelectItem::new("l", "v", Some("")).is_err());
        assert!(SelectItem::new("l", "v", Some(&"h".repeat(MAX_HINT + 1))).is_err());
        assert!(SelectItem::new("l", "v", Some("ok")).is_ok());
    }

    #[test]
    fn state_rejects_too_many_items() {
        let big: Vec<SelectItem> = (0..MAX_ITEMS + 1).map(|i| item(&format!("l{i}"), &format!("v{i}"))).collect();
        assert!(SelectState::new(big).is_err());
        let ok: Vec<SelectItem> = (0..MAX_ITEMS).map(|i| item(&format!("l{i}"), &format!("v{i}"))).collect();
        assert!(SelectState::new(ok).is_ok());
        assert!(SelectState::new(vec![]).is_ok()); // emptyView "No results found", :599-607
    }

    #[test]
    fn filter_empty_returns_all_stable() {
        let s = state();
        assert_eq!(s.filter(), vec![0, 1, 2]);
    }

    #[test]
    fn filter_case_insensitive_label() {
        let mut s = state();
        s.set_query("EXPORT").unwrap();
        assert_eq!(s.filter(), vec![0]);
    }

    #[test]
    fn filter_matches_hint() {
        let mut s = state();
        s.set_query("browser").unwrap();
        assert_eq!(s.filter(), vec![2]);
    }

    #[test]
    fn substring_not_fuzzy() {
        // Fuzzysort would subsequence-match "oen" in "open"; substring does not.
        let mut s = state();
        s.set_query("oen").unwrap();
        assert!(s.filter().is_empty());
        assert!(s.selected_item().is_none());
    }

    #[test]
    fn next_prev_wrap_and_empty_noop() {
        let mut s = state();
        s.select_prev(); // wraps to last, TS move(-1), :293
        assert_eq!(s.selected(), 2);
        s.select_next(); // wraps to first, :294-295
        assert_eq!(s.selected(), 0);
        s.set_query("zzz-no-match").unwrap();
        s.select_next();
        s.select_prev();
        assert_eq!(s.selected(), 0);
    }

    #[test]
    fn set_query_clamps_and_rejects_long() {
        let mut s = state();
        s.select_next();
        s.select_next();
        assert_eq!(s.selected(), 2);
        s.set_query("export").unwrap(); // view shrinks to 1 row
        assert_eq!(s.selected(), 0);
        assert_eq!(s.selected_item().unwrap().value, "session.export");
        assert!(s.set_query(&"q".repeat(MAX_QUERY + 1)).is_err());
    }

    #[test]
    fn move_to_value() {
        let mut s = state();
        assert!(s.move_to_value("session.subagents"));
        assert_eq!(s.selected_item().unwrap().label, "Background subagents");
        assert!(!s.move_to_value("missing"));
        s.set_query("export").unwrap();
        assert!(!s.move_to_value("session.subagents")); // filtered out
    }

    #[test]
    fn export_pick_markdown() {
        assert_eq!(ExportFormat::pick("md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::pick("Markdown"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::pick(".md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::pick("session-abc123.md"), Some(ExportFormat::Markdown));
        assert_eq!(ExportFormat::Markdown.extension(), ".md");
        assert_eq!(ExportFormat::pick("json"), None);
        assert_eq!(ExportFormat::pick("html"), None);
        assert_eq!(ExportFormat::pick(""), None);
    }
}
