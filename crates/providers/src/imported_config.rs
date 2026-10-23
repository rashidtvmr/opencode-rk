//! Native owned configuration: path resolution and provider loading.
//!
//! The native owned config dir is resolved (first match wins):
//!   `OPENCODE_RK_CONFIG_DIR` -> `XDG_CONFIG_HOME/opencode-rk` -> `HOME/.config/opencode-rk`
//! The canonical config file is `opencode.json` in that dir.
//!
//! This module reads the own config file as an *additional* normalized layer
//! of providers, merged with `custom_provider::list()` (the existing
//! `custom-providers.json` source). Project overrides are preserved by the
//! existing precedence logic in `configured_commands.rs`.

use std::io::Read;
use std::path::{Path, PathBuf};

use reqwest::Url;
use serde_json::{json, Map, Value};
use thiserror::Error;

/// Maximum bytes accepted for the own config file.
const MAX_OWN_CONFIG_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of providers in the own config.
const MAX_PROVIDERS: usize = 128;

/// Maximum model IDs per provider.
const MAX_MODELS_PER_PROVIDER: usize = 128;

#[derive(Debug, Error)]
pub enum ImportedConfigError {
    #[error("invalid provider configuration: {0}")]
    Invalid(String),
    #[error("provider configuration could not be read")]
    Io,
    #[error("invalid JSON in own config file")]
    Json,
    #[error("own config exceeds size limit")]
    TooLarge,
}

/// A normalized provider entry from the own config file.
///
/// Canonical shape (matches `foundation/src/config_import/adapters.rs`
/// `normalize_provider` output):
/// - `id`: provider identifier string
/// - `name`: human-readable name (optional, defaults to `id`)
/// - `base_url`: validated URL string
/// - `api_key_env`: environment variable name for the API key (optional)
/// - `buffered_model_ids`: list of model IDs (optional, defaults to empty)
/// - `models`: optional map of model IDs (not loaded into the runtime config
///   but recorded for future use; see notes)
#[derive(Clone, Debug)]
pub struct ImportedProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub buffered_model_ids: Vec<String>,
    /// Preserved from the source if present; not consumed by the runtime
    /// provider config (which uses `api_key_env`).
    pub models: Value,
}

/// Resolve the native owned config directory.
///
/// Precedence:
/// 1. `OPENCODE_RK_CONFIG_DIR` (if set and absolute)
/// 2. `$XDG_CONFIG_HOME/opencode-rk` (if `XDG_CONFIG_HOME` set and absolute)
/// 3. `$HOME/.config/opencode-rk` (if `HOME` set)
///
/// Returns `None` if no environment variable is available.
pub fn own_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OPENCODE_RK_CONFIG_DIR") {
        let path = PathBuf::from(dir);
        if path.is_absolute() && !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg);
        if path.is_absolute() && !path.as_os_str().is_empty() {
            return Some(path.join("opencode-rk"));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home);
        if path.is_absolute() && !path.as_os_str().is_empty() {
            return Some(path.join(".config/opencode-rk"));
        }
    }
    None
}

/// Returns the own config file path `opencode.json` in the resolved dir,
/// or `None` if the dir cannot be resolved.
pub fn own_config_path() -> Option<PathBuf> {
    own_config_dir().map(|dir| dir.join("opencode.json"))
}

/// Read the own config file and return the raw `providers` map as `Value`s.
///
/// The file is parsed as JSON (JSONC is not supported for the own config file;
/// use strict JSON). Returns an error on malformed JSON or invalid structure.
/// An absent file yields an empty map.
fn read_own_providers(path: &Path) -> Result<Map<String, Value>, ImportedConfigError> {
    let mut bytes = Vec::new();
    match std::fs::File::open(path) {
        Ok(file) => {
            file.take((MAX_OWN_CONFIG_BYTES as u64) + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| ImportedConfigError::Io)?;
            if bytes.len() > MAX_OWN_CONFIG_BYTES {
                return Err(ImportedConfigError::TooLarge);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Map::new());
        }
        Err(_) => return Err(ImportedConfigError::Io),
    }
    let root: Value = serde_json::from_slice(&bytes).map_err(|_| ImportedConfigError::Json)?;
    let root_map = root
        .as_object()
        .ok_or_else(|| ImportedConfigError::Invalid("own config root must be an object".into()))?;
    // The own config is an OpenCode-shaped document. Providers live under
    // `providers` (or the singular `provider` key, matching V2 parity).
    // If the key exists but is not an object, that is a malformed config:
    // return an explicit error rather than silently swallowing it.
    let providers = match root_map.get("providers") {
        Some(v) if v.is_object() => v.as_object().unwrap().clone(),
        Some(v) if v.is_null() => Map::new(),
        Some(_) => {
            return Err(ImportedConfigError::Invalid(
                "own config: \"providers\" must be an object".into(),
            ));
        }
        None => match root_map.get("provider") {
            Some(v) if v.is_object() => v.as_object().unwrap().clone(),
            Some(v) if v.is_null() => Map::new(),
            Some(_) => {
                return Err(ImportedConfigError::Invalid(
                    "own config: \"provider\" must be an object".into(),
                ));
            }
            None => Map::new(),
        },
    };
    Ok(providers)
}

