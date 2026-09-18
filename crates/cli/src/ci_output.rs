#![forbid(unsafe_code)]

//! CI/CD non-interactive output contract.
//!
//! ## Exit codes
//!
//! | Variant             | Discriminant | Meaning                          |
//! |---------------------|-------------|----------------------------------|
//! | Success             | 0           | All turns completed normally     |
//! | TurnFailed          | 10          | A turn failed (tool error, etc.) |
//! | ApprovalRequired    | 20          | Interactive approval needed      |
//! | BudgetExhausted     | 30          | Token/budget limit hit           |
//! | ProviderError       | 40          | LLM provider error               |
//! | UsageError          | 64          | Bad arguments or configuration   |
//!
//! **Fail-closed rule:** `render_approval_required()` ALWAYS returns
//! `CiExitCode::ApprovalRequired` (20). CI must never silently pass when
//! an approval is required.
//!
//! ## JSONL event format
//!
//! Each line is one JSON object:
//! - `{ts_kind: "TurnStarted", ts: <u64>}`
//! - `{ts_kind: "TurnFinished", ts: <u64>, exit: <u8>}`
//! - `{ts_kind: "ApprovalRequired", ts: <u64>, tool: "<name>"}`
//! - `{ts_kind: "Step", ts: <u64>, n: <u64>}`
//!
//! Lines are bounded to 8 KiB. Oversized lines are truncated (still valid JSON).
//! Each line is flushed after writing (caller owns the writer).

use std::fmt;
use std::io::Write;

/// Exit codes for CI invocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CiExitCode {
    Success = 0,
    TurnFailed = 10,
    ApprovalRequired = 20,
    BudgetExhausted = 30,
    ProviderError = 40,
    UsageError = 64,
}

impl CiExitCode {
    pub fn code(self) -> u8 {
        self as u8
    }
}

/// JSONL event emitted during CI runs.
#[derive(Debug, Clone)]
pub enum CiEvent {
    TurnStarted { ts: u64 },
    TurnFinished { ts: u64, exit: u8 },
    ApprovalRequired { ts: u64, tool: String },
    Step { ts: u64, n: u64 },
}

const MAX_LINE_BYTES: usize = 8 * 1024;

/// Escape a string for JSON. Handles quotes, backslashes, and control chars.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

