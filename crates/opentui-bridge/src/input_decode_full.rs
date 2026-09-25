#![forbid(unsafe_code)]
//! Raw stdin bytes -> [`Key2`], one key per `decode` call. Std-only.
use crate::input_events::{parse_one, PASTE_END, PASTE_START};
/// High-level key decoded from a byte sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key2 {
    Quit,
    Palette,
    Context,
    Help,
    Chat,
    Submit,
    Backspace,
    Esc,
    Text(char),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PgUp,
    PgDn,
    Unknown,
}
/// Decode one key from head of `buf`. Fail-closed: `(Unknown, 0)`.
#[must_use]
pub fn decode(buf: &[u8]) -> (Key2, usize) {
    let Some(&first) = buf.first() else {
        return (Key2::Unknown, 0);
    };
    if first == 0x1b {
        return decode_esc(buf);
    }
    if first == b'\r' || first == b'\n' {
        return (Key2::Submit, 1);
    }
    if first < 0x20 {
        return decode_ctrl(first);
    }
    match first {
        0x7f | 0x08 => (Key2::Backspace, 1),
        b'\r' | b'\n' => (Key2::Submit, 1),
        0x3f => (Key2::Help, 1), // `?`
        _ => decode_printable(buf),
    }
}
fn decode_ctrl(b: u8) -> (Key2, usize) {
    match b {
        0x03 | 0x04 => (Key2::Quit, 1),    // Ctrl-C / Ctrl-D
        0x07 => (Key2::Chat, 1),           // Ctrl-G
        0x10 | 0x14 => (Key2::Palette, 1), // Ctrl-P / Ctrl-T (was unwired)
        0x18 => (Key2::Context, 1),        // Ctrl-X
        _ => (Key2::Unknown, 1),
    }
}
fn decode_esc(buf: &[u8]) -> (Key2, usize) {
    if buf.starts_with(PASTE_START) {
        return match find_paste_end(buf) {
            Some(n) => (Key2::Unknown, n),
            None => (Key2::Unknown, 0),
        };
    }
    if buf.starts_with(PASTE_END) {
        return (Key2::Unknown, PASTE_END.len());
    }
    if buf.len() < 3 || buf[1] != b'[' {
        return (Key2::Esc, 1);
    }
    if let Some((ev, n)) = parse_one(buf) {
        if let crate::input::InputEvent::Key(k) = ev {
            return (map_csi(k.code), n);
        }
    }
    (Key2::Unknown, 0)
}
fn map_csi(code: u32) -> Key2 {
    match code {
        0x1100 => Key2::ArrowUp,
        0x1101 => Key2::ArrowDown,
        0x1102 => Key2::ArrowRight,
        0x1103 => Key2::ArrowLeft,
        0x1104 => Key2::Home,
        0x1105 => Key2::End,
        0x1107 => Key2::PgUp,
        0x1108 => Key2::PgDn,
        127 => Key2::Backspace,
        _ => Key2::Unknown,
    }
}
fn decode_printable(buf: &[u8]) -> (Key2, usize) {
    if let Some((ev, n)) = parse_one(buf) {
        if let crate::input::InputEvent::Key(k) = ev {
            if let Some(ch) = char::from_u32(k.code) {
                if !ch.is_control() {
                    return (Key2::Text(ch), n);
                }
            }
        }
    }
    (Key2::Unknown, 0)
}
fn find_paste_end(buf: &[u8]) -> Option<usize> {
    let body = &buf[PASTE_START.len()..];
    let rel = body.windows(PASTE_END.len()).position(|w| w == PASTE_END)?;
    Some(PASTE_START.len() + rel.min(4096) + PASTE_END.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ctrl() {
        assert_eq!(decode(&[0x14]), (Key2::Palette, 1));
        assert_eq!(decode(&[0x10]), (Key2::Palette, 1));
        assert_eq!(decode(&[0x03]), (Key2::Quit, 1));
        assert_eq!(decode(&[0x04]), (Key2::Quit, 1));
        assert_eq!(decode(&[0x07]), (Key2::Chat, 1));
        assert_eq!(decode(&[0x18]), (Key2::Context, 1));
    }
    #[test]
    fn esc_arrows() {
        assert_eq!(decode(b"\x1b"), (Key2::Esc, 1));
        assert_eq!(decode(b"\x1b[A"), (Key2::ArrowUp, 3));
        assert_eq!(decode(b"\x1b[B"), (Key2::ArrowDown, 3));
        assert_eq!(decode(b"\x1b[C"), (Key2::ArrowRight, 3));
        assert_eq!(decode(b"\x1b[D"), (Key2::ArrowLeft, 3));
    }
    #[test]
    fn nav() {
        assert_eq!(decode(b"\x1b[H"), (Key2::Home, 3));
        assert_eq!(decode(b"\x1b[F"), (Key2::End, 3));
        assert_eq!(decode(b"\x1b[5~"), (Key2::PgUp, 4));
        assert_eq!(decode(b"\x1b[6~"), (Key2::PgDn, 4));
    }
    #[test]
    fn keys() {
        assert_eq!(decode(b"?"), (Key2::Help, 1));
        assert_eq!(decode(b"\r"), (Key2::Submit, 1));
        assert_eq!(decode(b"\n"), (Key2::Submit, 1));
        assert_eq!(decode(&[0x7f]), (Key2::Backspace, 1));
    }
    #[test]
    fn text_bad() {
        assert_eq!(decode(b"a"), (Key2::Text('a'), 1));
        assert_eq!(decode(b""), (Key2::Unknown, 0));
        assert_eq!(decode(&[0xff]), (Key2::Unknown, 0));
        assert_eq!(decode(b"\x1b["), (Key2::Esc, 1));
    }
    #[test]
    fn paste() {
        let mut b = Vec::new();
        b.extend_from_slice(PASTE_START);
        b.extend_from_slice(b"hi");
        b.extend_from_slice(PASTE_END);
        assert_eq!(decode(&b), (Key2::Unknown, b.len()));
    }
}
