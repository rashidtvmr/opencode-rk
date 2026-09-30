//! Narrow, schema-filtered access to OpenCode's persisted API credentials.
use std::{env, path::PathBuf};
use serde_json::Value;
const MAX_AUTH_BYTES: usize = 1024 * 1024;
const MAX_AUTH_ENTRIES: usize = 256;
const MAX_API_KEY_BYTES: usize = 16 * 1024;

pub async fn api_key(provider: &str, ambient_name: &str) -> Option<String> {
    let content = env::var("OPENCODE_AUTH_CONTENT").ok();
    let bytes = if content.is_none() {
        let path = auth_path();
        tokio::task::spawn_blocking(move || std::fs::read(path).ok().filter(|b| b.len() <= MAX_AUTH_BYTES)).await.ok().flatten()
    } else { None };
    content.as_deref().and_then(|v| parse(v.as_bytes(), provider))
        .or_else(|| bytes.as_deref().and_then(|v| parse(v, provider)))
        .or_else(|| env::var(ambient_name).ok().filter(|v| !v.trim().is_empty() && v.len() <= MAX_API_KEY_BYTES))
}
fn auth_path() -> PathBuf {
    env::var_os("XDG_DATA_HOME").map(PathBuf::from).map(|p| p.join("opencode/auth.json"))
        .or_else(|| env::var_os("HOME").map(PathBuf::from).map(|p| p.join(".local/share/opencode/auth.json")))
        .unwrap_or_else(|| PathBuf::from(".local/share/opencode/auth.json"))
}
fn parse(bytes: &[u8], provider: &str) -> Option<String> {
    if bytes.len() > MAX_AUTH_BYTES { return None; }
    let root: Value = serde_json::from_slice(bytes).ok()?;
    let entries = root.as_object()?;
    if entries.len() > MAX_AUTH_ENTRIES { return None; }
    let item = entries.get(provider)?.as_object()?;
    if item.get("type")?.as_str()? != "api" { return None; }
    let key = item.get("key")?.as_str()?;
    (!key.trim().is_empty() && key.len() <= MAX_API_KEY_BYTES).then(|| key.to_owned())
}
