//! Bounded, read-only session-directory explorer for the web application.
//! No arbitrary global path and no ability to read hidden credential files.
use crate::{parse_session_id, session_workdir, ApiFailure, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path as FsPath, PathBuf};

const MAX_FILE_BYTES: u64 = 128 * 1024;
const MAX_ENTRIES: usize = 200;
const MAX_SCANNED: usize = 1024;

#[derive(Deserialize, Default)]
pub(crate) struct BrowseQuery {
    path: Option<String>,
}

fn forbidden_component(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with(".env")
        || matches!(
            lower.as_str(),
            ".git"
                | ".ssh"
                | ".aws"
                | ".npmrc"
                | ".pypirc"
                | ".netrc"
                | "credentials"
                | "secrets"
                | "id_rsa"
                | "id_ed25519"
                | "node_modules"
                | "target"
                | ".private-tmp"
        )
        || lower.starts_with("secret.")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
}

const MAX_LOOKUP_SCANNED: usize = 1800;
const MAX_LOOKUP_DEPTH: usize = 6;
const MAX_LOOKUP_RESULTS: usize = 50;

#[derive(Deserialize)]
pub(crate) struct LookupQuery {
    q: String,
}

fn lookup_files(root: &FsPath, query: &str) -> Result<Value, String> {
    if query.is_empty() {
        return Ok(json!({"results":[]}));
    }
    if query.len() > 128
        || query.chars().any(char::is_control)
        || query.contains('\\')
        || query.starts_with('/')
        || query
            .split('/')
            .any(|part| part == ".." || forbidden_component(part))
    {
        return Err("invalid file search query".into());
    }
    let root = root
        .canonicalize()
        .map_err(|_| "session directory unavailable")?;
    if !root.is_dir() {
        return Err("session directory unavailable".into());
    }
    let needle = query.to_ascii_lowercase();
    let mut pending = vec![(root, String::new(), 0usize)];
    let mut found: Vec<String> = Vec::new();
    let mut visited = 0usize;
    while let Some((dir, prefix, depth)) = pending.pop() {
        if depth > MAX_LOOKUP_DEPTH || visited >= MAX_LOOKUP_SCANNED {
            break;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries {
            visited += 1;
            if visited > MAX_LOOKUP_SCANNED {
                break;
            }
            let Ok(entry) = entry else { continue };
            let name = entry.file_name().to_string_lossy().to_string();
            if forbidden_component(&name) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if relative.len() > 1024 {
                continue;
            }
            if meta.is_dir() && depth < MAX_LOOKUP_DEPTH {
                pending.push((entry.path(), relative, depth + 1));
            } else if meta.is_file() && relative.to_ascii_lowercase().contains(&needle) {
                found.push(relative);
                if found.len() >= MAX_LOOKUP_RESULTS * 3 {
                    break;
                }
            }
        }
    }
    found.sort_by(|a, b| {
        let ap = !a.to_ascii_lowercase().starts_with(&needle);
        let bp = !b.to_ascii_lowercase().starts_with(&needle);
        ap.cmp(&bp).then_with(|| a.cmp(b))
    });
    found.truncate(MAX_LOOKUP_RESULTS);
    Ok(json!({"results":found}))
}

pub(crate) async fn lookup(
    State(state): State<AppState>,
    Path(raw_session): Path<String>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&raw_session)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    let root = session_workdir(&state.sessions, id, None).await?;
    tokio::task::spawn_blocking(move || lookup_files(&root, &query.q))
        .await
        .map_err(ApiFailure::internal)?
        .map(Json)
        .map_err(ApiFailure::bad_request)
}

pub(crate) fn safe_parts(path: &str) -> Result<Vec<&str>, String> {
    if path.len() > 1024 || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err("invalid file path".into());
    }
    if path.is_empty() || path == "." {
        return Ok(vec![]);
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == ".." || forbidden_component(part))
    {
        return Err("restricted file path".into());
    }
    Ok(parts)
}

