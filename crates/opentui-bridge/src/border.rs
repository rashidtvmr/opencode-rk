#![forbid(unsafe_code)]
//! TUI border presets (mirrors `packages/tui/src/ui/border.ts:1-21`
//! at TS commit a0d9b6c; pinned ref 95daf90 diverged, file:line cited
//! against a0d9b6c). No `Full` preset evidenced in TUI (only
//! `EmptyBorder`/`SplitBorder`); call-site overrides (e.g. `prompt/
//! index.tsx:1352-1355` `bottomLeft: "╹"`) stay inline at draw time.

use crate::buffer::{BORDER_CHARS_LEN, BorderSides, BorderStyle, border_chars};

/// TUI border presets (`border.ts:1-21`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BorderPreset {
    /// `EmptyBorder`: all blank except `horizontal: " "`.
    #[default]
    Empty,
    /// `SplitBorder`: `EmptyBorder` + `vertical: "┃"` (U+2503),
    /// drawn on left+right (`border.ts:16`).
    Split,
}

const SPACE: u32 = 0x20;
const VERTICAL_HEAVY: u32 = 0x2503; // ┃

/// 11-slot chars in `border_chars` order (topLeft..cross).
#[must_use]
pub const fn preset_chars(preset: BorderPreset) -> [u32; BORDER_CHARS_LEN] {
    let mut out = [SPACE; BORDER_CHARS_LEN];
    match preset {
        BorderPreset::Empty => out,
        BorderPreset::Split => {
            // Single vertical slot (index 5) overridden to ┃.
            let base = border_chars(BorderStyle::Single);
            let _ = base;
            out[5] = VERTICAL_HEAVY;
            out
        }
    }
}

/// Sides each preset draws (`SplitBorder.border`, `border.ts:16`).
#[must_use]
pub const fn preset_sides(preset: BorderPreset) -> BorderSides {
    match preset {
        BorderPreset::Empty => BorderSides::NONE,
        BorderPreset::Split => BorderSides(1 | 4), // LEFT | RIGHT
    }
}

/// Max dialog frame dimensions (bounded allocation guard).
pub const MAX_DIALOG_W: u32 = 128;
pub const MAX_DIALOG_H: u32 = 64;

/// Render `w`x`h` char frame with preset verticals; `None` on
/// zero dims or `w > 128` / `h > 64`.
#[must_use]
pub fn dialog_frame(w: u32, h: u32, preset: BorderPreset) -> Option<Vec<String>> {
    if w == 0 || h == 0 || w > MAX_DIALOG_W || h > MAX_DIALOG_H {
        return None;
    }
    let v = char::from_u32(preset_chars(preset)[5]).unwrap_or(' ');
    let mut rows = Vec::with_capacity(h as usize);
    for _ in 0..h {
        let mut s = String::with_capacity(w as usize);
        for x in 0..w {
            s.push(if x == 0 || x + 1 == w { v } else { ' ' });
        }
        rows.push(s);
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_vertical_is_heavy() {
        assert_eq!(preset_chars(BorderPreset::Split)[5], 0x2503);
        assert_eq!(preset_sides(BorderPreset::Split), BorderSides(1 | 4));
    }

    #[test]
    fn empty_is_blank() {
        assert!(preset_chars(BorderPreset::Empty).iter().all(|&c| c == SPACE));
        assert_eq!(preset_sides(BorderPreset::Empty), BorderSides::NONE);
    }

    #[test]
    fn zero_dims_none() {
        assert_eq!(dialog_frame(0, 3, BorderPreset::Split), None);
        assert_eq!(dialog_frame(3, 0, BorderPreset::Split), None);
    }

    #[test]
    fn oversize_none() {
        assert_eq!(dialog_frame(129, 3, BorderPreset::Split), None);
        assert_eq!(dialog_frame(3, 65, BorderPreset::Split), None);
    }

    #[test]
    fn frame_width_correct() {
        let rows = dialog_frame(5, 3, BorderPreset::Split).unwrap();
        assert_eq!(rows.len(), 3);
        for r in &rows {
            assert_eq!(r.chars().count(), 5);
            assert!(r.starts_with('┃') && r.ends_with('┃'));
        }
        let empty = dialog_frame(4, 2, BorderPreset::Empty).unwrap();
        assert!(empty.iter().all(|r| r == "    "));
    }
}
