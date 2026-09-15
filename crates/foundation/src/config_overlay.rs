//! OPS-005 pure location-scoped config overlay merge.
//!
//! Merges caller-supplied config documents (`Global < Project < Local`)
//! into one effective snapshot. Pure: no file discovery, no reads, no
//! policy-service mutation, no cache retention, no I/O, no clock, no
//! globals. Deterministic: same doc bytes yield identical key order and
//! winners. Higher scope wins per key; output keys sorted by key bytes.
//! Error paths carry key names only, never value bytes.

#![forbid(unsafe_code)]

/// Maximum key length in bytes (UTF-8 length is used; keys must be non-empty).
pub const MAX_KEY_BYTES: usize = 256;
/// Maximum value length in bytes.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
/// Maximum total entries retained across all docs.
pub const MAX_ENTRIES: usize = 4096;

/// Specificity rank: [`Scope::Global`] < [`Scope::Project`] < [`Scope::Local`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Global,
    Project,
    Local,
}

/// One caller-supplied config document at a fixed scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigDoc {
    pub scope: Scope,
    pub keys: Vec<(String, String)>,
}

/// Merged snapshot: keys sorted by key bytes plus per-key winning scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub keys: Vec<(String, String)>,
    pub winner: Vec<(String, Scope)>,
}

impl EffectiveConfig {
    /// Look up the merged value for `key`. Keys are sorted so this is a
    /// bounded binary search; no map is retained beyond the returned vecs.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys
            .binary_search_by(|(k, _)| k.as_str().cmp(key))
            .ok()
            .map(|i| self.keys[i].1.as_str())
    }
}

/// Overlay failures. Unit variants carry neither key names nor value
/// bytes, so `Display`/`Debug` can never leak secret material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayError {
    /// Empty docs slice; no empty config is returned.
    NoDocs,
    /// Duplicate key inside one doc; no partial merge is returned.
    DupKey,
    /// Empty/whitespace-only key, over-long key (`> 256` bytes) or
    /// over-long value (`> 64 KiB`).
    BadEntry,
    /// Total entries `> 4096`.
    TooMany,
}

impl std::fmt::Display for OverlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDocs => write!(f, "no config documents"),
            Self::DupKey => write!(f, "duplicate key in document"),
            Self::BadEntry => write!(f, "invalid config entry"),
            Self::TooMany => write!(f, "too many entries"),
        }
    }
}

impl std::error::Error for OverlayError {}

fn check_entry(key: &str, value: &str) -> Result<(), OverlayError> {
    if key.is_empty()
        || key.trim().is_empty()
        || key.len() > MAX_KEY_BYTES
        || value.len() > MAX_VALUE_BYTES
    {
        return Err(OverlayError::BadEntry);
    }
    Ok(())
}

/// Merge `docs` into one effective snapshot.
///
/// Order-independent: explicit [`Scope`] decides the winner per key, not
/// input position, so shuffled doc order yields identical winners.
/// Output keys are sorted by key bytes. Rejects before retention: caps
/// and duplicates fail with no partial merge returned.
pub fn merge_overlay(docs: &[ConfigDoc]) -> Result<EffectiveConfig, OverlayError> {
    if docs.is_empty() {
        return Err(OverlayError::NoDocs);
    }
    let total: usize = docs.iter().map(|d| d.keys.len()).sum();
    if total > MAX_ENTRIES {
        return Err(OverlayError::TooMany);
    }
    // Best (scope, value) per key. Insertion order is irrelevant: the
    // highest scope always wins, ties keep the first value seen.
    let mut best: Vec<(String, Scope, String)> = Vec::new();
    for doc in docs {
        let mut seen: Vec<&str> = Vec::with_capacity(doc.keys.len());
        for (k, v) in &doc.keys {
            if seen.contains(&k.as_str()) {
                return Err(OverlayError::DupKey);
            }
            seen.push(k.as_str());
            check_entry(k, v)?;
            if best.len() >= MAX_ENTRIES && !best.iter().any(|slot| slot.0 == *k) {
                return Err(OverlayError::TooMany);
            }
            match best.iter_mut().find(|(ek, _, _)| ek == k) {
                Some(slot) if doc.scope > slot.1 => {
                    slot.1 = doc.scope;
                    slot.2 = v.clone();
                }
                Some(_) => {}
                None => best.push((k.clone(), doc.scope, v.clone())),
            }
        }
    }
    best.sort_by(|a, b| a.0.cmp(&b.0));
    let keys = best
        .iter()
        .map(|(k, _, v)| (k.clone(), v.clone()))
        .collect();
    let winner = best.into_iter().map(|(k, s, _)| (k, s)).collect();
    Ok(EffectiveConfig { keys, winner })
}
