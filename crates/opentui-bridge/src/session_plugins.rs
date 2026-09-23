#![forbid(unsafe_code)]
//! Session plugin enable/disable list state.
//! Mirrors TS checkout a0d9b6c (NOT pinned 95daf90):
//! - `packages/tui/src/feature-plugins/system/plugins.tsx:150-232`
//!   `View` list + `flip` (activate/deactivate toggle) + `DialogSelect`
//!   cursor (`current`/`onMove`/`onSelect`).
//! - `row()` (`plugins.tsx:135-144`): title=id, category Internal/External,
//!   description=meta, footer=state, manager row disabled.
//! List UI state only. Commands/registry live in sibling `plugin_host`
//! lane; install flow (`Install`, `plugins.tsx:38-133`) not modeled here.

/// Max plugin id bytes (TS plain string id; Rust bounded).
pub const MAX_PLUGIN_NAME: usize = 128;
/// Max list entries (TS unbounded sorted array; Rust fail-closed).
pub const MAX_PLUGINS: usize = 256;
/// Max version string bytes.
pub const MAX_VERSION: usize = 64;

/// Id of the manager itself; never toggleable
/// (mirrors `disabled: item.id === id`, `plugins.tsx:9,142`).
pub const MANAGER_ID: &str = "internal:plugin-manager";

/// List errors (fail-closed; TS silently ignores unknown ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginListError {
    Empty,
    TooLong,
    VersionTooLong,
    Full,
    Duplicate,
}

impl core::fmt::Display for PluginListError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "plugin name empty"),
            Self::TooLong => write!(f, "plugin name too long"),
            Self::VersionTooLong => write!(f, "plugin version too long"),
            Self::Full => write!(f, "plugin list full"),
            Self::Duplicate => write!(f, "plugin already listed"),
        }
    }
}

impl std::error::Error for PluginListError {}

/// One row of the plugin dialog (mirrors `TuiPluginStatus`
/// id/enabled subset used by `row`/`flip`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEntry {
    pub name: String,
    pub enabled: bool,
    pub version: Option<String>,
}

impl PluginEntry {
    pub fn new(name: &str, enabled: bool, version: Option<&str>) -> Result<Self, PluginListError> {
        if name.is_empty() {
            return Err(PluginListError::Empty);
        }
        if name.len() > MAX_PLUGIN_NAME {
            return Err(PluginListError::TooLong);
        }
        let version = match version {
            Some(v) => {
                if v.len() > MAX_VERSION {
                    return Err(PluginListError::VersionTooLong);
                }
                Some(v.to_string())
            }
            None => None,
        };
        Ok(Self { name: name.to_string(), enabled, version })
    }

    #[must_use]
    pub fn locked(&self) -> bool {
        self.name == MANAGER_ID
    }

    pub fn toggle(&mut self) {
        if !self.locked() {
            self.enabled = !self.enabled;
        }
    }
}

/// Selectable plugin list (mirrors `list`/`cur` signals + `flip`).
#[derive(Debug, Default, Clone)]
pub struct PluginList {
    entries: Vec<PluginEntry>,
    selected: usize,
}

impl PluginList {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: Vec::new(), selected: 0 }
    }

    pub fn add(&mut self, entry: PluginEntry) -> Result<(), PluginListError> {
        if self.entries.iter().any(|e| e.name == entry.name) {
            return Err(PluginListError::Duplicate);
        }
        if self.entries.len() >= MAX_PLUGINS {
            return Err(PluginListError::Full);
        }
        self.entries.push(entry);
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn selected_entry(&self) -> Option<&PluginEntry> {
        self.entries.get(self.selected)
    }

    /// Clamp cursor into range (empty list pins to 0).
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.entries.len().saturating_sub(1).max(0));
        if self.entries.is_empty() {
            self.selected = 0;
        }
    }

    /// Cursor down, clamped at last row (mirrors `onMove` without wrap).
    pub fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.entries.len() - 1);
    }

    /// Cursor up, clamped at 0.
    pub fn select_prev(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
    }

    /// Flip named row enable flag; false when unknown or locked
    /// (mirrors `flip` no-op on missing item + disabled manager row).
    pub fn toggle(&mut self, name: &str) -> bool {
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(e) if !e.locked() => {
                e.enabled = !e.enabled;
                true
            }
            _ => false,
        }
    }

    /// Flip currently selected row; false when empty/locked.
    pub fn toggle_selected(&mut self) -> bool {
        match self.entries.get_mut(self.selected) {
            Some(e) if !e.locked() => {
                e.enabled = !e.enabled;
                true
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn enabled_count(&self) -> usize {
        self.entries.iter().filter(|e| e.enabled).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_of(names: &[&str]) -> PluginList {
        let mut l = PluginList::new();
        for n in names {
            l.add(PluginEntry::new(n, false, None).unwrap()).unwrap();
        }
        l
    }

    #[test]
    fn entry_name_version_bounds() {
        assert_eq!(PluginEntry::new("", false, None).unwrap_err(), PluginListError::Empty);
        assert_eq!(
            PluginEntry::new(&"x".repeat(MAX_PLUGIN_NAME + 1), false, None).unwrap_err(),
            PluginListError::TooLong
        );
        assert_eq!(
            PluginEntry::new("a", false, Some(&"v".repeat(MAX_VERSION + 1))).unwrap_err(),
            PluginListError::VersionTooLong
        );
        let e = PluginEntry::new("ext:foo", true, Some("1.0")).unwrap();
        assert_eq!(e.version.as_deref(), Some("1.0"));
    }

    #[test]
    fn add_bounded_duplicate_rejected() {
        let mut l = PluginList::new();
        l.add(PluginEntry::new("a", false, None).unwrap()).unwrap();
        assert_eq!(
            l.add(PluginEntry::new("a", false, None).unwrap()).unwrap_err(),
            PluginListError::Duplicate
        );
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn toggle_flips_and_reports() {
        let mut l = list_of(&["a", "b"]);
        assert!(l.toggle("a"));
        assert!(l.selected_entry().unwrap().enabled);
        assert!(!l.toggle("missing"));
        l.set_selected(1);
        assert!(l.toggle_selected());
        assert_eq!(l.enabled_count(), 2);
        l.set_selected(0);
        assert!(l.toggle_selected());
        assert_eq!(l.enabled_count(), 1);
    }

    #[test]
    fn manager_row_locked() {
        let mut l = list_of(&[MANAGER_ID, "ext:x"]);
        assert!(!l.toggle(MANAGER_ID));
        assert!(!l.toggle_selected());
        assert!(l.toggle("ext:x"));
        assert_eq!(l.enabled_count(), 1);
    }

    #[test]
    fn cursor_clamped_both_ends() {
        let mut l = list_of(&["a", "b", "c"]);
        l.select_prev();
        assert_eq!(l.selected(), 0);
        l.set_selected(99);
        assert_eq!(l.selected(), 2);
        l.select_next();
        assert_eq!(l.selected(), 2);
        l.set_selected(1);
        l.select_next();
        assert_eq!(l.selected(), 2);
        l.select_prev();
        assert_eq!(l.selected(), 1);
    }

    #[test]
    fn empty_list_cursor_safe() {
        let mut l = PluginList::new();
        assert!(l.is_empty());
        l.select_next();
        l.select_prev();
        l.set_selected(5);
        assert_eq!(l.selected(), 0);
        assert!(l.selected_entry().is_none());
        assert!(!l.toggle_selected());
    }
}
