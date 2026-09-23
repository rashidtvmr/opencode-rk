#![forbid(unsafe_code)]
//! Tool display labels (mirrors `tool/websearch.ts:39-43` @ a0d9b6c).
//!
//! Spec cited `util/tool-display.ts` (13 lines); absent in this checkout.
//! Actual source: `webSearchProviderLabel` + `ctx.metadata({title, metadata})`
//! (`tool/websearch.ts:117`, `tool/tool.ts:44-53`). `toolDisplayMetadata`
//! pending/structured rules do not exist upstream; port models them as a
//! pending-vs-final title pair. Fail-closed: unknown provider -> default
//! label; titles truncated to [`MAX_TITLE_LEN`] chars.

/// Max title chars (mirrors `webSearchModelName` `.slice(0, 100)`).
pub const MAX_TITLE_LEN: usize = 100;

/// TS `webSearchProviderLabel` (`tool/websearch.ts:39-43`).
#[must_use]
pub fn web_search_provider_label(provider: &str) -> &'static str {
    match provider {
        "parallel" => "Parallel Web Search",
        "exa" => "Exa Web Search",
        _ => "Web Search",
    }
}

/// Truncate to [`MAX_TITLE_LEN`] chars (fail-closed bound).
#[must_use]
pub fn truncate_title(s: &str) -> String {
    if s.chars().count() <= MAX_TITLE_LEN {
        return s.to_string();
    }
    s.chars().take(MAX_TITLE_LEN).collect()
}

/// Pending-vs-final display metadata (no upstream equivalent; fail-closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDisplay {
    pub title: String,
    pub pending: bool,
}

impl ToolDisplay {
    #[must_use]
    pub fn new(title: &str, pending: bool) -> Self {
        Self {
            title: truncate_title(title),
            pending,
        }
    }

    /// Pending renders with trailing ellipsis; final renders plain.
    #[must_use]
    pub fn rendered(&self) -> String {
        if self.pending {
            format!("{}...", self.title)
        } else {
            self.title.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_match_ts() {
        assert_eq!(web_search_provider_label("parallel"), "Parallel Web Search");
        assert_eq!(web_search_provider_label("exa"), "Exa Web Search");
        assert_eq!(web_search_provider_label("other"), "Web Search");
    }

    #[test]
    fn unknown_and_empty_default() {
        assert_eq!(web_search_provider_label(""), "Web Search");
        assert_eq!(web_search_provider_label("PARALLEL"), "Web Search");
    }

    #[test]
    fn title_truncated() {
        let long = "x".repeat(200);
        assert_eq!(truncate_title(&long).chars().count(), MAX_TITLE_LEN);
        assert_eq!(truncate_title("ok"), "ok");
    }

    #[test]
    fn pending_renders_ellipsis() {
        assert_eq!(ToolDisplay::new("q", true).rendered(), "q...");
        assert_eq!(ToolDisplay::new("q", false).rendered(), "q");
    }
}
