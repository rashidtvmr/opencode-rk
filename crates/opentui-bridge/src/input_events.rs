#![forbid(unsafe_code)]
//! Raw stdin bytes -> [`crate::input::InputEvent`]. Pure, bounded, no IO.
//!
//! TS truth: `packages/tui/src/keymap.tsx` key parsing; `prompt/index.tsx`
//! paste handling (`:1393,1402,1434` bracketed-paste + `decodePasteBytes`).
//! Divergence: simplified CSI set (arrows/home/end/pgup/pgdn); kitty
//! disambiguate+alternateKeys bits only; SGR mouse (`ESC[<b;x;yM/m`); UTF-8
//! multibyte via `str::from_utf8` on max-4-byte window. Invalid -> `None`.

use crate::input::{InputEvent, Key, MouseButton};

/// Max bytes consumed per `parse_one` call (longest SGR mouse seq).
pub const MAX_SEQ: usize = 16;

/// Bracketed-paste markers.
pub const PASTE_START: &[u8] = b"\x1b[200~";
pub const PASTE_END: &[u8] = b"\x1b[201~";

/// Paste delimiter seen at `buf` head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMark {
    Start,
    End,
}

/// `Some((event, consumed))` or `None` when no complete event at head.
#[must_use]
pub fn parse_one(buf: &[u8]) -> Option<(InputEvent, usize)> {
    let b = *buf.first()?;
    if b == 0x1b {
        return parse_esc(buf);
    }
    if b == 0x03 || b == 0x04 {
        return Some((
            InputEvent::Key(Key::new(u32::from(b), true, false, false)),
            1,
        ));
    }
    if b == b'\r' || b == b'\n' {
        return Some((InputEvent::Key(Key::plain(u32::from(b))), 1));
    }
    if b == b'\t' {
        return Some((InputEvent::Key(Key::plain(9)), 1));
    }
    if b == 0x7f || b == 0x08 {
        return Some((InputEvent::Key(Key::plain(127)), 1));
    }
    if b < 0x20 {
        return Some((
            InputEvent::Key(Key::new(u32::from(b), true, false, false)),
            1,
        ));
    }
    let len = utf8_len(buf)?;
    let s = core::str::from_utf8(&buf[..len]).ok()?;
    let ch = s.chars().next()?;
    Some((InputEvent::Key(Key::plain(ch as u32)), len))
}

fn utf8_len(buf: &[u8]) -> Option<usize> {
    let b = buf[0];
    if b < 0x80 {
        return Some(1);
    }
    let want = if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        return None;
    };
    if buf.len() < want {
        return None;
    }
    core::str::from_utf8(&buf[..want]).ok()?;
    Some(want)
}

fn parse_esc(buf: &[u8]) -> Option<(InputEvent, usize)> {
    if buf.starts_with(PASTE_START) {
        return Some((InputEvent::Focus(true), PASTE_START.len()));
    }
    if buf.starts_with(PASTE_END) {
        return Some((InputEvent::Focus(false), PASTE_END.len()));
    }
    if buf.len() < 3 || buf[1] != b'[' {
        return Some((InputEvent::Key(Key::plain(27)), 1));
    }
    if buf[2] == b'<' && buf.len() >= 6 {
        return parse_sgr_mouse(buf);
    }
    // Kitty: ESC[97;5u style (unicode + mods). mods bit0 shift,bit1 alt,bit2 ctrl.
    if let Some(u) = buf.iter().position(|&c| c == b'u') {
        if u <= MAX_SEQ {
            return parse_kitty(buf, u);
        }
    }
    // CSI letter: arrows + home/end/pgup/pgdn.
    let last = *buf
        .iter()
        .take(MAX_SEQ)
        .find(|&&c| c.is_ascii_alphabetic() || c == b'~')?;
    let code = match last {
        b'A' => 0x1100,
        b'B' => 0x1101,
        b'C' => 0x1102,
        b'D' => 0x1103,
        b'H' => 0x1104,
        b'F' => 0x1105,
        b'~' => parse_tilde(buf)?,
        _ => return None,
    };
    let n = buf.iter().position(|&c| c == last)? + 1;
    Some((InputEvent::Key(Key::plain(code)), n))
}

