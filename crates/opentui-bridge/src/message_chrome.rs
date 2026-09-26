#![forbid(unsafe_code)]
//! Message chrome: user/assistant headers, footers, reasoning labels (std only).
//!
//! Evidence (TS checkout a0d9b6c, `packages/tui/src/routes/session/index.tsx`):
//! - agent color `local.agent.color(...)` (:1374 user, :1543 assistant)
//! - ` QUEUED ` badge (:1436); timestamp `Locale.todayTimeOrDateTime` (:1429)
//! - compaction marker `find(type === "compaction")` (:1378), title `" Compaction "` (:1446)
//! - footer `▣ ` (:1546), titlecase mode (:1548), ` · {model}` (:1549),
//!   ` · {duration}` (:1551), ` · interrupted` (:1554)
//! - `PART_MAPPING` text/tool/reasoning (:1564-1568)
//! - REDACTED strip `.replace("[REDACTED]", "").trim()` (:1581)
//! - `Thinking: ` / `Thinking` (:1652), `Thought` (:1660), `": "` (:1661-1663),
//!   `" · "` duration sep (:1667-1671)
//!
//! Part bodies reuse [`crate::message_render::MessagePart`]; not redefined here.
//! Bounds (color 32, footer 256) are fail-closed port caps; no upstream equivalent.

use crate::message_render::MessagePart;

/// Fail-closed char bound for agent color (no upstream equivalent).
pub const MAX_COLOR_LEN: usize = 32;
/// Fail-closed char bound for footer line (no upstream equivalent).
pub const MAX_FOOTER_LEN: usize = 256;

fn bound_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// User vs assistant chrome (`UserMessage` :1350, `AssistantMessage` :1455).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeKind {
    User,
    Assistant,
}

/// Header/footer chrome for one message. Color/footer fail-closed bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageChrome {
    pub kind: ChromeKind,
    pub agent_color: String,
    pub queued: bool,
    pub timestamp: Option<String>,
    pub footer: Option<String>,
}

impl MessageChrome {
    #[must_use]
    pub fn new(kind: ChromeKind, agent_color: &str) -> Self {
        Self {
            kind,
            agent_color: bound_chars(agent_color, MAX_COLOR_LEN),
            queued: false,
            timestamp: None,
            footer: None,
        }
    }

    /// User: ` QUEUED ` (:1436) wins over timestamp (:1429).
    /// Assistant: `▣ {footer}` (:1546). Else empty.
    #[must_use]
    pub fn header_line(&self) -> String {
        match self.kind {
            ChromeKind::User => {
                if self.queued {
                    return " QUEUED ".to_string();
                }
                self.timestamp.clone().unwrap_or_default()
            }
            ChromeKind::Assistant => match &self.footer {
                Some(f) => format!("▣ {f}"),
                None => String::new(),
            },
        }
    }
}

/// Mirror `:1581`: drop OpenRouter `"[REDACTED]"` placeholder, trim.
#[must_use]
pub fn strip_redacted(text: &str) -> String {
    text.replace("[REDACTED]", "").trim().to_string()
}

/// Reasoning header label: done `Thought` (:1660), running `Thinking` (:1652).
#[must_use]
pub const fn thought_header(summary: bool) -> &'static str {
    if summary {
        "Thought"
    } else {
        "Thinking"
    }
}

/// Footer join `{mode} · {model}[ · {duration}][ · interrupted]`
/// (:1548 mode, :1549 model, :1551 duration, :1554 interrupted).
/// Fail-closed at [`MAX_FOOTER_LEN`] chars.
#[must_use]
pub fn footer_line(mode: &str, model: &str, duration: Option<&str>, interrupted: bool) -> String {
    let mut out = format!("{mode} · {model}");
    if let Some(d) = duration {
        out.push_str(" · ");
        out.push_str(d);
    }
    if interrupted {
        out.push_str(" · interrupted");
    }
    bound_chars(&out, MAX_FOOTER_LEN)
}

/// Port of `PART_MAPPING` (:1564-1568). Unknown kinds fail closed to `""`.
pub struct PartMap;

impl PartMap {
    #[must_use]
    pub fn map(part_kind: &str) -> &'static str {
        match part_kind {
            "text" => "TextPart",
            "tool" => "ToolPart",
            "reasoning" => "ReasoningPart",
            _ => "",
        }
    }

    /// Same mapping via [`MessagePart`] (no reasoning body variant there).
    #[must_use]
    pub fn map_part(part: &MessagePart) -> &'static str {
        match part {
            MessagePart::Text(_) => "TextPart",
            MessagePart::ToolCall { .. } => "ToolPart",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_beats_timestamp() {
        let mut c = MessageChrome::new(ChromeKind::User, "red");
        c.queued = true;
        c.timestamp = Some("today".to_string());
        assert_eq!(c.header_line(), " QUEUED ");
    }

    #[test]
    fn timestamp_passthrough() {
        let mut c = MessageChrome::new(ChromeKind::User, "red");
        c.timestamp = Some("Jun 6, 2026".to_string());
        assert_eq!(c.header_line(), "Jun 6, 2026");
        let bare = MessageChrome::new(ChromeKind::User, "red");
        assert_eq!(bare.header_line(), "");
    }

    #[test]
    fn assistant_footer_glyph() {
        let mut c = MessageChrome::new(ChromeKind::Assistant, "blue");
        c.footer = Some(footer_line("Build", "gpt-x", Some("1s"), false));
        assert_eq!(c.header_line(), "▣ Build · gpt-x · 1s");
        let bare = MessageChrome::new(ChromeKind::Assistant, "blue");
        assert_eq!(bare.header_line(), "");
    }

    #[test]
    fn interrupted_footer() {
        assert_eq!(footer_line("Build", "m", None, true), "Build · m · interrupted");
        assert_eq!(footer_line("Plan", "m", Some("2s"), false), "Plan · m · 2s");
    }

    #[test]
    fn bounds_fail_closed() {
        let c = MessageChrome::new(ChromeKind::User, &"c".repeat(40));
        assert_eq!(c.agent_color.chars().count(), MAX_COLOR_LEN);
        let f = footer_line(&"m".repeat(300), "x", None, false);
        assert_eq!(f.chars().count(), MAX_FOOTER_LEN);
    }

    #[test]
    fn strip_redacted_trims() {
        assert_eq!(strip_redacted("a [REDACTED] b"), "a  b");
        assert_eq!(strip_redacted("  [REDACTED]  "), "");
        assert_eq!(strip_redacted("clean"), "clean");
    }

    #[test]
    fn thought_labels() {
        assert_eq!(thought_header(true), "Thought");
        assert_eq!(thought_header(false), "Thinking");
    }

    #[test]
    fn part_mapping() {
        assert_eq!(PartMap::map("text"), "TextPart");
        assert_eq!(PartMap::map("tool"), "ToolPart");
        assert_eq!(PartMap::map("reasoning"), "ReasoningPart");
        assert_eq!(PartMap::map("file"), "");
        assert_eq!(PartMap::map_part(&MessagePart::text("hi")), "TextPart");
        assert_eq!(
            PartMap::map_part(&MessagePart::tool_call("bash", "running")),
            "ToolPart"
        );
    }
}
