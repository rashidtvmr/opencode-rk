#![forbid(unsafe_code)]
//! Model ref parse + bounded provider/model index.
//!
//! Evidence (TS checkout a0d9b6c; task notes divergence from 95daf90):
//! - `packages/tui/src/util/model.ts:3-6` `parse` splits on `/`,
//!   provider = first segment, model = rest joined with `/`.
//! - `packages/tui/src/util/model.ts:8-10` `index` builds id->provider map.
//! - `packages/tui/src/util/model.ts:12-20` `get` resolves provider then
//!   `provider.models[modelID]`; `name` (`:22-28`) falls back to modelID.
//! - `packages/opencode/src/session/prompt.ts:607` qualified form
//!   `${providerID}/${modelID}` for "Model not found" messages.
//!
//! Bound: `MAX_MODELS` caps total stored models (insert fail-closed false).

use std::collections::HashMap;

/// Max total models held by [`ModelIndex`].
pub const MAX_MODELS: usize = 512;

/// Split-first-slash model reference (`provider/model...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

impl ModelRef {
    /// TS `parse`: always succeeds; no `/` yields empty model.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value.find('/') {
            Some(i) => Self {
                provider: value[..i].to_string(),
                model: value[i + 1..].to_string(),
            },
            None => Self { provider: value.to_string(), model: String::new() },
        }
    }

    /// Qualified `provider/model` form (prompt.ts:607).
    #[must_use]
    pub fn qualified(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

/// Bounded provider -> (model -> display name) index.
#[derive(Debug, Default)]
pub struct ModelIndex {
    providers: HashMap<String, HashMap<String, String>>,
    len: usize,
}

impl ModelIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert model; false + no-op when full and key is new.
    pub fn insert(&mut self, provider: &str, model: &str, name: &str) -> bool {
        if let Some(models) = self.providers.get_mut(provider) {
            if models.contains_key(model) {
                models.insert(model.to_string(), name.to_string());
                return true;
            }
            if self.len >= MAX_MODELS {
                return false;
            }
            models.insert(model.to_string(), name.to_string());
            self.len += 1;
            return true;
        }
        if self.len >= MAX_MODELS {
            return false;
        }
        let mut models = HashMap::new();
        models.insert(model.to_string(), name.to_string());
        self.providers.insert(provider.to_string(), models);
        self.len += 1;
        true
    }

    /// TS `get`: provider miss or model miss yields None.
    #[must_use]
    pub fn get(&self, provider: &str, model: &str) -> Option<&str> {
        self.providers.get(provider)?.get(model).map(String::as_str)
    }

    /// TS `name`: display name or modelID fallback.
    /// Returns owned-or-borrowed: caller model str on miss, so returns
    /// `&str` tied to the longest-lived input via explicit lifetimes.
    #[must_use]
    pub fn name<'a>(&'a self, provider: &str, model: &'a str) -> &'a str {
        self.get(provider, model).unwrap_or(model)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_first_slash() {
        assert_eq!(
            ModelRef::parse("anthropic/claude"),
            ModelRef { provider: "anthropic".into(), model: "claude".into() }
        );
    }

    #[test]
    fn parse_joins_extra_slashes() {
        assert_eq!(ModelRef::parse("a/b/c").model, "b/c");
        assert_eq!(ModelRef::parse("solo").model, "");
    }

    #[test]
    fn qualified_roundtrip() {
        let r = ModelRef::parse("p/m/x");
        assert_eq!(r.qualified(), "p/m/x");
    }

    #[test]
    fn index_insert_get_name_fallback() {
        let mut idx = ModelIndex::new();
        assert!(idx.insert("p", "m", "Pretty"));
        assert_eq!(idx.get("p", "m"), Some("Pretty"));
        assert_eq!(idx.name("p", "missing"), "missing");
        assert_eq!(idx.get("nope", "m"), None);
    }

    #[test]
    fn index_bounded_fail_closed() {
        let mut idx = ModelIndex::new();
        for i in 0..MAX_MODELS {
            assert!(idx.insert("p", &format!("m{i}"), "n"));
        }
        assert!(!idx.insert("p", "overflow", "n"));
        assert!(idx.insert("p", "m0", "updated"));
        assert_eq!(idx.len(), MAX_MODELS);
    }
}