/// Validate an imported provider ID: same rules as `custom_provider::valid_id`.
pub fn valid_imported_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id != "openai"
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && id.as_bytes()[0].is_ascii_alphanumeric()
}

/// Validate a base URL: HTTPS or loopback HTTP, no credentials/query/fragment.
pub fn validate_imported_base_url(raw: &str) -> Result<(), ImportedConfigError> {
    if raw.len() > 2048 {
        return Err(ImportedConfigError::Invalid("base URL is too long".into()));
    }
    let url = Url::parse(raw).map_err(|_| {
        ImportedConfigError::Invalid("invalid base URL".into())
    })?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if url.host_str().is_none()
        || !(url.scheme() == "https" || url.scheme() == "http" && local)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ImportedConfigError::Invalid(
            "use HTTPS or loopback HTTP without URL credentials, query or fragment".into(),
        ));
    }
    Ok(())
}

/// Validate a model ID list.
fn validate_imported_model_ids(models: &[String]) -> Result<(), ImportedConfigError> {
    if models.len() > 128 {
        return Err(ImportedConfigError::Invalid(
            "too many models in imported provider".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for id in models {
        if id.is_empty()
            || id.len() > 256
            || id.chars().any(|c| c.is_control())
            || !seen.insert(id.clone())
        {
            return Err(ImportedConfigError::Invalid(
                "buffered_model_ids must contain at most 128 distinct nonempty model IDs of at most 256 bytes".into(),
            ));
        }
    }
    Ok(())
}

/// Load and decode all providers from the own config file.
///
/// This is the "imported providers layer" that `custom_provider::list()` and
/// `custom_provider::load()` merge on top of the existing `custom-providers.json`.
///
/// Returns an empty vector when the own config file is absent. Returns an
/// explicit error on malformed config (does not silently ignore).
pub fn load_imported_providers() -> Result<Vec<ImportedProviderConfig>, ImportedConfigError> {
    let path = match own_config_path() {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };
    decode_imported_providers(&path)
}

fn decode_imported_providers(path: &Path) -> Result<Vec<ImportedProviderConfig>, ImportedConfigError> {
    let providers = read_own_providers(path)?;
    if providers.len() > MAX_PROVIDERS {
        return Err(ImportedConfigError::Invalid(format!(
            "too many providers: {} > {}",
            providers.len(),
            MAX_PROVIDERS
        )));
    }
    let mut configs = Vec::with_capacity(providers.len());
    for (id, value) in &providers {
        // The built-in `openai` provider is handled natively; importing it
        // would conflict with the builtin path. Report it as not consumed
        // rather than failing the entire import plan.
        if id == "openai" {
            continue;
        }
        if !valid_imported_id(id) {
            return Err(ImportedConfigError::Invalid(format!(
                "invalid provider id: {id}"
            )));
        }
        let obj = value
            .as_object()
            .ok_or_else(|| ImportedConfigError::Invalid(format!("provider {id} must be an object")))?;
        let base_url = obj
            .get("base_url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ImportedConfigError::Invalid(format!(
                    "provider {id} missing base_url"
                ))
            })?;
        validate_imported_base_url(base_url)?;
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .to_owned();
        if name.is_empty() || name.len() > 256 || name.chars().any(|c| c.is_control()) {
            return Err(ImportedConfigError::Invalid(format!(
                "invalid provider name for {id}"
            )));
        }
        let api_key_env = obj
            .get("api_key_env")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned());
        let buffered_model_ids = obj
            .get("buffered_model_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        validate_imported_model_ids(&buffered_model_ids)?;
        let models = obj
            .get("models")
            .cloned()
            .unwrap_or(Value::Null);
        configs.push(ImportedProviderConfig {
            id: id.clone(),
            name,
            base_url: base_url.to_owned(),
            api_key_env,
            buffered_model_ids,
            models,
        });
    }
    Ok(configs)
}

/// Build a normalized `Value::Object` for a provider, matching the canonical
/// shape from `foundation/src/config_import/adapters.rs::normalize_provider`.
pub fn to_normalized_value(config: &ImportedProviderConfig) -> Value {
    let mut provider = Map::new();
    provider.insert("id".into(), Value::String(config.id.clone()));
    provider.insert("name".into(), Value::String(config.name.clone()));
    provider.insert("base_url".into(), Value::String(config.base_url.clone()));
    if let Some(env) = &config.api_key_env {
        provider.insert("api_key_env".into(), Value::String(env.clone()));
    }
    provider.insert(
        "buffered_model_ids".into(),
        Value::Array(
            config
                .buffered_model_ids
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        ),
    );
    if !config.models.is_null() {
        provider.insert("models".into(), config.models.clone());
    }
    Value::Object(provider)
}

