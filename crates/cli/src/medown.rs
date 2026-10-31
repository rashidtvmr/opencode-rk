#![forbid(unsafe_code)]
//! Minimal markdown renderer state (pure, no IO).
//!
//! Pattern: `native_status.rs` pure state. Parses `**bold**` and `` `code` ``
//! into [`Span`]s, strips `#` heading prefixes, exposes pure
//! [`render_plain`] that strips markers. Output capped at [`MAX_OUT`].

/// Max rendered/plain output bytes (char-boundary truncated).
pub const MAX_OUT: usize = 16384;

/// One inline run with style flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub code: bool,
}

fn strip_heading(line: &str) -> &str {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i] == b'#' {
        i += 1;
    }
    if i == 0 {
        return line;
    }
    if i < b.len() && b[i] == b' ' {
        i += 1;
    }
    line.get(i..).unwrap_or("")
}

fn truncate_floor(s: &mut String, max: usize) {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
}

/// Parse one line of inline markdown into styled spans.
/// Strips a leading `#` heading prefix; `**` toggles bold, `` ` `` toggles
/// code. Total text capped at [`MAX_OUT`] bytes.
#[must_use]
pub fn parse_inline(input: &str) -> Vec<Span> {
    let text = strip_heading(input);
    let mut spans: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut bold = false;
    let mut code = false;
    let flush = |buf: &mut String, spans: &mut Vec<Span>, bold: bool, code: bool| {
        if !buf.is_empty() {
            spans.push(Span {
                text: std::mem::take(buf),
                bold,
                code,
            });
        }
    };
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            flush(&mut buf, &mut spans, bold, code);
            code = !code;
            continue;
        }
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            flush(&mut buf, &mut spans, bold, code);
            bold = !bold;
            continue;
        }
        buf.push(c);
    }
    flush(&mut buf, &mut spans, bold, code);
    let mut budget = MAX_OUT;
    for s in spans.iter_mut() {
        if budget == 0 {
            s.text.clear();
        } else if s.text.len() > budget {
            truncate_floor(&mut s.text, budget);
            budget = 0;
        } else {
            budget -= s.text.len();
        }
    }
    spans.retain(|s| !s.text.is_empty());
    spans
}

/// Reasoning disclosure split: an optional bold title block plus the body.
///
/// Upstream: `packages/tui/src/context/thinking.ts:9-14` returns
/// `{ title: null, body: content }` when the leading bold block does not match,
/// so `body` is the trimmed whole text in that case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReasoningSummary {
    pub title: Option<String>,
    pub body: String,
}

/// Recognise the OpenAI Responses reasoning title block.
///
/// Upstream: `packages/tui/src/context/thinking.ts:10-14`, regex
/// `^\*\*([^*\n]+)\*\*(?:\r?\n\r?\n|$)` applied to `text.trim()`. A title is
/// only metadata when it is the whole text (still streaming) or is followed by
/// a blank line; `**Title**\nbody` therefore stays an ordinary body.
#[must_use]
pub fn reasoning_summary(text: &str) -> ReasoningSummary {
    let content = text.trim();
    if let Some(rest) = content.strip_prefix("**") {
        if let Some(close) = rest.find(['*', '\n']) {
            let title = &rest[..close];
            let after = &rest[close..];
            if !title.is_empty() {
                if let Some(tail) = after.strip_prefix("**") {
                    // Upstream consumes exactly the terminator and then applies
                    // `trimEnd()`; any further leading newlines stay in the body
                    // (`thinking.ts:13`).
                    let terminator = if tail.starts_with("\r\n\r\n") {
                        4
                    } else if tail.starts_with("\n\n") {
                        2
                    } else if tail.is_empty() {
                        0
                    } else {
                        return ReasoningSummary {
                            title: None,
                            body: content.to_string(),
                        };
                    };
                    return ReasoningSummary {
                        title: Some(title.trim().to_string()),
                        body: tail[terminator..].trim_end().to_string(),
                    };
                }
            }
        }
    }
    ReasoningSummary {
        title: None,
        body: content.to_string(),
    }
}

/// Drop the encryption placeholder OpenRouter inserts into reasoning text.
///
/// Upstream drops every occurrence (`packages/tui/src/mini/entry.body.ts:63`,
/// `/[REDACTED]/g`); `routes/session/message-parts.tsx:121` only drops the
/// first, which could leave a stray placeholder visible. Removal is global
/// here so redacted-only content never renders text.
#[must_use]
pub fn reasoning_content(text: &str) -> String {
    text.replace("[REDACTED]", "").trim().to_string()
}

