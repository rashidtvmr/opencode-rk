#![forbid(unsafe_code)]
//! Session rail + footer + question layout state (std only).
//!
//! TS checkout a0d9b6c (NOT pinned 95daf90), paths against a0d9b6c:
//! - `packages/tui/src/routes/session/sidebar.tsx:12-103`: session title
//!   (`:57-59`), sessionID (`:61`), workspace label (`:63-79`), share url
//!   (`:80-82`), footer version (`:90-98`), slots title/content/footer
//!   (`:49-55`, `:85`, `:90`) are view/plugin wiring, out of scope.
//! - `packages/tui/src/routes/session/footer.tsx:52-90`: directory left
//!   (`:54`), permissions count (`:63-68`), LSP count (`:69-71`), MCP
//!   count+error (`:72-84`), `/status` hint (`:85`).
//! - `packages/tui/src/routes/session/subagent-footer.tsx:17-31`: label from
//!   `@(\w+) subagent` title match + sibling index/total (`:25-30`);
//!   `:33-55` usage tokens+pct+cost; `:96-127` Parent/Prev/Next actions.
//! - `packages/tui/src/routes/session/question.tsx:355-456`: question text
//!   (`:359`), options list (`:364-399`), custom answer (`:400-453`);
//!   `:48-62` reply/reject, `:280-281` select/reject keys.
//! No overlap with `sidebar.rs` (feature-plugins sidebar panels).

/// Max rail entries (TS lists unbounded; fail-closed cap).
pub const MAX_RAIL_ENTRIES: usize = 256;
/// Max footer status chars.
pub const MAX_STATUS: usize = 512;
/// Max subagent compact line chars.
pub const MAX_COMPACT: usize = 256;
/// Max question text chars.
pub const MAX_QUESTION: usize = 1024;

/// Char-boundary-safe truncation to `max` chars.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    s.chars().take(max).collect()
}

/// Session rail: title/session/workspace/share/version + bounded entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRail {
    title: String,
    session_id: String,
    workspace: Option<String>,
    share_url: Option<String>,
    version: String,
    entries: Vec<String>,
    selected: usize,
}

impl SessionRail {
    #[must_use]
    pub fn new(title: &str, session_id: &str, version: &str) -> Self {
        Self {
            title: truncate(title, MAX_COMPACT),
            session_id: session_id.to_owned(),
            workspace: None,
            share_url: None,
            version: truncate(version, MAX_COMPACT),
            entries: Vec::new(),
            selected: 0,
        }
    }

    pub fn set_workspace(&mut self, name: Option<&str>) {
        self.workspace = name.map(|n| truncate(n, MAX_COMPACT));
    }

    pub fn set_share_url(&mut self, url: Option<&str>) {
        self.share_url = url.map(|u| truncate(u, MAX_COMPACT));
    }

    /// Replace entries; count capped at 256, each entry capped at 256 chars.
    pub fn set_entries(&mut self, entries: Vec<String>) {
        self.entries = entries
            .into_iter()
            .take(MAX_RAIL_ENTRIES)
            .map(|e| truncate(&e, MAX_COMPACT))
            .collect();
        self.clamp_selected();
    }

    /// Select clamped to `len - 1`; empty rail pins to 0.
    pub fn select(&mut self, index: usize) {
        self.selected = index;
        self.clamp_selected();
    }

    fn clamp_selected(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    #[must_use]
    pub fn selected_entry(&self) -> Option<&str> {
        self.entries.get(self.selected).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }
}

/// Session footer: status text (directory + LSP/MCP/permission summary) + busy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFooter {
    status: String,
    busy: bool,
}

impl SessionFooter {
    #[must_use]
    pub fn new(status: &str, busy: bool) -> Self {
        Self {
            status: truncate(status, MAX_STATUS),
            busy,
        }
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = truncate(status, MAX_STATUS);
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    #[must_use]
    pub fn busy(&self) -> bool {
        self.busy
    }
}

/// Subagent footer compact line: `Label (i of n) · usage`, capped 256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentFooter {
    line: String,
}

