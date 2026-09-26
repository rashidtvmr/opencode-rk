#![forbid(unsafe_code)]
//! Full tool display line (mirrors `util/tool-display.ts`).
//!
//! `tool_label` truncates names to 64 chars; `tool_line` appends
//! `[status]` and caps at 256 chars; `is_running` flags active states.

/// Max chars for a tool label (fail-closed bound).
pub const MAX_LABEL_LEN: usize = 64;
/// Max chars for a full tool line (fail-closed bound).
pub const MAX_LINE_LEN: usize = 256;

/// Human label for a tool name, trimmed and capped at 64 chars.
#[must_use]
pub fn tool_label(name: &str) -> String {
    let t = name.trim();
    if t.is_empty() {
        return "Tool".to_string();
    }
    if t.chars().count() <= MAX_LABEL_LEN {
        t.to_string()
    } else {
        t.chars().take(MAX_LABEL_LEN).collect()
    }
}

/// True while the tool is still active.
#[must_use]
pub fn is_running(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "running" | "pending" | "in_progress" | "in-progress" | "loading"
    )
}

/// `"<label> [<status>]"` capped at 256 chars; label alone if status empty.
#[must_use]
pub fn tool_line(name: &str, status: &str) -> String {
    let t = name.trim();
    let label = if t.is_empty() {
        "Tool".to_string()
    } else {
        t.to_string()
    };
    let s = status.trim();
    let raw = if s.is_empty() {
        label
    } else {
        format!("{label} [{s}]")
    };
    if raw.chars().count() <= MAX_LINE_LEN {
        raw
    } else {
        raw.chars().take(MAX_LINE_LEN).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_trims_and_defaults() {
        assert_eq!(tool_label("  grep  "), "grep");
        assert_eq!(tool_label(""), "Tool");
        assert_eq!(tool_label("   "), "Tool");
    }

    #[test]
    fn label_caps_at_64() {
        let long = "x".repeat(100);
        assert_eq!(tool_label(&long).chars().count(), MAX_LABEL_LEN);
    }

    #[test]
    fn running_states() {
        assert!(is_running("running"));
        assert!(is_running("pending"));
        assert!(is_running("Loading"));
        assert!(!is_running("done"));
        assert!(!is_running(""));
    }

    #[test]
    fn line_combines_and_caps() {
        assert_eq!(tool_line("grep", "running"), "grep [running]");
        assert_eq!(tool_line("grep", ""), "grep");
        let long = "y".repeat(300);
        assert_eq!(tool_line(&long, "running").chars().count(), MAX_LINE_LEN);
    }
}
