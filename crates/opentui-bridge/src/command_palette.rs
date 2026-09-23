#![forbid(unsafe_code)]
//! Command-palette state (mirrors `packages/tui/src/component/command-palette.tsx`).
//!
//! TS source (checkout a0d9b6c): `command-palette.tsx:15-17` filters out
//! hidden commands and the self command (`keymap.tsx:22`
//! `COMMAND_PALETTE_COMMAND = "command.palette.show"`); `command-palette.tsx:48-61`
//! maps entries to `{title, description, category, footer, value}` and
//! `command-palette.tsx:64-76` hoists `suggested` entries when unfiltered.
//! Filtering itself is delegated to `DialogSelect`; ranking here is new.
//!
//! Divergence: `prompt/autocomplete.tsx:502-525` ranks via `fuzzysort` with a
//! prefix boost (`score *= 2`) plus frecency. `fuzzysort` is unavailable in
//! this std-only crate, so [`PaletteState::rank`] uses case-insensitive
//! prefix > substring scoring, stable within tier, and drops non-matches.
//! No frecency input exists at this layer. Callers must exclude hidden entries
//! and [`COMMAND_PALETTE_COMMAND`] before pushing (mirrors TS filter).

/// Self command; never listed (mirrors `keymap.tsx:22`).
pub const COMMAND_PALETTE_COMMAND: &str = "command.palette.show";
/// Max entries (fail-closed: [`PaletteState::push`] errs beyond).
pub const MAX_ENTRIES: usize = 512;
/// Max query chars (fail-closed: [`PaletteState::set_query`] truncates).
pub const MAX_QUERY: usize = 256;
/// Max name chars.
pub const MAX_NAME: usize = 128;
/// Max description chars.
pub const MAX_DESC: usize = 256;
/// Max aliases per entry.
pub const MAX_ALIASES: usize = 8;
/// Max alias chars.
pub const MAX_ALIAS: usize = 64;

/// One palette row: command name plus display metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
}

impl PaletteEntry {
    pub fn new(name: &str, description: &str, aliases: &[&str]) -> Result<Self, &'static str> {
        if name.is_empty() {
            return Err("name empty");
        }
        if name.chars().count() > MAX_NAME {
            return Err("name too long");
        }
        if description.chars().count() > MAX_DESC {
            return Err("description too long");
        }
        if aliases.len() > MAX_ALIASES {
            return Err("too many aliases");
        }
        for a in aliases {
            if a.is_empty() || a.chars().count() > MAX_ALIAS {
                return Err("bad alias");
            }
        }
        Ok(Self {
            name: name.to_string(),
            description: description.to_string(),
            aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
        })
    }

    /// 2 = prefix hit, 1 = substring hit, 0 = no match (case-insensitive,
    /// over name, aliases, and description).
    #[must_use]
    pub fn score(&self, query: &str) -> u8 {
        let q = query.to_lowercase();
        if q.is_empty() {
            return 1;
        }
        let name = self.name.to_lowercase();
        if name.starts_with(&q) {
            return 2;
        }
        if self.aliases.iter().any(|a| a.to_lowercase().starts_with(&q)) {
            return 2;
        }
        if name.contains(&q)
            || self.aliases.iter().any(|a| a.to_lowercase().contains(&q))
            || self.description.to_lowercase().contains(&q)
        {
            return 1;
        }
        0
    }
}

/// Bounded entry list plus query cursor and selection.
#[derive(Debug, Default, Clone)]
pub struct PaletteState {
    entries: Vec<PaletteEntry>,
    query: String,
    selected: usize,
}

