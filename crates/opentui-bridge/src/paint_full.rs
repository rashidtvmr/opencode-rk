#![forbid(unsafe_code)]
//! Full-frame painter: title, status, transcript window, composer, footer.
//!
//! Mirrors `crates/cli/src/tui_entry.rs:452-543` (`native_page_lines`, Chat
//! branch): clamped width/height, title row, status row, separator rule,
//! transcript tail window, composer row, footer hints, truncate to height
//! with per-line width clip. Transcript rows reuse
//! [`crate::transcript_paint::paint_lines`]; every row is clipped with
//! [`crate::unicode_width::clip_to_width`].
//! ponytail: plain-text rows only, no color spans; add styling when a
//! theme/span crate is accepted.

use crate::transcript_paint::paint_lines;
use crate::unicode_width::clip_to_width;

/// Input for one full chat frame.
pub struct FrameInput {
    /// Title row (e.g. `"OpenCode RK -- <session>"`).
    pub title: String,
    /// Status row (e.g. model + key hints).
    pub status_line: String,
    /// Raw transcript bodies, oldest first.
    pub transcript_lines: Vec<String>,
    /// Current composer draft (without `"> "` prefix).
    pub draft: String,
    /// Footer hint segments joined with `" | "`.
    pub footer_hints: Vec<String>,
    /// Requested frame width (columns); clamped to >= 20.
    pub width: usize,
    /// Requested frame height (rows); clamped to >= 8.
    pub height: usize,
}

/// Default footer when [`FrameInput::footer_hints`] is empty.
pub const DEFAULT_FOOTER: &str = "Enter send | Backspace edit | Ctrl+P commands";

/// Placeholder body when there is no transcript yet.
pub const EMPTY_PLACEHOLDER: &str = "Start typing to send a turn.";

fn clip(s: &str, width: usize) -> String {
    clip_to_width(s, width).0
}

/// Paint one full chat frame of exactly `height` (clamped) rows.
///
/// Layout mirrors `tui_entry.rs` Chat branch: title, status, rule,
/// transcript tail window (`height - 7` rows), blank pad to `height - 3`,
/// rule, `"> draft"` composer, footer. Every row is width-clipped.
#[must_use]
pub fn paint_frame(input: &FrameInput) -> Vec<String> {
    let width = input.width.max(20);
    let height = input.height.max(8);
    let mut lines = Vec::with_capacity(height);
    lines.push(clip(&input.title, width));
    lines.push(clip(&input.status_line, width));
    lines.push(clip(&"─".repeat(width.min(120)), width));

    let body_rows = height.saturating_sub(7);
    if input.transcript_lines.is_empty() {
        lines.push(clip(EMPTY_PLACEHOLDER, width));
    } else {
        let entries: Vec<(String, String)> = input
            .transcript_lines
            .iter()
            .map(|l| ("assistant".to_string(), l.clone()))
            .collect();
        let painted = paint_lines(&entries, width);
        let start = painted.len().saturating_sub(body_rows);
        lines.extend(painted[start..].iter().cloned());
    }
    while lines.len() < height.saturating_sub(3) {
        lines.push(String::new());
    }

    lines.push(clip(&"─".repeat(width.min(120)), width));
    lines.push(clip(&format!("> {}", input.draft), width));
    let footer = if input.footer_hints.is_empty() {
        DEFAULT_FOOTER.to_string()
    } else {
        input.footer_hints.join(" | ")
    };
    lines.push(clip(&footer, width));

    lines.truncate(height);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> FrameInput {
        FrameInput {
            title: "OpenCode RK -- chat".into(),
            status_line: "model: x | Ctrl+P palette".into(),
            transcript_lines: vec!["hello".into(), "world".into()],
            draft: "hi".into(),
            footer_hints: vec!["Enter send".into(), "? help".into()],
            width: 40,
            height: 12,
        }
    }

    #[test]
    fn frame_height_truncates() {
        let mut i = input();
        i.height = 8;
        let out = paint_frame(&i);
        assert_eq!(out.len(), 8);
    }

    #[test]
    fn lines_clip_to_width() {
        let mut i = input();
        i.title = "t".repeat(100);
        i.width = 20;
        i.height = 8;
        let out = paint_frame(&i);
        assert!(out
            .iter()
            .all(|l| crate::unicode_width::line_width(l) <= 20));
        assert_eq!(out.len(), 8);
    }

    #[test]
    fn empty_transcript_placeholder() {
        let mut i = input();
        i.transcript_lines.clear();
        let out = paint_frame(&i);
        assert!(out.iter().any(|l| l.contains(EMPTY_PLACEHOLDER)));
    }

    #[test]
    fn draft_shown() {
        let out = paint_frame(&input());
        assert!(out.iter().any(|l| l == "> hi"));
    }

    #[test]
    fn hints_shown() {
        let out = paint_frame(&input());
        assert!(out
            .iter()
            .any(|l| l.contains("Enter send") && l.contains("? help")));
    }

    #[test]
    fn clamps_minimum() {
        let mut i = input();
        i.width = 5;
        i.height = 2;
        let out = paint_frame(&i);
        assert_eq!(out.len(), 8);
        assert!(out
            .iter()
            .all(|l| crate::unicode_width::line_width(l) <= 20));
    }
}
