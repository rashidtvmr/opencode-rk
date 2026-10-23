//! Catalogue-backed native model choices and upstream-compatible model recents.
use serde_json::{json, Value};
use std::{
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_MODELS: usize = 5000;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_RECENT: usize = 32;
// Keep aligned with the private own-config limit in providers::imported_config.
const MAX_OWN_CONFIG_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Model {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub context_window: Option<u64>,
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
                context_window:entry.get("context_window").and_then(Value::as_u64).filter(|value|*value>0),
            });
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

pub(super) fn selected(models: &[Model]) -> Option<Model> {
    selected_with_sources(
        models,
        state_path().as_deref(),
        opencode_rk_providers::imported_config::own_config_path().as_deref(),
    )
}

/// Pure selection core; production `selected()` resolves the canonical own
/// state/config paths, tests pass explicit fixture paths (no env access).
///
/// Precedence: explicit recent state first, then the canonical own config
/// root `model` (exact catalogued `provider/model` identity; malformed,
/// missing, over-limit or unavailable identities are skipped honestly), then
/// the plain catalogue fallback (gpt-5.6, else first row).
pub(super) fn selected_with_sources(
    models: &[Model],
    state: Option<&Path>,
    config: Option<&Path>,
) -> Option<Model> {
    if let Some(path) = state {
        if let Some(model) = recent_selected(models, path) {
            return Some(model);
        }
    }
    if let Some(path) = config {
        if let Some(model) = imported_selected(models, path) {
            return Some(model);
        }
    }
    models
        .iter()
        .find(|model| model.id == "gpt-5.6")
        .or_else(|| models.first())
        .cloned()
}

/// Select the provider to initialize before its model catalogue is loaded.
///
/// Provider ids are deliberately validated against the daemon's provider
/// registry first: a recent entry or an imported model must not turn into an
/// arbitrary network request.  The configured model is only used for its
/// qualified provider portion; model identity itself is checked later against
/// the catalogue returned by that provider.
pub(super) fn startup_provider(
    recent: Option<&str>,
    configured_model: Option<&str>,
    known_providers: &[&str],
) -> String {
    let known = |provider: &str| provider == "openai" || known_providers.iter().any(|id| *id == provider);
    if let Some(provider) = recent.filter(|provider| known(provider)) {
        return provider.to_owned();
    }
    if let Some((provider, _model)) = configured_model.and_then(|model| model.trim().split_once('/')) {
        if known(provider) {
            return provider.to_owned();
        }
    }
    "openai".to_owned()
}

/// Read only the root qualified model identity from the bounded own config.
/// Keeping this reader shared by provider bootstrap and catalogue selection
/// avoids a second unbounded/config-semantic JSON parser.
pub(super) fn configured_model_identity(path: &Path) -> Option<String> {
    let value = read_bounded_json(path, MAX_OWN_CONFIG_BYTES).ok()?;
    let configured = value.get("model")?.as_str()?.trim();
    if configured.is_empty() || configured.len() > 512 || !configured.contains('/') {
        return None;
    }
    Some(configured.to_owned())
}

fn recent_selected(models: &[Model], path: &Path) -> Option<Model> {
    let Ok(value) = read_state(&PathBuf::from(path)) else { return None };
    let recent = value["recent"].as_array()?;
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
    None
}

