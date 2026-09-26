#![forbid(unsafe_code)]
//! Buffered byte-stream decoder: stdin bytes -> [`InputChunk`].
//!
//! TS truth: `packages/tui/src/keymap.tsx` (key parsing: arrows, ctrl-C/D
//! quit, ctrl-P/X/G + `?` page routes, Enter submit) and
//! `packages/tui/src/prompt/index.tsx` (`:1393,1402,1434` bracketed-paste +
//! `decodePasteBytes`). Arrows reuse [`crate::input_events::parse_one`];
//! paste markers reuse [`crate::loop_events`] (`PASTE_START`/`PASTE_END`).
//! Divergence: arrows/home/end/pgup/pgdn fold to [`InputChunk::Noop`]
//! (navigation owned elsewhere); printable runs coalesce into one
//! [`InputChunk::SubmitText`]; Enter submits pending text; raw bytes carry
//! no resize (see [`map_event`] passthrough); invalid stops fail-closed.

use crate::input::InputEvent;
use crate::input_events::{parse_one, PASTE_END, PASTE_START};
use crate::loop_events::MAX_PASTE;

/// Max chunks returned per [`decode_bytes`] call.
pub const MAX_CHUNKS: usize = 64;
/// Max text bytes coalesced into one [`InputChunk::SubmitText`].
pub const MAX_TEXT: usize = 4096;

/// Overlay/page route requested by key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Palette,
    Context,
    Help,
    Chat,
}

/// Buffered input chunk decoded from a byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputChunk {
    Quit,
    Page(Page),
    SubmitText(String),
    EditBackspace,
    Paste(String),
    Resize,
    Noop,
}

/// Map one parsed event. `Resize` passes through; mouse/focus and unbound
/// keys fold to `Noop`. Enter yields empty `SubmitText` (use
/// [`decode_bytes`] for pending-text submit).
#[must_use]
pub fn map_event(ev: InputEvent) -> InputChunk {
    match ev {
        InputEvent::Resize { .. } => InputChunk::Resize,
        InputEvent::Mouse { .. } | InputEvent::Focus(_) => InputChunk::Noop,
        InputEvent::Key(k) => map_key(k.code, k.ctrl),
    }
}

fn map_key(code: u32, ctrl: bool) -> InputChunk {
    if ctrl {
        return match code {
            0x03 | 0x04 => InputChunk::Quit,
            80 | 112 => InputChunk::Page(Page::Palette),
            88 | 120 => InputChunk::Page(Page::Context),
            7 => InputChunk::Page(Page::Chat),
            _ => InputChunk::Noop,
        };
    }
    match code {
        13 | 10 => InputChunk::SubmitText(String::new()),
        63 => InputChunk::Page(Page::Help),
        127 => InputChunk::EditBackspace,
        27 | 0x1100..=0x1108 => InputChunk::Noop,
        _ => match char::from_u32(code) {
            Some(ch) if !ch.is_control() => InputChunk::SubmitText(ch.to_string()),
            _ => InputChunk::Noop,
        },
    }
}

fn flush(out: &mut Vec<InputChunk>, pending: &mut String) {
    if !pending.is_empty() {
        out.push(InputChunk::SubmitText(core::mem::take(pending)));
    }
}

/// Decode complete chunks from head of `buf`. Returns chunks + bytes
/// consumed. Fail-closed: stops at first incomplete/invalid sequence
/// (unterminated paste included) and leaves it unconsumed.
#[must_use]
pub fn decode_bytes(buf: &[u8]) -> (Vec<InputChunk>, usize) {
    let mut out = Vec::new();
    let mut pending = String::new();
    let mut at = 0;
    while at < buf.len() && out.len() < MAX_CHUNKS {
        let rest = &buf[at..];
        if rest.starts_with(PASTE_START) {
            match take_paste(rest) {
                Some((text, n)) => {
                    flush(&mut out, &mut pending);
                    out.push(InputChunk::Paste(text));
                    at += n;
                    continue;
                }
                None => break,
            }
        }
        match parse_one(rest) {
            Some((ev, n)) => {
                at += n;
                match map_event(ev) {
                    InputChunk::SubmitText(t) if !t.is_empty() => {
                        if pending.len() + t.len() > MAX_TEXT {
                            flush(&mut out, &mut pending);
                        }
                        pending.push_str(&t);
                    }
                    InputChunk::SubmitText(_) => {
                        out.push(InputChunk::SubmitText(core::mem::take(&mut pending)));
                    }
                    InputChunk::Noop => {
                        flush(&mut out, &mut pending);
                        out.push(InputChunk::Noop);
                    }
                    chunk => {
                        flush(&mut out, &mut pending);
                        out.push(chunk);
                    }
                }
            }
            None => break,
        }
    }
    flush(&mut out, &mut pending);
    (out, at)
}

/// Fenced paste at head: `START payload END`. Bounded to [`MAX_PASTE`].
fn take_paste(buf: &[u8]) -> Option<(String, usize)> {
    let body = &buf[PASTE_START.len()..];
    let end = body.windows(PASTE_END.len()).position(|w| w == PASTE_END)?;
    let raw = &body[..end.min(MAX_PASTE)];
    let text = core::str::from_utf8(raw).ok()?.to_string();
    Some((text, PASTE_START.len() + end + PASTE_END.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Key;

    #[test]
    fn quit_ctrl_c_and_d() {
        assert_eq!(decode_bytes(&[0x03]), (vec![InputChunk::Quit], 1));
        assert_eq!(decode_bytes(&[0x04]), (vec![InputChunk::Quit], 1));
    }

    #[test]
    fn arrows_decode_to_noop() {
        let (chunks, n) = decode_bytes(b"\x1b[A\x1b[D");
        assert_eq!(chunks, vec![InputChunk::Noop, InputChunk::Noop]);
        assert_eq!(n, 6);
    }

    #[test]
    fn paste_fenced() {
        let mut buf = Vec::new();
        buf.extend_from_slice(PASTE_START);
        buf.extend_from_slice(b"hello");
        buf.extend_from_slice(PASTE_END);
        let len = buf.len();
        assert_eq!(
            decode_bytes(&buf),
            (vec![InputChunk::Paste("hello".to_string())], len)
        );
    }

    #[test]
    fn multibyte_coalesced() {
        let (chunks, n) = decode_bytes("hé".as_bytes());
        assert_eq!(chunks, vec![InputChunk::SubmitText("hé".to_string())]);
        assert_eq!(n, 3);
    }

    #[test]
    fn invalid_fail_closed() {
        assert_eq!(decode_bytes(&[0xff]), (Vec::new(), 0));
        assert_eq!(decode_bytes(&[]), (Vec::new(), 0));
    }

    #[test]
    fn resize_passthrough() {
        assert_eq!(
            map_event(InputEvent::Resize { w: 80, h: 24 }),
            InputChunk::Resize
        );
        assert_eq!(
            map_event(InputEvent::Key(Key::plain(127))),
            InputChunk::EditBackspace
        );
    }

    #[test]
    fn text_then_enter_submits() {
        let (chunks, n) = decode_bytes(b"hi\r");
        assert_eq!(chunks, vec![InputChunk::SubmitText("hi".to_string())]);
        assert_eq!(n, 3);
    }
}