impl SubagentFooter {
    #[must_use]
    pub fn new(label: &str, index: usize, total: usize, usage: Option<&str>) -> Self {
        let mut line = if total > 0 {
            format!("{label} ({index} of {total})")
        } else {
            label.to_owned()
        };
        if let Some(u) = usage {
            if !u.is_empty() {
                line.push_str(" · ");
                line.push_str(u);
            }
        }
        Self {
            line: truncate(&line, MAX_COMPACT),
        }
    }

    #[must_use]
    pub fn line(&self) -> &str {
        &self.line
    }
}

/// Question prompt: bounded question text, accept/deny via `resolve(bool)`.
/// First `resolve` wins (fail-closed against double-submit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionPrompt {
    question: String,
    decision: Option<bool>,
}

impl QuestionPrompt {
    #[must_use]
    pub fn new(question: &str) -> Self {
        Self {
            question: truncate(question, MAX_QUESTION),
            decision: None,
        }
    }

    /// Returns false when already resolved (second resolve ignored).
    pub fn resolve(&mut self, accept: bool) -> bool {
        if self.decision.is_some() {
            return false;
        }
        self.decision = Some(accept);
        true
    }

    #[must_use]
    pub fn question(&self) -> &str {
        &self.question
    }

    #[must_use]
    pub fn decision(&self) -> Option<bool> {
        self.decision
    }

    #[must_use]
    pub fn resolved(&self) -> bool {
        self.decision.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_entries_capped_at_256() {
        let mut r = SessionRail::new("t", "s", "v");
        r.set_entries((0..300).map(|i| i.to_string()).collect());
        assert_eq!(r.len(), MAX_RAIL_ENTRIES);
    }

    #[test]
    fn rail_selected_clamps_and_empty_pins_zero() {
        let mut r = SessionRail::new("t", "s", "v");
        r.select(9);
        assert_eq!(r.selected(), 0);
        r.set_entries(vec!["a".into(), "b".into()]);
        r.select(99);
        assert_eq!(r.selected(), 1);
        assert_eq!(r.selected_entry(), Some("b"));
    }

    #[test]
    fn rail_title_and_entry_truncated() {
        let mut r = SessionRail::new(&"t".repeat(300), "s", "v");
        assert!(r.title().chars().count() <= MAX_COMPACT);
        r.set_entries(vec!["e".repeat(300)]);
        assert!(r.selected_entry().unwrap().chars().count() <= MAX_COMPACT);
    }

    #[test]
    fn footer_status_capped_at_512() {
        let f = SessionFooter::new(&"s".repeat(600), true);
        assert_eq!(f.status().chars().count(), MAX_STATUS);
        assert!(f.busy());
    }

    #[test]
    fn subagent_line_capped_at_256() {
        let f = SubagentFooter::new(&"L".repeat(200), 1, 3, Some(&"u".repeat(200)));
        assert!(f.line().chars().count() <= MAX_COMPACT);
        assert!(f.line().contains("(1 of 3)"));
        let bare = SubagentFooter::new("Subagent", 0, 0, None);
        assert_eq!(bare.line(), "Subagent");
    }

    #[test]
    fn question_capped_and_first_resolve_wins() {
        let mut q = QuestionPrompt::new(&"q".repeat(1200));
        assert_eq!(q.question().chars().count(), MAX_QUESTION);
        assert!(!q.resolved());
        assert!(q.resolve(true));
        assert_eq!(q.decision(), Some(true));
        assert!(!q.resolve(false));
        assert_eq!(q.decision(), Some(true));
    }

    #[test]
    fn question_deny_path() {
        let mut q = QuestionPrompt::new("proceed?");
        assert!(q.resolve(false));
        assert_eq!(q.decision(), Some(false));
        assert!(q.resolved());
    }
}
