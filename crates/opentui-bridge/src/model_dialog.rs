#![forbid(unsafe_code)]
//! Model dialog ranking helpers (mirrors `packages/tui/src/component/dialog-model.tsx`).
//!
//! Evidence (TS checkout a0d9b6c, `dialog-model.tsx:23-183`):
//! - fav/recent sections: favorites first (`:53`), recents deduped minus
//!   favorites (`:54-59`), remainder last (`:129`).
//! - recency order: `recentModels` prepends most-recent, dedups, caps 10
//!   (`context/local.tsx:35-49`); higher `recent` rank = more recent.
//! - disabled: opencode provider + id includes `-nano` (`:43`, `:81`).
//! - Free footer: `cost.input == 0 && provider opencode` (`:44`, `:82`);
//!   section sort puts `footer == "Free"` first, then releaseDate desc,
//!   then title (`sortModelOptions :186-197`).
//! - session footer: `· {model()}` where `model() = Model.name(...)`
//!   (`routes/session/index.tsx:1461`, `:1549`).
//! - qualified debug form `providerID/modelID` (`dialog-debug.tsx:32`);
//!   name fallback to modelID via `crate::model_ref::ModelIndex::name`.
//!
//! Divergence: TS filters with `fuzzysort.go` on title/category (`:122`)
//! when a query is typed. This module is std-only ranking (fav, recent,
//! title); fuzzy matching stays TS-side and is NOT reimplemented here.
//! `is_denied` has no TS counterpart; it is a new fail-closed Rust-side
//! guard over a caller-supplied denylist.

use crate::model_ref::ModelRef;

/// Max chars for [`ModelEntry`] id/name (truncate, never reject).
pub const MAX_MODEL_ID: usize = 128;
/// Max chars for display name (truncate, never reject).
pub const MAX_MODEL_NAME: usize = 128;
/// TS `:43`/`:81` use `includes("-nano")`; no single nano model exists.
pub const NANO_MARKER: &str = "-nano";
/// Footer label emitted by TS for zero-cost opencode models (`:44`, `:82`).
pub const FREE_LABEL: &str = "Free";

/// Rankable model entry. `recent`: higher = more recently selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub fav: bool,
    pub recent: u64,
}

impl ModelEntry {
    #[must_use]
    pub fn new(id: &str, name: &str, fav: bool, recent: u64) -> Self {
        Self {
            id: truncate(id, MAX_MODEL_ID),
            name: truncate(name, MAX_MODEL_NAME),
            fav,
            recent,
        }
    }

    /// Qualified `provider/model` debug form (`dialog-debug.tsx:32`).
    #[must_use]
    pub fn qualified(&self, provider: &str) -> String {
        ModelRef { provider: provider.to_string(), model: self.id.clone() }.qualified()
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

/// Fav first, then recent desc, then name asc (evidenced `:53-59`, `:129`;
/// name tiebreak mirrors `sortModelOptions` title key).
pub fn sort_entries(entries: &mut [ModelEntry]) {
    entries.sort_by(|a, b| {
        b.fav
            .cmp(&a.fav)
            .then_with(|| b.recent.cmp(&a.recent))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Fail-closed denylist guard (new, no TS counterpart): true on exact match.
#[must_use]
pub fn is_denied(id: &str, denylist: &[String]) -> bool {
    denylist.iter().any(|d| d == id)
}

/// TS disabled rule (`:43`, `:81`): opencode provider + `-nano` in id.
#[must_use]
pub fn is_nano(provider: &str, id: &str) -> bool {
    provider == "opencode" && id.contains(NANO_MARKER)
}

/// TS Free rule emits footer `"Free"` (`:44`, `:82`); Rust side matches the
/// emitted label since cost data lives TS-side.
#[must_use]
pub fn is_free(footer: &str) -> bool {
    footer == FREE_LABEL
}

/// Session footer label `· {name}` (`index.tsx:1549`).
#[must_use]
pub fn footer_model_label(name: &str) -> String {
    format!("· {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_id_and_name() {
        let e = ModelEntry::new(&"x".repeat(200), &"y".repeat(200), false, 0);
        assert_eq!(e.id.len(), MAX_MODEL_ID);
        assert_eq!(e.name.len(), MAX_MODEL_NAME);
    }

    #[test]
    fn fav_sorts_first() {
        let mut v =
            vec![ModelEntry::new("b", "b", false, 99), ModelEntry::new("a", "a", true, 0)];
        sort_entries(&mut v);
        assert!(v[0].fav);
    }

    #[test]
    fn recent_desc_within_same_fav() {
        let mut v = vec![
            ModelEntry::new("old", "old", false, 1),
            ModelEntry::new("new", "new", false, 5),
        ];
        sort_entries(&mut v);
        assert_eq!(v[0].id, "new");
    }

    #[test]
    fn name_tiebreak() {
        let mut v =
            vec![ModelEntry::new("b", "b", false, 0), ModelEntry::new("a", "a", false, 0)];
        sort_entries(&mut v);
        assert_eq!(v[0].id, "a");
    }

    #[test]
    fn deny_and_nano() {
        assert!(is_denied("m", &["m".to_string()]));
        assert!(!is_denied("m", &["other".to_string()]));
        assert!(is_nano("opencode", "gpt-5-nano-x"));
        assert!(!is_nano("anthropic", "x-nano-y"));
    }

    #[test]
    fn free_and_footer_label() {
        assert!(is_free("Free"));
        assert!(!is_free("free"));
        assert_eq!(footer_model_label("Claude"), "· Claude");
    }

    #[test]
    fn qualified_debug_form() {
        let e = ModelEntry::new("m", "M", false, 0);
        assert_eq!(e.qualified("p"), "p/m");
    }
}
