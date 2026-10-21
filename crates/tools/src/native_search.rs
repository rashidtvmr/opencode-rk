//! Bounded, broker-authorized filesystem search primitives.
//!
//! This module is deliberately synchronous: callers must run it in
//! `spawn_blocking`. It uses the rooted [`FileTool`] for file bytes and
//! directory listings, so every open is descriptor-relative and `O_NOFOLLOW`.
//! Symlinks are never traversed or read, lexical `..` is rejected, and every
//! resource (visits, results, depth, bytes, output) has a hard ceiling.

use crate::file_ops::{FileOperation, FileTool};
use opencode_rk_security::{Decision, FileAction, OperationIntent, PermissionBroker};
use serde_json::{Value, json};
use std::fs;
use std::path::{Component, Path, PathBuf};

const MAX_VISITS: usize = 10_000;
const MAX_RESULTS: usize = 1_000;
const MAX_OUTPUT_BYTES: usize = 1_048_576;
const MAX_FILE_BYTES: usize = 1_048_576;
const MAX_TOTAL_READ_BYTES: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_QUERY_BYTES: usize = 256;
const MAX_PATH_BYTES: usize = 4096;
const MAX_GLOB_PATTERN_BYTES: usize = 256;
const MAX_GLOB_TEXT_BYTES: usize = 4096;
const MAX_GLOB_WORK: usize = MAX_GLOB_PATTERN_BYTES * MAX_GLOB_TEXT_BYTES;
const LISTING_CAP: usize = 1_000;