/// Render-ready reasoning body, or `None` when nothing visible remains.
///
/// Upstream: `packages/tui/src/mini/entry.body.ts:62-65` returns no body at all
/// when the placeholder-stripped text is empty (MD-006: redacted-only reasoning
/// renders nothing).
#[must_use]
pub fn reasoning_body(text: &str) -> Option<String> {
    let body = reasoning_content(text);
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// Strip markdown markers (`**`, `` ` ``, `#` heading prefixes) to plain
/// text, line by line. Capped at [`MAX_OUT`] bytes.
#[must_use]
pub fn render_plain(input: &str) -> String {
    let mut out = String::new();
    for (i, line) in input.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if out.len() >= MAX_OUT {
                break;
            }
        }
        let stripped = strip_heading(line).replace("**", "").replace('`', "");
        // Respect byte budget on char boundary per line push.
        let room = MAX_OUT.saturating_sub(out.len());
        if stripped.len() > room {
            let mut cut = room;
            while !stripped.is_char_boundary(cut) {
                cut = cut.saturating_sub(1);
            }
            out.push_str(&stripped[..cut]);
            break;
        }
        out.push_str(&stripped);
    }
    truncate_floor(&mut out, MAX_OUT);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_markers_stripped() {
        assert_eq!(render_plain("a **b** c"), "a b c");
        let spans = parse_inline("a **b** c");
        assert_eq!(spans.len(), 3);
        assert!(spans[1].bold);
        assert_eq!(spans[1].text, "b");
    }

    #[test]
    fn code_markers_stripped() {
        assert_eq!(render_plain("a `b` c"), "a b c");
        let spans = parse_inline("a `b` c");
        assert!(spans[1].code);
        assert_eq!(spans[1].text, "b");
    }

    #[test]
    fn heading_prefix_stripped() {
        assert_eq!(render_plain("# hello"), "hello");
        assert_eq!(render_plain("## hi"), "hi");
        assert_eq!(parse_inline("# hello")[0].text, "hello");
    }

    #[test]
    fn output_capped() {
        let big = "x".repeat(MAX_OUT + 100);
        assert_eq!(render_plain(&big).len(), MAX_OUT);
        let total: usize = parse_inline(&big).iter().map(|s| s.text.len()).sum();
        assert_eq!(total, MAX_OUT);
    }

    /// MD-006. Upstream `packages/tui/src/context/thinking.ts:10-14`.
    #[test]
    fn reasoning_title_block_splits_from_body() {
        let summary = reasoning_summary("**Inspecting PR workflow**\n\nWe looked at the diff.");
        assert_eq!(summary.title.as_deref(), Some("Inspecting PR workflow"));
        assert_eq!(summary.body, "We looked at the diff.");
        // CRLF terminator (`(?:\r?\n\r?\n)` in the upstream regex).
        let crlf = reasoning_summary("**T**\r\n\r\nbody\r\n");
        assert_eq!(crlf.title.as_deref(), Some("T"));
        assert_eq!(crlf.body, "body");
        // Streaming: a complete title still awaiting its body is metadata.
        let awaiting = reasoning_summary("**Planning next steps**");
        assert_eq!(awaiting.title.as_deref(), Some("Planning next steps"));
        assert_eq!(awaiting.body, "");
        // One extra leading blank line is part of the body (upstream only
        // consumes the matched terminator, then `trimEnd()`).
        let extra = reasoning_summary("**T**\n\n\nbody");
        assert_eq!(extra.body, "\nbody");
    }

    /// MD-006. Ordinary prose keeps its markers as body text.
    #[test]
    fn reasoning_non_title_blocks_stay_body() {
        let prose = reasoning_summary("**Not** a title block\nsecond line");
        assert_eq!(prose.title, None);
        assert_eq!(prose.body, "**Not** a title block\nsecond line");
        let inline = reasoning_summary("We **bolded** a word\n\nand continued");
        assert_eq!(inline.title, None);
        assert_eq!(inline.body, "We **bolded** a word\n\nand continued");
        let empty = reasoning_summary("   ");
        assert_eq!(empty.title, None);
        assert_eq!(empty.body, "");
    }

    /// MD-006. Upstream `mini/entry.body.ts:62-65`: redacted-only content
    /// yields no body, so the reasoning block renders nothing.
    #[test]
    fn reasoning_redacted_only_renders_nothing() {
        assert_eq!(reasoning_body("[REDACTED]"), None);
        assert_eq!(reasoning_body("  [REDACTED]\n"), None);
        assert_eq!(reasoning_content("[REDACTED]  [REDACTED]"), "");
        assert_eq!(reasoning_body("kept [REDACTED] text").as_deref(), Some("kept  text"));
        assert_eq!(reasoning_content("  [REDACTED]body  "), "body");
    }
}
