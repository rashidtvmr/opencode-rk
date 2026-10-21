//! Catalogue-backed native model choices and upstream-compatible model recents.
use serde_json::{json, Value};
use std::{
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

const MAX_MODELS: usize = 5000;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_RECENT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Model {
    pub id: String,
    pub name: String,
    pub provider_id: String,
}

impl Model {
    pub fn qualified(&self) -> String { format!("{}/{}", self.provider_id, self.id) }
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty() || self.id.to_lowercase().contains(&query)
            || self.name.to_lowercase().contains(&query)
            || self.qualified().to_lowercase().contains(&query)
    }
}

/// Rows are catalogue indices, never arbitrary query text.
pub(super) fn visible_rows(models: &[Model], query: &str) -> Vec<usize> {
    models.iter().enumerate().filter_map(|(index, model)| model.matches(query).then_some(index)).collect()
}
pub(super) fn clamp_row(row: usize, visible_len: usize) -> usize { row.min(visible_len.saturating_sub(1)) }
pub(super) fn move_row(row: usize, visible_len: usize, down: bool) -> usize {
    if visible_len == 0 { return 0; }
    if down { row.saturating_add(1).min(visible_len - 1) } else { row.saturating_sub(1) }
}
pub(super) fn selected_row(models: &[Model], query: &str, row: usize) -> Option<Model> {
    let rows = visible_rows(models, query);
    rows.get(clamp_row(row, rows.len())).and_then(|index| models.get(*index)).cloned()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Dialog {
    None,
    Provider,
    ProviderId,
    BaseUrl,
    ApiKey,
    Model,
}

pub(super) fn catalogue(body: &str) -> Result<Vec<Model>, String> {
    if body.len() > 8 * 1024 * 1024 { return Err("model catalogue is too large".into()); }
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid model catalogue")?;
    let entries = value["models"]
        .as_array()
        .ok_or("missing model catalogue")?;
    if entries.len() > MAX_MODELS { return Err("model catalogue exceeds 5000-model limit".into()); }
    let mut models = Vec::new();
    for entry in entries.iter().take(MAX_MODELS) {
        let provider = entry["provider_id"].as_str().unwrap_or("openai");
        if provider.is_empty() || provider.len() > 128 || !provider.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            continue;
        }
        let Some(id) = entry["model_id"].as_str().or_else(|| entry["id"].as_str()) else {
            continue;
        };
        let name = entry["name"].as_str().unwrap_or(id);
        if id.is_empty()
            || id.len() > 256
            || name.len() > 256
            || id.chars().any(char::is_control)
            || name.chars().any(char::is_control)
        {
            continue;
        }
        if !models.iter().any(|model: &Model| model.id == id && model.provider_id == provider) {
            models.push(Model {
                id: id.to_owned(),
                name: name.to_owned(),
                provider_id: provider.to_owned(),
            });
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

pub(super) fn selected(models: &[Model]) -> Option<Model> {
    if let Some(path) = state_path() {
        if let Ok(value) = read_state(&path) {
            if let Some(recent) = value["recent"].as_array() {
                for entry in recent.iter().take(MAX_RECENT) {
                    if let Some(provider) = entry["providerID"].as_str() {
                        if let Some(model) = models
                            .iter()
                            .find(|model| entry["modelID"].as_str() == Some(&model.id) && model.provider_id == provider)
                        {
                            return Some(model.clone());
                        }
                    }
                }
            }
        }
    }
    models
        .iter()
        .find(|model| model.id == "gpt-5.6")
        .or_else(|| models.first())
        .cloned()
}

pub(super) fn recent_provider() -> Option<String> {
    let value = read_state(&state_path()?).ok()?;
    value["recent"].as_array()?.first()?["providerID"].as_str().map(str::to_owned)
}

pub(super) struct ModelLoad {
    pub provider_id: String,
    rx: tokio::sync::mpsc::Receiver<Result<Vec<Model>, String>>,
    task: tokio::task::JoinHandle<()>,
}

impl ModelLoad {
    pub fn start(origin: &str, auth: Option<&str>, provider: &str) -> Result<Self, String> {
        crate::native_debug::event("models.discovery_start", json!({"provider":provider,"credential_present":auth.is_some()}));
        if !provider.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || provider.is_empty() {
            return Err("invalid provider id".into());
        }
        let auth = auth.filter(|token| crate::daemon_client::is_wellformed_token(token))
            .ok_or("daemon authentication unavailable")?.to_owned();
        let url = reqwest::Url::parse(origin).map_err(|_| "invalid daemon URL")?;
        if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]" | "::1")) || !url.username().is_empty() || url.password().is_some() {
            return Err("model discovery requires authenticated local daemon".into());
        }
        let endpoint = format!("{}/api/providers/{provider}/models", origin.trim_end_matches('/'));
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let task = tokio::spawn(async move {
            let result = async {
                let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())
                    .timeout(std::time::Duration::from_secs(16)).build().map_err(|_| "unable to create model discovery client")?;
                let mut response = client.get(endpoint).header("Authorization", crate::daemon_client::authorization_header(&auth))
                    .send().await.map_err(|_| "model discovery timed out or daemon unavailable")?;
                if !response.status().is_success() { return Err(format!("model discovery failed (HTTP {}); check provider URL and API key", response.status().as_u16())); }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| "model discovery response interrupted")? {
                    if bytes.len() + chunk.len() > 8 * 1024 * 1024 { return Err("model list exceeds size limit".into()); }
                    bytes.extend_from_slice(&chunk);
                }
                catalogue(std::str::from_utf8(&bytes).map_err(|_| "invalid model list encoding")?)
            }.await;
            crate::native_debug::event("models.discovery_result", json!({"ok":result.is_ok(),"count":result.as_ref().ok().map(Vec::len)}));
            let _ = tx.send(result).await;
        });
        Ok(Self { rx, task, provider_id: provider.to_owned() })
    }

    pub fn poll(&mut self) -> Option<Result<Vec<Model>, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => Some(Err("model discovery interrupted".into())),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
        }
    }
}

