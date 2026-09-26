//! Scoped KV store + project/args context.
//!
//! Mirrors `packages/tui/src/context/kv.tsx:51-56` (flat `get`/`set` over a
//! single store, persisted to `kv.json`), `args.tsx:3-11` (`Args` option bag),
//! `project.tsx:19,85-88` (root = `instance.path.directory`, falls back to
//! `sdk.directory`), `directory.ts:11-16` (`abbreviateHome` + `:branch`),
//! `path-format.tsx:15-24` (`formatPath`: in-base -> relative, else
//! `abbreviateHome`), `runtime.tsx:3-9` (`abbreviateHome`) @ a0d9b6c.
//!
//! Divergences: TS store is unbounded/unscoped (single global namespace, no
//! per-scope isolation to mirror); this port bounds entries/keys/values
//! fail-closed. TS `Args` has no `query` field; `query` added per lane spec
//! alongside the TS-faithful optionals.

/// Max entries kept. `set` past this fails instead of growing.
pub const MAX_KV: usize = 256;
/// Max key chars kept.
pub const MAX_KEY_LEN: usize = 128;
/// Max value chars kept.
pub const MAX_VALUE_LEN: usize = 4096;

/// Fail-closed bound violations from [`KvStore::set`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvError {
    TooManyEntries,
    KeyTooLong,
    ValueTooLong,
}

/// Flat string KV store (TS `kv.tsx` has one global namespace, no scopes).
#[derive(Debug, Clone, Default)]
pub struct KvStore {
    entries: Vec<(String, String)>,
}

impl KvStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// TS `kv.tsx:51-53`: `store[key] ?? defaultValue`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// TS `kv.tsx:54`: upsert. New keys past [`MAX_KV`] fail; overlong
    /// key/value fails. Returns the previous value, if any.
    pub fn set(&mut self, key: &str, value: &str) -> Result<Option<String>, KvError> {
        if key.chars().count() > MAX_KEY_LEN {
            return Err(KvError::KeyTooLong);
        }
        if value.chars().count() > MAX_VALUE_LEN {
            return Err(KvError::ValueTooLong);
        }
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| k == key) {
            let prev = std::mem::replace(&mut slot.1, value.to_string());
            return Ok(Some(prev));
        }
        if self.entries.len() >= MAX_KV {
            return Err(KvError::TooManyEntries);
        }
        self.entries.push((key.to_string(), value.to_string()));
        Ok(None)
    }

    /// Not in TS (store only grows via `set`); needed so bounded tests and
    /// callers can evict without wiping the store.
    pub fn delete(&mut self, key: &str) -> Option<String> {
        self.entries
            .iter()
            .position(|(k, _)| k == key)
            .map(|i| self.entries.remove(i).1)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// TS `runtime.tsx:3-9` `abbreviateHome`.
#[must_use]
pub fn tildefy(input: &str, home: &str) -> String {
    if home.is_empty() {
        return input.to_string();
    }
    match relative_to(home, input) {
        None => input.to_string(),
        Some(rel) if rel.is_empty() => "~".to_string(),
        Some(rel) if rel == ".." || rel.starts_with("../") || rel.starts_with('/') => {
            input.to_string()
        }
        Some(rel) => format!("~/{rel}"),
    }
}

/// `path.relative(home, input)` for `/`-separated absolute paths.
/// `None` when either side is not absolute (TS then returns `input`).
fn relative_to(base: &str, path: &str) -> Option<String> {
    if !base.starts_with('/') || !path.starts_with('/') {
        return None;
    }
    let mut base_parts = base.split('/').filter(|s| !s.is_empty());
    let mut path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    for b in base_parts.by_ref() {
        if path_parts.is_empty() || path_parts[0] != b {
            let mut rel = vec![".."];
            rel.extend(std::iter::once(b));
            rel.extend(base_parts);
            rel.extend(path_parts);
            return Some(rel.join("/"));
        }
        path_parts.remove(0);
    }
    if path_parts.is_empty() {
        return Some(String::new());
    }
    Some(path_parts.join("/"))
}

/// Project root context (`project.tsx:19,85-88`).
#[derive(Debug, Clone)]
pub struct Project {
    pub root: String,
    pub home: String,
}

impl Project {
    #[must_use]
    pub fn new(root: &str, home: &str) -> Self {
        Self {
            root: root.to_string(),
            home: home.to_string(),
        }
    }

    /// TS `path-format.tsx:15-24`: empty -> `""`; relative input resolves
    /// against root; equal -> `"."`; under root -> relative;
    /// else `abbreviateHome` (TS `directory.ts:12-14` adds `:branch`, which
    /// lives in sync state and is out of scope here).
    #[must_use]
    pub fn display(&self, path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        let absolute = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{}/{}", self.root.trim_end_matches('/'), path)
        };
        match relative_to(&self.root, &absolute) {
            Some(rel) if rel.is_empty() => ".".to_string(),
            Some(rel) if rel != ".." && !rel.starts_with("../") && !rel.starts_with('/') => rel,
            _ => tildefy(&absolute, &self.home),
        }
    }
}

