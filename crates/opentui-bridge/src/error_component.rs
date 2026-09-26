#![forbid(unsafe_code)]
//! Crash error view (mirrors `packages/tui/src/component/error-component.tsx`,
//! TS checkout a0d9b6c, NOT pinned 95daf90).
//!
//! TS headline always "opencode crashed" (`error-component.tsx:111`), message
//! fallback "An unknown error occurred." (`:43`), `openConsoleOnError: false`
//! (`app.tsx:201`) mirrored by `show_console_hint=false` in [`from_tagged`].
//! Message source follows `util/error.ts:5-17` branches via
//! [`crate::cli_error`] (nested `cause.body` wins, Cli/Account tags only).
//! Word-wrap inlined here; not importing collapse/text (cycle avoidance).

use crate::cli_error::{self, ErrorKind, TaggedError};

/// Max message lines kept.
pub const MAX_ERROR_LINES: usize = 32;
/// Max message chars kept.
pub const MAX_ERROR_CHARS: usize = 4096;
/// Fallback when message empty (`error-component.tsx:43`).
pub const FALLBACK_MESSAGE: &str = "An unknown error occurred.";
/// Hint row when console hint enabled.
pub const CONSOLE_HINT: &str = "Open console for details (app.console)";

/// Bounded crash view: title + message + console hint flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorView {
    pub title: String,
    pub message: String,
    pub show_console_hint: bool,
}

/// Char-clip then line-cap. Empty after trim gets fallback in constructor.
#[must_use]
pub fn bound_message(raw: &str) -> String {
    let clipped: String = raw.chars().take(MAX_ERROR_CHARS).collect();
    let lines: Vec<&str> = clipped.split('\n').collect();
    if lines.len() <= MAX_ERROR_LINES {
        return clipped;
    }
    lines[..MAX_ERROR_LINES].join("\n")
}

/// Greedy word-wrap one logical line at `width` chars (char count, not
/// display width; ponytail: upgrade path `unicode-width`). Long words hard-split.
fn wrap_logical(line: &str, width: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let mut cur = String::new();
    let mut cur_n = 0usize;
    for word in line.split(' ') {
        if word.chars().count() > width {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                cur_n = 0;
            }
            let chars: Vec<char> = word.chars().collect();
            for chunk in chars.chunks(width) {
                out.push(chunk.iter().collect());
            }
            continue;
        }
        let add = if cur_n == 0 { 0 } else { 1 } + word.chars().count();
        if cur_n + add > width {
            out.push(std::mem::take(&mut cur));
            cur_n = 0;
        }
        if cur_n > 0 {
            cur.push(' ');
        }
        cur.push_str(word);
        cur_n += add;
    }
    out.push(cur);
}

/// Innermost known tag in cause chain (mirrors `error.ts:6-9` precedence).
#[must_use]
fn innermost_kind(err: &TaggedError) -> ErrorKind {
    let mut chain: Vec<&TaggedError> = Vec::new();
    let mut cur = err;
    loop {
        chain.push(cur);
        match cur.cause.as_deref() {
            Some(next) => cur = next,
            None => break,
        }
    }
    for node in chain.iter().rev() {
        let k = cli_error::classify(&node.tag);
        if k != ErrorKind::Unknown {
            return k;
        }
    }
    ErrorKind::Unknown
}

impl ErrorView {
    pub fn new(title: &str, message: &str, show_console_hint: bool) -> Self {
        let title = if title.is_empty() { "opencode crashed".to_string() } else { title.to_string() };
        let bounded = bound_message(message);
        let message = if bounded.trim().is_empty() {
            FALLBACK_MESSAGE.to_string()
        } else {
            bounded
        };
        Self { title, message, show_console_hint }
    }