impl Drop for ModelLoad {
    fn drop(&mut self) {
        crate::native_debug::event("models.discovery_owner_drop", json!({"provider":self.provider_id,"finished":self.task.is_finished()}));
        self.task.abort();
    }
}

pub(super) fn save_model(model: &Model) -> Result<(), String> {
    let path = state_path().ok_or("unable to resolve model state directory")?;
    let mut value = read_state(&path)?;
    let mut recent = vec![json!({"providerID":model.provider_id, "modelID":model.id})];
    if let Some(previous) = value["recent"].as_array() {
        recent.extend(
            previous
                .iter()
                .filter(|entry| {
                    entry["providerID"].is_string()
                        && entry["modelID"].is_string()
                        && !(entry["providerID"].as_str() == Some(&model.provider_id)
                            && entry["modelID"].as_str() == Some(&model.id))
                })
                .take(MAX_RECENT - 1)
                .cloned(),
        );
    }
    value["recent"] = Value::Array(recent);
    if value.get("favorite").is_none() {
        value["favorite"] = json!([]);
    }
    if value.get("variant").is_none() {
        value["variant"] = json!({});
    }
    let bytes = serde_json::to_vec(&value).map_err(|_| "unable to encode model state")?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("model state exceeds size limit".to_owned());
    }
    let parent = path.parent().ok_or("invalid model state path")?;
    fs::create_dir_all(parent).map_err(|_| "unable to create model state directory")?;
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "unable to create model state")?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)
    })();
    if result.is_err() {
        if let Err(error) = fs::remove_file(&temporary) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("model state temporary-file cleanup failed: {error}");
            }
        }
    }
    result.map_err(|_| "unable to persist model selection".to_owned())
}

fn state_path() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/state"))
        })
        .map(|path| path.join("opencode/model.json"))
}

fn read_state(path: &PathBuf) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_STATE_BYTES as u64 => {}
        _ => return Err("model state is not a bounded regular file".to_owned()),
    }
    let file = fs::File::open(path).map_err(|_| "unable to read model state")?;
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unable to read model state")?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("model state exceeds size limit".to_owned());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid model state")?;
    if !value.is_object() {
        return Err("invalid model state".to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod picker_tests {
    use super::*;

    #[test]
    fn custom_catalogue_preserves_router_identity_and_complete_list() {
        let rows: Vec<_> = (0..1159).map(|i| json!({"id":format!("route/model-{i}"),"provider_id":"9router","name":format!("Model {i}")})).collect();
        let models = catalogue(&json!({"models": rows}).to_string()).unwrap();
        assert_eq!(models.len(), 1159);
        let index = models.iter().position(|model| model.id == "route/model-1158").unwrap();
        assert_eq!(models[index].qualified(), "9router/route/model-1158");
        assert_eq!(visible_rows(&models, "9router/route/model-1158"), vec![index]);
    }
    fn models() -> Vec<Model> { vec![Model { id: "j-model".into(), name: "Jöhn 🚀".into(), provider_id: "openai".into() }, Model { id: "k-model".into(), name: "Kappa".into(), provider_id: "openai".into() }] }
    #[test] fn query_is_utf8_and_jk_are_search_text() {
        let m = models();
        assert_eq!(visible_rows(&m, "öhn"), vec![0]);
        assert_eq!(visible_rows(&m, "j"), vec![0]);
        assert_eq!(visible_rows(&m, "k"), vec![1]);
    }
    #[test] fn focused_row_clamps_and_no_result_cannot_select() {
        let m = models();
        assert_eq!(selected_row(&m, "k", 99).unwrap().id, "k-model");
        assert!(selected_row(&m, "no-match", 0).is_none());
        assert_eq!(move_row(99, 0, true), 0);
    }
    #[test] fn exact_identity_not_variant_or_typed_fallback() {
        let m = models();
        assert_eq!(selected_row(&m, "j-model-extra", 0), None);
        assert_eq!(m[0].qualified(), "openai/j-model");
    }
}