/// Canonical own config root `model` (`opencode.json`), bounded by its own
/// config limit; only the root `model` key is read, never credential fields.
/// Exact provider/model identity is required; invalid identities fall through.
fn imported_selected(models: &[Model], path: &Path) -> Option<Model> {
    let configured = configured_model_identity(path)?;
    models
        .iter()
        .find(|model| {
            model.qualified() == configured
                || configured
                    .split_once('/')
                    .is_some_and(|(provider, id)| model.provider_id == provider && model.id == id)
        })
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

fn read_state(path: &Path) -> Result<Value, String> {
    read_bounded_json(path, MAX_STATE_BYTES)
}

fn read_bounded_json(path: &Path, max_bytes: usize) -> Result<Value, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Ok(metadata) if metadata.is_file() && metadata.len() <= max_bytes as u64 => {}
        _ => return Err("model state is not a bounded regular file".to_owned()),
    }
    let file = fs::File::open(path).map_err(|_| "unable to read model state")?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unable to read model state")?;
    if bytes.len() > max_bytes {
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

    struct TempDir {
        root: PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            let root = std::env::temp_dir().join(format!("native-setup-import-{tag}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture dir");
            Self { root }
        }
        fn write(&self, name: &str, body: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::write(&path, body).expect("fixture file");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture_models() -> Vec<Model> {
        vec![
            Model { id: "x".into(), name: "Acme X".into(), provider_id: "acme".into(), context_window: None },
            Model { id: "gpt-5.6".into(), name: "Fallback".into(), provider_id: "openai".into(), context_window: None },
        ]
    }

    #[test]
    fn imported_catalogued_model_is_default_without_recent_selection() {
        let dir = TempDir::new("default");
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn explicit_recent_selection_beats_imported_config_model() {
        let dir = TempDir::new("recent-beats-import");
        let state = dir.write("model.json", r#"{"recent":[{"providerID":"openai","modelID":"gpt-5.6"}]}"#);
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let picked = selected_with_sources(&fixture_models(), Some(&state), Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn imported_model_survives_large_agent_prompt_within_own_config_limit() {
        let dir = TempDir::new("large-own-config");
        let body = json!({
            "model": "acme/x",
            "agent": { "large_prompt": "p".repeat(MAX_STATE_BYTES + 1) }
        })
        .to_string();
        assert!(body.len() > MAX_STATE_BYTES);
        assert!(body.len() <= MAX_OWN_CONFIG_BYTES);
        let config = dir.write("opencode.json", &body);

        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|model| model.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn imported_model_over_own_config_limit_falls_back() {
        let dir = TempDir::new("oversized-own-config");
        let body = json!({
            "model": "acme/x",
            "agent": { "large_prompt": "p".repeat(MAX_OWN_CONFIG_BYTES) }
        })
        .to_string();
        assert!(body.len() > MAX_OWN_CONFIG_BYTES);
        let config = dir.write("opencode.json", &body);

        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|model| model.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn unknown_or_malformed_imported_model_falls_back_without_panic() {
        let dir = TempDir::new("import-fallbacks");
        let models = fixture_models();
        let malformed = dir.write("malformed.opencode.json", "{");
        let numeric = dir.write("numeric.opencode.json", r#"{"model":123}"#);
        let empty = dir.write("empty.opencode.json", r#"{"model":""}"#);
        let unknown = dir.write("unknown.opencode.json", r#"{"model":"acme/nonexistent"}"#);
        for path in [&malformed, &numeric, &empty, &unknown] {
            let picked = selected_with_sources(&models, None, Some(path));
            assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        }
        let picked = selected_with_sources(&models, None, Some(&dir.root.join("absent.opencode.json")));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        // No gpt-5.6 row in the catalogue: the unknown import must never
        // conjure an entry; selection falls through to the fallback honestly.
        let fallback = vec![fixture_models()[0].clone()];
        let picked = selected_with_sources(&fallback, None, Some(&unknown));
        assert_eq!(picked.map(|m| m.qualified()), Some("acme/x".into()));
    }

    #[test]
    fn imported_model_requires_exact_provider_identity() {
        let dir = TempDir::new("import-identity");
        // Same catalog id, different provider: provider/model identity only.
        let config = dir.write("opencode.json", r#"{"model":"other/gpt-5.6"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
        // A bare model id without provider qualification is never auto-matched.
        let config = dir.write("bare.opencode.json", r#"{"model":"x"}"#);
        let picked = selected_with_sources(&fixture_models(), None, Some(&config));
        assert_eq!(picked.map(|m| m.qualified()), Some("openai/gpt-5.6".into()));
    }

    #[test]
    fn startup_provider_prefers_recent_then_imported_then_openai_and_validates_registry() {
        let known = ["acme", "9router"];
        assert_eq!(startup_provider(Some("9router"), Some("acme/x"), &known), "9router");
        assert_eq!(startup_provider(Some("missing"), Some("acme/x"), &known), "acme");
        assert_eq!(startup_provider(None, Some("missing/x"), &known), "openai");
        assert_eq!(startup_provider(Some("custom"), Some("acme/x"), &known), "acme");
    }

    #[test]
    fn imported_model_identity_drives_actual_startup_provider_before_catalogue() {
        let dir = TempDir::new("startup-import");
        let config = dir.write("opencode.json", r#"{"model":"acme/x"}"#);
        let identity = configured_model_identity(&config);
        assert_eq!(identity.as_deref(), Some("acme/x"));
        // The model is intentionally absent until the provider request
        // completes; only its validated provider is selected at bootstrap.
        assert_eq!(startup_provider(None, identity.as_deref(), &["acme"]), "acme");
        let loaded = catalogue(r#"{"models":[{"provider_id":"acme","id":"x","name":"Acme X"}]}"#).unwrap();
        assert_eq!(selected_with_sources(&loaded, None, Some(&config)).unwrap().qualified(), "acme/x");
    }

    #[test]
    fn custom_catalogue_preserves_router_identity_and_complete_list() {
        let rows: Vec<_> = (0..1159).map(|i| json!({"id":format!("route/model-{i}"),"provider_id":"9router","name":format!("Model {i}")})).collect();
        let models = catalogue(&json!({"models": rows}).to_string()).unwrap();
        assert_eq!(models.len(), 1159);
        let index = models.iter().position(|model| model.id == "route/model-1158").unwrap();
        assert_eq!(models[index].qualified(), "9router/route/model-1158");
        assert_eq!(visible_rows(&models, "9router/route/model-1158"), vec![index]);
    }
    fn models() -> Vec<Model> { vec![Model { id: "j-model".into(), name: "Jöhn 🚀".into(), provider_id: "openai".into(),context_window:None }, Model { id: "k-model".into(), name: "Kappa".into(), provider_id: "openai".into(),context_window:None }] }
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
