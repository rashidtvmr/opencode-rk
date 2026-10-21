//! Durable, bounded custom OpenAI-compatible provider configuration.
use std::{collections::BTreeSet, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, OnceLock}, time::Duration};
use reqwest::{redirect::Policy, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const MAX_MODELS_BODY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_MODELS: usize = 5000;
const MAX_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
pub struct CustomProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub buffered_model_ids: Vec<String>,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

#[derive(Debug, Error)]
pub enum CustomProviderError {
    #[error("invalid provider configuration: {0}")] Invalid(String),
    #[error("provider request failed: {0}")] Request(String),
    #[error("provider response exceeded {0} bytes")] TooLarge(usize),
    #[error("provider returned HTTP {0}")] Http(u16),
    #[error("provider returned invalid JSON")] Json,
    #[error("provider configuration could not be read or written")] Io,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id != "openai"
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && id.as_bytes()[0].is_ascii_alphanumeric()
}

pub fn validate_base_url(raw: &str) -> Result<Url, CustomProviderError> {
    if raw.len() > 2048 { return Err(CustomProviderError::Invalid("base URL is too long".into())); }
    let url = Url::parse(raw).map_err(|_| CustomProviderError::Invalid("invalid base URL".into()))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]" | "::1"));
    if url.host_str().is_none() || !(url.scheme() == "https" || url.scheme() == "http" && local)
        || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err(CustomProviderError::Invalid("use HTTPS or loopback HTTP without URL credentials, query or fragment".into()));
    }
    Ok(url)
}

pub fn config_path() -> Option<PathBuf> {
    let root = std::env::var_os("OPENCODE_CONFIG_DIR").map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).map(|p| p.join("opencode")))
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from).map(|p| p.join(".config/opencode")))?;
    root.is_absolute().then(|| root.join("custom-providers.json"))
}

fn read_root(path: &Path) -> Result<Value, CustomProviderError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => return Err(CustomProviderError::Io),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({"providers":{}})),
        Err(_) => return Err(CustomProviderError::Io),
        _ => {}
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|_| CustomProviderError::Io)?.take(MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes).map_err(|_| CustomProviderError::Io)?;
    if bytes.len() > MAX_CONFIG_BYTES { return Err(CustomProviderError::TooLarge(MAX_CONFIG_BYTES)); }
    let root: Value = serde_json::from_slice(&bytes).map_err(|_| CustomProviderError::Json)?;
    if !root.is_object() || !root["providers"].is_object() { return Err(CustomProviderError::Invalid("providers must be an object".into())); }
    Ok(root)
}

fn decode_configs(root: &Value) -> Result<Vec<CustomProviderConfig>, CustomProviderError> {
    let providers = root["providers"].as_object().ok_or(CustomProviderError::Json)?;
    if providers.len() > 128 { return Err(CustomProviderError::Invalid("too many providers".into())); }
    providers.iter().map(|(id, value)| {
        let base_url = value["base_url"].as_str().ok_or(CustomProviderError::Json)?;
        let name = value["name"].as_str().unwrap_or(id);
        if !valid_id(id) || name.len() > 256 || name.chars().any(char::is_control) { return Err(CustomProviderError::Invalid("invalid provider id or name".into())); }
        validate_base_url(base_url)?;
        let buffered_model_ids = value.get("buffered_model_ids").cloned().map(serde_json::from_value::<Vec<String>>).transpose().map_err(|_| CustomProviderError::Invalid("buffered_model_ids must be an array of model IDs".into()))?.unwrap_or_default();
        validate_buffered_models(&buffered_model_ids)?;
        Ok(CustomProviderConfig { id: id.clone(), name: name.into(), base_url: base_url.into(), api_key: None, buffered_model_ids })
    }).collect()
}

pub async fn list() -> Result<Vec<CustomProviderConfig>, CustomProviderError> {
    let path = config_path().ok_or(CustomProviderError::Io)?;
    tokio::task::spawn_blocking(move || decode_configs(&read_root(&path)?)).await.map_err(|_| CustomProviderError::Io)?
}

pub async fn load(id: &str) -> Result<Option<CustomProviderConfig>, CustomProviderError> {
    if !valid_id(id) { return Err(CustomProviderError::Invalid("invalid provider id".into())); }
    let mut config = list().await?.into_iter().find(|config| config.id == id);
    if let Some(config) = &mut config {
        config.api_key = crate::persisted_auth::api_key(id, &format!("{}_API_KEY", id.to_uppercase())).await;
    }
    Ok(config)
}

pub async fn save(config: CustomProviderConfig) -> Result<(), CustomProviderError> {
    save_options(config, false).await
}