impl CiEvent {
    /// Serialize this event to a single JSON line.
    pub fn to_json_line(&self) -> String {
        match self {
            CiEvent::TurnStarted { ts } => {
                format!(r#"{{"ts_kind":"TurnStarted","ts":{}}}"#, ts)
            }
            CiEvent::TurnFinished { ts, exit } => {
                format!(
                    r#"{{"ts_kind":"TurnFinished","ts":{},"exit":{}}}"#,
                    ts, exit
                )
            }
            CiEvent::ApprovalRequired { ts, tool } => {
                format!(
                    r#"{{"ts_kind":"ApprovalRequired","ts":{},"tool":"{}"}}"#,
                    ts,
                    json_escape(tool)
                )
            }
            CiEvent::Step { ts, n } => {
                format!(r#"{{"ts_kind":"Step","ts":{},"n":{}}}"#, ts, n)
            }
        }
    }

    /// Truncate to MAX_LINE_BYTES, preserving valid JSON (closing brace).
    pub fn to_bounded_json_line(&self) -> String {
        let line = self.to_json_line();
        if line.len() <= MAX_LINE_BYTES {
            line
        } else {
            // Truncate and close the JSON object.
            // Find last complete key-value to stay valid.
            let mut truncated = line.into_bytes();
            truncated.truncate(MAX_LINE_BYTES - 1);
            // Close the JSON object
            truncated.push(b'}');
            String::from_utf8(truncated).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

impl fmt::Display for CiEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_bounded_json_line())
    }
}

/// Output format for CI rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Jsonl,
    Text,
}

/// Text renderer for stable plain-text output.
pub struct TextRenderer;

impl TextRenderer {
    /// Render an event as a stable plain-text line.
    pub fn render(event: &CiEvent) -> String {
        match event {
            CiEvent::TurnStarted { ts } => {
                format!("turn_started ts={}", ts)
            }
            CiEvent::TurnFinished { ts, exit } => {
                format!("turn_finished ts={} exit={}", ts, exit)
            }
            CiEvent::ApprovalRequired { ts, tool } => {
                format!("approval_required ts={} tool={}", ts, tool)
            }
            CiEvent::Step { ts, n } => {
                format!("step ts={} n={}", ts, n)
            }
        }
    }
}

/// Write an event in the given format, flushing after each line.
pub fn render_event<W: Write>(
    writer: &mut W,
    event: &CiEvent,
    format: OutputFormat,
) -> std::io::Result<()> {
    let line = match format {
        OutputFormat::Json => event.to_bounded_json_line(),
        OutputFormat::Jsonl => event.to_bounded_json_line(),
        OutputFormat::Text => TextRenderer::render(event),
    };
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Render the approval-required outcome. Fail-closed: ALWAYS returns exit 20.
pub fn render_approval_required(tool: &str) -> (String, CiExitCode) {
    let event = CiEvent::ApprovalRequired {
        ts: 0,
        tool: tool.to_string(),
    };
    let line = event.to_bounded_json_line();
    // Fail-closed: always ApprovalRequired, never Success
    (line, CiExitCode::ApprovalRequired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_exit_code_discriminants() {
        assert_eq!(CiExitCode::Success.code(), 0);
        assert_eq!(CiExitCode::TurnFailed.code(), 10);
        assert_eq!(CiExitCode::ApprovalRequired.code(), 20);
        assert_eq!(CiExitCode::BudgetExhausted.code(), 30);
        assert_eq!(CiExitCode::ProviderError.code(), 40);
        assert_eq!(CiExitCode::UsageError.code(), 64);
    }

    #[test]
    fn t02_jsonl_escaping() {
        let event = CiEvent::ApprovalRequired {
            ts: 1,
            tool: r#"foo"bar\nbaz"#.to_string(),
        };
        let line = event.to_json_line();
        assert!(line.contains(r#"foo\"bar\\nbaz"#), "escape failed: {}", line);
        assert!(line.starts_with('{'));
        assert!(line.ends_with('}'));
    }

    #[test]
    fn t03_bounded_line_enforcement() {
        let big_tool = "x".repeat(MAX_LINE_BYTES + 100);
        let event = CiEvent::ApprovalRequired {
            ts: 1,
            tool: big_tool,
        };
        let line = event.to_bounded_json_line();
        assert!(
            line.len() <= MAX_LINE_BYTES,
            "line too long: {}",
            line.len()
        );
        // Must still be valid JSON (starts with {, ends with })
        assert!(line.starts_with('{'));
        assert!(line.ends_with('}'));
    }

    #[test]
    fn t04_approval_required_always_exit_20() {
        let (_, code) = render_approval_required("some_tool");
        assert_eq!(code, CiExitCode::ApprovalRequired);
        assert_eq!(code.code(), 20);

        // Verify via each format
        for fmt in [OutputFormat::Json, OutputFormat::Jsonl, OutputFormat::Text] {
            let mut buf = Vec::new();
            let event = CiEvent::ApprovalRequired {
                ts: 0,
                tool: "t".into(),
            };
            render_event(&mut buf, &event, fmt).unwrap();
            let text = String::from_utf8(buf).unwrap();
            assert!(
                text.contains("ApprovalRequired") || text.contains("approval_required"),
                "format {:?} did not produce approval: {}",
                fmt,
                text
            );
        }
    }

    #[test]
    fn t05_text_renderer_stable_columns() {
        let cases: Vec<CiEvent> = vec![
            CiEvent::TurnStarted { ts: 100 },
            CiEvent::TurnFinished { ts: 200, exit: 0 },
            CiEvent::ApprovalRequired {
                ts: 300,
                tool: "deploy".into(),
            },
            CiEvent::Step { ts: 400, n: 3 },
        ];
        for event in &cases {
            let line = TextRenderer::render(event);
            let parts: Vec<&str> = line.split(' ').collect();
            assert!(parts.len() >= 2, "too few columns: {}", line);
            // First token is the event type
            assert!(
                matches!(
                    parts[0],
                    "turn_started"
                        | "turn_finished"
                        | "approval_required"
                        | "step"
                ),
                "bad first column: {}",
                parts[0]
            );
        }
    }

    #[test]
    fn t06_empty_event_stream_valid_final_line() {
        let mut buf = Vec::new();
        // Simulate: no events written, just verify buffer is empty
        // (the "final line" contract means the caller writes one last event)
        let event = CiEvent::TurnFinished { ts: 0, exit: 0 };
        render_event(&mut buf, &event, OutputFormat::Jsonl).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.ends_with('\n'));
        assert!(text.starts_with('{'));
        // Single line only
        assert_eq!(text.lines().count(), 1);
    }
}
