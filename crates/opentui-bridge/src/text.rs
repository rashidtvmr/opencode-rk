//! Text/edit buffer FFI surface.
//!
//! Signatures mirror `packages/native/src/lib.zig` (branch `rust-bridge`),
//! as captured in the ABI extract. Only exports present in that extract are
//! declared here — nothing is invented:
//! `createEditBuffer`, `destroyEditBuffer`, `editBufferInsertText`,
//! `editBufferGetText`, and any `syntaxStyleRegister` do NOT appear in the
//! extract, so they are intentionally absent.

/// Opaque native handle (`NativeHandle = handles.Handle`; 0 = invalid).
pub type NativeHandle = u32;

/// Invalid/sentinel handle returned on failure.
pub const INVALID_HANDLE: NativeHandle = 0;

/// Byte cap for one draw call. Mirrors
/// [`crate::safe_renderer::MAX_TEXT_BYTES`]; re-checked here so callers can
/// validate before touching a renderer.
pub use crate::safe_renderer::MAX_TEXT_BYTES;
use crate::safe_renderer::BridgeError;

/// Reject text over [`MAX_TEXT_BYTES`] (byte length, matching `draw_text`).
pub fn check_text_bytes(text: &str) -> Result<(), BridgeError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(BridgeError::TextTooLarge);
    }
    Ok(())
}

/// Take at most `max_chars` chars. Char-count clip, not display width
/// (mirrors `render_once`; upgrade path: `unicode-width`).
/// ponytail: char-count not display width; upgrade: unicode-width.
#[must_use]
pub fn clip_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Hard-wrap `text` at `width` chars per line, splitting on `\n` first.
/// Zero width yields no rows. Fails [`BridgeError::TextTooLarge`] when
/// `text` exceeds [`MAX_TEXT_BYTES`].
pub fn wrap_text(text: &str, width: usize) -> Result<Vec<String>, BridgeError> {
    check_text_bytes(text)?;
    if width == 0 {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for logical in text.split('\n') {
        let chars: Vec<char> = logical.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
            continue;
        }
        for chunk in chars.chunks(width) {
            out.push(chunk.iter().collect());
        }
    }
    Ok(out)
}

/// Clip draw rows for `render_once`-style draws: rows beyond `rows` skipped,
/// empty clipped lines skipped (caller draws nothing), each kept line
/// `(y, clipped)` with at most `cols` chars.
/// Fails [`BridgeError::TextTooLarge`] when any line exceeds
/// [`MAX_TEXT_BYTES`] (validation precedes clipping, matching `draw_text`).
pub fn clip_lines(
    lines: &[String],
    cols: usize,
    rows: usize,
) -> Result<Vec<(u32, String)>, BridgeError> {
    if lines.iter().any(|l| l.len() > MAX_TEXT_BYTES) {
        return Err(BridgeError::TextTooLarge);
    }
    let mut out = Vec::new();
    for (y, line) in lines.iter().enumerate().take(rows) {
        let clipped: String = line.chars().take(cols).collect();
        if clipped.is_empty() {
            continue;
        }
        out.push((y as u32, clipped));
    }
    Ok(out)
}

/// Opaque styled-text chunk (`text_buffer.StyledChunk`).
/// Layout is native-owned; only passed by pointer, never constructed here.
#[repr(C)]
pub struct StyledChunk {
    _opaque: [u8; 0],
}

