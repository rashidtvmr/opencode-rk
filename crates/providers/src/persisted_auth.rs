//! Narrow, schema-filtered access to OpenCode's persisted API credentials.
use serde_json::Value;
use std::{env, fs::OpenOptions, io::Read, path::PathBuf};
const MAX_AUTH_BYTES: usize = 1024 * 1024;
const MAX_AUTH_ENTRIES: usize = 256;
const MAX_API_KEY_BYTES: usize = 16 * 1024;

pub async fn api_key(provider: &str, ambient_name: &str) -> Option<String> {
    let content = env::var("OPENCODE_AUTH_CONTENT").ok();
    // Upstream uses a truthy inline value, parses it as the complete auth
    // source, and falls through to disk only when it is empty or invalid JSON.
    let inline = content
        .as_deref()
        .filter(|raw| !raw.is_empty())
        .filter(|raw| raw.len() <= MAX_AUTH_BYTES)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let bytes = if inline.is_none() {
        let path = auth_path();
        tokio::task::spawn_blocking(move || path.and_then(|path| read_bounded_file(&path)))
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    inline
        .as_ref()
        .and_then(|value| api_key_from_value(value, provider))
        .or_else(|| {
            inline
                .is_none()
                .then(|| bytes.as_deref().and_then(|b| parse(b, provider)))
                .flatten()
        })
        .or_else(|| {
            env::var(ambient_name)
                .ok()
                .filter(|v| !v.trim().is_empty() && v.len() <= MAX_API_KEY_BYTES)
        })
}
fn auth_path() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && !p.as_os_str().is_empty())
        .map(|p| p.join("opencode/auth.json"))
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute() && !p.as_os_str().is_empty())
                .map(|p| p.join(".local/share/opencode/auth.json"))
        })
}

fn read_bounded_file(path: &PathBuf) -> Option<Vec<u8>> {
    // Inspect before opening so a configured FIFO/device is rejected without
    // entering a potentially blocking open. This check is not TOCTOU-proof.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = OpenOptions::new().read(true).open(path).ok()?;
    // Recheck the opened handle; this still does not claim race protection.
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::with_capacity(MAX_AUTH_BYTES.saturating_add(1));
    file.take((MAX_AUTH_BYTES as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}
fn parse(bytes: &[u8], provider: &str) -> Option<String> {
    if bytes.len() > MAX_AUTH_BYTES {
        return None;
    }
    let root: Value = serde_json::from_slice(bytes).ok()?;
    api_key_from_value(&root, provider)
}

fn api_key_from_value(root: &Value, provider: &str) -> Option<String> {
    let entries = root.as_object()?;
    if entries.len() > MAX_AUTH_ENTRIES {
        return None;
    }
    let item = entries.get(provider)?.as_object()?;
    if item.get("type")?.as_str()? != "api" {
        return None;
    }
    if let Some(metadata) = item.get("metadata") {
        let metadata = metadata.as_object()?;
        if metadata.values().any(|value| !value.is_string()) {
            return None;
        }
    }
    let key = item.get("key")?.as_str()?;
    (!key.trim().is_empty() && key.len() <= MAX_API_KEY_BYTES).then(|| key.to_owned())
}
