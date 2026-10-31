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

use crate::safe_renderer::BridgeError;
/// Byte cap for one draw call. Mirrors
/// [`crate::safe_renderer::MAX_TEXT_BYTES`]; re-checked here so callers can
/// validate before touching a renderer.
pub use crate::safe_renderer::MAX_TEXT_BYTES;

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

// ---------------------------------------------------------------------------
// Display-cell measurement (FFI-05).
//
// The audited OpenTUI boundary (`docs/ui-parity/opencode-ui-parity-kit/
// audit-opentui-boundary.md` §2, refs [N2]/[N5]) says the native header exposes
// text views whose measurement is the authority for wrapping, and ref [N2] pins
// numeric dimensions to **terminal display cells**. The vendored library does
// export `textBufferGetPlainText`/`bufferWriteResolvedChars` but no exported
// symbol returns a wrapped intrinsic size, and no audited header records the
// signature of the editor/text-view measurement entry points, so no extern is
// declared here (fail closed). Measurement is modelled in Rust with display
// cells; equivalence with the native text view is NOT demonstrated and must be
// checked against native fixtures before this is treated as parity.
// ---------------------------------------------------------------------------

/// Cells occupied by one char: 0 (combining/zero width), 2 (wide) or 1.
///
/// Ranges follow the Unicode East Asian Width / zero-width classes that matter
/// for terminal layout. The table is intentionally conservative: unknown ranges
/// fall back to 1 cell rather than guessing.
#[must_use]
pub const fn char_cells(ch: char) -> usize {
    let c = ch as u32;
    if is_zero_width(c) {
        return 0;
    }
    if is_wide(c) {
        2
    } else {
        1
    }
}

const fn is_zero_width(c: u32) -> bool {
    matches!(c,
        0x0300..=0x036F // combining diacritical marks
        | 0x0483..=0x0489 // combining Cyrillic
        | 0x0591..=0x05BD // Hebrew points
        | 0x0610..=0x061A // Arabic marks
        | 0x064B..=0x065F
        | 0x0E31..=0x0E3A // Thai marks
        | 0x0E47..=0x0E4E
        | 0x200B..=0x200F // zero width space/joiner, LRM/RLM
        | 0x202A..=0x202E // bidi controls
        | 0x2060..=0x2064
        | 0xFE00..=0xFE0F // variation selectors
        | 0xFE20..=0xFE2F // combining half marks
        | 0xFEFF          // BOM / ZWNBSP
        | 0xE0100..=0xE01EF // variation selectors supplement
    )
}

const fn is_wide(c: u32) -> bool {
    matches!(c,
        0x1100..=0x115F // Hangul Jamo initial
        | 0x231A..=0x231B
        | 0x2329..=0x232A
        | 0x23E9..=0x23F3
        | 0x25FD..=0x25FE
        | 0x2614..=0x2615
        | 0x2648..=0x2653
        | 0x267F
        | 0x2693
        | 0x26A1
        | 0x26AA..=0x26AB
        | 0x26BD..=0x26BE
        | 0x26C4..=0x26C5
        | 0x26CE
        | 0x26D4
        | 0x26EA
        | 0x26F2..=0x26F3
        | 0x26F5
        | 0x26FA
        | 0x26FD
        | 0x2705
        | 0x270A..=0x270B
        | 0x2728
        | 0x274C
        | 0x274E
        | 0x2753..=0x2755
        | 0x2757
        | 0x2795..=0x2797
        | 0x27B0
        | 0x27BF
        | 0x2B1B..=0x2B1C
        | 0x2B50
        | 0x2B55
        | 0x2E80..=0x303E // CJK radicals, Kangxi, CJK symbols
        | 0x3041..=0x33FF // Hiragana .. CJK compatibility
        | 0x3400..=0x4DBF // CJK ext A
        | 0x4E00..=0x9FFF // CJK unified
        | 0xA000..=0xA4CF // Yi
        | 0xA960..=0xA97F // Hangul Jamo ext A
        | 0xAC00..=0xD7A3 // Hangul syllables
        | 0xF900..=0xFAFF // CJK compatibility ideographs
        | 0xFE10..=0xFE19 // vertical forms
        | 0xFE30..=0xFE6F // CJK compatibility forms
        | 0xFF00..=0xFF60 // fullwidth forms
        | 0xFFE0..=0xFFE6
        | 0x16FE0..=0x16FE4
        | 0x17000..=0x18AFF // Tangut
        | 0x1B000..=0x1B2FF // Kana supplement
        | 0x1F004
        | 0x1F0CF
        | 0x1F18E
        | 0x1F191..=0x1F19A
        | 0x1F200..=0x1F320 // enclosed ideographs, emoji
        | 0x1F32D..=0x1F335
        | 0x1F337..=0x1F37C
        | 0x1F37E..=0x1F393
        | 0x1F3A0..=0x1F3CA
        | 0x1F3CF..=0x1F3D3
        | 0x1F3E0..=0x1F3F0
        | 0x1F3F4
        | 0x1F3F8..=0x1F43E
        | 0x1F440
        | 0x1F442..=0x1F4FC
        | 0x1F4FF..=0x1F53D
        | 0x1F54B..=0x1F54E
        | 0x1F550..=0x1F567
        | 0x1F57A
        | 0x1F595..=0x1F596
        | 0x1F5A4
        | 0x1F5FB..=0x1F64F
        | 0x1F680..=0x1F6C5
        | 0x1F6CC
        | 0x1F6D0..=0x1F6D2
        | 0x1F6EB..=0x1F6EC
        | 0x1F6F4..=0x1F6FC
        | 0x1F7E0..=0x1F7EB
        | 0x1F90C..=0x1F93A
        | 0x1F93C..=0x1F945
        | 0x1F947..=0x1F9FF
        | 0x1FA70..=0x1FAFF
        | 0x20000..=0x3FFFD // CJK ext B+
    )
}

