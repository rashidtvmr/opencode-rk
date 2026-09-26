#![forbid(unsafe_code)]
//! Chat paint callsite: frame lines via [`PaintAdapter`].
//!
//! Thin wrapper so `tui_entry.rs:paint_native` sources rows from the bridge
//! instead of hand-rolling frame math. ponytail: title/status rows only;
//! add spans when a theme crate lands.

use crate::paint_adapter::PaintAdapter;

/// Build one framed chat frame; transcript is oldest-first.
#[must_use]
pub fn build_chat_lines(
    title: &str,
    status: &str,
    transcript: &[String],
    draft: &str,
    width: usize,
    height: usize,
) -> Vec<String> {
    PaintAdapter::new(title, status, width, height).build_frame(transcript, draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn frame_has_title_and_composer() {
        let out = build_chat_lines("T", "S", &t(&["hi"]), "abc", 40, 12);
        assert_eq!(out.len(), 12);
        assert_eq!(out[0], "T");
        assert!(out.iter().any(|l| l == "> abc"));
    }

    #[test]
    fn empty_transcript_placeholder() {
        let out = build_chat_lines("T", "S", &[], "", 40, 8);
        assert!(out.iter().any(|l| l.contains("Start typing")));
    }

    #[test]
    fn clamps_small_dims() {
        let out = build_chat_lines("T", "S", &[], "", 5, 2);
        assert_eq!(out.len(), 8);
    }

    #[test]
    fn clips_wide_rows() {
        let wide = "y".repeat(100);
        let out = build_chat_lines(&"x".repeat(100), "S", &t(&[wide.as_str()]), "", 20, 8);
        assert!(out.iter().all(|l| l.chars().count() <= 20));
    }

    #[test]
    fn transcript_tail_kept() {
        let tr: Vec<String> = (0..10).map(|i| format!("m{i}")).collect();
        let out = build_chat_lines("T", "S", &tr, "", 40, 8);
        assert!(out.iter().any(|l| l == "assistant> m9"));
    }
}
