//! Catalogue-backed native model choices and upstream-compatible model recents.
use serde_json::{json, Value};
use std::{
    env, fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

const MAX_MODELS: usize = 500;
const MAX_STATE_BYTES: usize = 64 * 1024;
const MAX_RECENT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Model {
    pub id: String,
    pub name: String,
}

impl Model {
    pub fn qualified(&self) -> String { format!("openai/{}", self.id) }
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
    ApiKey,
    Model,
}

pub(super) fn catalogue(body: &str) -> Result<Vec<Model>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid model catalogue")?;
    let entries = value["models"]
        .as_array()
        .ok_or("missing model catalogue")?;
    let mut models = Vec::new();
    for entry in entries.iter().take(MAX_MODELS) {
        // Other providers remain unavailable until they have a native turn adapter.
        if entry["provider_id"].as_str() != Some("openai") {
            continue;
        }
        let Some(id) = entry["model_id"].as_str() else {
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
        if !models.iter().any(|model: &Model| model.id == id) {
            models.push(Model {
                id: id.to_owned(),
                name: name.to_owned(),
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
                    if entry["providerID"].as_str() == Some("openai") {
                        if let Some(model) = models
                            .iter()
                            .find(|model| entry["modelID"].as_str() == Some(&model.id))
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

pub(super) fn save_model(model: &Model) -> Result<(), String> {
    let path = state_path().ok_or("unable to resolve model state directory")?;
    let mut value = read_state(&path)?;
    let mut recent = vec![json!({"providerID":"openai", "modelID":model.id})];
    if let Some(previous) = value["recent"].as_array() {
        recent.extend(
            previous
                .iter()
                .filter(|entry| {
                    entry["providerID"].is_string()
                        && entry["modelID"].is_string()
                        && !(entry["providerID"].as_str() == Some("openai")
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
    fn models() -> Vec<Model> { vec![Model { id: "j-model".into(), name: "Jöhn 🚀".into() }, Model { id: "k-model".into(), name: "Kappa".into() }] }
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
