//! Bounded right-side TUI info panel (UI-019 slice).
//!
//! Pure caller-supplied snapshot renderer. No I/O, no clock, no threads,
//! no secrets by type: only labels, ids, counts, and safe diagnostic text
//! cross this boundary.

#![forbid(unsafe_code)]

/// Maximum active tool labels shown.
pub const MAX_INFO_TOOLS: usize = 16;
/// Maximum warnings shown.
pub const MAX_INFO_WARNINGS: usize = 8;
/// Maximum label length in chars.
pub const MAX_LABEL_CHARS: usize = 128;
/// Maximum cwd label length in chars.
pub const MAX_CWD_CHARS: usize = 256;
/// Minimum panel width accepted by [`render`].
pub const MIN_INFO_WIDTH: usize = 20;

/// Caller-supplied live metadata. All values are display labels and counts;
/// token values and credentials are unrepresentable here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoSnapshot {
    pub cwd_label: String,
    pub workspace: String,
    pub session_label: String,
    pub provider_id: String,
    pub model: String,
    pub auth_mode: String,
    pub ctx_window: u64,
    pub ctx_used: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_micros: Option<u64>,
    pub mcp_connected: bool,
    pub mcp_enabled: u64,
    pub mcp_disabled: u64,
    pub active_tools: Vec<String>,
    pub queue_depth: u64,
    pub status_line: String,
    pub updated_ms: u64,
    pub warnings: Vec<String>,
}

/// Bounded count projection with honest truncation flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelCounts {
    pub tools_shown: u64,
    pub warnings_shown: u64,
    pub truncated: bool,
}

/// Info-panel failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoError {
    EmptyField,
    TooNarrow,
}

impl std::fmt::Display for InfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField => f.write_str("info panel field must not be empty"),
            Self::TooNarrow => f.write_str("info panel is too narrow"),
        }
    }
}

impl std::error::Error for InfoError {}

/// Count tools/warnings with truncation honesty and saturating math.
#[must_use]
pub fn summarize(snapshot: &InfoSnapshot) -> PanelCounts {
    let tools_shown = snapshot.active_tools.len().min(MAX_INFO_TOOLS) as u64;
    let warnings_shown = snapshot.warnings.len().min(MAX_INFO_WARNINGS) as u64;
    let truncated =
        snapshot.active_tools.len() > MAX_INFO_TOOLS || snapshot.warnings.len() > MAX_INFO_WARNINGS;
    PanelCounts {
        tools_shown,
        warnings_shown,
        truncated,
    }
}

/// Pure deterministic renderer. Full layout at width >= 100, compact at < 60.
pub fn render(snapshot: &InfoSnapshot, width: usize) -> Result<Vec<String>, InfoError> {
    if snapshot.workspace.trim().is_empty()
        || snapshot.session_label.trim().is_empty()
        || snapshot.cwd_label.trim().is_empty()
        || snapshot.provider_id.trim().is_empty()
        || snapshot.model.trim().is_empty()
    {
        return Err(InfoError::EmptyField);
    }
    if width < MIN_INFO_WIDTH {
        return Err(InfoError::TooNarrow);
    }
    let counts = summarize(snapshot);
    let remaining = snapshot.ctx_window.saturating_sub(snapshot.ctx_used);
    let mcp_state = if snapshot.mcp_connected {
        "connected"
    } else {
        "disconnected"
    };
    let cost = snapshot
        .cost_micros
        .map_or_else(|| "none".to_owned(), |c| format!("{c}u"));
    let lines: Vec<String> = if width < 60 {
        let mut out = vec![
            safe_line(&format!(
                "ws: {} sess: {}",
                safe_label(&snapshot.workspace, MAX_LABEL_CHARS),
                safe_label(&snapshot.session_label, MAX_LABEL_CHARS)
            )),
            safe_line(&format!(
                "{} / {} ctx {}/{}",
                safe_label(&snapshot.provider_id, MAX_LABEL_CHARS),
                safe_label(&snapshot.model, MAX_LABEL_CHARS),
                snapshot.ctx_used,
                remaining
            )),
            safe_line(&format!(
                "mcp:{} e={} d={} q={}",
                mcp_state, snapshot.mcp_enabled, snapshot.mcp_disabled, snapshot.queue_depth
            )),
        ];
        for t in capped(&snapshot.active_tools, MAX_INFO_TOOLS, "tool") {
            out.push(safe_line(&t));
        }
        for w in capped(&snapshot.warnings, MAX_INFO_WARNINGS, "warn") {
            out.push(safe_line(&w));
        }
        out
    } else {
        let mut out = vec![
            safe_line(&format!(
                "CWD: {}",
                safe_label(&snapshot.cwd_label, MAX_CWD_CHARS)
            )),
            safe_line(&format!(
                "Workspace: {}  Session: {}",
                safe_label(&snapshot.workspace, MAX_LABEL_CHARS),
                safe_label(&snapshot.session_label, MAX_LABEL_CHARS)
            )),
            safe_line(&format!(
                "Provider: {}  Model: {}  Auth: {}",
                safe_label(&snapshot.provider_id, MAX_LABEL_CHARS),
                safe_label(&snapshot.model, MAX_LABEL_CHARS),
                safe_label(&snapshot.auth_mode, MAX_LABEL_CHARS)
            )),
            safe_line(&format!(
                "Context: used={} remaining={} window={}",
                snapshot.ctx_used, remaining, snapshot.ctx_window
            )),
            safe_line(&format!(
                "Tokens: in={} out={} cost={}",
                snapshot.tokens_in, snapshot.tokens_out, cost
            )),
            safe_line(&format!(
                "MCP: {} enabled={} disabled={} queue={}",
                mcp_state, snapshot.mcp_enabled, snapshot.mcp_disabled, snapshot.queue_depth
            )),
            safe_line(&format!(
                "Status: {}  Updated: {}",
                safe_label(&snapshot.status_line, MAX_LABEL_CHARS),
                snapshot.updated_ms
            )),
        ];
        let tools = tool_line(
            &snapshot.active_tools,
            counts.truncated && snapshot.active_tools.len() > MAX_INFO_TOOLS,
        );
        out.push(safe_line(&tools));
        let warns = warn_line(&snapshot.warnings);
        out.push(safe_line(&warns));
        out
    };
    Ok(lines.into_iter().map(|l| fit(&l, width)).collect())
}

