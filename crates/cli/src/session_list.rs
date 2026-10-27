#![forbid(unsafe_code)]
//! Portable authenticated session-list validation shared by native and
//! non-native snapshot consumers. This module has no UI dependencies.

use serde_json::Value;
use std::collections::HashSet;

pub const MAX_ROWS: usize = 256;
pub const MAX_TITLE_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_owned(); }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_owned()
}

fn sanitize_title(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !c.is_control() || *c == ' ').collect();
    truncate_utf8(&cleaned, MAX_TITLE_BYTES)
}

/// Parse and validate the authenticated backend session listing. Raw title
/// limits and controls are checked before sanitation; invalid entries reject
/// the complete snapshot rather than being silently normalized or dropped.
pub fn parse_rows(value: &Value) -> Result<Vec<SessionRow>, String> {
    let raw: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => match map.get("sessions") {
            Some(Value::Array(items)) => items.iter().collect(),
            Some(other) => return Err(format!("expected sessions array, got {other}")),
            None => return Err("missing `sessions` array".into()),
        },
        other => return Err(format!("unexpected payload: {other}")),
    };
    if raw.len() > MAX_ROWS {
        return Err(format!("server sent {} sessions; refusing to present a truncated {MAX_ROWS}-row list", raw.len()));
    }
    let mut rows = Vec::with_capacity(raw.len());
    let mut ids = HashSet::with_capacity(raw.len());
    for entry in raw {
        let obj = entry.as_object().ok_or_else(|| format!("session entry not an object: {entry}"))?;
        let state = obj.get("state").map(|s| s.as_str().unwrap_or_default()).unwrap_or_default();
        if state == "archived" || state == "deleted" { continue; }
        let id = obj.get("id").and_then(Value::as_str).ok_or_else(|| "session entry missing string `id`".to_owned())?;
        id.parse::<opencode_rk_contracts::SessionId>().map_err(|e| format!("invalid session id {id:?}: {e}"))?;
        if !ids.insert(id.to_owned()) {
            return Err(format!("duplicate session id {id:?}"));
        }
        let title_raw = obj.get("title").and_then(Value::as_str).unwrap_or("");
        if title_raw.len() > MAX_TITLE_BYTES {
            return Err(format!("session {id} title exceeds {MAX_TITLE_BYTES} bytes ({} bytes)", title_raw.len()));
        }
        if title_raw.chars().any(|c| c.is_control() && c != ' ') {
            return Err(format!("session {id} title contains control characters"));
        }
        rows.push(SessionRow { id: id.to_owned(), title: sanitize_title(title_raw) });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ID: &str = "550e8400-e29b-41d4-a716-446655440000";
    const ID2: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

    #[test]
    fn rejects_oversized_raw_title_without_truncating() {
        assert!(parse_rows(&json!({"sessions":[{"id":ID,"title":"x".repeat(MAX_TITLE_BYTES + 1)}]})).is_err());
    }
    #[test]
    fn rejects_control_title() {
        assert!(parse_rows(&json!({"sessions":[{"id":ID,"title":"bad\ncontrol"}]})).is_err());
    }
    #[test]
    fn rejects_duplicate_active_ids() {
        assert!(parse_rows(&json!({"sessions":[{"id":ID,"title":"a"},{"id":ID,"title":"b"}]})).is_err());
    }
    #[test]
    fn accepts_blank_title_and_bounded_valid_rows() {
        let rows = parse_rows(&json!({"sessions":[{"id":ID,"title":""},{"id":ID2,"title":"ok"}]})).unwrap();
        assert_eq!(rows[0].title, "");
        assert_eq!(rows.len(), 2);
    }
}