pub fn configured_models_for(provider_id: &str) -> Result<Vec<(String, String)>, ImportedConfigError> {
    let Some(config) = load_imported_providers()?.into_iter().find(|p| p.id == provider_id) else { return Ok(Vec::new()); };
    let Some(models) = config.models.as_object() else {
        return if config.models.is_array() { configured_model_array(&config.models) } else { Ok(Vec::new()) };
    };
    let mut result = Vec::new();
    for (id, definition) in models.iter().take(MAX_MODELS_PER_PROVIDER) {
        if id.is_empty() || id.len() > 256 || id.chars().any(|c| c.is_control()) { continue; }
        let name = definition.as_object().and_then(|obj| obj.get("name")).and_then(Value::as_str).unwrap_or(id);
        if name.is_empty() || name.len() > 256 || name.chars().any(|c| c.is_control()) { continue; }
        if result.iter().any(|(existing, _)| existing == id) { continue; }
        result.push((id.clone(), name.to_owned()));
    }
    Ok(result)
}

fn configured_model_array(models: &Value) -> Result<Vec<(String, String)>, ImportedConfigError> {
    let mut result = Vec::new();
    for value in models.as_array().into_iter().flatten().take(MAX_MODELS_PER_PROVIDER) {
        let Some(id) = value.as_str() else { continue; };
        if id.is_empty() || id.len() > 256 || id.chars().any(|c| c.is_control()) { continue; }
        if result.iter().any(|(existing, _)| existing == id) { continue; }
        result.push((id.to_owned(), id.to_owned()));
    }
    Ok(result)
}

/// Resolve the API key for an imported provider.
///
/// Resolution order (same as `custom_provider::load`):
/// 1. Environment variable named by `api_key_env` (if set)
/// 2. Persisted auth for this provider id (via `persisted_auth::api_key`)
pub async fn resolve_api_key(config: &ImportedProviderConfig) -> Option<String> {
    if let Some(env_name) = &config.api_key_env {
        if let Ok(value) = std::env::var(env_name) {
            if !value.trim().is_empty() && value.len() <= 16 * 1024 {
                return Some(value);
            }
        }
    }
    crate::persisted_auth::api_key(&config.id, &format!("{}_API_KEY", config.id.to_uppercase()))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids_are_accepted() {
        assert!(valid_imported_id("9router"));
        assert!(valid_imported_id("my-provider-1"));
        assert!(valid_imported_id("a"));
    }

    #[test]
    fn invalid_ids_are_rejected() {
        assert!(!valid_imported_id(""));
        assert!(!valid_imported_id("OpenAI"));
        assert!(!valid_imported_id("openai"));
        assert!(!valid_imported_id(" provider"));
        assert!(!valid_imported_id("a".repeat(129)));
    }

    #[test]
    fn base_url_validation_accepts_safe_urls() {
        assert!(validate_imported_base_url("https://api.example.com/v1").is_ok());
        assert!(validate_imported_base_url("http://localhost:8080/v1").is_ok());
        assert!(validate_imported_base_url("http://127.0.0.1:1234/v1").is_ok());
    }

    #[test]
    fn base_url_validation_rejects_unsafe_urls() {
        assert!(validate_imported_base_url("http://example.org/v1").is_err());
        // credentials
        assert!(validate_imported_base_url("https://key@example.org/v1").is_err());
        // query
        assert!(validate_imported_base_url("https://example.org/v1?key=x").is_err());
        // fragment
        assert!(validate_imported_base_url("https://example.org/v1#frag").is_err());
        // non-http(s)
        assert!(validate_imported_base_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn to_normalized_value_has_canonical_shape() {
        let config = ImportedProviderConfig {
            id: "my-provider".into(),
            name: "My Provider".into(),
            base_url: "https://api.example.com/v1".into(),
            api_key_env: Some("MY_PROVIDER_API_KEY".into()),
            buffered_model_ids: vec!["model-a".into(), "model-b".into()],
            models: json!(null),
        };
        let value = to_normalized_value(&config);
        assert_eq!(value["id"], "my-provider");
        assert_eq!(value["name"], "My Provider");
        assert_eq!(value["base_url"], "https://api.example.com/v1");
        assert_eq!(value["api_key_env"], "MY_PROVIDER_API_KEY");
        assert_eq!(
            value["buffered_model_ids"][0],
            json!("model-a")
        );
        assert_eq!(value["buffered_model_ids"].as_array().unwrap().len(), 2);
    }
}