/// Display width of `text` in terminal cells (no newline handling).
#[must_use]
pub fn display_width(text: &str) -> usize {
    text.chars().map(char_cells).sum()
}

/// Split `text` into rows of at most `cols` display cells.
///
/// Existing `\n` starts a new row. A wide char that would straddle the right
/// edge moves to the next row (it is never split), and combining marks stay
/// with their base char. `cols == 0` yields no rows.
#[must_use]
pub fn wrap_cells(text: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut row = String::new();
    for logical in text.split('\n') {
        row.clear();
        let mut used = 0usize;
        for ch in logical.chars() {
            let w = char_cells(ch);
            if used + w > cols && used > 0 {
                rows.push(core::mem::take(&mut row));
                used = 0;
            }
            if w > cols {
                // Wider than the whole row: fail closed by dropping it rather
                // than emitting a row that overflows the requested width.
                continue;
            }
            row.push(ch);
            used += w;
        }
        rows.push(core::mem::take(&mut row));
    }
    rows
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
    pub fn textBufferAppend(tb_handle: NativeHandle, data_ptr: *const u8, data_len: u32);
    pub fn textBufferAppendFromMemId(tb_handle: NativeHandle, id: u8);
    pub fn textBufferLoadFile(tb_handle: NativeHandle, path_ptr: *const u8, path_len: u32) -> bool;
    pub fn textBufferSetStyledText(
        tb_handle: NativeHandle,
        chunks_ptr: *const StyledChunk,
        chunk_count: u32,
    );
    pub fn textBufferGetLineCount(tb_handle: NativeHandle) -> u32;
    pub fn textBufferGetPlainText(tb_handle: NativeHandle, out_ptr: *mut u8, max_len: u32) -> u32;
    pub fn editBufferSetText(edit_handle: NativeHandle, text_ptr: *const u8, text_len: u32);
    pub fn editBufferSetTextFromMem(edit_handle: NativeHandle, mem_id: u8);
    pub fn editBufferReplaceText(edit_handle: NativeHandle, text_ptr: *const u8, text_len: u32);
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
        assert_eq!(super::wrap_text("héllo", 2).unwrap(), vec!["hé", "ll", "o"]);
        assert!(super::wrap_text("abc", 0).unwrap().is_empty());
        assert_eq!(
            super::wrap_text(&"x".repeat(MAX_TEXT_BYTES + 1), 80).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    #[test]
    fn ffi05_char_cells_classes() {
        // Narrow, wide (CJK/emoji), zero-width (combining, ZWJ, VS16).
        assert_eq!(super::char_cells('a'), 1);
        assert_eq!(super::char_cells('é'), 1, "precomposed stays narrow");
        assert_eq!(super::char_cells('中'), 2);
        assert_eq!(super::char_cells('あ'), 2);
        assert_eq!(super::char_cells('한'), 2);
        assert_eq!(super::char_cells('\u{1F600}'), 2, "emoji is wide");
        assert_eq!(super::char_cells('\u{0301}'), 0, "combining acute");
        assert_eq!(super::char_cells('\u{200B}'), 0, "zero width space");
        assert_eq!(super::char_cells('\u{FE0F}'), 0, "variation selector");
        assert_eq!(super::display_width("a中\u{0301}"), 3);
        assert_eq!(super::display_width(""), 0);
    }

    #[test]
    fn ffi05_wrap_cells_never_splits_wide_glyphs() {
        // 4 cells: "中中" fits exactly; the third wide glyph moves down.
        assert_eq!(
            super::wrap_cells("中中中", 4),
            vec!["中中".to_owned(), "中".to_owned()]
        );
        // A wide char that would straddle the edge starts a new row.
        assert_eq!(
            super::wrap_cells("ab中", 3),
            vec!["ab".to_owned(), "中".to_owned()]
        );
        // Combining marks stay attached to the preceding base char.
        assert_eq!(
            super::wrap_cells("e\u{0301}e\u{0301}", 1),
            vec!["e\u{0301}".to_owned(), "e\u{0301}".to_owned()]
        );
        // Newlines always break; cols == 0 yields nothing.
        assert_eq!(
            super::wrap_cells("ab\nc", 10),
            vec!["ab".to_owned(), "c".to_owned()]
        );
        assert!(super::wrap_cells("abc", 0).is_empty());
        // Every emitted row respects the requested width.
        for row in super::wrap_cells("a中b😀c\u{0301}d", 3) {
            assert!(super::display_width(&row) <= 3, "row overflows: {row:?}");
        }
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