    /// Map `TaggedError` per `error.ts` branches; hint false (`app.tsx:201`).
    #[must_use]
    pub fn from_tagged(err: &TaggedError) -> Self {
        let title = match innermost_kind(err) {
            ErrorKind::Cli => "Error",
            ErrorKind::AccountService | ErrorKind::AccountTransport => "Account error",
            ErrorKind::Unknown => "opencode crashed",
        };
        let raw = cli_error::cli_error_message(err).unwrap_or_else(|| cli_error::message_of(err));
        Self::new(title, &raw, false)
    }

    /// Enable console hint (renderer with `openConsoleOnError: true`).
    #[must_use]
    pub fn with_console_hint(mut self) -> Self {
        self.show_console_hint = true;
        self
    }

    /// Title row, blank, wrapped message (re-capped to `MAX_ERROR_LINES`
    /// after wrap expansion), optional hint row. Zero width yields no rows.
    #[must_use]
    pub fn render_lines(&self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        let mut out = vec![self.title.clone(), String::new()];
        for logical in self.message.split('\n') {
            wrap_logical(logical, width, &mut out);
        }
        // Re-cap: wrap expansion must not break the line bound.
        if out.len() > MAX_ERROR_LINES + 2 {
            out.truncate(MAX_ERROR_LINES + 2);
        }
        if self.show_console_hint {
            out.push(String::new());
            out.push(CONSOLE_HINT.to_string());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_maps_title_and_message() {
        let v = ErrorView::from_tagged(&TaggedError::new("CliError", "boom"));
        assert_eq!(v.title, "Error");
        assert_eq!(v.message, "boom");
        assert!(!v.show_console_hint);
    }

    #[test]
    fn account_kinds_map_title() {
        for tag in ["AccountServiceError", "AccountTransportError"] {
            let v = ErrorView::from_tagged(&TaggedError::new(tag, "auth broke"));
            assert_eq!(v.title, "Account error");
            assert_eq!(v.message, "auth broke");
        }
    }

    #[test]
    fn unknown_falls_back_and_empty_message() {
        let v = ErrorView::from_tagged(&TaggedError::new("Nope", "x"));
        assert_eq!(v.title, "opencode crashed");
        assert_eq!(v.message, "x");
        let e = ErrorView::new("T", "   ", false);
        assert_eq!(e.message, FALLBACK_MESSAGE);
        let e2 = ErrorView::new("", "m", false);
        assert_eq!(e2.title, "opencode crashed");
    }

    #[test]
    fn nested_cause_wins_for_title_and_message() {
        let err = TaggedError::new("Nope", "outer")
            .with_cause(TaggedError::new("AccountServiceError", "inner"));
        let v = ErrorView::from_tagged(&err);
        assert_eq!(v.title, "Account error");
        assert_eq!(v.message, "inner");
    }

    #[test]
    fn bounds_chars_and_lines() {
        let long = "y".repeat(MAX_ERROR_CHARS + 100);
        let v = ErrorView::new("T", &long, false);
        assert_eq!(v.message.chars().count(), MAX_ERROR_CHARS);
        let many = (0..MAX_ERROR_LINES + 10).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n");
        let v2 = ErrorView::new("T", &many, false);
        assert_eq!(v2.message.split('\n').count(), MAX_ERROR_LINES);
    }

    #[test]
    fn render_wraps_and_hint() {
        let v = ErrorView::new("Error", "aa bb cc dd", false).with_console_hint();
        let lines = v.render_lines(5);
        assert_eq!(lines[0], "Error");
        assert!(lines.contains(&CONSOLE_HINT.to_string()));
        for l in lines.iter().filter(|l| !l.is_empty() && *l != CONSOLE_HINT && *l != "Error") {
            assert!(l.chars().count() <= 5, "over width: {l:?}");
        }
        assert!(ErrorView::new("T", "m", false).render_lines(0).is_empty());
        // Long word hard-splits.
        let w = ErrorView::new("T", "abcdefgh", false).render_lines(3);
        assert!(w.contains(&"abc".to_string()) && w.contains(&"def".to_string()));
    }
}
