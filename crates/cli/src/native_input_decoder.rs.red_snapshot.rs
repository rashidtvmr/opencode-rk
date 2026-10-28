//! RED snapshot: tests for Ctrl-Tab/Ctrl-Shift-Tab/Alt-Down/Alt-Up/Ctrl-1..0
//! Compiles standalone with rustc. Baseline-extracting existing missing behavior.
//! These tests FAIL on current decoder: tab-navigation and Ctrl-digit keybinds
//! are silently dropped by decode_kitty, Alt arrows never emit TabNext/TabPrevious.

use std::time::{Duration, Instant};

const ESCAPE_GRACE: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    Byte(u8),
    Escape,
    ArrowUp,
    ArrowDown,
    TabNext,
    TabPrevious,
    TabSlot(usize),
    Paste(String),
    Mouse { x: u16, y: u16, kind: u8, modifiers: u8 },
}

// --- decoder internals copied to make a standalone test harness ---

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ground,
    Escape,
    Csi,
    Ss3,
    DiscardCsi,
    DiscardSs3,
    ControlString { bell_terminated: bool, escape: bool, abandoned: bool },
    Paste,
    DiscardPaste,
    AbandonedPaste,
}

const MAX_SEQUENCE: usize = 32;
const MAX_EVENTS: usize = 8;
const MAX_PASTE_BYTES: usize = 32 * 1024;
const PASTE_FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROL_STRING_TIMEOUT: Duration = Duration::from_secs(5);
const PASTE_END: &[u8] = b"\x1b[201~";

#[derive(Debug)]
pub struct TerminalInputDecoder {
    state: State,
    sequence: [u8; MAX_SEQUENCE],
    len: usize,
    deadline: Option<Instant>,
    paste: Vec<u8>,
    paste_end_len: usize,
    paste_deadline: Option<Instant>,
    string_deadline: Option<Instant>,
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
            string_deadline: None,
            events: std::array::from_fn(|_| None),
            head: 0,
            count: 0,
        }
    }
}

impl TerminalInputDecoder {
    pub fn push(&mut self, byte: u8, now: Instant) {
        self.expire(now);
        if matches!(byte, 3 | 4)
            && !matches!(self.state, State::Paste | State::DiscardPaste | State::AbandonedPaste)
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
                b']' => self.begin_control_string(true, now),
                b'P' | b'_' | b'^' | b'X' => self.begin_control_string(false, now),
                0x1b => self.deadline = Some(now + ESCAPE_GRACE),
                _ => {
                    self.emit(InputEvent::Escape);
                    self.reset();
                    self.push(byte, now);
                }
            },
            State::Csi | State::Ss3 => self.sequence_byte(byte, now),
            State::ControlString { bell_terminated, escape, abandoned } => {
                if (bell_terminated && byte == 0x07) || (escape && byte == b'\\') {
                    self.reset();
                } else {
                    self.state = State::ControlString {
                        bell_terminated,
                        escape: byte == 0x1b,
                        abandoned,
                    };
                    self.deadline = if abandoned {
                        (byte == 0x1b).then(|| now + ESCAPE_GRACE)
                    } else {
                        self.string_deadline.map(|end| end.min(now + ESCAPE_GRACE))
                    };
                }
            }
            State::Paste | State::DiscardPaste | State::AbandonedPaste => self.paste_byte(byte, now),
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

