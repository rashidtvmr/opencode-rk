//! Provider/model catalog derived from models.dev-compatible JSON.
#![forbid(unsafe_code)]

pub mod cache;
pub mod events;
pub mod registry;
pub mod search;
pub mod stats;

use opencode_rk_contracts::{ModelId, ProviderId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
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
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
    #[must_use]
    pub fn model_count(&self) -> usize {
        self.providers.values().map(|p| p.models.len()).sum()
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
    #[test]
    fn parses() {
        let c = Catalog::from_models_dev_api_json(FIXTURE.as_bytes()).unwrap();
        assert_eq!(c.provider_count(), 2);
        assert_eq!(c.model_count(), 2);
        assert_eq!(
            c.model("openai", "gpt-x").unwrap().context_window,
            Some(200000)
        );
    }
    #[test]
    fn capability_filter() {
        let c = Catalog::from_models_dev_api_json(FIXTURE.as_bytes()).unwrap();
        let r = c
            .search(&CatalogQuery {
                reasoning: Some(true),
                tools: Some(true),
                ..CatalogQuery::default()
            })
            .unwrap();
        assert_eq!(r.len(), 1);
    }
}
