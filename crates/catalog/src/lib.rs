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

// --- CUSTOM-PROVIDER-CATALOG-RESOLVER-W1 ---
// Bounded limits for custom (project-configured) providers. These caps
// prevent unbounded retained output and limit the memory cost of resolving
// user-supplied configuration into typed structures.
pub const MAX_CUSTOM_ENV_VARS: usize = 16;
pub const MAX_CUSTOM_HEADERS: usize = 16;
pub const MAX_CUSTOM_BODY_BYTES: usize = 8 * 1024;
pub const MAX_BASE_URL_BYTES: usize = 256;
pub const MAX_NPM_LEN: usize = 128;
pub const MAX_MODEL_WIRE_ID_LEN: usize = 512;

/// Bounded options for a custom (project-configured) provider.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomProviderOptions {
    pub base_url: String,
    pub headers: BTreeMap<String, String>,
    pub body: BTreeMap<String, Value>,
}

/// Resolved bounded configuration for a custom (project-configured) provider.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomProviderConfig {
    pub id: ProviderId,
    pub name: String,
    pub npm: Option<String>,
    pub env_candidates: Vec<String>,
    pub options: CustomProviderOptions,
    pub models: BTreeMap<String, CustomModelConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomModelConfig {
    pub name: String,
    pub wire_id: String,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CustomProviderError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("type mismatch for field: {0}")]
    TypeMismatch(String),
    #[error("bound exceeded: {0}")]
    BoundExceeded(String),
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
}

impl CustomProviderConfig {
    pub fn model(&self, model_key: &str) -> Option<&CustomModelConfig> {
        self.models.get(model_key)
    }

    pub fn model_summary(&self, model_key: &str) -> Result<ModelSummary, CatalogError> {
        let model = self
            .models
            .get(model_key)
            .ok_or_else(|| CatalogError::ModelNotFound {
                provider: self.id.as_str().to_owned(),
                model: model_key.to_owned(),
            })?;
        Ok(ModelSummary {
            provider_id: self.id.clone(),
            model_id: ModelId::new(model_key.to_owned())?,
            name: model.name.clone(),
            family: None,
            reasoning: false,
            tool_call: false,
            attachment: false,
            structured_output: false,
            context_window: None,
            status: None,
        })
    }
}

