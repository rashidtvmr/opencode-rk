//! Byte-feed adapter: stdin bytes in, action labels out.
//!
//! Bridges [`crate::native_input::decode_bytes`] to the line-loop caller:
//! callers [`feed_bytes`] accumulate raw stdin bytes (capped), then
//! [`drain_step`] decodes and drains complete chunks into action labels.

#![forbid(unsafe_code)]

use crate::native_input::{decode_bytes, InputChunk, Page, MAX_TEXT};

/// Append `bytes` to `buf`, keeping at most [`MAX_TEXT`] head bytes.
/// Returns the kept length.
pub fn feed_bytes(buf: &mut Vec<u8>, bytes: &[u8]) -> usize {
    let room = MAX_TEXT.saturating_sub(buf.len());
    let take = room.min(bytes.len());
    buf.extend_from_slice(&bytes[..take]);
    buf.len()
}

fn push_text(out: &mut Vec<String>, text: &str) {
    for ch in text.chars() {
        out.push(format!("type:{ch}"));
    }
}

fn label_chunk(out: &mut Vec<String>, chunk: &InputChunk) {
    match chunk {
        InputChunk::Quit => out.push("quit".to_string()),
        InputChunk::Page(Page::Palette) => out.push("palette".to_string()),
        InputChunk::Page(Page::Context) => out.push("context".to_string()),
        InputChunk::Page(Page::Help) => out.push("help".to_string()),
        InputChunk::Page(Page::Chat) => out.push("chat".to_string()),
        InputChunk::EditBackspace => out.push("backspace".to_string()),
        InputChunk::SubmitText(t) if t.is_empty() => out.push("submit".to_string()),
        InputChunk::SubmitText(t) => push_text(out, t),
        InputChunk::Paste(t) => push_text(out, t),
        InputChunk::Resize | InputChunk::Noop => {}
    }
}

/// Decode complete chunks from `buf`, drain consumed bytes, return labels.
/// Empty buffer yields empty vec. Undecodable head yields `["invalid"]`
/// and clears the buffer (fail-closed, no infinite stall).
pub fn drain_step(buf: &mut Vec<u8>) -> Vec<String> {
    if buf.is_empty() {
        return Vec::new();
    }
    let (chunks, consumed) = decode_bytes(buf);
    if consumed == 0 && chunks.is_empty() {
        buf.clear();
        return vec!["invalid".to_string()];
    }
    buf.drain(..consumed.min(buf.len()));
    let mut out = Vec::new();
    for chunk in &chunks {
        label_chunk(&mut out, chunk);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_caps_at_max_text() {
        let mut buf = Vec::new();
        let n = feed_bytes(&mut buf, &vec![b'a'; MAX_TEXT + 100]);
        assert_eq!(n, MAX_TEXT);
        assert_eq!(buf.len(), MAX_TEXT);
    }

    #[test]
    fn feed_room_respected() {
        let mut buf = vec![b'x'; MAX_TEXT - 2];
        let n = feed_bytes(&mut buf, b"abcdef");
        assert_eq!(n, MAX_TEXT);
        assert_eq!(&buf[MAX_TEXT - 2..], b"ab");
    }

    #[test]
    fn drain_clears_consumed() {
        let mut buf = b"hi\r".to_vec();
        let labels = drain_step(&mut buf);
        assert!(buf.is_empty());
        assert!(labels
            .iter()
            .any(|l| l == "submit" || l.starts_with("type:")));
    }

    #[test]
    fn submit_label_on_enter() {
        let mut buf = b"\r".to_vec();
        assert_eq!(drain_step(&mut buf), vec!["submit".to_string()]);
    }

    #[test]
    fn quit_label_on_ctrl_c() {
        let mut buf = vec![0x03];
        assert_eq!(drain_step(&mut buf), vec!["quit".to_string()]);
    }

    #[test]
    fn type_label_per_char() {
        let mut buf = b"ab".to_vec();
        let (chunks, consumed) = decode_bytes(&buf);
        assert!(!chunks.is_empty() && consumed > 0);
        let labels = drain_step(&mut buf);
        assert!(labels.contains(&"type:a".to_string()));
        assert!(labels.contains(&"type:b".to_string()));
    }

    #[test]
    fn invalid_safe_clears() {
        let mut buf = vec![0xff];
        assert_eq!(drain_step(&mut buf), vec!["invalid".to_string()]);
        assert!(buf.is_empty());
    }
}
