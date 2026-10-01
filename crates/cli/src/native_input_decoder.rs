//! Fixed-size framing for raw terminal input.
//!
//! Terminal escape sequences are a protocol, not composer text.  This decoder
//! deliberately has no allocation: an incomplete prefix is retained for a
//! short, bounded grace period and malformed sequences are discarded through
//! their terminator.  The caller can therefore keep its existing UTF-8
//! decoder unchanged for ordinary bytes.

use std::time::{Duration, Instant};

pub(crate) const ESCAPE_GRACE: Duration = Duration::from_millis(150);
const MAX_SEQUENCE: usize = 32;
const MAX_EVENTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputEvent {
    Byte(u8),
    Escape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ground,
    Escape,
    Csi,
    Ss3,
    DiscardCsi,
    DiscardSs3,
}

#[derive(Debug)]
pub(crate) struct TerminalInputDecoder {
    state: State,
    sequence: [u8; MAX_SEQUENCE],
    len: usize,
    deadline: Option<Instant>,
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
            events: [None; MAX_EVENTS],
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
        // Ctrl-C/Ctrl-D must remain immediate even while a hostile sequence is
        // incomplete; they are never swallowed by protocol framing.
        if matches!(byte, 3 | 4) {
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
            self.reset();
            self.emit(InputEvent::Escape);
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
            if self.state == State::Csi && byte == b'u' {
                self.decode_kitty();
            }
            self.reset();
        } else if byte == 0x1b {
            self.state = State::Escape;
            self.len = 0;
        } else if self.len < MAX_SEQUENCE {
            self.sequence[self.len] = byte;
            self.len += 1;
        } else {
            self.state = if self.state == State::Csi {
                State::DiscardCsi
            } else {
                State::DiscardSs3
            };
            self.len = 0;
        }
    }

    fn decode_kitty(&mut self) {
        let mut values = [0u32; 3];
        let mut n = 0usize;
        let mut value = 0u32;
        let mut have_digit = false;
        for &byte in &self.sequence[..self.len] {
            if byte.is_ascii_digit() {
                have_digit = true;
                value = value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add((byte - b'0') as u32))
                    .unwrap_or(u32::MAX);
            } else if byte == b';' && have_digit && n < values.len() {
                values[n] = value;
                n += 1;
                value = 0;
                have_digit = false;
            } else {
                return;
            }
        }
        if !have_digit || n >= values.len() {
            return;
        }
        values[n] = value;
        let codepoint = values[0];
        let modifier = values[1];
        let event_type = values[2];
        if event_type > 0 && event_type != 1 {
            return;
        } // release/repeat are not text
        if (0xE000..=0xF8FF).contains(&codepoint) || codepoint > 0x10FFFF {
            return;
        }
        let Some(character) = char::from_u32(codepoint) else {
            return;
        };
        let modifier_bits = modifier.saturating_sub(1);
        if modifier_bits & 0b100 != 0 && character.is_ascii() {
            self.emit(InputEvent::Byte((character as u8) & 0x1f));
        } else if modifier_bits == 0 {
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
    }
}
