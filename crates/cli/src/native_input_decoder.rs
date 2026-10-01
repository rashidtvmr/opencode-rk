//! Bounded framing for raw terminal input.
//!
//! Terminal escape sequences are a protocol, not composer text.  This decoder
//! retains fixed-size keyboard prefixes and at most one 32 KiB paste body.
//! Only a complete, valid UTF-8 paste becomes an atomic event; rejected frames
//! are consumed through their closing marker without leaking keyboard events.
//! The caller's four-byte UTF-8 decoder still handles ordinary input.

use std::time::{Duration, Instant};

pub(crate) const ESCAPE_GRACE: Duration = Duration::from_millis(150);
pub(crate) const MAX_PASTE_BYTES: usize = 32 * 1024;
const PASTE_FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const PASTE_END: &[u8] = b"\x1b[201~";
const MAX_SEQUENCE: usize = 32;
const MAX_EVENTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InputEvent {
    Byte(u8),
    Escape,
    Paste(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ground,
    Escape,
    Csi,
    Ss3,
    DiscardCsi,
    DiscardSs3,
    Paste,
    DiscardPaste,
    AbandonedPaste,
}

#[derive(Debug)]
pub(crate) struct TerminalInputDecoder {
    state: State,
    sequence: [u8; MAX_SEQUENCE],
    len: usize,
    deadline: Option<Instant>,
    paste: Vec<u8>,
    paste_end_len: usize,
    paste_deadline: Option<Instant>,
    events: [Option<InputEvent>; MAX_EVENTS],
    head: usize,
    count: usize,
}

impl Default for TerminalInputDecoder {
    fn default() -> Self {
        Self {
            state: State::Ground,
            sequence: [0; MAX_SEQUENCE],
            len: 0,
            deadline: None,
            paste: Vec::new(),
            paste_end_len: 0,
            paste_deadline: None,
            events: std::array::from_fn(|_| None),
            head: 0,
            count: 0,
        }
    }
}

impl TerminalInputDecoder {
    pub(crate) fn has_event(&self) -> bool {
        self.count != 0
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn pop(&mut self) -> Option<InputEvent> {
        if self.count == 0 {
            return None;
        }
        let event = self.events[self.head].take();
        self.head = (self.head + 1) % MAX_EVENTS;
        self.count -= 1;
        event
    }

    pub(crate) fn push(&mut self, byte: u8, now: Instant) {
        self.expire(now);
        // Ctrl-C/Ctrl-D must remain immediate even while a hostile sequence is
        // incomplete, but inside paste framing they are literal payload bytes.
        if matches!(byte, 3 | 4)
            && !matches!(
                self.state,
                State::Paste | State::DiscardPaste | State::AbandonedPaste
            )
        {
            self.reset();
            self.emit(InputEvent::Byte(byte));
            return;
        }
        match self.state {
            State::Ground => {
                if byte == 0x1b {
                    self.state = State::Escape;
                    self.deadline = Some(now + ESCAPE_GRACE);
                } else {
                    self.emit(InputEvent::Byte(byte));
                }
            }
            State::Escape => match byte {
                b'[' => self.begin(State::Csi, now),
                b'O' => self.begin(State::Ss3, now),
                0x1b => self.deadline = Some(now + ESCAPE_GRACE),
                _ => {
                    self.emit(InputEvent::Escape);
                    self.reset();
                    self.push(byte, now);
                }
            },
            State::Csi | State::Ss3 => self.sequence_byte(byte, now),
            State::Paste | State::DiscardPaste | State::AbandonedPaste => {
                self.paste_byte(byte, now)
            }
            State::DiscardCsi | State::DiscardSs3 => {
                if (0x40..=0x7e).contains(&byte) {
                    self.reset();
                } else if byte == 0x1b {
                    self.state = State::Escape;
                    self.deadline = Some(now + ESCAPE_GRACE);
                }
            }
        }
    }

    pub(crate) fn expire(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            if self.state == State::Paste {
                self.discard_paste();
                self.state = State::AbandonedPaste;
                return;
            }
            // Only a bare ESC has an ambiguous user meaning.  A timed-out
            // CSI/SS3 (including an overflow discard state) is protocol
            // garbage and must not clear the composer or synthesize Escape.
            // After an abandoned paste, a fresh, quiet standalone ESC allows
            // explicit recovery. Oversized frames never use this escape hatch.
            if self.state == State::Escape
                || (self.state == State::AbandonedPaste && self.paste_end_len == 1)
            {
                self.reset();
                self.emit(InputEvent::Escape);
            } else {
                self.state = match self.state {
                    State::Csi => State::DiscardCsi,
                    State::Ss3 => State::DiscardSs3,
                    State::DiscardCsi => State::DiscardCsi,
                    State::DiscardSs3 => State::DiscardSs3,
                    State::Paste | State::DiscardPaste | State::AbandonedPaste => self.state,
                    State::Ground | State::Escape => State::Ground,
                };
                self.len = 0;
                self.deadline = None;
            }
        }
    }

    fn begin(&mut self, state: State, now: Instant) {
        self.state = state;
        self.len = 0;
        self.deadline = Some(now + ESCAPE_GRACE);
    }

    fn sequence_byte(&mut self, byte: u8, now: Instant) {
        self.deadline = Some(now + ESCAPE_GRACE);
        if (0x40..=0x7e).contains(&byte) {
            if self.state == State::Csi && byte == b'~' && &self.sequence[..self.len] == b"200" {
                self.begin_paste(now);
                return;
            }
            if self.state == State::Csi && byte == b'u' {
                self.decode_kitty();
            }
            self.reset();
        } else if byte == 0x1b {
            self.state = State::Escape;
            self.len = 0;
        } else if self.len < MAX_SEQUENCE {
            // CSI parameter/intermediate bytes are printable protocol bytes;
            // other controls are malformed and enter fail-closed discard.
            if byte < 0x20 {
                self.state = if self.state == State::Csi {
                    State::DiscardCsi
                } else {
                    State::DiscardSs3
                };
                self.deadline = None;
            } else {
                self.sequence[self.len] = byte;
                self.len += 1;
            }
        } else {
            self.state = if self.state == State::Csi {
                State::DiscardCsi
            } else {
                State::DiscardSs3
            };
            self.len = 0;
            self.deadline = None;
        }
    }

    fn begin_paste(&mut self, now: Instant) {
        self.state = State::Paste;
        self.len = 0;
        self.paste = Vec::with_capacity(MAX_PASTE_BYTES);
        self.paste_end_len = 0;
        self.paste_deadline = Some(now + PASTE_FRAME_TIMEOUT);
        self.deadline = Some(now + ESCAPE_GRACE);
    }

    fn paste_byte(&mut self, byte: u8, now: Instant) {
        if self.state == State::Paste {
            // Bound both idle time and total frame lifetime: repeated bytes
            // cannot keep an incomplete frame alive indefinitely.
            self.deadline = self.paste_deadline.map(|end| end.min(now + ESCAPE_GRACE));
        }
        if byte == PASTE_END[self.paste_end_len] {
            self.paste_end_len += 1;
            if self.paste_end_len == PASTE_END.len() {
                let body = std::mem::take(&mut self.paste);
                let complete = self.state == State::Paste;
                self.reset();
                if complete {
                    // Reject invalid UTF-8 as a whole. Normalize only after
                    // framing, so CR/LF can never reach keyboard dispatch.
                    if let Ok(text) = String::from_utf8(body) {
                        self.emit(InputEvent::Paste(
                            text.replace("\r\n", "\n").replace('\r', "\n"),
                        ));
                    }
                }
                return;
            }
        } else {
            // The retained marker prefix is literal body if it fails to
            // match. The marker has no overlapping prefix except a new ESC.
            self.append_paste(&PASTE_END[..self.paste_end_len]);
            self.paste_end_len = 0;
            if byte == 0x1b {
                self.paste_end_len = 1;
            } else {
                self.append_paste(&[byte]);
            }
        }
        if self.state == State::AbandonedPaste {
            self.deadline = (self.paste_end_len == 1).then(|| now + ESCAPE_GRACE);
        }
    }

    fn append_paste(&mut self, bytes: &[u8]) {
        if self.state != State::Paste {
            return;
        }
        if self
            .paste
            .len()
            .checked_add(bytes.len())
            .is_some_and(|len| len <= MAX_PASTE_BYTES)
        {
            self.paste.extend_from_slice(bytes);
        } else {
            self.discard_paste();
        }
    }

    fn discard_paste(&mut self) {
        self.state = State::DiscardPaste;
        self.paste = Vec::new();
        self.paste_deadline = None;
        self.deadline = None;
        // Keep marker progress while consuming the rest of a rejected frame.
    }

    fn decode_kitty(&mut self) {
        // Kitty keyboard: codepoint;mod[:event]u.  The event field is after
        // the colon, not a third semicolon-separated parameter.  A missing
        // event means press (the frozen c fixture is CSI 99;1u).
        let mut split = self.sequence[..self.len].splitn(2, |byte| *byte == b';');
        let Some(codepoint) = parse_number(split.next().unwrap_or_default()) else {
            return;
        };
        let Some(modifiers) = split.next() else {
            return;
        };
        let (modifier, event_type) = match modifiers.iter().position(|byte| *byte == b':') {
            Some(index) => {
                let Some(modifier) = parse_number(&modifiers[..index]) else {
                    return;
                };
                let Some(event) = parse_number(&modifiers[index + 1..]) else {
                    return;
                };
                (modifier, event)
            }
            None => {
                let Some(modifier) = parse_number(modifiers) else {
                    return;
                };
                (modifier, 1)
            }
        };
        if !matches!(event_type, 1 | 2) {
            return;
        } // release is not text; repeat has the same printable press value
        if (0xE000..=0xF8FF).contains(&codepoint) || codepoint > 0x10FFFF {
            return;
        }
        let Some(character) = char::from_u32(codepoint) else {
            return;
        };
        if modifier == 0 {
            return;
        }
        let modifier_bits = modifier - 1;
        // Only the explicit Ctrl-letter mapping is text-safe.  In particular,
        // never turn modified punctuation into arbitrary C0 controls.
        const SHIFT: u32 = 0b001;
        const CTRL: u32 = 0b100;
        const CAPS_LOCK: u32 = 0b1_000_000;
        const NUM_LOCK: u32 = 0b10_000_000;
        let harmless_locks = CAPS_LOCK | NUM_LOCK;
        if modifier_bits & CTRL != 0
            && character.is_ascii_alphabetic()
            && modifier_bits & !(CTRL | SHIFT | harmless_locks) == 0
        {
            self.emit(InputEvent::Byte(
                character.to_ascii_uppercase() as u8 & 0x1f,
            ));
        } else if modifier_bits & !(SHIFT | harmless_locks) == 0 {
            let character =
                if character.is_ascii_alphabetic() && (modifier_bits & (SHIFT | CAPS_LOCK)) != 0 {
                    character.to_ascii_uppercase()
                } else {
                    character
                };
            let mut bytes = [0u8; 4];
            for &byte in character.encode_utf8(&mut bytes).as_bytes() {
                self.emit(InputEvent::Byte(byte));
            }
        }
    }

    fn emit(&mut self, event: InputEvent) {
        if self.count == MAX_EVENTS {
            return;
        }
        let tail = (self.head + self.count) % MAX_EVENTS;
        self.events[tail] = Some(event);
        self.count += 1;
    }

    fn reset(&mut self) {
        self.state = State::Ground;
        self.len = 0;
        self.deadline = None;
        self.paste.clear();
        self.paste_end_len = 0;
        self.paste_deadline = None;
    }
}

fn parse_number(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add((byte - b'0') as u32))?;
    }
    Some(value)
}