fn parse_tilde(buf: &[u8]) -> Option<u32> {
    let n = buf.iter().position(|&c| c == b'~')?;
    let num: u32 = core::str::from_utf8(&buf[2..n]).ok()?.parse().ok()?;
    match num {
        1 => Some(0x1104),
        2 => Some(0x1106),
        3 => Some(127),
        4 => Some(0x1105),
        5 => Some(0x1107),
        6 => Some(0x1108),
        _ => None,
    }
}

fn parse_kitty(buf: &[u8], u: usize) -> Option<(InputEvent, usize)> {
    let inner = core::str::from_utf8(&buf[2..u]).ok()?;
    let (code_s, mods_s) = match inner.split_once(';') {
        Some(p) => p,
        None => (inner, "1"),
    };
    let code: u32 = code_s.parse().ok()?;
    let mods: u32 = mods_s.parse::<u32>().unwrap_or(1).saturating_sub(1);
    let key = Key::new(code, mods & 4 != 0, mods & 2 != 0, mods & 1 != 0);
    Some((InputEvent::Key(key), u + 1))
}

fn parse_sgr_mouse(buf: &[u8]) -> Option<(InputEvent, usize)> {
    // ESC [ < b ; x ; y M|m
    if buf.get(2) != Some(&b'<') {
        return None;
    }
    let end = buf.iter().position(|&c| c == b'M' || c == b'm')?;
    if end > MAX_SEQ {
        return None;
    }
    let inner = core::str::from_utf8(&buf[3..end]).ok()?;
    let mut it = inner.split(';');
    let b: u32 = it.next()?.parse().ok()?;
    let x: u16 = it.next()?.parse().ok()?;
    let y: u16 = it.next()?.parse().ok()?;
    let button = match b % 4 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        _ => MouseButton::Right,
    };
    Some((InputEvent::Mouse { x, y, button }, end + 1))
}

/// Paste marker at head, if any.
#[must_use]
pub fn paste_mark(buf: &[u8]) -> Option<(PasteMark, usize)> {
    if buf.starts_with(PASTE_START) {
        Some((PasteMark::Start, PASTE_START.len()))
    } else if buf.starts_with(PASTE_END) {
        Some((PasteMark::End, PASTE_END.len()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_up() {
        let (e, n) = parse_one(b"\x1b[A rest").unwrap();
        assert_eq!(e, InputEvent::Key(Key::plain(0x1100)));
        assert_eq!(n, 3);
    }

    #[test]
    fn arrows_all() {
        assert_eq!(
            parse_one(b"\x1b[B").unwrap().0,
            InputEvent::Key(Key::plain(0x1101))
        );
        assert_eq!(
            parse_one(b"\x1b[C").unwrap().0,
            InputEvent::Key(Key::plain(0x1102))
        );
        assert_eq!(
            parse_one(b"\x1b[D").unwrap().0,
            InputEvent::Key(Key::plain(0x1103))
        );
    }

    #[test]
    fn utf8_multibyte() {
        let (e, n) = parse_one("é".as_bytes()).unwrap();
        assert_eq!(e, InputEvent::Key(Key::plain(0xe9)));
        assert_eq!(n, 2);
    }

    #[test]
    fn kitty_ctrl_a() {
        let (e, n) = parse_one(b"\x1b[97;5u").unwrap();
        assert_eq!(e, InputEvent::Key(Key::new(97, true, false, false)));
        assert_eq!(n, 7);
    }

    #[test]
    fn paste_markers() {
        assert_eq!(paste_mark(PASTE_START), Some((PasteMark::Start, 6)));
        assert_eq!(paste_mark(PASTE_END), Some((PasteMark::End, 6)));
    }

    #[test]
    fn sgr_mouse() {
        let (e, _) = parse_one(b"\x1b[<0;10;20M").unwrap();
        assert_eq!(
            e,
            InputEvent::Mouse {
                x: 10,
                y: 20,
                button: MouseButton::Left
            }
        );
    }

    #[test]
    fn invalid_byte_none() {
        assert_eq!(parse_one(&[0xff]), None);
        assert_eq!(parse_one(&[]), None);
    }
}