fn validate_body_value(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), CustomProviderError> {
    const MAX_BODY_DEPTH: usize = 8;
    const MAX_BODY_NODES: usize = 512;
    *nodes = nodes
        .checked_add(1)
        .ok_or(CustomProviderError::BoundExceeded("body".to_string()))?;
    if depth > MAX_BODY_DEPTH {
        return Err(CustomProviderError::BoundExceeded("body".to_string()));
    }
    if *nodes > MAX_BODY_NODES {
        return Err(CustomProviderError::BoundExceeded("body".to_string()));
    }
    match value {
        Value::Object(map) => {
            if map.len() > MAX_CUSTOM_CONTAINER_ITEMS {
                return Err(CustomProviderError::BoundExceeded("body".to_string()));
            }
            for v in map.values() {
                validate_body_value(v, depth + 1, nodes)?;
            }
        }
        Value::Array(arr) => {
            if arr.len() > MAX_CUSTOM_CONTAINER_ITEMS {
                return Err(CustomProviderError::BoundExceeded("body".to_string()));
            }
            for v in arr {
                validate_body_value(v, depth + 1, nodes)?;
            }
        }
        Value::String(s) => {
            if s.len() > MAX_VALUE_BYTES {
                return Err(CustomProviderError::BoundExceeded("body value".to_string()));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

const MAX_CUSTOM_CONTAINER_ITEMS: usize = 128;
const MAX_VALUE_BYTES: usize = 512;

fn validate_config_value(value: &Value) -> Result<(), CustomProviderError> {
    let mut nodes = 0usize;
    validate_body_value(value, 0, &mut nodes)
}

pub fn resolve_custom_provider(
    provider_id: &str,
    provider_config: &Value,
) -> Result<CustomProviderConfig, CustomProviderError> {
    let validated_id = ProviderId::new(provider_id.to_owned())
        .map_err(|_| CustomProviderError::InvalidIdentifier(provider_id.to_owned()))?;

    let config_obj = provider_config
        .as_object()
        .ok_or(CustomProviderError::TypeMismatch("config".to_string()))?;

    let provider = config_obj
        .get(provider_id)
        .and_then(|v| v.as_object())
        .ok_or(CustomProviderError::MissingField("provider".to_string()))?;

    let name = provider
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(CustomProviderError::MissingField("name".to_string()))?
        .to_owned();

    let npm = match provider.get("npm") {
        None => None,
        Some(v) => {
            let s = v
                .as_str()
                .ok_or(CustomProviderError::TypeMismatch("npm".to_string()))?;
            if s.len() > MAX_NPM_LEN {
                return Err(CustomProviderError::BoundExceeded("npm".to_string()));
            }
            Some(s.to_owned())
        }
    };

    let env_candidates = match provider.get("env") {
        None => Vec::new(),
        Some(v) => {
            let arr = v
                .as_array()
                .ok_or(CustomProviderError::TypeMismatch("env".to_string()))?;
            if arr.len() > MAX_CUSTOM_ENV_VARS {
                return Err(CustomProviderError::BoundExceeded("env".to_string()));
            }
            let mut candidates = Vec::with_capacity(arr.len());
            for item in arr {
                let env_name = item
                    .as_str()
                    .ok_or(CustomProviderError::TypeMismatch("env entry".to_string()))?;
                ProviderId::new(env_name.to_owned())
                    .map_err(|_| CustomProviderError::InvalidIdentifier(env_name.to_owned()))?;
                candidates.push(env_name.to_owned());
            }
            candidates
        }
    };

    let options_obj = provider
        .get("options")
        .and_then(|v| v.as_object())
        .ok_or(CustomProviderError::MissingField("options".to_string()))?
        .clone();

    let base_url = options_obj
        .get("baseURL")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    if base_url.len() > MAX_BASE_URL_BYTES {
        return Err(CustomProviderError::BoundExceeded("baseURL".to_string()));
    }

    let headers_map = match options_obj.get("headers") {
        None => BTreeMap::new(),
        Some(v) => {
            let obj = v
                .as_object()
                .ok_or(CustomProviderError::TypeMismatch("headers".to_string()))?;
            if obj.len() > MAX_CUSTOM_HEADERS {
                return Err(CustomProviderError::BoundExceeded("headers".to_string()));
            }
            let mut map = BTreeMap::new();
            for (key, val) in obj {
                let val_str = val.as_str().ok_or(CustomProviderError::TypeMismatch(
                    "header value".to_string(),
                ))?;
                map.insert(key.clone(), val_str.to_owned());
            }
            map
        }
    };

    let body_map = match options_obj.get("body") {
        None => BTreeMap::new(),
        Some(v) => {
            let obj = v
                .as_object()
                .ok_or(CustomProviderError::TypeMismatch("body".to_string()))?;
            let mut map = BTreeMap::new();
            for (key, val) in obj {
                validate_config_value(val)?;
                map.insert(key.clone(), val.clone());
            }
            let serialized = serde_json::to_string(&map)
                .map_err(|_| CustomProviderError::TypeMismatch("body".to_string()))?;
            if serialized.len() > MAX_CUSTOM_BODY_BYTES {
                return Err(CustomProviderError::BoundExceeded("body".to_string()));
            }
            map
        }
    };

    let options = CustomProviderOptions {
        base_url,
        headers: headers_map,
        body: body_map,
    };

    let models_obj = provider
        .get("models")
        .and_then(|v| v.as_object())
        .ok_or(CustomProviderError::MissingField("models".to_string()))?;

    let mut models = BTreeMap::new();
    for (model_key, model_val) in models_obj {
        let model_obj = model_val
            .as_object()
            .ok_or(CustomProviderError::TypeMismatch("model".to_string()))?;

        let model_name = model_obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or(CustomProviderError::MissingField(
                "models[*].name".to_string(),
            ))?
            .to_owned();

        let wire_id = match model_obj.get("id") {
            Some(v) => {
                let id = v
                    .as_str()
                    .ok_or(CustomProviderError::TypeMismatch("model id".to_string()))?;
                if id.len() > MAX_MODEL_WIRE_ID_LEN {
                    return Err(CustomProviderError::BoundExceeded("model id".to_string()));
                }
                id.to_owned()
            }
            None => {
                if model_key.len() > MAX_MODEL_WIRE_ID_LEN {
                    return Err(CustomProviderError::BoundExceeded("model key".to_string()));
                }
                model_key.clone()
            }
        };

        models.insert(
            model_key.clone(),
            CustomModelConfig {
                name: model_name,
                wire_id,
            },
        );
    }

    Ok(CustomProviderConfig {
        id: validated_id,
        name,
        npm,
        env_candidates,
        options,
        models,
    })
}

impl From<CustomProviderError> for CatalogError {
    fn from(e: CustomProviderError) -> Self {
        CatalogError::Contract(opencode_rk_contracts::ContractError::InvalidIdentifier {
            kind: "custom_provider",
            value: e.to_string(),
        })
    }
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

    // --- CUSTOM-PROVIDER-CATALOG-RESOLVER-W1 tests ---

    #[test]
    fn crs_t01_resolve_valid_custom_provider() {
        let config: serde_json::Value = serde_json::from_str(
            r#"{"acme":{"name":"Acme","npm":"@ai-sdk/openai-compatible","env":["FIRST","SECOND"],"options":{"baseURL":"https://acme.test/v1","headers":{"x-custom":"h1"},"body":{"key":"val"}},"models":{"m1":{"name":"Model1","id":"wire-1"}}}}"#,
        )
        .unwrap();
        let resolved = resolve_custom_provider("acme", &config).expect("resolve valid");
        assert_eq!(resolved.id.as_str(), "acme");
        assert_eq!(resolved.name, "Acme");
        assert_eq!(resolved.npm, Some("@ai-sdk/openai-compatible".to_string()));
        assert_eq!(
            resolved.env_candidates,
            vec!["FIRST".to_string(), "SECOND".to_string()]
        );
        assert_eq!(resolved.options.base_url, "https://acme.test/v1");
        assert_eq!(resolved.options.headers.len(), 1);
        assert_eq!(resolved.options.headers.get("x-custom").unwrap(), "h1");
        assert_eq!(resolved.options.body.len(), 1);
    }

    #[test]
    fn crs_t02_model_wire_id_mapping() {
        let config: serde_json::Value = serde_json::from_str(
            r#"{"acme":{"name":"Acme","env":["KEY"],"options":{},"models":{"model-key":{"name":"Model","id":"wire-model"},"default-id":{"name":"Default"}}}}"#,
        )
        .unwrap();
        let resolved = resolve_custom_provider("acme", &config).expect("resolve");
        let wire = resolved.models.get("model-key").expect("model-key present");
        assert_eq!(wire.wire_id, "wire-model");
        let default = resolved
            .models
            .get("default-id")
            .expect("default-id present");
        assert_eq!(default.wire_id, "default-id");
    }

    #[test]
    fn crs_t03_fail_closed_malformed() {
        let config: serde_json::Value = serde_json::from_str(
            r#"{"acme":{"npm":"pkg","env":["KEY"],"options":{},"models":{}}}"#,
        )
        .unwrap();
        assert!(matches!(
            resolve_custom_provider("acme", &config),
            Err(CustomProviderError::MissingField(_))
        ));

        let config2: serde_json::Value = serde_json::from_str(r#"{"acme":"string"}"#).unwrap();
        assert!(resolve_custom_provider("acme", &config2).is_err());

        let config3: serde_json::Value =
            serde_json::from_str(r#"{"acme":{"name":123,"env":["KEY"],"options":{},"models":{}}}"#)
                .unwrap();
        assert!(resolve_custom_provider("acme", &config3).is_err());
    }

    #[test]
    fn crs_t04_bounded_env_and_headers() {
        let mut env_vec = vec!["KEY".to_string()];
        for i in 0..MAX_CUSTOM_ENV_VARS + 1 {
            env_vec.push(format!("KEY_{}", i));
        }
        let config: serde_json::Value = serde_json::json!({
            "acme": {
                "name": "Acme",
                "env": env_vec,
                "options": {
                    "headers": { "h0": "v0", "h1": "v1" },
                    "baseURL": "https://x.test"
                },
                "models": {}
            }
        });
        let resolved = resolve_custom_provider("acme", &config);
        assert!(resolved.is_err(), "excess env vars must be rejected");
    }

    #[test]
    fn crs_t05_no_secret_leak_in_summary() {
        let config: serde_json::Value = serde_json::from_str(
            r#"{"acme":{"name":"Acme","npm":"@ai-sdk/openai-compatible","env":["ACME_API_KEY"],"options":{"baseURL":"https://acme.test","headers":{"x-internal":"secret-value"},"body":{"secret_key":"sk-abc123"}},"models":{"m1":{"name":"Model","id":"wire-1"}}}}"#,
        )
        .unwrap();
        let resolved = resolve_custom_provider("acme", &config).expect("resolve");
        let summary: ModelSummary = resolved.model_summary("m1").expect("model summary");
        assert_eq!(summary.provider_id.as_str(), "acme");
        assert_eq!(summary.model_id.as_str(), "m1");
        assert_eq!(summary.name, "Model");
        assert_eq!(summary.family, None);
        assert!(!summary.reasoning);
        assert!(!summary.tool_call);
        let debug = format!("{:?}", summary);
        assert!(!debug.contains("secret-value"));
        assert!(!debug.contains("sk-abc123"));
        assert!(!debug.contains("@ai-sdk/openai-compatible"));
        assert!(!debug.contains("ACME_API_KEY"));
    }

    #[test]
    fn crs_t06_body_bounded_bytes() {
        let big_value = "x".repeat(MAX_CUSTOM_BODY_BYTES + 1);
        let config: serde_json::Value = serde_json::json!({
            "acme": {
                "name": "Acme",
                "env": ["KEY"],
                "options": {
                    "baseURL": "https://acme.test",
                    "body": { "big": big_value }
                },
                "models": {}
            }
        });
        let resolved = resolve_custom_provider("acme", &config);
        assert!(resolved.is_err(), "oversized body must be rejected");
    }
}