fn capped(items: &[String], cap: usize, _kind: &str) -> Vec<String> {
    let mut out: Vec<String> = items
        .iter()
        .take(cap)
        .map(|s| safe_label(s, MAX_LABEL_CHARS))
        .collect();
    if items.len() > cap {
        out.push(format!("+{} more", items.len() - cap));
    }
    out
}

fn tool_line(tools: &[String], truncated: bool) -> String {
    let shown: Vec<String> = tools
        .iter()
        .take(MAX_INFO_TOOLS)
        .map(|s| safe_label(s, MAX_LABEL_CHARS))
        .collect();
    let mut s = if shown.is_empty() {
        "Tools: none".to_owned()
    } else {
        format!("Tools: {}", shown.join(", "))
    };
    if truncated || tools.len() > MAX_INFO_TOOLS {
        s.push_str(&format!(
            " +{} more",
            tools.len().saturating_sub(MAX_INFO_TOOLS)
        ));
    }
    s
}

fn warn_line(warnings: &[String]) -> String {
    let shown: Vec<String> = warnings
        .iter()
        .take(MAX_INFO_WARNINGS)
        .map(|s| safe_label(s, MAX_LABEL_CHARS))
        .collect();
    let mut s = if shown.is_empty() {
        "Warnings: none".to_owned()
    } else {
        format!("Warnings: {}", shown.join("; "))
    };
    if warnings.len() > MAX_INFO_WARNINGS {
        s.push_str(&format!(
            " +{} more",
            warnings.len().saturating_sub(MAX_INFO_WARNINGS)
        ));
    }
    s
}

fn safe_label(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let mut clean = String::new();
    for (i, ch) in trimmed.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '.' | '_' | ':' | '/' | '-' | ' ' | '=' | '(' | ')' | '+' | ','
            )
        {
            clean.push(ch);
        } else {
            clean.push('_');
        }
    }
    redact(&clean)
}

fn redact(value: &str) -> String {
    // Strip sk-style secret values only; structural words like "Tokens"
    // must survive verbatim.
    let mut out = value.to_owned();
    loop {
        let lower = out.to_lowercase();
        let Some(pos) = lower.find("sk-") else { break };
        let mut end = pos + 3;
        while end < out.len()
            && !out.as_bytes()[end].is_ascii_whitespace()
            && out.as_bytes()[end] != b','
        {
            end += 1;
        }
        if end == pos + 3 {
            break;
        }
        out.replace_range(pos..end, "[redacted]");
    }
    out
}

fn safe_line(value: &str) -> String {
    redact(value)
}

fn fit(value: &str, width: usize) -> String {
    if value.len() <= width {
        return value.to_owned();
    }
    // Keep truncation markers visible: cut the middle payload, not the tail.
    if let Some(pos) = value.find(" +") {
        let tail = &value[pos..];
        let head_room = width.saturating_sub(tail.len());
        if head_room >= 10 {
            let mut out: String = value.chars().take(head_room).collect();
            out.push_str(tail);
            if out.len() <= width {
                return out;
            }
        }
    }
    let mut out: String = value.chars().take(width.saturating_sub(3)).collect();
    out.push_str("...");
    out
}
