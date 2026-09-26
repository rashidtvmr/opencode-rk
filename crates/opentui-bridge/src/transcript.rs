#![forbid(unsafe_code)]
//! Transcript entry rendering (mirrors `packages/tui/src/util/transcript.ts`).
//!
//! Covers `TranscriptOptions` (thinking/toolDetails/assistantMetadata),
//! `formatPart` branches (text / reasoning / tool, else skipped), and the
//! `pending` early-return shape from `util/tool-display.ts`
//! (`toolDisplayMetadata` returns `{}` for pending state).
//! Prompt part/display helpers (`prompt/part.ts`, `prompt/display.ts`) carry
//! no transcript content types, so nothing is mirrored from them.
//! Divergence note: TS checkout at /home/rashid/projects/opencode is a0d9b6c,
//! not pinned 95daf90; line refs below track a0d9b6c.

/// Max chars for `TranscriptEntry.text` (fail-closed bound).
pub const MAX_ENTRY: usize = 65536;
/// Max parts per entry (fail-closed bound).
pub const MAX_PARTS: usize = 64;
/// Per-part render cap before the truncation note applies.
const MAX_PART_RENDER: usize = 4096;

/// Mirrors TS `TranscriptOptions` (`transcript.ts:5-10`); `providers` omitted
/// (no provider registry on this side of the bridge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptOptions {
    pub thinking: bool,
    pub tool_details: bool,
    pub assistant_metadata: bool,
}

impl TranscriptOptions {
    #[must_use]
    pub const fn new(thinking: bool, tool_details: bool, assistant_metadata: bool) -> Self {
        Self { thinking, tool_details, assistant_metadata }
    }
}

impl Default for TranscriptOptions {
    fn default() -> Self {
        Self { thinking: false, tool_details: false, assistant_metadata: false }
    }
}

/// Tool lifecycle; TS statuses are pending/running/completed/error
/// (`transcript.ts:101-106`, `tool-display.ts:9` pending guard).
/// Variant is named `Failed` (Rust reserves no `Error` here and matches
/// `ToolState::Failed` in sibling modules) but its wire label is `"error"`
/// verbatim per TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Pending,
    Running,
    Completed,
    Failed,
}

impl ToolState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "error",
        }
    }
}

/// Evidenced part kinds only: TS `formatPart` renders text/reasoning/tool and
/// returns `""` for everything else (`transcript.ts:84-112`), so no Image
/// variant is mirrored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartKind {
    Text(String),
    Reasoning(String),
    ToolCall { tool: String, state: ToolState },
}

/// Message role (`transcript.ts:54-58` user vs assistant branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// Bounds failure for [`TranscriptEntry::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptError {
    TextTooLong,
    TooManyParts,
}

/// One message with its rendered parts (`MessageWithParts`, `transcript.ts:21-24`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptEntry {
    pub role: Role,
    pub text: String,
    pub parts: Vec<PartKind>,
}

impl TranscriptEntry {
    /// Fail-closed: rejects over-cap text / part lists.
    pub fn new(role: Role, text: String, parts: Vec<PartKind>) -> Result<Self, TranscriptError> {
        if text.len() > MAX_ENTRY {
            return Err(TranscriptError::TextTooLong);
        }
        if parts.len() > MAX_PARTS {
            return Err(TranscriptError::TooManyParts);
        }
        Ok(Self { role, text, parts })
    }
}

// Duplicated from collapse.rs to avoid cycles: 3-line truncation with note.
fn truncate_note(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &s[..end])
}

fn render_part(part: &PartKind, opts: &TranscriptOptions) -> String {
    match part {
        PartKind::Text(s) => format!("{}\n\n", truncate_note(s, MAX_PART_RENDER)),
        // TS `transcript.ts:89-94`: reasoning emits only when thinking is on.
        PartKind::Reasoning(s) => {
            if opts.thinking {
                format!("_Thinking:_\n\n{}\n\n", truncate_note(s, MAX_PART_RENDER))
            } else {
                String::new()
            }
        }
        // TS `transcript.ts:96-109`: tool header always, input/output/error
        // only under toolDetails. State label plays the details role here.
        PartKind::ToolCall { tool, state } => {
            if opts.tool_details {
                format!("**Tool: {}** [{}]\n\n", truncate_note(tool, MAX_PART_RENDER), state.label())
            } else {
                format!("**Tool: {}**\n\n", truncate_note(tool, MAX_PART_RENDER))
            }
        }
    }
}

/// Render one entry; mirrors `formatMessage` header + `formatPart` per part.
#[must_use]
pub fn render_entry(entry: &TranscriptEntry, opts: &TranscriptOptions) -> String {
    let mut out = match entry.role {
        Role::User => "## User\n\n".to_string(),
        // No model/provider data on this side; metadata flag keeps the same
        // header (TS `formatAssistantHeader`, `transcript.ts:67-82`).
        Role::Assistant => "## Assistant\n\n".to_string(),
    };
    let _ = opts.assistant_metadata;
    if !entry.text.is_empty() {
        out.push_str(&truncate_note(&entry.text, MAX_PART_RENDER));
        out.push_str("\n\n");
    }
    for part in &entry.parts {
        out.push_str(&render_part(part, opts));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with(parts: Vec<PartKind>) -> TranscriptEntry {
        TranscriptEntry::new(Role::Assistant, String::new(), parts).unwrap()
    }

    #[test]
    fn thinking_hidden_by_default() {
        let e = entry_with(vec![PartKind::Reasoning("deep".into())]);
        assert!(!render_entry(&e, &TranscriptOptions::default()).contains("deep"));
    }

    #[test]
    fn thinking_shown_when_enabled() {
        let e = entry_with(vec![PartKind::Reasoning("deep".into())]);
        let opts = TranscriptOptions::new(true, false, false);
        let out = render_entry(&e, &opts);
        assert!(out.contains("_Thinking:_") && out.contains("deep"));
    }

    #[test]
    fn tool_label_hidden_without_details() {
        let e = entry_with(vec![PartKind::ToolCall { tool: "grep".into(), state: ToolState::Completed }]);
        assert!(!render_entry(&e, &TranscriptOptions::default()).contains("completed"));
    }

    #[test]
    fn tool_label_shown_with_details() {
        let e = entry_with(vec![PartKind::ToolCall { tool: "grep".into(), state: ToolState::Completed }]);
        let opts = TranscriptOptions::new(false, true, false);
        let out = render_entry(&e, &opts);
        assert!(out.contains("**Tool: grep**") && out.contains("completed"));
    }

    #[test]
    fn pending_renders_label() {
        let e = entry_with(vec![PartKind::ToolCall { tool: "bash".into(), state: ToolState::Pending }]);
        let opts = TranscriptOptions::new(false, true, false);
        assert!(render_entry(&e, &opts).contains("pending"));
    }

    #[test]
    fn over_cap_text_errs() {
        assert_eq!(
            TranscriptEntry::new(Role::User, "x".repeat(MAX_ENTRY + 1), vec![]),
            Err(TranscriptError::TextTooLong)
        );
    }

    #[test]
    fn empty_text_ok() {
        assert!(TranscriptEntry::new(Role::User, String::new(), vec![]).is_ok());
    }

    #[test]
    fn parts_overflow_errs() {
        let parts = (0..MAX_PARTS + 1).map(|i| PartKind::Text(i.to_string())).collect();
        assert_eq!(
            TranscriptEntry::new(Role::User, String::new(), parts),
            Err(TranscriptError::TooManyParts)
        );
    }
}
