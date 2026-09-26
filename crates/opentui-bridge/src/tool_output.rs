#![forbid(unsafe_code)]
//! Tool output budgets + inline row (mirrors
//! `packages/tui/src/routes/session/index.tsx:1702-1985,2046,2349,2587-2710`
//! @ a0d9b6c).
//!
//! `should_hide` mirrors `shouldHide` (:1707-1711). Budgets mirror `maxLines`
//! 3 (:1796 Generic), 10 (:2046 Shell), 4 (:2349 Execute); the char budget
//! `maxLines * max(20, width-6)` belongs to `crate::collapse`. `line`
//! mirrors `InlineToolRow` (:1907-1985): pending renders `~ {pending}`
//! (:1954); denied strikethrough is attribute-only upstream
//! (`TextAttributes.STRIKETHROUGH`), encoded here as `~~..~~`. `failed`
//! switches color upstream only, no text change. Title bound reuses
//! `crate::tool_display::truncate_title`; `input()` (:2613-2620) emits
//! `[k=v, ...]` with no trunc bound of its own.

use crate::tool_display::truncate_title;

/// Generic tool preview line budget (:1796).
pub const BUDGET_GENERIC: usize = 3;
/// Shell output preview line budget (:2046).
pub const BUDGET_SHELL: usize = 10;
/// Execute output preview line budget (:2349).
pub const BUDGET_EXECUTE: usize = 4;

/// 14 entries of the `toolDisplays` set (:2630-2645), dispatched by the
/// `Switch` (:1734-1778). Unknown tools hit the `GenericTool` fallback
/// (:1776-1778), i.e. `None` from [`ToolKind::from_tool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Shell,
    Write,
    Glob,
    Read,
    Grep,
    WebFetch,
    WebSearch,
    Task,
    Execute,
    Edit,
    ApplyPatch,
    TodoWrite,
    Question,
    Skill,
}

impl ToolKind {
    /// TS `toolDisplay` (:2647-2649): known tool -> kind, else generic.
    #[must_use]
    pub const fn from_tool(tool: &str) -> Option<Self> {
        match tool.as_bytes() {
            b"bash" => Some(Self::Shell),
            b"write" => Some(Self::Write),
            b"glob" => Some(Self::Glob),
            b"read" => Some(Self::Read),
            b"grep" => Some(Self::Grep),
            b"webfetch" => Some(Self::WebFetch),
            b"websearch" => Some(Self::WebSearch),
            b"task" => Some(Self::Task),
            b"execute" => Some(Self::Execute),
            b"edit" => Some(Self::Edit),
            b"apply_patch" => Some(Self::ApplyPatch),
            b"todowrite" => Some(Self::TodoWrite),
            b"question" => Some(Self::Question),
            b"skill" => Some(Self::Skill),
            _ => None,
        }
    }

    /// Evidenced `icon` prop per renderer. Task/Execute vary by status
    /// upstream (`✓` completed, `✗` error); this is the non-final default.
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Shell => "$",      // :2090
            Self::Write => "←",     // :2123
            Self::Glob => "✱",       // :2138
            Self::Read => "→",       // :2162
            Self::Grep => "✱",       // :2186
            Self::WebFetch => "%",   // :2198
            Self::WebSearch => "◈",  // :2206
            Self::Task => "│",       // :2291 (`✓` when completed)
            Self::Execute => "│",    // :2363 (`✓` completed, `✗` on error)
            Self::Edit => "←",       // :2433
            Self::ApplyPatch => "%", // :2509
            Self::TodoWrite => "⚙",  // :2530
            Self::Question => "→",   // :2571
            Self::Skill => "→",      // :2581
        }
    }

    /// Evidenced `pending` prop per renderer.
    #[must_use]
    pub const fn pending(self) -> &'static str {
        match self {
            Self::Shell => "Writing command...",      // :2090
            Self::Write => "Preparing write...",      // :2124
            Self::Glob => "Finding files...",         // :2138
            Self::Read => "Reading file...",          // :2163
            Self::Grep => "Searching content...",     // :2186
            Self::WebFetch => "Fetching from the web...", // :2198
            Self::WebSearch => "Searching web...",    // :2206
            Self::Task => "Delegating...",            // :2296
            Self::Execute => "execute",               // :2366
            Self::Edit => "Preparing edit...",        // :2433
            Self::ApplyPatch => "Preparing patch...", // :2509
            Self::TodoWrite => "Updating todos...",   // :2531
            Self::Question => "Asking questions...",  // :2571
            Self::Skill => "Loading skill...",        // :2581
        }
    }
}

/// `shouldHide` (:1707-1711): hidden iff details off and tool completed
/// (running/error/pending tools always show).
#[must_use]
pub const fn should_hide(completed: bool, show_details: bool) -> bool {
    !show_details && completed
}

/// Minimal `InlineToolRow` (:1907-1925): icon/pending/complete/failure/
/// spinner/strikethrough/error-expand props collapse to these fields;
/// color/spinner/click handling is renderer-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineRow {
    pub icon: String,
    pub title: String,
    pub pending: String,
    pub failed: bool,
    pub strikethrough: bool,
}

impl InlineRow {
    #[must_use]
    pub fn new(icon: &str, title: &str, pending: &str, failed: bool, strikethrough: bool) -> Self {
        Self {
            icon: icon.to_string(),
            title: truncate_title(title),
            pending: pending.to_string(),
            failed,
            strikethrough,
        }
    }