fn browse(root: &FsPath, path: &str) -> Result<Value, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "session directory unavailable")?;
    if !root.is_dir() {
        return Err("session directory unavailable".into());
    }
    let parts = safe_parts(path)?;
    let mut cursor = root.clone();
    for part in parts {
        cursor.push(part);
        let meta = std::fs::symlink_metadata(&cursor).map_err(|_| "file unavailable")?;
        if meta.file_type().is_symlink() {
            return Err("symbolic links cannot be browsed".into());
        }
    }
    let canonical = cursor.canonicalize().map_err(|_| "file unavailable")?;
    if !canonical.starts_with(&root) {
        return Err("outside the session directory".into());
    }
    let meta = std::fs::symlink_metadata(&canonical).map_err(|_| "file unavailable")?;
    if meta.is_dir() {
        let mut entries = Vec::new();
        let listing = std::fs::read_dir(canonical).map_err(|_| "directory unavailable")?;
        for (count, item) in listing.enumerate() {
            if count >= MAX_SCANNED {
                break;
            }
            let item = item.map_err(|_| "directory unavailable")?;
            let name = item.file_name().to_string_lossy().to_string();
            if forbidden_component(&name) {
                continue;
            }
            let meta =
                std::fs::symlink_metadata(item.path()).map_err(|_| "directory unavailable")?;
            if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
                continue;
            }
            let relative = if path == "." || path.is_empty() {
                name.clone()
            } else {
                format!("{path}/{name}")
            };
            entries.push(
                json!({"name":name,"path":relative,"is_dir":meta.is_dir(),"bytes":meta.len()}),
            );
        }
        entries.sort_by(|a, b| {
            let a_dir = a.get("is_dir").and_then(Value::as_bool).unwrap_or(false);
            let b_dir = b.get("is_dir").and_then(Value::as_bool).unwrap_or(false);
            b_dir.cmp(&a_dir).then_with(|| {
                a.get("name")
                    .and_then(Value::as_str)
                    .cmp(&b.get("name").and_then(Value::as_str))
            })
        });
        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);
        return Ok(
            json!({"type":"directory","path":path,"entries":entries,"truncated":truncated,"read_only":true}),
        );
    }
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err("not a readable text file (128 KiB limit)".into());
    }
    let data = std::fs::read(canonical).map_err(|_| "file unavailable")?;
    let text = String::from_utf8(data).map_err(|_| "binary files cannot be previewed")?;
    if text.chars().any(|ch| ch == '\0') {
        return Err("binary files cannot be previewed".into());
    }
    Ok(json!({"type":"text","path":path,"text":text,"bytes":meta.len(),"read_only":true}))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(raw_session): Path<String>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let session = parse_session_id(&raw_session)?;
    state
        .sessions
        .get(session)
        .await
        .map_err(ApiFailure::internal)?;
    let root = session_workdir(&state.sessions, session, None).await?;
    let path = query.path.unwrap_or_else(|| ".".to_owned());
    tokio::task::spawn_blocking(move || browse(&root, &path))
        .await
        .map_err(ApiFailure::internal)?
        .map(Json)
        .map_err(ApiFailure::bad_request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_path_traversal_and_credentials() {
        for value in [
            "../.env",
            "/etc/passwd",
            "src/../../etc/passwd",
            ".env",
            ".env.local",
            "nested/.git/config",
            ".npmrc",
            "src/secret.json",
            "src/token.pem",
            "C:\\Users\\x",
        ] {
            assert!(safe_parts(value).is_err(), "{value}");
        }
    }
    #[test]
    fn lists_and_reads_bounded_text() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("readme.md"), "# Hello").unwrap();
        std::fs::write(root.path().join(".env.production"), "PRIVATE").unwrap();
        let listing = browse(root.path(), ".").unwrap();
        let entries = listing["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "readme.md");
        assert_eq!(browse(root.path(), "readme.md").unwrap()["text"], "# Hello");
    }
    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc", root.path().join("link")).unwrap();
        assert!(browse(root.path(), "link/passwd").is_err());
    }
    #[test]
    fn file_lookup_is_bounded_and_ignores_secrets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/App.tsx"), "export default 1").unwrap();
        std::fs::write(root.path().join(".env.local"), "TOKEN").unwrap();
        assert_eq!(
            lookup_files(root.path(), "app").unwrap()["results"][0],
            "src/App.tsx"
        );
        assert!(lookup_files(root.path(), "env").unwrap()["results"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(lookup_files(root.path(), "../secrets").is_err());
    }
}
