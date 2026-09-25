#![forbid(unsafe_code)]
//! Bridge-side adapter over [`crate::paint_full::paint_frame`].
//!
//! Mirrors `crates/cli/src/tui_entry.rs:483-509` (Chat branch): transcript
//! tail window oldest-first, composer `"> draft"` row, footer hints.
//! ponytail: plain rows only; add styling when a span/theme crate lands.

use crate::paint_full::{paint_frame, FrameInput};

fn cap(s: &str) -> String {
    s.chars().take(256).collect()
}

/// Owned chat-frame config for the bridge side.
pub struct PaintAdapter {
    /// Title row; capped to 256 chars.
    pub title: String,
    /// Status row; capped to 256 chars.
    pub status: String,
    /// Frame width (columns); clamped to >= 20 at paint time.
    pub width: usize,
    /// Frame height (rows); clamped to >= 8 at paint time.
    pub height: usize,
}

impl PaintAdapter {
    /// Build with 256-char caps applied to title/status.
    #[must_use]
    pub fn new(title: &str, status: &str, width: usize, height: usize) -> Self {
        Self {
            title: cap(title),
            status: cap(status),
            width,
            height,
        }
    }

    /// Paint one frame; transcript is oldest-first, footer defaults.
    #[must_use]
    pub fn build_frame(&self, transcript: &[String], draft: &str) -> Vec<String> {
        paint_frame(&FrameInput {
            title: self.title.clone(),
            status_line: self.status.clone(),
            transcript_lines: transcript.to_vec(),
            draft: draft.to_string(),
            footer_hints: Vec::new(),
            width: self.width,
            height: self.height,
        })
    }

    /// Clamped frame height (>= 8).
    #[must_use]
    pub fn frame_height(&self) -> usize {
        self.height.max(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> PaintAdapter {
        PaintAdapter::new("t", "s", 40, 12)
    }

    #[test]
    fn empty_placeholder() {
        let out = adapter().build_frame(&[], "");
        assert!(out.iter().any(|l| l.contains("Start typing")));
    }

    #[test]
    fn tail_window() {
        let a = PaintAdapter::new("t", "s", 40, 8);
        let t: Vec<String> = (0..10).map(|i| format!("m{i}")).collect();
        let out = a.build_frame(&t, "");
        assert!(!out.iter().any(|l| l == "m0"));
        assert!(out.iter().any(|l| l == "assistant> m9"));
    }

    #[test]
    fn height_clamp() {
        let a = PaintAdapter::new("t", "s", 40, 2);
        assert_eq!(a.frame_height(), 8);
        assert_eq!(a.build_frame(&[], "").len(), 8);
    }

    #[test]
    fn width_clip() {
        let a = PaintAdapter::new(&"t".repeat(100), "s", 20, 8);
        let out = a.build_frame(&[], "");
        assert!(out
            .iter()
            .all(|l| crate::unicode_width::line_width(l) <= 20));
    }

    #[test]
    fn draft_row() {
        let out = adapter().build_frame(&["hi".into()], "abc");
        assert!(out.iter().any(|l| l == "> abc"));
    }

    #[test]
    fn caps_title_status() {
        let a = PaintAdapter::new(&"x".repeat(300), &"y".repeat(300), 40, 8);
        assert_eq!(a.title.chars().count(), 256);
        assert_eq!(a.status.chars().count(), 256);
    }
}
