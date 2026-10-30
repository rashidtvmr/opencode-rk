//! Provider/model catalog derived from models.dev-compatible JSON.
#![forbid(unsafe_code)]

pub mod cache;
pub mod config;
pub mod events;
pub mod health;
pub mod registry;
pub mod risk_graph;
pub mod search;
pub mod stats;

use opencode_rk_contracts::{ModelId, ProviderId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

const BUNDLED_PROVIDER_METADATA: &str = include_str!("assets/models-dev-providers.json");
use thiserror::Error;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelsDevProvider {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub models: BTreeMap<String, ModelsDevModel>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelsDevModel {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub attachment: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub structured_output: bool,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub open_weights: bool,
    #[serde(default)]
    pub limit: Option<ModelLimit>,
    #[serde(default)]
    pub modalities: Option<Modalities>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ModelLimit {
    #[serde(default)]
    pub context: Option<u64>,
    #[serde(default)]
    pub input: Option<u64>,
    #[serde(default)]
    pub output: Option<u64>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Modalities {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModelSummary {
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub name: String,
    pub family: Option<String>,
    pub reasoning: bool,
    pub tool_call: bool,
    pub attachment: bool,
    pub structured_output: bool,
    pub context_window: Option<u64>,
    pub status: Option<String>,
}
/// Provider discovery metadata from models.dev. Environment variables and
/// API URLs are descriptive catalog fields; they do not grant or infer auth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderSummary {
    pub id: ProviderId,
    pub name: String,
    pub environment: Vec<String>,
    pub api: Option<String>,
}

/// The selected OpenCode V2 Add Provider popular order.
pub const POPULAR_PROVIDER_IDS: &[&str] = &[
    "opencode-go", "opencode", "anthropic", "github-copilot", "openai",
    "google", "openrouter", "vercel",
];

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    providers: BTreeMap<String, ModelsDevProvider>,
}
impl Catalog {
    pub fn from_models_dev_api_json(bytes: &[u8]) -> Result<Self, CatalogError> {
        let providers: BTreeMap<String, ModelsDevProvider> = serde_json::from_slice(bytes)?;
        for (provider_key, provider) in &providers {
            ProviderId::new(provider_key.clone())?;
            for model_key in provider.models.keys() {
                ModelId::new(model_key.clone())?;
            }
        }
        Ok(Self { providers })
    }

    /// Parse a cache when it is valid, otherwise use the selected V2 bundled
    /// metadata floor. This makes first-run, missing-cache and corrupt-cache
    /// installs show the complete provider roster without inventing auth.
    pub fn from_cache_or_bundled(cache: Option<&[u8]>) -> Result<Self, CatalogError> {
        let bundled = Self::from_models_dev_api_json(BUNDLED_PROVIDER_METADATA.as_bytes())?;
        let Some(bytes) = cache else { return Ok(bundled); };
        let Ok(cached) = Self::from_models_dev_api_json(bytes) else { return Ok(bundled); };
        Ok(Self::merge_catalog_floor(&bundled, &cached))
    }

    /// Overlay a syntactically valid cache onto the complete bundled floor by
    /// resolved provider ID. Empty/partial caches therefore cannot hide seeded
    /// providers; cached metadata wins only for the matching provider.
    fn merge_catalog_floor(floor: &Self, overlay: &Self) -> Self {
        let mut providers = floor.providers.clone();
        let mut overlay_by_id = BTreeMap::new();
        for (key, provider) in &overlay.providers {
            let resolved = provider.id.as_deref().and_then(|id| ProviderId::new(id.to_owned()).ok())
                .or_else(|| ProviderId::new(key.clone()).ok());
            if let Some(id) = resolved {
                overlay_by_id.entry(id.to_string()).or_insert((key, provider));
            }
        }
        let mut floor_keys = BTreeMap::new();
        for (key, provider) in &floor.providers {
            let resolved = provider.id.as_deref().and_then(|id| ProviderId::new(id.to_owned()).ok())
                .or_else(|| ProviderId::new(key.clone()).ok());
            if let Some(id) = resolved { floor_keys.insert(id.to_string(), key.clone()); }
        }
        for (id, (key, provider)) in overlay_by_id {
            if let Some(floor_key) = floor_keys.get(&id) {
                providers.insert(floor_key.clone(), provider.clone());
            } else {
                providers.insert(key.clone(), provider.clone());
            }
        }
        Self { providers }
    }

    /// Parse the bundled provider metadata floor directly.
    pub fn bundled() -> Result<Self, CatalogError> {
        Self::from_models_dev_api_json(BUNDLED_PROVIDER_METADATA.as_bytes())
    }

    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
    #[must_use]
    pub fn model_count(&self) -> usize {
        self.providers.values().map(|p| p.models.len()).sum()
    }
    /// Return all provider entries, including providers with zero models.
    /// Explicit source `provider.id` wins over the JSON map key when valid;
    /// the map key is the source-backed fallback when it is absent or invalid.
    pub fn providers(&self) -> Vec<ProviderSummary> {
        let mut seen = HashSet::new();
        self.providers.iter().filter_map(|(key, provider)| {
            let id = provider.id.as_deref()
                .and_then(|id| ProviderId::new(id.to_owned()).ok())
                .or_else(|| ProviderId::new(key.clone()).ok())?;
            if !seen.insert(id.clone()) { return None; }
            Some(ProviderSummary {
                id,
                name: provider.name.clone(),
                environment: provider.env.clone(),
                api: provider.api.clone(),
            })
        }).collect()
    }

    /// Popular providers retain upstream order; all others sort by display
    /// name and ID for stable rendering independent of map insertion order.
    pub fn ordered_providers(&self) -> Vec<ProviderSummary> {
        let mut rows = self.providers();
        rows.sort_by(|a, b| {
            let rank = |id: &str| POPULAR_PROVIDER_IDS.iter().position(|item| *item == id);
            match (rank(a.id.as_str()), rank(b.id.as_str())) {
                (Some(left), Some(right)) => left.cmp(&right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)),
            }
        });
        rows
    }

    /// Search provider IDs and display names, preserving provider ordering.
    pub fn search_providers(&self, query: Option<&str>) -> Vec<ProviderSummary> {
        let needle = query.unwrap_or("").trim().to_lowercase();
        self.ordered_providers().into_iter().filter(|provider| {
            needle.is_empty()
                || provider.id.as_str().to_lowercase().contains(&needle)
                || provider.name.to_lowercase().contains(&needle)
        }).collect()
    }

    /// Merge catalog metadata into explicit/configured rows. Existing rows win
    /// for URL and name; catalog rows add missing IDs only. Duplicate explicit
    /// IDs are retained once, in deterministic first-seen order.
    pub fn merge_provider_rows(
        catalog: &[ProviderSummary],
        existing: &[ProviderSummary],
    ) -> Vec<ProviderSummary> {
        let mut rows = Vec::with_capacity(existing.len() + catalog.len());
        let mut seen = HashSet::new();
        // Existing configured/daemon rows win, including their URL and name.
        for provider in existing.iter().chain(catalog.iter()) {
            if seen.insert(provider.id.clone()) { rows.push(provider.clone()); }
        }
        rows
    }

    pub fn model(&self, provider: &str, model: &str) -> Result<ModelSummary, CatalogError> {
        let p = self
            .providers
            .get(provider)
            .ok_or_else(|| CatalogError::ProviderNotFound(provider.to_owned()))?;
        let m = p
            .models
            .get(model)
            .ok_or_else(|| CatalogError::ModelNotFound {
                provider: provider.to_owned(),
                model: model.to_owned(),
            })?;
        summarize(provider, model, m)
    }
    pub fn search(&self, q: &CatalogQuery) -> Result<Vec<ModelSummary>, CatalogError> {
        let needle = q.text.as_deref().unwrap_or("").trim().to_ascii_lowercase();
        let mut results = Vec::new();
        for (provider_id, provider) in &self.providers {
            if let Some(filter) = q.provider.as_deref() {
                if provider_id != filter {
                    continue;
                }
            }
            for (model_id, model) in &provider.models {
                if q.reasoning == Some(true) && !model.reasoning {
                    continue;
                }
                if q.tools == Some(true) && !model.tool_call {
                    continue;
                }
                if q.attachments == Some(true) && !model.attachment {
                    continue;
                }
                if q.structured_output == Some(true) && !model.structured_output {
                    continue;
                }
                if !needle.is_empty() {
                    let family = model.family.as_deref().unwrap_or("");
                    let searchable = format!("{provider_id} {model_id} {} {family}", model.name)
                        .to_ascii_lowercase();
                    if !searchable.contains(&needle) {
                        continue;
                    }
                }
                results.push(summarize(provider_id, model_id, model)?);
                if results.len() >= q.limit.clamp(1, 500) {
                    return Ok(results);
                }
            }
        }
        Ok(results)
    }
}
#[derive(Clone, Debug)]
pub struct CatalogQuery {
    pub text: Option<String>,
    pub provider: Option<String>,
    pub reasoning: Option<bool>,
    pub tools: Option<bool>,
    pub attachments: Option<bool>,
    pub structured_output: Option<bool>,
    pub limit: usize,
}
impl Default for CatalogQuery {
    fn default() -> Self {
        Self {
            text: None,
            provider: None,
            reasoning: None,
            tools: None,
            attachments: None,
            structured_output: None,
            limit: 100,
        }
    }
}
fn summarize(
    provider: &str,
    model: &str,
    r: &ModelsDevModel,
) -> Result<ModelSummary, CatalogError> {
    Ok(ModelSummary {
        provider_id: ProviderId::new(provider.to_owned())?,
        model_id: ModelId::new(model.to_owned())?,
        name: r.name.clone(),
        family: r.family.clone(),
        reasoning: r.reasoning,
        tool_call: r.tool_call,
        attachment: r.attachment,
        structured_output: r.structured_output,
        context_window: r.limit.as_ref().and_then(|l| l.context),
        status: r.status.clone(),
    })
}
#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("invalid models.dev JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid catalog contract: {0}")]
    Contract(#[from] opencode_rk_contracts::ContractError),
    #[error("provider not found: {0}")]
    ProviderNotFound(String),
    #[error("model not found: {provider}/{model}")]
    ModelNotFound { provider: String, model: String },
}
#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{"openai":{"name":"OpenAI","models":{"gpt-x":{"name":"GPT X","family":"gpt","reasoning":true,"tool_call":true,"attachment":true,"structured_output":true,"limit":{"context":200000}}}},"deepseek":{"name":"DeepSeek","models":{"deepseek-code":{"name":"DeepSeek Code","tool_call":true}}}}"#;

    fn row(id: &str, name: &str, api: Option<&str>) -> ProviderSummary {
        ProviderSummary { id: ProviderId::new(id).unwrap(), name: name.into(), environment: vec![], api: api.map(str::to_owned) }
    }

    #[test]
    fn parses() {
        let c = Catalog::from_models_dev_api_json(FIXTURE.as_bytes()).unwrap();
        assert_eq!(c.provider_count(), 2);
        assert_eq!(c.model_count(), 2);
        assert_eq!(c.model("openai", "gpt-x").unwrap().context_window, Some(200000));
    }

    #[test]
    fn capability_filter() {
        let c = Catalog::from_models_dev_api_json(FIXTURE.as_bytes()).unwrap();
        let r = c.search(&CatalogQuery { reasoning: Some(true), tools: Some(true), ..CatalogQuery::default() }).unwrap();
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn fresh_missing_and_corrupt_cache_use_complete_bundled_floor() {
        let fresh = Catalog::from_cache_or_bundled(None).unwrap();
        let corrupt = Catalog::from_cache_or_bundled(Some(b"not-json")).unwrap();
        assert_eq!(fresh.provider_count(), 232);
        assert_eq!(corrupt.provider_count(), 232);
        assert!(fresh.providers().iter().any(|provider| provider.id.as_str() == "qvac"));
    }

    #[test]
    fn valid_partial_and_empty_cache_keep_complete_bundled_floor() {
        let partial = br#"{"qvac":{"name":"Cached QVAC","models":{}}}"#;
        let empty = br#"{}"#;
        let floor = Catalog::bundled().unwrap();
        let partial_catalog = Catalog::from_cache_or_bundled(Some(partial)).unwrap();
        let empty_catalog = Catalog::from_cache_or_bundled(Some(empty)).unwrap();
        assert_eq!(floor.provider_count(), 232);
        assert_eq!(partial_catalog.provider_count(), 232);
        assert_eq!(empty_catalog.provider_count(), 232);
        assert_eq!(partial_catalog.providers().iter().find(|p| p.id.as_str() == "qvac").unwrap().name, "Cached QVAC");
    }

    #[test]
    fn explicit_id_wins_and_invalid_id_falls_back_to_map_key() {
        let json = r#"{"map-key":{"id":"source-id","name":"Source","models":{}},"fallback":{"id":"bad/id","name":"Fallback","models":{}}}"#;
        let ids: Vec<_> = Catalog::from_models_dev_api_json(json.as_bytes()).unwrap().providers().into_iter().map(|p| p.id.to_string()).collect();
        assert_eq!(ids, vec!["fallback", "source-id"]);
    }

    #[test]
    fn duplicate_explicit_ids_are_emitted_once_deterministically() {
        let json = r#"{"a":{"id":"same","name":"A","models":{}},"b":{"id":"same","name":"B","models":{}}}"#;
        let rows = Catalog::from_models_dev_api_json(json.as_bytes()).unwrap().providers();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
    }

    #[test]
    fn popular_order_unicode_search_and_url_precedence() {
        let json = r#"{"vercel":{"name":"Vercel","models":{}},"openai":{"name":"OpenAI","models":{}},"zeta":{"name":"Zeta","models":{}},"alpha":{"name":"Alpha","models":{}},"opencode":{"name":"OpenCode","models":{}},"jp":{"name":"日本語プロバイダ","models":{}}}"#;
        let c = Catalog::from_models_dev_api_json(json.as_bytes()).unwrap();
        let ids: Vec<_> = c.ordered_providers().into_iter().map(|p| p.id.to_string()).collect();
        // Rust's standard ordering is intentionally Unicode-scalar
        // deterministic. The upstream browser uses localeCompare, whose
        // result depends on the active Intl locale and is not a portable Rust
        // contract; do not encode a locale-specific browser expectation here.
        assert_eq!(ids, vec!["opencode", "openai", "vercel", "alpha", "zeta", "jp"]);
        assert_eq!(c.search_providers(Some("日本語"))[0].id.as_str(), "jp");
        let catalog = vec![row("openai", "Catalog OpenAI", Some("https://catalog")), row("new", "New", Some("https://new"))];
        let existing = vec![row("openai", "Configured OpenAI", Some("https://configured"))];
        let merged = Catalog::merge_provider_rows(&catalog, &existing);
        let existing_with_duplicate = vec![existing[0].clone(), existing[0].clone()];
        let merged = Catalog::merge_provider_rows(&catalog, &existing_with_duplicate);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].api.as_deref(), Some("https://configured"));
        assert_eq!(merged[0].name, "Configured OpenAI");
    }
}
