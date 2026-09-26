#![forbid(unsafe_code)]
//! Session message list-item render plan (std only).
//!
//! Evidence (TS checkout a0d9b6c):
//! - part kinds `packages/tui/src/routes/session/index.tsx:387`
//!   `part.type === "text"`, `:219` `part.type === "tool"`
//! - tool states `index.tsx:2336` `"running" | "completed" | "error"`,
//!   `:2345` `pending | running` loading check
//! - collapse limits `index.tsx:1796` GenericTool `maxLines = 3`,
//!   `:2046` Shell `maxLines = 10`, `:2349` Execute `4`,
//!   `maxChars = maxLines * max(20, width - 6)` (`:1797`, `:2047`, `:2349`)
//! - line truncation `packages/tui/src/util/locale.ts:61-64`
//!   `truncate(str, len) = str.slice(0, len - 1) + "…"`; call sites
//!   `index.tsx:203` len 50 (title), `:2273` len 80 (retry message)
//! - tool label bound reuses [`crate::tool_display`] (`MAX_TITLE_LEN` 100,
//!   mirrors `tool/websearch.ts:39-43` `.slice(0, 100)`)
//! - collapse reuses [`crate::collapse::collapse_tool_output`] (mirrors
//!   `packages/tui/src/util/collapse-tool-output.ts:1-19`)
//!
//! Bounds (`MAX_TEXT_LEN` 8k, `MAX_PARTS` 256) are fail-closed port caps;
//! no upstream equivalent.

use crate::collapse::collapse_tool_output;
use crate::tool_display::truncate_title;

/// Max chars per text part (fail-closed port bound).
pub const MAX_TEXT_LEN: usize = 8192;
/// Max parts per plan (fail-closed port bound).
pub const MAX_PARTS: usize = 256;

/// Evidenced part kinds only: text + tool-call (`index.tsx:387`, `:219`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessagePart {
    Text(String),
    ToolCall { tool: String, state: String },
}

fn bound_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

impl MessagePart {
    /// Text part, fail-closed at [`MAX_TEXT_LEN`] chars.
    #[must_use]
    pub fn text(s: &str) -> Self {
        Self::Text(bound_chars(s, MAX_TEXT_LEN))
    }

    /// Tool-call part; tool/state reuse `truncate_title` (100-char cap).
    #[must_use]
    pub fn tool_call(tool: &str, state: &str) -> Self {
        Self::ToolCall { tool: truncate_title(tool), state: truncate_title(state) }
    }
}

/// Kept parts plus whether content was cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPlan {
    pub parts: Vec<MessagePart>,
    pub collapsed: bool,
}

/// Keep first `collapse_limit` parts (clamped to [`MAX_PARTS`]);
/// `collapsed` when input was cut. Fail-closed: limit 0 keeps nothing.
#[must_use]
pub fn plan(parts: Vec<MessagePart>, collapse_limit: usize) -> RenderPlan {
    let limit = collapse_limit.min(MAX_PARTS);
    if parts.len() <= limit {
        return RenderPlan { parts, collapsed: false };
    }
    let mut kept = parts;
    kept.truncate(limit);
    RenderPlan { parts: kept, collapsed: true }
}

/// Mirror `Locale.truncate` (`locale.ts:61-64`): passthrough when
/// `chars <= width`, else first `width - 1` chars + "…".
/// Fail-closed: width 0 yields empty string.
#[must_use]
pub fn trunc_line(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if s.chars().count() <= width {
        return s.to_string();
    }
    let head: String = s.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Collapse a text part body per bridge rules: `max_lines` is one of the
/// evidenced limits (3 / 10 / 4); `max_chars = max_lines * max(20, width-6)`.
#[must_use]
pub fn collapse_text(text: &str, max_lines: usize, width: usize) -> crate::collapse::Collapse {
    let max_chars = max_lines.saturating_mul(width.saturating_sub(6).max(20));
    collapse_tool_output(text, max_lines, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_limit_passthrough() {
        let p = plan(vec![MessagePart::text("a"), MessagePart::tool_call("bash", "completed")], 10);
        assert_eq!(p.parts.len(), 2);
        assert!(!p.collapsed);
    }

    #[test]
    fn over_limit_collapses() {
        let parts = vec![MessagePart::text("a"), MessagePart::text("b"), MessagePart::text("c")];
        let p = plan(parts, 2);
        assert_eq!(p.parts.len(), 2);
        assert!(p.collapsed);
    }

    #[test]
    fn zero_limit_fail_closed() {
        let p = plan(vec![MessagePart::text("a")], 0);
        assert!(p.parts.is_empty() && p.collapsed);
        let empty = plan(vec![], 0);
        assert!(!empty.collapsed);
    }

    #[test]
    fn text_bounded_8k() {
        let long = "x".repeat(MAX_TEXT_LEN + 10);
        let MessagePart::Text(t) = MessagePart::text(&long) else { panic!("text") };
        assert_eq!(t.chars().count(), MAX_TEXT_LEN);
    }

    #[test]
    fn tool_bounded_via_truncate_title() {
        let long = "y".repeat(200);
        let MessagePart::ToolCall { tool, state } = MessagePart::tool_call(&long, "running") else {
            panic!("tool")
        };
        assert_eq!(tool.chars().count(), crate::tool_display::MAX_TITLE_LEN);
        assert_eq!(state, "running");
    }

    #[test]
    fn trunc_line_matches_locale_truncate() {
        assert_eq!(trunc_line("abc", 50), "abc");
        assert_eq!(trunc_line("abcdef", 5), "abcd…");
        assert_eq!(trunc_line("anything", 0), "");
        assert_eq!(trunc_line("😀😀😀", 2), "😀…");
    }

    #[test]
    fn collapse_text_uses_bridge_rules() {
        let body = "a\nb\nc\nd";
        let out = collapse_text(body, 3, 80);
        assert!(out.overflow);
        assert!(out.preview.contains("…"));
        let fit = collapse_text("a\nb", 3, 80);
        assert!(!fit.overflow);
    }
}