/// UI registration updates that omit capabilities retain the on-disk mode
/// under the same writer lock as the update (no read/update race).
pub async fn save_preserving_buffered_models(config: CustomProviderConfig) -> Result<(), CustomProviderError> {
    save_options(config, true).await
}

async fn save_options(config: CustomProviderConfig, preserve_buffered: bool) -> Result<(), CustomProviderError> {
    if !valid_id(&config.id) || config.name.is_empty() || config.name.len() > 256 || config.name.chars().any(char::is_control)
        || config.api_key.as_ref().is_some_and(|key| key.len() > 16 * 1024 || key.chars().any(char::is_control)) {
        return Err(CustomProviderError::Invalid("invalid provider id, name or credential".into()));
    }
    validate_base_url(&config.base_url)?;
    validate_buffered_models(&config.buffered_model_ids)?;
    let path = config_path().ok_or(CustomProviderError::Io)?;
    static WRITER: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = WRITER.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1))).clone().acquire_owned().await.map_err(|_| CustomProviderError::Io)?;
    tokio::task::spawn_blocking(move || { let _permit = permit; save_at_with_mode(&path, &config, preserve_buffered) }).await.map_err(|_| CustomProviderError::Io)?
}

fn save_at(path: &Path, config: &CustomProviderConfig) -> Result<(), CustomProviderError> {
    save_at_with_mode(path, config, false)
}

fn save_at_with_mode(path: &Path, config: &CustomProviderConfig, preserve_buffered: bool) -> Result<(), CustomProviderError> {
    validate_buffered_models(&config.buffered_model_ids)?;
    let mut root = read_root(path)?;
    let providers = root["providers"].as_object_mut().ok_or(CustomProviderError::Json)?;
    if providers.len() >= 128 && !providers.contains_key(&config.id) { return Err(CustomProviderError::Invalid("too many providers".into())); }
    // Preserve additional provider settings and unrelated entries.
    let item = providers.entry(config.id.clone()).or_insert_with(|| json!({})).as_object_mut().ok_or(CustomProviderError::Json)?;
    item.insert("name".into(), json!(config.name));
    item.insert("base_url".into(), json!(config.base_url));
    if !preserve_buffered || !item.contains_key("buffered_model_ids") {
        item.insert("buffered_model_ids".into(), json!(config.buffered_model_ids));
    }
    let bytes = serde_json::to_vec_pretty(&root).map_err(|_| CustomProviderError::Json)?;
    if bytes.len() > MAX_CONFIG_BYTES { return Err(CustomProviderError::TooLarge(MAX_CONFIG_BYTES)); }
    let parent = path.parent().ok_or(CustomProviderError::Io)?;
    std::fs::create_dir_all(parent).map_err(|_| CustomProviderError::Io)?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file = options.open(&temporary).map_err(|_| CustomProviderError::Io)?;
    let result = (|| { file.write_all(&bytes)?; file.sync_all()?; std::fs::rename(&temporary, path) })();
    if result.is_err() { let _ = std::fs::remove_file(&temporary); }
    result.map_err(|_| CustomProviderError::Io)
}

fn validate_buffered_models(models: &[String]) -> Result<(), CustomProviderError> {
    let mut seen = BTreeSet::new();
    if models.len() > 128 || models.iter().any(|id| id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) || !seen.insert(id)) {
        return Err(CustomProviderError::Invalid("buffered_model_ids must contain at most 128 distinct nonempty model IDs of at most 256 bytes".into()));
    }
    Ok(())
}

fn parse_models(bytes: &[u8]) -> Result<Vec<(String, String)>, CustomProviderError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| CustomProviderError::Json)?;
    let rows = value["data"].as_array().ok_or(CustomProviderError::Json)?;
    if rows.len() > MAX_MODELS { return Err(CustomProviderError::Invalid("model list exceeds 5000 models".into())); }
    let mut seen = BTreeSet::new();
    rows.iter().map(|row| {
        let id = row["id"].as_str().ok_or(CustomProviderError::Json)?;
        let name = row["name"].as_str().unwrap_or(id);
        if id.is_empty() || id.len() > 256 || name.len() > 256 || id.chars().chain(name.chars()).any(char::is_control) { return Err(CustomProviderError::Invalid("invalid model identifier or name".into())); }
        Ok((id.to_owned(), name.to_owned()))
    }).filter(|row: &Result<(String,String), CustomProviderError>| match row { Ok((id,_)) => seen.insert(id.clone()), Err(_) => true }).collect()
}