    pub fn has_event(&self) -> bool { self.count != 0 }
    pub fn next_deadline(&self) -> Option<Instant> { self.deadline }
    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.count == 0 { return None; }
        let event = self.events[self.head].take();
        self.head = (self.head + 1) % MAX_EVENTS;
        self.count -= 1;
        event
    }

    fn expire(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            if let State::ControlString { bell_terminated, escape, abandoned } = self.state {
                if abandoned && escape {
                    self.reset();
                    self.emit(InputEvent::Escape);
                } else {
                    self.state = State::ControlString { bell_terminated, escape, abandoned: true };
                    self.deadline = None;
                    self.string_deadline = None;
                }
                return;
            }
            if self.state == State::Paste {
                self.discard_paste();
                self.state = State::AbandonedPaste;
                return;
            }
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
                    State::ControlString { .. } => self.state,
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

    fn begin_control_string(&mut self, bell_terminated: bool, now: Instant) {
        self.reset();
        self.state = State::ControlString { bell_terminated, escape: false, abandoned: false };
        self.string_deadline = Some(now + CONTROL_STRING_TIMEOUT);
        self.deadline = Some(now + ESCAPE_GRACE);
    }

    fn sequence_byte(&mut self, byte: u8, now: Instant) {
        self.deadline = Some(now + ESCAPE_GRACE);
        if (0x40..=0x7e).contains(&byte) {
            if self.state == State::Csi && byte == b'~' && &self.sequence[..self.len] == b"200" {
                self.begin_paste(now);
                return;
            }
            if matches!(byte, b'A' | b'B')
                && (self.len == 0
                    || (self.state == State::Csi && &self.sequence[..self.len] == b"1"))
            {
                self.emit(if byte == b'A' { InputEvent::ArrowUp } else { InputEvent::ArrowDown });
            } else if self.state == State::Csi && byte == b'u' {
                self.decode_kitty();
            } else if self.state == State::Csi
                && matches!(byte, b'M' | b'm')
                && self.sequence.first() == Some(&b'<')
            {
                if let Some(event) = decode_mouse(&self.sequence[..self.len], byte) {
                    self.emit(event);
                }
            }
            self.reset();
        } else if byte == 0x1b {
            self.state = State::Escape;
            self.len = 0;
        } else if self.len < MAX_SEQUENCE {
            if byte < 0x20 {
                self.state = if self.state == State::Csi { State::DiscardCsi } else { State::DiscardSs3 };
                self.deadline = None;
            } else {
                self.sequence[self.len] = byte;
                self.len += 1;
            }
        } else {
            self.state = if self.state == State::Csi { State::DiscardCsi } else { State::DiscardSs3 };
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
            self.deadline = self.paste_deadline.map(|end| end.min(now + ESCAPE_GRACE));
        }
        if byte == PASTE_END[self.paste_end_len] {
            self.paste_end_len += 1;
            if self.paste_end_len == PASTE_END.len() {
                let body = std::mem::take(&mut self.paste);
                let complete = self.state == State::Paste;
                self.reset();
                if complete {
                    if let Ok(text) = String::from_utf8(body) {
                        self.emit(InputEvent::Paste(
                            text.replace("\r\n", "\n").replace('\r', "\n"),
                        ));
                    }
                }
                return;
            }
        } else {
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
        if self.state != State::Paste { return; }
        if self.paste.len().checked_add(bytes.len()).is_some_and(|len| len <= MAX_PASTE_BYTES) {
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
    }

    fn decode_kitty(&mut self) {
        // Kitty keyboard: codepoint;mod[:event]u.  Event field is after the colon.
        // A missing event means press (event_type=1).
        let mut split = self.sequence[..self.len].splitn(2, |byte| *byte == b';');
        let Some(codepoint) = parse_number(split.next().unwrap_or_default()) else { return };
        let Some(modifiers) = split.next() else { return };
        let (modifier, event_type) = match modifiers.iter().position(|byte| *byte == b':') {
            Some(index) => {
                let Some(modifier) = parse_number(&modifiers[..index]) else { return };
                let Some(event) = parse_number(&modifiers[index + 1..]) else { return };
                (modifier, event)
            }
            None => {
                let Some(modifier) = parse_number(modifiers) else { return };
                (modifier, 1)
            }
        };
        if !matches!(event_type, 1 | 2) { return }
        // TODO: this snapshot uses the CURRENT (RED) decode_kitty which DROPS tab-nav/Ctrl-digit.
        // The GREEN patch will implement TabNext / TabPrevious / TabSlot here.
        if (0xE000..=0xF8FF).contains(&codepoint) || codepoint > 0x10FFFF { return }
        let Some(_character) = char::from_u32(codepoint) else { return };
        if modifier == 0 { return }
        let _modifier_bits = modifier - 1;
        const SHIFT: u32 = 0b001;
        const CTRL: u32 = 0b100;
        const CAPS_LOCK: u32 = 0b1_000_000;
        const NUM_LOCK: u32 = 0b10_000_000;
        let harmless_locks = CAPS_LOCK | NUM_LOCK;
        let character = char::from_u32(codepoint).unwrap();
        if _modifier_bits & CTRL != 0
            && character.is_ascii_alphabetic()
            && _modifier_bits & !(CTRL | SHIFT | harmless_locks) == 0
        {
            self.emit(InputEvent::Byte(character.to_ascii_uppercase() as u8 & 0x1f));
        } else if _modifier_bits & !(SHIFT | harmless_locks) == 0 {
            let character = if character.is_ascii_alphabetic() && (_modifier_bits & (SHIFT | CAPS_LOCK)) != 0 {
                character.to_ascii_uppercase()
            } else { character };
            let mut bytes = [0u8; 4];
            for &byte in character.encode_utf8(&mut bytes).as_bytes() {
                self.emit(InputEvent::Byte(byte));
            }
        }
    }

    fn emit(&mut self, event: InputEvent) {
        if self.count == MAX_EVENTS { return }
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
        self.string_deadline = None;
    }
}

fn decode_mouse(sequence: &[u8], end: u8) -> Option<InputEvent> {
    let parameters = sequence.strip_prefix(b"<")?;
    let mut fields = parameters.split(|byte| *byte == b';');
    let button = parse_number(fields.next()?)?;
    let x = u16::try_from(parse_number(fields.next()?)?.checked_sub(1)?).ok()?;
    let y = u16::try_from(parse_number(fields.next()?)?.checked_sub(1)?).ok()?;
    if fields.next().is_some() { return None }
    let modifiers = (button & 28) as u8;
    let kind = match (button & !28, end) {
        (0, b'M') => 0,
        (32, b'M') => 1,
        (35, b'M') => 7,
        (0 | 3, b'm') => 2,
        (64, b'M') => 3,
        (65, b'M') => 4,
        (2, b'M') => 5,
        (2, b'm') => 6,
        _ => return None,
    };
    Some(InputEvent::Mouse { x, y, kind, modifiers })
}

fn parse_number(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() { return None }
    let mut value = 0u32;
    for byte in bytes {
        if !byte.is_ascii_digit() { return None }
        value = value.checked_mul(10).and_then(|value| value.checked_add((byte - b'0') as u32))?;
    }
    Some(value)
}

// ---
#[cfg(test)]
mod tab_nav_red_tests {
    use super::*;

    fn feed(decoder: &mut TerminalInputDecoder, bytes: &[u8]) {
        let now = Instant::now();
        for &byte in bytes { decoder.push(byte, now); }
    }

    // Ctrl-Tab => CSI 9;5u (modifier 5 => Ctrl, codepoint 9 = tab)
    #[test]
    fn ctrl_tab_emits_tab_next() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[9;5u");
        assert_eq!(d.pop(), Some(InputEvent::TabNext));
    }

    // Ctrl-Shift-Tab => CSI 9;6u (modifier 6 => Ctrl+Shift)
    #[test]
    fn ctrl_shift_tab_emits_tab_previous() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[9;6u");
        assert_eq!(d.pop(), Some(InputEvent::TabPrevious));
    }

    // Alt-Down => CSI 9;3u (modifier 3 => Alt), codepoint 9 => TabNext
    #[test]
    fn alt_down_emits_tab_next() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[9;3u");
        assert_eq!(d.pop(), Some(InputEvent::TabNext));
    }

    // Alt-Up => CSI 8;3u (codepoint 8 = backspace, Alt modifier) => TabPrevious
    #[test]
    fn alt_up_emits_tab_previous() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[8;3u");
        assert_eq!(d.pop(), Some(InputEvent::TabPrevious));
    }

    // Ctrl-1 => CSI 49;5u (codepoint 49 = '1') => TabSlot(0)
    #[test]
    fn ctrl_digit_1_emits_tab_slot_0() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[49;5u");
        assert_eq!(d.pop(), Some(InputEvent::TabSlot(0)));
    }

    // Ctrl-0 => CSI 48;5u (codepoint 48 = '0') => TabSlot(9)
    #[test]
    fn ctrl_digit_0_emits_tab_slot_9() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[48;5u");
        assert_eq!(d.pop(), Some(InputEvent::TabSlot(9)));
    }

    // Ctrl-5 => TabSlot(4)
    #[test]
    fn ctrl_digit_5_emits_tab_slot_4() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[53;5u");
        assert_eq!(d.pop(), Some(InputEvent::TabSlot(4)));
    }

    // Ctrl-digit release must NOT emit (only keypress accepted)
    #[test]
    fn ctrl_digit_release_is_dropped() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[49:2u");
        assert_eq!(d.pop(), None);
    }

    // Plain ArrowUp/Down still work (preserved)
    #[test]
    fn plain_arrows_preserved() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[A");
        assert_eq!(d.pop(), Some(InputEvent::ArrowUp));
        feed(&mut d, b"\x1b[B");
        assert_eq!(d.pop(), Some(InputEvent::ArrowDown));
    }

    // Overflow/malformed still discarded (no byte injection)
    #[test]
    fn malformed_csi_u_does_not_inject_bytes() {
        let mut d = TerminalInputDecoder::default();
        feed(&mut d, b"\x1b[99999999999999999999;5u");
        assert_eq!(d.pop(), None);
        feed(&mut d, b"X");
        assert_eq!(d.pop(), Some(InputEvent::Byte(b'X')));
    }
}
