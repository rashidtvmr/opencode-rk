#![forbid(unsafe_code)]
//! Tool display metadata (mirrors `packages/tui/src/util/tool-display.ts:7` @ a0d9b6c).
//!
//! TS `toolDisplayMetadata(state)` returns `{}` when `status` is `"pending"`
//! or no `structured` object exists, else the `structured` map. The bridge
//! flattens that map to `title:` / `desc:` prefix lines inside `state`:
//! pending still yields empty metadata; no structured lines fall back to the
//! tool name. Bound mirrors `tool_display::MAX_TITLE_LEN` (duplicated here so
//! this file compiles standalone under `rustc --test`; no `crate::` import).
//!
//! `ponytail:` one struct + one parser; upgrade path is a real `structured`
//! map type, add only when a caller needs non-string values.

/// Max chars per field (mirrors `tool_display::MAX_TITLE_LEN`).
pub const MAX_META_LEN: usize = 100;

/// Flattened display metadata; empty means "no metadata" (TS `{}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolMeta {
    pub title: String,
    pub description: String,
}

impl ToolMeta {
    /// Empty metadata (TS `{}`).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
        }
    }

    /// True when both fields are empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.description.is_empty()
    }
}

/// Trimmed, char-boundary-safe truncation to [`MAX_META_LEN`].
fn truncate(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= MAX_META_LEN {
        return s.to_string();
    }
    s.chars().take(MAX_META_LEN).collect()
}

/// Value of the first non-empty line with one of `prefixes`, if any.
fn prefixed<'a>(state: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    state
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            prefixes
                .iter()
                .find_map(|p| t.strip_prefix(p))
                .map(str::trim)
                .filter(|v| !v.is_empty())
        })
        .next()
}

/// TS `toolDisplayMetadata` flattened: `status == "pending"` yields empty
/// metadata regardless of `state`; else the first `title:` / `desc:` (or
/// `description:`) line values; a missing title falls back to `tool`.
#[must_use]
pub fn tool_display_metadata(state: &str, tool: &str, status: &str) -> ToolMeta {
    if status.trim() == "pending" {
        return ToolMeta::empty();
    }
    let title = prefixed(state, &["title:"])
        .map(truncate)
        .unwrap_or_else(|| truncate(tool));
    let description = prefixed(state, &["desc:", "description:"])
        .map(truncate)
        .unwrap_or_default();
    ToolMeta { title, description }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_yields_empty_despite_body() {
        let m = tool_display_metadata("title: Hi\ndesc: Yo", "bash", "pending");
        assert!(m.is_empty());
        assert_eq!(m, ToolMeta::empty());
    }

    #[test]
    fn titled_parses_prefix_lines() {
        let m = tool_display_metadata("title: Read foo.ts\ndesc: 12 lines", "read", "completed");
        assert_eq!(m.title, "Read foo.ts");
        assert_eq!(m.description, "12 lines");
    }

    #[test]
    fn fallback_uses_tool_name() {
        let m = tool_display_metadata("some unstructured output", "bash", "completed");
        assert_eq!(m.title, "bash");
        assert_eq!(m.description, "");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert!(tool_display_metadata("", "", "completed").is_empty());
        assert!(tool_display_metadata("", "", "pending").is_empty());
    }

    #[test]
    fn long_fields_truncated() {
        let m = tool_display_metadata(&format!("title: {}", "x".repeat(200)), "t", "completed");
        assert_eq!(m.title.chars().count(), MAX_META_LEN);
    }

    #[test]
    fn description_prefix_alias() {
        let m = tool_display_metadata("description: full text", "t", "error");
        assert_eq!(m.title, "t");
        assert_eq!(m.description, "full text");
    }
}
