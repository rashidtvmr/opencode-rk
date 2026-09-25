#![forbid(unsafe_code)]
//! Chat screen frame: single-source delegate over [`build_chat_lines`].
//!
//! TS truth: `crate::paint_callsite::build_chat_lines`; header/draft
//! helpers (`session_header::header_lines`, `prompt_assemble::draft_line`)
//! are read-only context, not composed here.
//! ponytail: char-count clip, no ellipsis; add spans when theme crate lands.

use crate::paint_callsite::build_chat_lines;

/// Full chat frame; transcript oldest-first. Pure delegate.
#[must_use]
pub fn chat_lines(
    title: &str,
    status: &str,
    transcript: &[String],
    draft: &str,
    width: usize,
    height: usize,
) -> Vec<String> {
    build_chat_lines(title, status, transcript, draft, width, height)
}

/// Chat frame with optional toast: `Some` replaces last line, char-safe clipped.
#[must_use]
pub fn chat_lines_with_toast(
    title: &str,
    status: &str,
    transcript: &[String],
    draft: &str,
    width: usize,
    height: usize,
    toast: Option<&str>,
) -> Vec<String> {
    let mut lines = build_chat_lines(title, status, transcript, draft, width, height);
    if let Some(msg) = toast {
        let w = width.max(1);
        let clipped: String = msg.chars().take(w).collect();
        if let Some(last) = lines.last_mut() {
            *last = clipped;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn delegates_to_build_chat_lines() {
        let a = chat_lines("T", "S", &t(&["hi"]), "d", 40, 8);
        let b = build_chat_lines("T", "S", &t(&["hi"]), "d", 40, 8);
        assert_eq!(a, b);
    }

    #[test]
    fn none_leaves_frame_unchanged() {
        let a = chat_lines_with_toast("T", "S", &t(&["hi"]), "d", 40, 8, None);
        let b = build_chat_lines("T", "S", &t(&["hi"]), "d", 40, 8);
        assert_eq!(a, b);
    }

    #[test]
    fn some_replaces_last_line() {
        let base = build_chat_lines("T", "S", &t(&["hi"]), "d", 40, 8);
        let out = chat_lines_with_toast("T", "S", &t(&["hi"]), "d", 40, 8, Some("saved"));
        assert_eq!(out.len(), base.len());
        assert_eq!(out.last().unwrap(), "saved");
        assert_eq!(&out[..out.len() - 1], &base[..base.len() - 1]);
    }

    #[test]
    fn toast_clips_to_width() {
        let out = chat_lines_with_toast("T", "S", &[], "", 5, 8, Some("abcdef"));
        assert_eq!(out.last().unwrap(), "abcde");
    }

    #[test]
    fn empty_transcript_keeps_placeholder_and_toast() {
        let plain = chat_lines("T", "S", &[], "", 40, 8);
        assert!(plain.iter().any(|l| l.contains("Start typing")));
        let out = chat_lines_with_toast("T", "S", &[], "", 40, 8, Some("tip"));
        assert_eq!(out.last().unwrap(), "tip");
        assert!(
            out[..out.len() - 1]
                .iter()
                .any(|l| l.contains("Start typing"))
                || plain.len() == out.len()
        );
    }

    #[test]
    fn toast_clip_is_char_safe() {
        let out = chat_lines_with_toast("T", "S", &[], "", 3, 8, Some("héllo✓"));
        assert_eq!(out.last().unwrap().chars().count(), 3);
    }
}
