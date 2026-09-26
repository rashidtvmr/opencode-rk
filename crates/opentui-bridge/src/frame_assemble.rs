#![forbid(unsafe_code)]
//! Frame assembly: chat lines plus toast overlay on last row.
//!
//! TS truth: `build_chat_lines` owns layout/exact height; toast overlays
//! the footer row when present, char-safe clipped to effective width.
//! ponytail: plain rows only; no footer re-layout, add when caller needs it.

use crate::paint_callsite::build_chat_lines;

/// Assemble one frame; `toast` replaces the last line when `Some`.
/// Always returns exactly `height.max(8)` rows.
#[must_use]
pub fn assemble_frame(
    title: &str,
    status: &str,
    transcript: &[String],
    draft: &str,
    toast: Option<&str>,
    width: usize,
    height: usize,
) -> Vec<String> {
    let mut lines = build_chat_lines(title, status, transcript, draft, width, height);
    let want = height.max(8);
    while lines.len() < want {
        lines.push(String::new());
    }
    lines.truncate(want);
    if let Some(msg) = toast {
        let clipped: String = msg.chars().take(width.max(20)).collect();
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
    fn exact_height_no_toast() {
        let out = assemble_frame("T", "S", &t(&["hi"]), "d", None, 40, 10);
        assert_eq!(out.len(), 10);
    }

    #[test]
    fn toast_replaces_last_line() {
        let out = assemble_frame("T", "S", &t(&["hi"]), "d", Some("saved"), 40, 8);
        assert_eq!(out.len(), 8);
        assert_eq!(out[7], "saved");
    }

    #[test]
    fn toast_clipped_to_width() {
        let out = assemble_frame("T", "S", &[], "", Some("abcdef"), 20, 8);
        assert_eq!(out[7], "abcdef");
        let out2 = assemble_frame("T", "S", &[], "", Some("abcdef"), 20, 8);
        assert!(out2.iter().all(|l| l.chars().count() <= 20));
    }

    #[test]
    fn clamps_small_dims() {
        let out = assemble_frame("T", "S", &[], "", None, 5, 2);
        assert_eq!(out.len(), 8);
        let over = assemble_frame("T", "S", &[], "", Some("x"), 0, 0);
        assert_eq!(over.len(), 8);
    }

    #[test]
    fn none_keeps_footer_row() {
        let base = build_chat_lines("T", "S", &[], "", 40, 8);
        let out = assemble_frame("T", "S", &[], "", None, 40, 8);
        assert_eq!(out, base);
    }

    #[test]
    fn toast_unicode_char_safe() {
        let msg = "\u{25b3}\u{25b3}\u{25b3}\u{25b3}";
        let out = assemble_frame("T", "S", &[], "", Some(msg), 20, 8);
        assert_eq!(out[7], msg);
        assert_eq!(out[7].chars().count(), 4);
    }
}