/// CLI args context (`args.tsx:3-11`) plus lane-spec `query`.
#[derive(Debug, Clone, Default)]
pub struct Args {
    pub query: Vec<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub prompt: Option<String>,
    pub continue_session: bool,
    pub session_id: Option<String>,
    pub fork: bool,
    pub auto: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_roundtrip() {
        let mut kv = KvStore::new();
        assert_eq!(kv.set("a", "1"), Ok(None));
        assert_eq!(kv.get("a"), Some("1"));
        assert_eq!(kv.set("a", "2"), Ok(Some("1".to_string())));
        assert_eq!(kv.get("a"), Some("2"));
    }

    #[test]
    fn missing_key_none() {
        let kv = KvStore::new();
        assert_eq!(kv.get("nope"), None);
    }

    #[test]
    fn delete_removes() {
        let mut kv = KvStore::new();
        kv.set("a", "1").unwrap();
        assert_eq!(kv.delete("a"), Some("1".to_string()));
        assert_eq!(kv.get("a"), None);
        assert_eq!(kv.delete("a"), None);
    }

    #[test]
    fn overflow_errs() {
        let mut kv = KvStore::new();
        for i in 0..MAX_KV {
            kv.set(&format!("k{i}"), "v").unwrap();
        }
        assert_eq!(kv.set("one-more", "v"), Err(KvError::TooManyEntries));
    }

    #[test]
    fn key_value_bounds_err() {
        let mut kv = KvStore::new();
        let long_key = "k".repeat(MAX_KEY_LEN + 1);
        let long_val = "v".repeat(MAX_VALUE_LEN + 1);
        assert_eq!(kv.set(&long_key, "v"), Err(KvError::KeyTooLong));
        assert_eq!(kv.set("k", &long_val), Err(KvError::ValueTooLong));
        assert!(kv.is_empty());
    }

    #[test]
    fn tilde_render() {
        assert_eq!(tildefy("/home/u/a/b", "/home/u"), "~/a/b");
        assert_eq!(tildefy("/home/u", "/home/u"), "~");
        assert_eq!(tildefy("/etc/x", "/home/u"), "/etc/x");
        assert_eq!(tildefy("/home/u", ""), "/home/u");
    }

    #[test]
    fn display_relative_and_dot() {
        let p = Project::new("/repo", "/home/u");
        assert_eq!(p.display("/repo/src/main.rs"), "src/main.rs");
        assert_eq!(p.display("/repo"), ".");
        assert_eq!(p.display("src/lib.rs"), "src/lib.rs");
        assert_eq!(p.display(""), "");
    }

    #[test]
    fn display_outside_root_tildefies() {
        let p = Project::new("/repo", "/home/u");
        assert_eq!(p.display("/home/u/notes.md"), "~/notes.md");
        assert_eq!(p.display("/etc/hosts"), "/etc/hosts");
    }
}