pub fn search(
    input: &Value,
    cwd: &Path,
    broker: &PermissionBroker,
    kind: &str,
) -> Result<String, String> {
    if kind != "grep" && kind != "glob" {
        return Err(format!("unsupported search kind: {kind}"));
    }
    let root = cwd.canonicalize().map_err(|e| format!("cwd: {e}"))?;

    let explicit_query = input
        .get("query")
        .or_else(|| input.get("pattern"))
        .and_then(Value::as_str);
    let include = input.get("include").and_then(Value::as_str);
    let query = explicit_query.or(include).ok_or("query is required")?;
    if query.is_empty() {
        return Err("query is required".into());
    }
    if query.len() > MAX_QUERY_BYTES || query.contains('\0') {
        return Err(format!("query must be at most {MAX_QUERY_BYTES} bytes"));
    }
    if let Some(include) = include {
        if include.len() > MAX_GLOB_PATTERN_BYTES || include.contains('\0') {
            return Err(format!(
                "include must be at most {MAX_GLOB_PATTERN_BYTES} bytes"
            ));
        }
    }
    if kind == "grep" && input.get("literal").and_then(Value::as_bool) == Some(false) {
        return Err("regex search is unavailable; literal must be true".into());
    }

    let hidden = input
        .get("hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let case_sensitive = input
        .get("caseSensitive")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_entries = usize_field(input, "maxEntries", MAX_VISITS)?.min(MAX_VISITS);
    let max_bytes = usize_field(input, "maxBytes", MAX_OUTPUT_BYTES)?.min(MAX_OUTPUT_BYTES);
    let max_file_bytes =
        usize_field(input, "maxFileBytes", MAX_FILE_BYTES)?.min(MAX_TOTAL_READ_BYTES);
    let max_depth = usize_field(input, "maxDepth", MAX_DEPTH)?.min(MAX_DEPTH);
    let limit = usize_field(input, "limit", MAX_RESULTS)?.min(MAX_RESULTS);

    let start = resolve_start(&root, input)?;
    reject_symlink_components(&root, &start)?;
    authorize_start(broker, &start)?;
    let metadata = fs::symlink_metadata(&start).map_err(|e| format!("{}: {e}", start.display()))?;
    if metadata.file_type().is_symlink() {
        return Err("path contains a symlink; refusing to follow it".into());
    }

    let tool = FileTool::with_project_root(cwd);
    let mut state = State {
        results: Vec::new(),
        output: String::new(),
        entry_bytes: 0,
        output_bytes: 0,
        visited: 0,
        read_bytes: 0,
        max_entries,
        max_bytes,
        max_file_bytes,
        max_depth,
        limit,
        truncated: false,
        stop: false,
    };
    let include = if explicit_query.is_some() {
        include
    } else {
        None
    };

    if metadata.is_file() {
        process_file(
            &tool,
            &root,
            &start,
            query,
            kind,
            include,
            case_sensitive,
            &mut state,
        )?;
    } else if metadata.is_dir() {
        walk(
            &tool,
            &root,
            start,
            query,
            kind,
            include,
            hidden,
            case_sensitive,
            &mut state,
            broker,
        )?;
    } else {
        return Err("path is not a regular file or directory".into());
    }

    Ok(json!({
        "output": state.output,
        "result": state.results,
        "entries": state.visited,
        "truncated": state.truncated,
    })
    .to_string())
}

struct State {
    results: Vec<Value>,
    output: String,
    entry_bytes: usize,
    output_bytes: usize,
    visited: usize,
    read_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    max_file_bytes: usize,
    max_depth: usize,
    limit: usize,
    truncated: bool,
    stop: bool,
}

fn walk(
    tool: &FileTool,
    root: &Path,
    start: PathBuf,
    query: &str,
    kind: &str,
    include: Option<&str>,
    hidden: bool,
    case_sensitive: bool,
    state: &mut State,
    broker: &PermissionBroker,
) -> Result<(), String> {
    let mut stack = vec![(start, 0usize)];
    while let Some((path, depth)) = stack.pop() {
        if state.stop {
            break;
        }
        if depth > state.max_depth {
            state.truncated = true;
            continue;
        }
        if depth > 0 {
            if state.visited >= state.max_entries {
                state.truncated = true;
                break;
            }
            state.visited += 1;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                state.truncated = true;
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            process_file(
                tool,
                root,
                &path,
                query,
                kind,
                include,
                case_sensitive,
                state,
            )?;
            continue;
        }
        if !metadata.is_dir() {
            continue;
        }

        let names = match list_names(tool, &path) {
            Ok(names) => names,
            Err(_) => {
                state.truncated = true;
                continue;
            }
        };
        if names.len() >= LISTING_CAP {
            state.truncated = true;
        }
        let child_depth = depth + 1;
        if child_depth > state.max_depth {
            if !names.is_empty() {
                state.truncated = true;
            }
            continue;
        }
        for name in names.iter().rev() {
            if name.is_empty() || name == "." || name == ".." {
                continue;
            }
            if !hidden && name.starts_with('.') {
                continue;
            }
            let child = path.join(name);
            if child.as_os_str().len() > MAX_PATH_BYTES {
                state.truncated = true;
                continue;
            }
            if !entry_allowed(broker, &child) {
                continue;
            }
            stack.push((child, child_depth));
        }
    }
    Ok(())
}

fn process_file(
    tool: &FileTool,
    root: &Path,
    path: &Path,
    query: &str,
    kind: &str,
    include: Option<&str>,
    case_sensitive: bool,
    state: &mut State,
) -> Result<(), String> {
    let rel = relative_path(root, path);
    if kind == "glob" {
        if glob_match(query, &rel, case_sensitive) {
            add_result(state, json!({ "path": rel }), rel.clone());
        }
        return Ok(());
    }

    if let Some(include) = include {
        if !include_matches(include, &rel, case_sensitive) {
            return Ok(());
        }
    }

    let remaining = MAX_TOTAL_READ_BYTES.saturating_sub(state.read_bytes);
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            state.truncated = true;
            return Ok(());
        }
    };
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if !metadata.is_file() {
        return Ok(());
    }
    if metadata.len() > state.max_file_bytes as u64 || metadata.len() > remaining as u64 {
        state.truncated = true;
        return Ok(());
    }

    let result = match tool.execute(FileOperation::Read {
        path: path.to_path_buf(),
        offset: None,
        limit: None,
    }) {
        Ok(result) => result,
        Err(_) => {
            state.truncated = true;
            return Ok(());
        }
    };
    if !result.success {
        state.truncated = true;
        return Ok(());
    }
    let content = result.content;
    if content.len() > state.max_file_bytes || content.len() > remaining {
        state.truncated = true;
        return Ok(());
    }
    state.read_bytes = state.read_bytes.saturating_add(content.len());
    if content.as_bytes().contains(&0) {
        return Ok(());
    }

    let needle = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };
    for (number, line) in content.lines().enumerate() {
        if state.stop {
            break;
        }
        let haystack = if case_sensitive {
            line.to_owned()
        } else {
            line.to_lowercase()
        };
        if !haystack.contains(&needle) {
            continue;
        }
        let value = json!({
            "path": rel,
            "lineNumber": number + 1,
            "line": line,
        });
        let output_line = format!("{}:{}:{}", rel, number + 1, line);
        if !add_result(state, value, output_line) {
            break;
        }
    }
    Ok(())
}

