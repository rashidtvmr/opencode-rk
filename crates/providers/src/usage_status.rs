//! Bounded, deterministic provider usage and rate-limit status.
//!
//! The caller owns provider registration and store lifetime. This module does
//! not read a clock, persist data, perform I/O, or retain request payloads.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

/// Maximum number of usage entries retained by one store.
pub const MAX_USAGE_ENTRIES: usize = 256;
/// Maximum input plus output tokens accepted by one record operation.
pub const MAX_DELTA_TOKENS: u64 = 10_000_000;
/// Maximum number of provider identifiers retained by one store.
pub const MAX_USAGE_PROVIDERS: usize = 256;

/// Explicit identity of one provider/model/authentication-source bucket.
///
/// `auth_source` is an identifier such as `api-key` or `oauth`; credential
/// material is not accepted by this type's API or retained by the store.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UsageKey {
    pub provider_id: String,
    pub model: String,
    pub auth_source: String,
}

impl<'de> serde::Deserialize<'de> for UsageKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Fields {
            provider_id: String,
            model: String,
            auth_source: String,
        }

        let fields = Fields::deserialize(deserializer)?;
        Self::new(fields.provider_id, fields.model, fields.auth_source)
            .map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Debug for UsageKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsageKey")
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .field("auth_source", &self.auth_source)
            .finish()
    }
}

impl UsageKey {
    /// Construct a key, rejecting an empty identity segment.
    pub fn new(
        provider_id: impl Into<String>,
        model: impl Into<String>,
        auth_source: impl Into<String>,
    ) -> Result<Self, UsageError> {
        let key = Self {
            provider_id: provider_id.into(),
            model: model.into(),
            auth_source: auth_source.into(),
        };
        key.validate()?;
        Ok(key)
    }

    fn validate(&self) -> Result<(), UsageError> {
        if self.provider_id.is_empty() || self.model.is_empty() || self.auth_source.is_empty() {
            return Err(UsageError::EmptyField);
        }
        Ok(())
    }
}

/// Additive usage counters plus provider-reported rate-limit metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UsageCounters {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: Option<u64>,
    pub rate_limit_reset_ms: Option<u64>,
}

/// Additive per-call usage plus optional provider metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UsageDelta {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: Option<u64>,
    pub rate_limit_reset_ms: Option<u64>,
}

/// One snapshot row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UsageEntry {
    pub key: UsageKey,
    pub counters: UsageCounters,
}

/// Bounded status projection. Entries are sorted by provider, model, then auth
/// source. `truncated` is retained for forward-compatible snapshot caps.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UsageSnapshot {
    pub entries: Vec<UsageEntry>,
    pub truncated: bool,
}

/// Alias useful to status-panel callers.
pub type UsageStatus = UsageSnapshot;

/// Usage-store failures. Variants carry no caller-provided text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
pub enum UsageError {
    #[error("usage identity field is empty")]
    EmptyField,
    #[error("provider is not registered")]
    UnknownProvider,
    #[error("usage entry capacity exceeded")]
    Overflow,
    #[error("usage delta exceeds token cap")]
    DeltaTooLarge,
}

/// Caller-owned bounded usage store.
///
/// A provider must be registered before a record for it is accepted. Existing
/// entries update in place even when the entry cap has been reached. A failed
/// operation leaves both provider and usage state unchanged.
#[derive(Clone, Debug, Default)]
pub struct UsageStore {
    providers: BTreeSet<String>,
    entries: BTreeMap<UsageKey, UsageCounters>,
}

