#![forbid(unsafe_code)]
//! Paint transcript entries into width-clipped terminal lines.
//!
//! Thin view over [`crate::transcript::render_entry`]: role prefix +
//! display-width clip ([`crate::unicode_width::clip_to_width`]). Tool
//! lifecycle markers mirror `transcript.ts:101-106` status strings
//! (`"completed"` / `"error"` verbatim).
//! ponytail: plain-text lines only, no color spans; add styling when a
//! theme/span crate is accepted.

use crate::unicode_width::clip_to_width;

/// Max painted lines per call (fail-closed bound).
pub const MAX_LINES: usize = 500;

/// Paint `(role, text)` entries into terminal lines of at most `width`
/// display columns. Role `"user"` paints `user> `, anything else paints
/// `assistant> `. A body containing `error` (case-insensitive) gains an
/// `[error] ` marker, else one containing `completed` gains `[ok] `.
/// Output caps at [`MAX_LINES`] lines; `width == 0` yields empty output.
#[must_use]
pub fn paint_lines(entries: &[(String, String)], width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (role, text) in entries {
        let prefix = if role == "user" {
            "user> "
        } else {
            "assistant> "
        };
        if text.is_empty() {
            let (line, _) = clip_to_width(prefix, width);
            out.push(line);
            if out.len() >= MAX_LINES {
                break;
            }
            continue;
        }
        for body in text.split('\n') {
            let lower = body.to_ascii_lowercase();
            let marker = if lower.contains("error") {
                "[error] "
            } else if lower.contains("completed") {
                "[ok] "
            } else {
                ""
            };
            let full = format!("{prefix}{marker}{body}");
            let (line, _) = clip_to_width(&full, width);
            out.push(line);
            if out.len() >= MAX_LINES {
                break;
            }
        }
        if out.len() >= MAX_LINES {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(r, t)| ((*r).to_string(), (*t).to_string()))
            .collect()
    }

    #[test]
    fn user_prefix() {
        let out = paint_lines(&entries(&[("user", "hello")]), 40);
        assert_eq!(out, vec!["user> hello".to_string()]);
    }

    #[test]
    fn assistant_prefix() {
        let out = paint_lines(&entries(&[("assistant", "hi")]), 40);
        assert_eq!(out, vec!["assistant> hi".to_string()]);
    }

    #[test]
    fn clips_to_width() {
        let out = paint_lines(&entries(&[("user", "abcdef")]), 8);
        assert_eq!(out, vec!["user> ab".to_string()]);
    }

    #[test]
    fn error_marker() {
        let out = paint_lines(&entries(&[("assistant", "tool failed: error x")]), 80);
        assert_eq!(
            out,
            vec!["assistant> [error] tool failed: error x".to_string()]
        );
    }

    #[test]
    fn completed_marker() {
        let out = paint_lines(&entries(&[("assistant", "job completed ok")]), 80);
        assert_eq!(out, vec!["assistant> [ok] job completed ok".to_string()]);
    }

    #[test]
    fn caps_at_500() {
        let many = vec!["x".to_string(); 10];
        let es: Vec<(String, String)> = many.into_iter().map(|t| ("user".to_string(), t)).collect();
        let out = paint_lines(&es, 80);
        assert!(out.len() <= MAX_LINES);
        let big: Vec<(String, String)> = (0..600)
            .map(|i| ("user".to_string(), format!("line {i}\nline {i}b")))
            .collect();
        assert_eq!(paint_lines(&big, 80).len(), MAX_LINES);
    }

    #[test]
    fn empty_input_empty_output() {
        assert!(paint_lines(&[], 80).is_empty());
        assert!(paint_lines(&entries(&[]), 0).is_empty());
    }
}