    /// Complete -> `{icon} {title}` (:1959-1974); no complete ->
    /// `~ {pending}` (:1948-1956). `failed` without complete swaps in
    /// `failure ?? children` upstream (:1972); failure text is per-tool so
    /// callers pass it as `title`.
    #[must_use]
    pub fn line(&self) -> String {
        let body = if self.title.is_empty() {
            format!("~ {}", self.pending)
        } else {
            format!("{} {}", self.icon, self.title)
        };
        if self.strikethrough {
            format!("~~{body}~~")
        } else {
            body
        }
    }
}

/// Evidenced `state.metadata` keys read by the renderers (:2587-2710).
pub const META_KEYS: &[&str] = &[
    "output",      // :1721 Shell :2044, Execute :2347
    "diff",        // Edit :2402, ApplyPatch files carry patch
    "diagnostics", // Write :2107, Edit :2429, ApplyPatch :2502
    "todos",       // TodoWrite :2521
    "questions",   // Question :2545
    "answers",     // Question :2546
    "sessionId",   // Task :2220 (camelCase upstream)
    "numResults",  // WebSearch :2208
    "count",       // Glob :2141 (Grep uses `matches` :2189)
];

/// Whether a key is one of the evidenced metadata keys.
#[must_use]
pub fn is_meta_key(key: &str) -> bool {
    META_KEYS.contains(&key)
}

/// Single primitive input value, bounded. `input()` (:2613-2620) keeps only
/// string/number/bool props and emits `[k=v, ...]` ("" when none) with no
/// trunc of its own; the bound reuses the title limit.
#[must_use]
pub fn format_input_primitive(s: &str) -> String {
    truncate_title(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_match_tsx_max_lines() {
        assert_eq!(BUDGET_GENERIC, 3);
        assert_eq!(BUDGET_SHELL, 10);
        assert_eq!(BUDGET_EXECUTE, 4);
    }

    #[test]
    fn should_hide_only_completed_when_details_off() {
        assert!(should_hide(true, false));
        assert!(!should_hide(true, true));
        assert!(!should_hide(false, false));
        assert!(!should_hide(false, true));
    }

    #[test]
    fn from_tool_covers_14_and_falls_back() {
        let known = [
            ("bash", ToolKind::Shell),
            ("write", ToolKind::Write),
            ("glob", ToolKind::Glob),
            ("read", ToolKind::Read),
            ("grep", ToolKind::Grep),
            ("webfetch", ToolKind::WebFetch),
            ("websearch", ToolKind::WebSearch),
            ("task", ToolKind::Task),
            ("execute", ToolKind::Execute),
            ("edit", ToolKind::Edit),
            ("apply_patch", ToolKind::ApplyPatch),
            ("todowrite", ToolKind::TodoWrite),
            ("question", ToolKind::Question),
            ("skill", ToolKind::Skill),
        ];
        assert_eq!(known.len(), 14);
        for (name, kind) in known {
            assert_eq!(ToolKind::from_tool(name), Some(kind));
        }
        assert_eq!(ToolKind::from_tool("plan_exit"), None);
        assert_eq!(ToolKind::from_tool(""), None);
    }

    #[test]
    fn icons_and_pendings_match_tsx() {
        assert_eq!(ToolKind::Shell.icon(), "$");
        assert_eq!(ToolKind::Shell.pending(), "Writing command...");
        assert_eq!(ToolKind::Grep.icon(), "✱");
        assert_eq!(ToolKind::WebSearch.icon(), "◈");
        assert_eq!(ToolKind::Task.pending(), "Delegating...");
        assert_eq!(ToolKind::Execute.pending(), "execute");
        assert_eq!(ToolKind::TodoWrite.pending(), "Updating todos...");
        assert_eq!(ToolKind::Skill.pending(), "Loading skill...");
    }

    #[test]
    fn pending_row_renders_tilde() {
        let row = InlineRow::new("$", "", "Writing command...", false, false);
        assert_eq!(row.line(), "~ Writing command...");
    }

    #[test]
    fn complete_row_and_title_bound() {
        let row = InlineRow::new("→", "Read foo.ts", "Reading file...", false, false);
        assert_eq!(row.line(), "→ Read foo.ts");
        let long = "x".repeat(200);
        let bounded = InlineRow::new("$", &long, "Writing command...", false, false);
        assert_eq!(bounded.title.chars().count(), crate::tool_display::MAX_TITLE_LEN);
    }

    #[test]
    fn strikethrough_wraps_failed_keeps_text() {
        let denied = InlineRow::new("$", "ls", "Writing command...", false, true);
        assert_eq!(denied.line(), "~~$ ls~~");
        let failed = InlineRow::new("%", "Patch", "Preparing patch...", true, false);
        assert_eq!(failed.line(), "% Patch");
    }

    #[test]
    fn meta_keys_evidenced() {
        assert_eq!(META_KEYS.len(), 9);
        for key in ["output", "diff", "diagnostics", "todos", "questions", "answers", "sessionId", "numResults", "count"] {
            assert!(is_meta_key(key), "{key}");
        }
        assert!(!is_meta_key("matches"));
        assert!(!is_meta_key(""));
    }

    #[test]
    fn primitive_bounded() {
        assert_eq!(format_input_primitive("ls"), "ls");
        let long = "y".repeat(200);
        assert_eq!(
            format_input_primitive(&long).chars().count(),
            crate::tool_display::MAX_TITLE_LEN
        );
    }
}