impl UsageStore {
    /// Create an empty store with no registered providers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a store with caller-supplied registered providers.
    pub fn with_providers<I, S>(providers: I) -> Result<Self, UsageError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut store = Self::new();
        for provider in providers {
            store.register_provider(provider)?;
        }
        Ok(store)
    }

    /// Create a store from the provider routing registry without retaining
    /// provider configuration, URLs, environment names, or credentials.
    pub fn from_registry(registry: &crate::registry::ProviderRegistry) -> Result<Self, UsageError> {
        Self::with_providers(
            registry
                .list_by_priority()
                .into_iter()
                .map(|provider| provider.id.as_str()),
        )
    }

    /// Register one provider identity. Duplicate registration is harmless.
    pub fn register_provider(&mut self, provider_id: impl Into<String>) -> Result<(), UsageError> {
        let provider_id = provider_id.into();
        if provider_id.is_empty() {
            return Err(UsageError::EmptyField);
        }
        if self.providers.contains(&provider_id) {
            return Ok(());
        }
        if self.providers.len() >= MAX_USAGE_PROVIDERS {
            return Err(UsageError::Overflow);
        }
        self.providers.insert(provider_id);
        Ok(())
    }

    /// Alias for callers that model provider registration as an add operation.
    pub fn add_provider(&mut self, provider_id: impl Into<String>) -> Result<(), UsageError> {
        self.register_provider(provider_id)
    }

    /// Returns registered provider ids in deterministic order.
    #[must_use]
    pub fn providers(&self) -> Vec<String> {
        self.providers.iter().cloned().collect()
    }

    /// Number of retained usage entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no usage entries are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Record one bounded delta.
    pub fn record<K, D>(&mut self, key: K, delta: D) -> Result<(), UsageError>
    where
        K: Into<UsageKey>,
        D: Into<UsageDelta>,
    {
        record(self, key, delta)
    }

    /// Build a bounded deterministic snapshot.
    #[must_use]
    pub fn snapshot(&self) -> UsageSnapshot {
        snapshot(self)
    }

    /// Remove one key. Unknown keys are harmless no-ops.
    pub fn reset<K: Into<UsageKey>>(&mut self, key: K) {
        reset(self, key)
    }

    /// Clear all retained usage while keeping the provider registry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Read one retained counter set without exposing mutable store state.
    #[must_use]
    pub fn get<K: Into<UsageKey>>(&self, key: K) -> Option<&UsageCounters> {
        let key = key.into();
        self.entries.get(&key)
    }
}

/// Record `delta` for `key` in `store`.
pub fn record<K, D>(store: &mut UsageStore, key: K, delta: D) -> Result<(), UsageError>
where
    K: Into<UsageKey>,
    D: Into<UsageDelta>,
{
    let key = key.into();
    let delta = delta.into();

    key.validate()?;
    if !store.providers.contains(&key.provider_id) {
        return Err(UsageError::UnknownProvider);
    }
    if delta.input_tokens > MAX_DELTA_TOKENS
        || delta.output_tokens > MAX_DELTA_TOKENS
        || delta
            .input_tokens
            .checked_add(delta.output_tokens)
            .is_none_or(|tokens| tokens > MAX_DELTA_TOKENS)
    {
        return Err(UsageError::DeltaTooLarge);
    }

    if !store.entries.contains_key(&key) && store.entries.len() >= MAX_USAGE_ENTRIES {
        return Err(UsageError::Overflow);
    }

    let counters = store.entries.entry(key).or_default();
    counters.requests = counters.requests.saturating_add(delta.requests);
    counters.input_tokens = counters.input_tokens.saturating_add(delta.input_tokens);
    counters.output_tokens = counters.output_tokens.saturating_add(delta.output_tokens);
    if let Some(cost) = delta.cost_micros {
        counters.cost_micros = Some(
            counters
                .cost_micros
                .unwrap_or_default()
                .saturating_add(cost),
        );
    }
    if let Some(reset_ms) = delta.rate_limit_reset_ms {
        counters.rate_limit_reset_ms = Some(reset_ms);
    }
    Ok(())
}

/// Return a deterministic snapshot capped at [`MAX_USAGE_ENTRIES`].
#[must_use]
pub fn snapshot(store: &UsageStore) -> UsageSnapshot {
    let truncated = store.entries.len() > MAX_USAGE_ENTRIES;
    let entries = store
        .entries
        .iter()
        .take(MAX_USAGE_ENTRIES)
        .map(|(key, counters)| UsageEntry {
            key: key.clone(),
            counters: counters.clone(),
        })
        .collect();
    UsageSnapshot { entries, truncated }
}

/// Remove one key. Unknown or malformed keys do not affect the store.
pub fn reset<K: Into<UsageKey>>(store: &mut UsageStore, key: K) {
    let key = key.into();
    store.entries.remove(&key);
}
