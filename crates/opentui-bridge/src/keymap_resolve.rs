#![forbid(unsafe_code)]
//! Resolve a key sequence through the active mode path.
//!
//! The OpenTUI keymap exposes mode-sensitive layers and binding flags. This
//! module keeps the resolver std-only and caller-owned: `Binding` is a small
//! normalized row, while `stack` is a snapshot ordered from base to newest.
//! The newest mode is searched first. A fallthrough match is retained while
//! parent modes are searched; a terminating match replaces it.

/// Base mode used by the OpenTUI mode stack.
pub const BASE_MODE: &str = "base";

/// A key sequence and the mode requested by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyQuery {
    pub seq: String,
    pub mode: String,
}

impl KeyQuery {
    #[must_use]
    pub fn new(seq: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            seq: seq.into(),
            mode: mode.into(),
        }
    }
}

/// One normalized mode-scoped binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub mode: String,
    pub seq: String,
    pub action: String,
    pub prevent_default: bool,
    pub fallthrough: bool,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            mode: BASE_MODE.to_string(),
            seq: String::new(),
            action: String::new(),
            prevent_default: true,
            fallthrough: false,
        }
    }
}

impl Binding {
    #[must_use]
    pub fn new(mode: impl Into<String>, seq: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            mode: mode.into(),
            seq: seq.into(),
            action: action.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_prevent_default(mut self, value: bool) -> Self {
        self.prevent_default = value;
        self
    }

    #[must_use]
    pub fn with_fallthrough(mut self, value: bool) -> Self {
        self.fallthrough = value;
        self
    }
}

/// Result of resolving one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveOutcome {
    pub action: Option<String>,
    pub prevent_default: bool,
    pub fallthrough: bool,
}

impl ResolveOutcome {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            action: None,
            prevent_default: false,
            fallthrough: false,
        }
    }
}

/// Resolve `query` against `tables` using the base-to-newest `stack`.
#[must_use]
pub fn resolve(query: &KeyQuery, tables: &[Binding], stack: &[String]) -> ResolveOutcome {
    let seq = query.seq.trim();
    let query_mode = query.mode.trim();
    let mut fallthrough_match: Option<&Binding> = None;

    if !seq.is_empty() && !query_mode.is_empty() {
        if let Some(binding) = find_binding(tables, query_mode, seq) {
            if !binding.fallthrough {
                return outcome(binding);
            }
            fallthrough_match = Some(binding);
        }
    }

    for mode in stack.iter().rev() {
        let mode = mode.trim();
        if mode.is_empty() || mode == query_mode {
            continue;
        }
        let Some(binding) = find_binding(tables, mode, seq) else {
            continue;
        };
        if !binding.fallthrough {
            return outcome(binding);
        }
        fallthrough_match = Some(binding);
    }

    fallthrough_match
        .map(outcome)
        .unwrap_or_else(ResolveOutcome::none)
}

fn find_binding<'a>(tables: &'a [Binding], mode: &str, seq: &str) -> Option<&'a Binding> {
    tables
        .iter()
        .find(|binding| binding.mode.trim() == mode && binding.seq.trim() == seq)
}

fn outcome(binding: &Binding) -> ResolveOutcome {
    ResolveOutcome {
        action: Some(binding.action.clone()),
        prevent_default: binding.prevent_default,
        fallthrough: binding.fallthrough,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn direct_hit() {
        let binding = Binding::new("dialog", "x", "dialog.close");
        let outcome = resolve(
            &KeyQuery::new("x", "dialog"),
            &[binding],
            &modes(&["base", "dialog"]),
        );
        assert_eq!(outcome.action.as_deref(), Some("dialog.close"));
        assert!(outcome.prevent_default);
        assert!(!outcome.fallthrough);
    }

    #[test]
    fn fallthrough_walks_to_parent() {
        let child = Binding::new("dialog", "x", "dialog.child").with_fallthrough(true);
        let parent = Binding::new("base", "x", "base.parent");
        let outcome = resolve(
            &KeyQuery::new("x", "dialog"),
            &[child, parent],
            &modes(&["base", "dialog"]),
        );
        assert_eq!(outcome.action.as_deref(), Some("base.parent"));
        assert!(outcome.prevent_default);
        assert!(!outcome.fallthrough);
    }

    #[test]
    fn no_match_is_none() {
        let outcome = resolve(
            &KeyQuery::new("missing", "dialog"),
            &[Binding::new("base", "x", "base.x")],
            &modes(&["base", "dialog"]),
        );
        assert_eq!(outcome, ResolveOutcome::none());
    }

    #[test]
    fn input_paste_keeps_prevent_default_false() {
        let outcome = resolve(
            &KeyQuery::new("ctrl+v", "base"),
            &[Binding::new("base", "ctrl+v", "prompt.paste").with_prevent_default(false)],
            &modes(&["base"]),
        );
        assert_eq!(outcome.action.as_deref(), Some("prompt.paste"));
        assert!(!outcome.prevent_default);
        assert!(!outcome.fallthrough);
    }

    #[test]
    fn newest_mode_wins_in_lifo_order() {
        let parent = Binding::new("parent", "x", "parent.action");
        let child = Binding::new("child", "x", "child.action");
        let outcome = resolve(
            &KeyQuery::new("x", "child"),
            &[parent, child],
            &modes(&["base", "parent", "child"]),
        );
        assert_eq!(outcome.action.as_deref(), Some("child.action"));
    }
}