impl PaletteState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fail-closed `Err` when full.
    pub fn push(&mut self, entry: PaletteEntry) -> Result<(), &'static str> {
        if self.entries.len() >= MAX_ENTRIES {
            return Err("palette full");
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Sets query, truncating to [`MAX_QUERY`] chars; resets selection.
    pub fn set_query(&mut self, query: &str) {
        let mut s = String::new();
        for c in query.chars().take(MAX_QUERY) {
            s.push(c);
        }
        self.query = s;
        self.selected = 0;
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Ranked entry indices: prefix tier first, then substring, stable
    /// within tier; non-matches excluded (empty query returns all in order).
    #[must_use]
    pub fn rank(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.entries.len()).collect();
        }
        let mut prefix = Vec::new();
        let mut sub = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            match e.score(&self.query) {
                2 => prefix.push(i),
                1 => sub.push(i),
                _ => {}
            }
        }
        prefix.extend(sub);
        prefix
    }

    /// Visible row count = ranked length.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.rank().len()
    }

    /// Selected position within [`Self::rank`], clamped.
    #[must_use]
    pub fn selected(&self) -> usize {
        let n = self.visible_count();
        if n == 0 { 0 } else { self.selected.min(n - 1) }
    }

    /// Entry under the cursor, or `None` when rank is empty.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&PaletteEntry> {
        self.rank().get(self.selected()).map(|&i| &self.entries[i])
    }

    /// Move down, wrapping; no-op when rank is empty.
    pub fn select_next(&mut self) {
        let n = self.visible_count();
        if n == 0 {
            return;
        }
        self.selected = (self.selected() + 1) % n;
    }

    /// Move up, wrapping; no-op when rank is empty.
    pub fn select_prev(&mut self) {
        let n = self.visible_count();
        if n == 0 {
            return;
        }
        let cur = self.selected();
        self.selected = if cur == 0 { n - 1 } else { cur - 1 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PaletteState {
        let mut s = PaletteState::new();
        s.push(PaletteEntry::new("session.new", "Start session", &["new"]).unwrap())
            .unwrap();
        s.push(PaletteEntry::new("session.open", "Open session", &[]).unwrap())
            .unwrap();
        s.push(PaletteEntry::new("theme.dark", "Dark mode", &["dark-mode"]).unwrap())
            .unwrap();
        s
    }

    #[test]
    fn empty_query_returns_all_in_order() {
        let s = state();
        assert_eq!(s.rank(), vec![0, 1, 2]);
        assert_eq!(s.visible_count(), 3);
    }

    #[test]
    fn prefix_beats_substring() {
        let mut s = state();
        s.set_query("session");
        assert_eq!(s.rank(), vec![0, 1]);
        s.set_query("open");
        assert_eq!(s.rank(), vec![1]);
    }

    #[test]
    fn alias_and_description_match() {
        let mut s = state();
        s.set_query("dark-mode");
        assert_eq!(s.rank(), vec![2]);
        s.set_query("dark mode");
        assert_eq!(s.rank(), vec![2]);
    }

    #[test]
    fn no_match_rank_empty_and_nav_noop() {
        let mut s = state();
        s.set_query("zzz");
        assert!(s.rank().is_empty());
        assert_eq!(s.visible_count(), 0);
        assert_eq!(s.selected_entry(), None);
        s.select_next();
        s.select_prev();
        assert_eq!(s.selected(), 0);
    }

    #[test]
    fn select_wraps_and_clamps() {
        let mut s = state();
        s.select_prev();
        assert_eq!(s.selected(), 2);
        s.select_next();
        assert_eq!(s.selected(), 0);
        s.set_query("theme");
        assert_eq!(s.selected(), 0);
        assert_eq!(s.selected_entry().unwrap().name, "theme.dark");
    }

    #[test]
    fn bounds_fail_closed() {
        let mut s = PaletteState::new();
        for i in 0..MAX_ENTRIES {
            s.push(PaletteEntry::new(&format!("c{i}"), "", &[]).unwrap()).unwrap();
        }
        assert!(s.push(PaletteEntry::new("extra", "", &[]).unwrap()).is_err());
        assert!(PaletteEntry::new("", "", &[]).is_err());
        assert!(PaletteEntry::new(&"n".repeat(MAX_NAME + 1), "", &[]).is_err());
        assert!(PaletteEntry::new("n", &"d".repeat(MAX_DESC + 1), &[]).is_err());
        assert!(PaletteEntry::new("n", "", &["a"; MAX_ALIASES + 1]).is_err());
        s.set_query(&"q".repeat(MAX_QUERY + 10));
        assert_eq!(s.query().chars().count(), MAX_QUERY);
    }

    #[test]
    fn self_command_const_matches_ts() {
        assert_eq!(COMMAND_PALETTE_COMMAND, "command.palette.show");
    }
}
