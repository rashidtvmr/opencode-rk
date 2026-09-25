#![forbid(unsafe_code)]
//! Raw stdin bytes -> [`HostAction`]. Pure, bounded, std-only.
//!
//! TS truth: `packages/tui/src/keymap.tsx` key parsing (arrows/home/end,
//! ctrl-C/D quit, `?` help); `prompt/index.tsx` paste handling
//! (`:1393,1402,1434` bracketed-paste + `decodePasteBytes`).
//! Divergence: simplified CSI set from `input_events::parse_one`; Resize
//! arrives as [`InputEvent::Resize`] via [`map_event`] (raw bytes carry no
//! resize); Page routes: ctrl-P palette, ctrl-X context, `?` help,
//! ctrl-G chat; Enter submits; ctrl-C/D/Esc-quit byte quits.

use crate::input::InputEvent;
use crate::input_events::{parse_one, PASTE_END, PASTE_START};

/// Max actions returned per [`drain_bytes`] call.
pub const MAX_ACTIONS: usize = 64;
/// Max pasted bytes collected between PASTE markers.
pub const MAX_PASTE: usize = 4096;

/// Host-level action decoded from input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAction {
    Key(u32),
    Paste(String),
    Resize,
    Quit,
    Page(Page),
    Submit,
}

/// Overlay/page route requested by key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Palette,
    Context,
    Help,
    Chat,
}

impl HostAction {
    /// Key payload as char, if valid.
    #[must_use]
    pub const fn key_char(code: u32) -> Option<char> {
        char::from_u32(code)
    }
}

/// Map one parsed event. `None` = dropped (mouse/focus noise).
#[must_use]
pub fn map_event(ev: InputEvent) -> Option<HostAction> {
    match ev {
        InputEvent::Resize { .. } => Some(HostAction::Resize),
        InputEvent::Mouse { .. } | InputEvent::Focus(_) => None,
        InputEvent::Key(k) => Some(map_key(k.code, k.ctrl)),
    }
}

fn map_key(code: u32, ctrl: bool) -> HostAction {
    if ctrl {
        match code {
            0x03 | 0x04 => return HostAction::Quit,
            80 | 112 => return HostAction::Page(Page::Palette), // ctrl-P
            88 | 120 => return HostAction::Page(Page::Context), // ctrl-X
            7 => return HostAction::Page(Page::Chat),           // ctrl-G
            _ => return HostAction::Key(code),
        }
    }
    match code {
        13 | 10 => HostAction::Submit,
        63 => HostAction::Page(Page::Help), // `?`
        _ => HostAction::Key(code),
    }
}

/// Drain complete actions from head of `buf`. Returns actions + bytes
/// consumed. Fail-closed: stops at first incomplete/invalid sequence;
/// unterminated paste is left unconsumed.
#[must_use]
pub fn drain_bytes(buf: &[u8]) -> (Vec<HostAction>, usize) {
    let mut out = Vec::new();
    let mut at = 0;
    while at < buf.len() && out.len() < MAX_ACTIONS {
        let rest = &buf[at..];
        if rest.starts_with(PASTE_START) {
            match take_paste(rest) {
                Some((text, n)) => {
                    out.push(HostAction::Paste(text));
                    at += n;
                    continue;
                }
                None => break, // unterminated: wait for more bytes
            }
        }
        match parse_one(rest) {
            Some((ev, n)) => {
                at += n;
                if let Some(a) = map_event(ev) {
                    out.push(a);
                }
            }
            None => break,
        }
    }
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
    fn arrows_map_to_keys() {
        let (acts, n) = drain_bytes(b"\x1b[A\x1b[B");
        assert_eq!(acts, vec![HostAction::Key(0x1100), HostAction::Key(0x1101)]);
        assert_eq!(n, 6);
    }

    #[test]
    fn paste_fenced_by_markers() {
        let mut buf = Vec::new();
        buf.extend_from_slice(PASTE_START);
        buf.extend_from_slice(b"hello");
        buf.extend_from_slice(PASTE_END);
        let (acts, n) = drain_bytes(&buf);
        assert_eq!(acts, vec![HostAction::Paste("hello".to_string())]);
        assert_eq!(n, buf.len());
    }

    #[test]
    fn quit_byte_quits() {
        let (acts, n) = drain_bytes(&[0x03]);
        assert_eq!(acts, vec![HostAction::Quit]);
        assert_eq!(n, 1);
    }

    #[test]
    fn resize_passthrough() {
        assert_eq!(
            map_event(InputEvent::Resize { w: 80, h: 24 }),
            Some(HostAction::Resize)
        );
    }

    #[test]
    fn invalid_yields_none() {
        let (acts, n) = drain_bytes(&[0xff]);
        assert!(acts.is_empty());
        assert_eq!(n, 0);
        let (acts, n) = drain_bytes(&[]);
        assert!(acts.is_empty());
        assert_eq!(n, 0);
    }

    #[test]
    fn multibyte_key() {
        let bytes = "é".as_bytes();
        let (acts, n) = drain_bytes(bytes);
        assert_eq!(acts, vec![HostAction::Key(0xe9)]);
        assert_eq!(n, 2);
        assert_eq!(HostAction::key_char(0xe9), Some('é'));
        assert_eq!(
            map_event(InputEvent::Key(Key::plain(13))),
            Some(HostAction::Submit)
        );
    }
}