#[cfg(feature = "native")]
unsafe extern "C" {
    pub fn createTextBuffer(width_method: u8) -> NativeHandle;
    pub fn destroyTextBuffer(tb_handle: NativeHandle);
    pub fn textBufferGetLength(tb_handle: NativeHandle) -> u32;
    pub fn textBufferGetByteSize(tb_handle: NativeHandle) -> u32;
    pub fn textBufferReset(tb_handle: NativeHandle);
    pub fn textBufferClear(tb_handle: NativeHandle);
    pub fn textBufferSetDefaultFg(tb_handle: NativeHandle, fg: *const u16);
    pub fn textBufferSetDefaultBg(tb_handle: NativeHandle, bg: *const u16);
    pub fn textBufferSetTextFromMem(tb_handle: NativeHandle, id: u8);
    pub fn textBufferAppend(
        tb_handle: NativeHandle,
        data_ptr: *const u8,
        data_len: u32,
    );
    pub fn textBufferAppendFromMemId(tb_handle: NativeHandle, id: u8);
    pub fn textBufferLoadFile(
        tb_handle: NativeHandle,
        path_ptr: *const u8,
        path_len: u32,
    ) -> bool;
    pub fn textBufferSetStyledText(
        tb_handle: NativeHandle,
        chunks_ptr: *const StyledChunk,
        chunk_count: u32,
    );
    pub fn textBufferGetLineCount(tb_handle: NativeHandle) -> u32;
    pub fn textBufferGetPlainText(
        tb_handle: NativeHandle,
        out_ptr: *mut u8,
        max_len: u32,
    ) -> u32;
    pub fn editBufferSetText(
        edit_handle: NativeHandle,
        text_ptr: *const u8,
        text_len: u32,
    );
    pub fn editBufferSetTextFromMem(edit_handle: NativeHandle, mem_id: u8);
    pub fn editBufferReplaceText(
        edit_handle: NativeHandle,
        text_ptr: *const u8,
        text_len: u32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn native_handle_layout_stable() {
        assert_eq!(size_of::<NativeHandle>(), 4);
        assert_eq!(INVALID_HANDLE, 0);
    }

    use crate::safe_renderer::BridgeError;

    #[test]
    fn check_text_bytes_boundary() {
        assert!(super::check_text_bytes("").is_ok());
        assert!(super::check_text_bytes(&"x".repeat(MAX_TEXT_BYTES)).is_ok());
        assert_eq!(
            super::check_text_bytes(&"x".repeat(MAX_TEXT_BYTES + 1)).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    #[test]
    fn clip_chars_boundary_safe() {
        assert_eq!(super::clip_chars("hello", 10), "hello");
        assert_eq!(super::clip_chars("hello", 2), "he");
        assert_eq!(super::clip_chars("hello", 0), "");
        assert_eq!(super::clip_chars("héllo", 2), "hé");
        assert_eq!(super::clip_chars("", 5), "");
    }

    #[test]
    fn wrap_text_hard_wrap_chars() {
        assert_eq!(
            super::wrap_text("abcdef", 2).unwrap(),
            vec!["ab", "cd", "ef"]
        );
        assert_eq!(
            super::wrap_text("ab\ncde", 2).unwrap(),
            vec!["ab", "cd", "e"]
        );
        assert_eq!(super::wrap_text("", 4).unwrap(), vec![""]);
        assert_eq!(
            super::wrap_text("héllo", 2).unwrap(),
            vec!["hé", "ll", "o"]
        );
        assert!(super::wrap_text("abc", 0).unwrap().is_empty());
        assert_eq!(
            super::wrap_text(&"x".repeat(MAX_TEXT_BYTES + 1), 80).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    #[test]
    fn clip_lines_draw_prep() {
        let lines = vec![
            "hello".to_owned(),
            "".to_owned(),
            "world!".to_owned(),
            "skip".to_owned(),
        ];
        assert_eq!(
            super::clip_lines(&lines, 5, 2).unwrap(),
            vec![(0, "hello".to_owned())]
        );
        assert_eq!(
            super::clip_lines(&lines, 3, 10).unwrap(),
            vec![
                (0, "hel".to_owned()),
                (2, "wor".to_owned()),
                (3, "ski".to_owned())
            ]
        );
        assert!(super::clip_lines(&lines, 0, 10).unwrap().is_empty());
        assert!(super::clip_lines(&lines, 10, 0).unwrap().is_empty());
        let big = vec!["x".repeat(MAX_TEXT_BYTES + 1)];
        assert_eq!(
            super::clip_lines(&big, 80, 24).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }
}
