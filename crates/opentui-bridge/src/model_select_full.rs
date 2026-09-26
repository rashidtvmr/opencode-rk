#![forbid(unsafe_code)]
//! Local model/provider selection (MISSING).
//!
//! Evidence (`model_dialog.rs:8`): recency list only (`recentModels`
//! prepend/dedupe/cap 10); no persisted local provider/model selection,
//! flags static. This type is the bounded std-only selection record.

/// Max chars for provider id (truncate, never reject).
pub const MAX_PROVIDER: usize = 64;
/// Max chars for model id (truncate, never reject).
pub const MAX_MODEL: usize = 64;
/// Max chars for combined `provider/model` label.
pub const MAX_LABEL: usize = 128;

/// Bounded local selection: `provider` + `model`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSel {
    pub provider: String,
    pub model: String,
}

impl ModelSel {
    #[must_use]
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: truncate(provider, MAX_PROVIDER),
            model: truncate(model, MAX_MODEL),
        }
    }

    /// Combined `provider/model` label, char-safe truncated to 128.
    #[must_use]
    pub fn label(&self) -> String {
        truncate(&format!("{}/{}", self.provider, self.model), MAX_LABEL)
    }
}

/// Valid selection: both non-empty and within 64-char caps.
#[must_use]
pub fn select_ok(provider: &str, model: &str) -> bool {
    !provider.is_empty()
        && !model.is_empty()
        && provider.chars().count() <= MAX_PROVIDER
        && model.chars().count() <= MAX_MODEL
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_requires_both_non_empty() {
        assert!(select_ok("p", "m"));
        assert!(!select_ok("", "m"));
        assert!(!select_ok("p", ""));
        assert!(!select_ok("", ""));
    }

    #[test]
    fn ok_rejects_over_cap() {
        let long = "x".repeat(65);
        assert!(!select_ok(&long, "m"));
        assert!(!select_ok("p", &long));
        assert!(select_ok(&"x".repeat(64), &"y".repeat(64)));
    }

    #[test]
    fn new_truncates_to_64() {
        let s = ModelSel::new(&"p".repeat(100), &"m".repeat(100));
        assert_eq!(s.provider.chars().count(), MAX_PROVIDER);
        assert_eq!(s.model.chars().count(), MAX_MODEL);
    }

    #[test]
    fn label_form_and_cap() {
        let s = ModelSel::new("p", "m");
        assert_eq!(s.label(), "p/m");
        let s = ModelSel::new(&"p".repeat(64), &"m".repeat(64));
        assert_eq!(s.label().chars().count(), MAX_LABEL);
    }

    #[test]
    fn label_char_safe() {
        let s = ModelSel::new("héllo", "wörld");
        assert_eq!(s.label(), "héllo/wörld");
    }
}