pub async fn discover_models(config: &CustomProviderConfig) -> Result<Vec<(String,String)>, CustomProviderError> {
    let base = validate_base_url(&config.base_url)?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(12)).redirect(Policy::none()).no_proxy().build().map_err(|_| CustomProviderError::Request("unable to initialize HTTP client".into()))?;
    let mut request = client.get(format!("{}/models", base.as_str().trim_end_matches('/')));
    if config.id == "9router" { request = request.header("x-9r-internal-models-fetch", "1"); }
    if let Some(key) = config.api_key.as_deref().filter(|key| !key.trim().is_empty()) { request = request.bearer_auth(key); }
    let mut response = request.send().await.map_err(|_| CustomProviderError::Request("connection failed or timed out".into()))?;
    if !response.status().is_success() { return Err(CustomProviderError::Http(response.status().as_u16())); }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| CustomProviderError::Request("response interrupted".into()))? {
        if bytes.len() + chunk.len() > MAX_MODELS_BODY_BYTES { return Err(CustomProviderError::TooLarge(MAX_MODELS_BODY_BYTES)); }
        bytes.extend_from_slice(&chunk);
    }
    parse_models(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn url_rejects_credentials_redirect_targets_and_remote_http() {
        for url in ["http://example.org/v1", "https://key@example.org/v1", "http://localhost:20128/v1?key=secret", "file:///etc/passwd"] { assert!(validate_base_url(url).is_err()); }
        assert!(validate_base_url("http://localhost:20128/v1").is_ok());
    }
    #[test] fn full_router_catalogue_and_slash_ids_are_preserved() {
        let data: Vec<_> = (0..1159).map(|index| json!({"id":format!("route/model-{index}")})).collect();
        let models = parse_models(&serde_json::to_vec(&json!({"data":data})).unwrap()).unwrap();
        assert_eq!(models.len(), 1159); assert_eq!(models[1158].0, "route/model-1158");
    }
    #[test] fn malformed_model_catalogue_is_not_empty_success() { assert!(parse_models(b"{\"error\":\"unauthorized\"}").is_err()); }
    #[test] fn persistence_preserves_settings_and_excludes_credentials() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("custom.json");
        std::fs::write(&path, br#"{"other":true,"providers":{"keep":{"name":"Keep","base_url":"https://example.org/v1"},"9router":{"extra":true}}}"#).unwrap();
        save_at(&path, &CustomProviderConfig { id:"9router".into(), name:"9router".into(), base_url:"http://localhost:20128/v1".into(), api_key:Some("fixture-key".into()), buffered_model_ids:Vec::new() }).unwrap();
        let root = read_root(&path).unwrap(); assert_eq!(root["other"], true); assert_eq!(root["providers"]["9router"]["extra"], true); assert!(root["providers"]["keep"].is_object());
        assert!(!std::fs::read_to_string(path).unwrap().contains("fixture-key"));
    }
    #[test] fn buffered_mode_persists_and_omitted_update_preserves_it() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("custom.json");
        let mut config = CustomProviderConfig { id:"9router".into(),name:"Router".into(),base_url:"http://localhost:20128/v1".into(),api_key:None,buffered_model_ids:vec!["oc/deepseek-v4.1-flash-free".into()] };
        save_at(&path,&config).unwrap();
        assert_eq!(decode_configs(&read_root(&path).unwrap()).unwrap()[0].buffered_model_ids,config.buffered_model_ids);
        config.name = "Renamed".into(); config.buffered_model_ids.clear();
        save_at_with_mode(&path,&config,true).unwrap();
        let loaded = decode_configs(&read_root(&path).unwrap()).unwrap();
        assert_eq!(loaded[0].name,"Renamed");
        assert_eq!(loaded[0].buffered_model_ids,vec!["oc/deepseek-v4.1-flash-free"]);
        save_at(&path,&config).unwrap();
        assert!(decode_configs(&read_root(&path).unwrap()).unwrap()[0].buffered_model_ids.is_empty());
    }
    #[test] fn buffered_mode_defaults_empty_and_rejects_malformed_or_unbounded_ids() {
        let mut root = json!({"providers":{"9router":{"base_url":"http://localhost:20128/v1"}}});
        assert!(decode_configs(&root).unwrap()[0].buffered_model_ids.is_empty());
        for invalid in [json!(true),json!([""]),json!(["x","x"]),json!(["secret\n"]),json!(["x".repeat(257)]),json!((0..129).map(|index|format!("m{index}")).collect::<Vec<_>>())] {
            root["providers"]["9router"]["buffered_model_ids"] = invalid;
            assert!(decode_configs(&root).is_err());
        }
    }
}