fn add_result(state: &mut State, value: Value, output_line: String) -> bool {
    if state.results.len() >= state.limit {
        state.truncated = true;
        state.stop = true;
        return false;
    }
    let entry_len = serde_json::to_vec(&value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    let output_len = output_line.len().saturating_add(1);
    if state.entry_bytes.saturating_add(entry_len) > state.max_bytes
        || state.output_bytes.saturating_add(output_len) > state.max_bytes
    {
        state.truncated = true;
        state.stop = true;
        return false;
    }
    state.entry_bytes = state.entry_bytes.saturating_add(entry_len);
    state.output_bytes = state.output_bytes.saturating_add(output_len);
    if !state.output.is_empty() {
        state.output.push('\n');
    }
    state.output.push_str(&output_line);
    state.results.push(value);
    true
}

fn list_names(tool: &FileTool, path: &Path) -> Result<Vec<String>, String> {
    let result = tool
        .execute(FileOperation::Read {
            path: path.to_path_buf(),
            offset: None,
            limit: None,
        })
        .map_err(|e| e.to_string())?;
    if !result.success {
        return Err(result
            .error
            .unwrap_or_else(|| "directory read failed".into()));
    }
    serde_json::from_str(&result.content)
        .map_err(|_| "directory listing is not a name array".into())
}

fn resolve_start(root: &Path, input: &Value) -> Result<PathBuf, String> {
    let raw = input.get("path").and_then(Value::as_str).unwrap_or(".");
    if raw.len() > MAX_PATH_BYTES {
        return Err("path is too long".into());
    }
    let path = Path::new(raw);
    for component in path.components() {
        match component {
            Component::ParentDir => return Err("path traversal is not allowed".into()),
            Component::Prefix(_) => return Err("path prefix is not allowed".into()),
            _ => {}
        }
    }
    if path.is_absolute() {
        let canonical = path.canonicalize().map_err(|e| format!("path: {e}"))?;
        if !canonical.starts_with(root) {
            return Err("path escapes cwd".into());
        }
        Ok(canonical)
    } else {
        let start = root.join(path);
        if !start.starts_with(root) {
            return Err("path escapes cwd".into());
        }
        Ok(start)
    }
}

fn reject_symlink_components(root: &Path, start: &Path) -> Result<(), String> {
    let relative = start
        .strip_prefix(root)
        .map_err(|_| "path escapes cwd".to_owned())?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("path contains a symlink; refusing to follow it".into());
            }
            Ok(_) => {}
            Err(error) => return Err(format!("{}: {error}", current.display())),
        }
    }
    Ok(())
}

fn authorize_start(broker: &PermissionBroker, path: &Path) -> Result<(), String> {
    match broker.authorize(&OperationIntent::File {
        action: FileAction::Read,
        path: path.to_path_buf(),
    }) {
        Decision::Allow => Ok(()),
        Decision::Deny { reason } => Err(format!("read denied: {reason}")),
        Decision::RequireHuman { reason, .. } => {
            Err(format!("read requires human approval: {reason}"))
        }
    }
}

fn entry_allowed(broker: &PermissionBroker, path: &Path) -> bool {
    matches!(
        broker.authorize(&OperationIntent::File {
            action: FileAction::Read,
            path: path.to_path_buf()
        }),
        Decision::Allow
    )
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn include_matches(pattern: &str, rel: &str, case_sensitive: bool) -> bool {
    if pattern.contains('/') {
        glob_match(pattern, rel, case_sensitive)
    } else {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        glob_match(pattern, name, case_sensitive)
    }
}

fn usize_field(value: &Value, key: &str, default: usize) -> Result<usize, String> {
    match value.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .ok_or_else(|| format!("{key} must be a non-negative integer")),
    }
}

/// Segment-aware bounded glob: `*` excludes `/`, `**` crosses directories,
/// and `**/` also matches zero directory components.
fn glob_match(pattern: &str, text: &str, case_sensitive: bool) -> bool {
    if pattern.len() > MAX_GLOB_PATTERN_BYTES || text.len() > MAX_GLOB_TEXT_BYTES {
        return false;
    }
    let pattern: Vec<char> = if case_sensitive {
        pattern.chars().collect()
    } else {
        pattern.to_lowercase().chars().collect()
    };
    let text: Vec<char> = if case_sensitive {
        text.chars().collect()
    } else {
        text.to_lowercase().chars().collect()
    };
    if pattern.is_empty() {
        return text.is_empty();
    }
    let work = (pattern.len() + 1).saturating_mul(text.len() + 1);
    if work > MAX_GLOB_WORK {
        return false;
    }

    let mut matched = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    matched[0][0] = true;
    for i in 0..pattern.len() {
        for j in 0..=text.len() {
            if !matched[i][j] {
                continue;
            }
            match pattern[i] {
                '*' => {
                    let globstar = i + 1 < pattern.len() && pattern[i + 1] == '*';
                    if globstar {
                        matched[i + 2][j] = true;
                        if j < text.len() {
                            matched[i][j + 1] = true;
                        }
                        if i + 2 < pattern.len() && pattern[i + 2] == '/' {
                            matched[i + 3][j] = true;
                        }
                    } else {
                        matched[i + 1][j] = true;
                        if j < text.len() && text[j] != '/' {
                            matched[i][j + 1] = true;
                        }
                    }
                }
                '?' => {
                    if j < text.len() && text[j] != '/' {
                        matched[i + 1][j + 1] = true;
                    }
                }
                literal => {
                    if j < text.len() && literal == text[j] {
                        matched[i + 1][j + 1] = true;
                    }
                }
            }
        }
    }
    matched[pattern.len()][text.len()]
}
