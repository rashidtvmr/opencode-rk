//! Bounded framing for raw terminal input.
//!
//! Terminal escape sequences are a protocol, not composer text.  This decoder
//! retains fixed-size keyboard prefixes and at most one 32 KiB paste body.
//! Only a complete, valid UTF-8 paste becomes an atomic event; rejected frames
//! are consumed through their closing marker without leaking keyboard events.
//! OSC and other terminal reply strings are discarded with constant-size state
//! through their terminator, including replies split across input reads.
//! The caller's four-byte UTF-8 decoder still handles ordinary input.
//!
//! Upstream reference at pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`
//! (`packages/tui/src/component/prompt/index.tsx:1097-1477`): paste frames are
//! normalized (`\r\n`/`\r` to `\n`) before any text reaches the composer, the
//! Return family stays distinct so the submit keymap decides, and IME/Kitty
//! scalars are decoded as complete UTF-8 units rather than byte offsets.
//!
//! FFI-11: the shared input subset is cross-checked against the native
//! bridge's stdin adapter (`opentui-bridge/src/terminal_input.rs`) by feeding
//! identical byte frames to both (`mod ffi11_*`). Keep local, proved
//! equivalent: this decoder stays the sole composer feed because it is the
//! one that intentionally diverges where the bridge over-delivers —
//! (1) invalid-UTF8 and >32 KiB paste frames fail closed here (rejected
//! whole, never reaching the composer), while the bridge hands through
//! lossy-replaced/truncated bytes;
//! (2) OSC 10/11 replies surface here as an `OscColorReply { index, rgb }`
//! payload (THEME-13), while the bridge keeps only `Opaque { code }`;
//! (3) deadline/abandoned-paste states (escape grace, paste-frame and
//! control-string timeouts, recovery via a quiet ESC) exist only here, so a
//! stalled terminal frame can never stall the composer.

use std::time::{Duration, Instant};

pub(crate) const ESCAPE_GRACE: Duration = Duration::from_millis(150);
pub(crate) const MAX_PASTE_BYTES: usize = 32 * 1024;
const PASTE_FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const CONTROL_STRING_TIMEOUT: Duration = Duration::from_secs(5);
const PASTE_END: &[u8] = b"\x1b[201~";
const MAX_SEQUENCE: usize = 32;
const MAX_EVENTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InputEvent {
    Byte(u8),
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    /// CSI 5~ — `DialogSelect` page movement (ten options,
    /// `ui/dialog-select.tsx:1590-1602`). No shell key binding consumes it.
    PageUp,
    /// CSI 6~ — `DialogSelect` page movement.
    PageDown,
    /// Home (`CSI H` or `CSI 1~`) — jump to the start of a content viewer.
    Home,
    /// End (`CSI F` or `CSI 4~`) — jump to the end of a content viewer.
    End,
    /// CSI 1;3A — Alt+Up (`session.tab.previous`, `config/keybind.ts:116`).
    /// Decoded separately so modifier chords never become typed text or an
    /// unmodified arrow that a dialog would consume.
    AltArrowUp,
    /// CSI 1;3B — Alt+Down (`session.tab.next`, `config/keybind.ts:113`).
    AltArrowDown,
    /// CSI 1;4B — Alt+Shift+Down (`session.tab.next_unread`,
    /// `config/keybind.ts:115`).
    AltShiftArrowDown,
    /// CSI 1;4A — Alt+Shift+Up (`session.tab.previous_unread`,
    /// `config/keybind.ts:116`).
    AltShiftArrowUp,
    /// Kitty Ctrl+Tab (CSI 9;5u). Upstream `session.tab.next` is
    /// `ctrl+tab,alt+down` (`config/keybind.ts:113`); the `alt+down` half is
    /// already decoded as `AltArrowDown`, and no `alt+tab` binding exists.
    TabNext,
    /// Kitty Ctrl+Shift+Tab (CSI 9;6u). Upstream `session.tab.previous` is
    /// `ctrl+shift+tab,alt+up` (`config/keybind.ts:114`); the `alt+up` half is
    /// already decoded as `AltArrowUp`. `alt+backspace` is NOT a tab binding -
    /// upstream uses it for `input.delete.word.backward` (`:244`).
    TabPrevious,
    /// Kitty Shift+Tab (CSI 9;2u) and legacy back-tab (CSI Z).
    /// Upstream `agent.cycle` is `shift+tab` (`config/keybind.ts:176`),
    /// routed to `local.agent.move(1)` (`app.tsx:974-981`). Decoded
    /// distinctly so a shift+tab never becomes typed text or an
    /// ordinary Tab (which stays `session.tab.next`-free composer text).
    AgentCycle,
    /// Kitty Ctrl+digit (CSI 49;5u .. 57;5u, 48;5u)
    /// (`session.tab.select.{1..10}`, `config/keybind.ts:152-161`).
    TabSlot(usize),
    /// Kitty Ctrl+Shift+T (CSI 116;6u) (`session.tab.reopen`,
    /// `config/keybind.ts:118`).
    TabReopen,
    /// Kitty Shift+Return (CSI 13;2u). The caller resolves this as newline;
    /// it must not become Byte(13), which is the bare submit key.
    ShiftEnter,
    /// Kitty Ctrl+Return (CSI 13;5u). The caller resolves this through the
    /// existing Ctrl-J/SubmitKeymap path rather than treating it as CR.
    CtrlJ,
    /// Kitty Alt+Return (CSI 13;3u). The upstream keymap treats it as newline.
    AltEnter,
    Paste(String),
    Mouse { x: u16, y: u16, kind: u8, modifiers: u8 },
    /// CSI I (`\x1b[I`) - terminal focus gained. The upstream parser
    /// matches the exact two-byte-final sequence and only reports edges
    /// (`@opentui/core` chunk-node-51kpf0mz.js:8774-8785); same edge-dedup
    /// duty applies to this transport, it never emits duplicates of its own
    /// state, only one event per received report.
    FocusGained,
    /// CSI O (`\x1b[O`) - terminal focus lost; the renderer counterpart of
    /// `FocusGained`, consumed as the `focused` fact of TAB-21
    /// (`context/session-tabs.tsx:94-99`).
    FocusLost,
    /// OSC 10/11 default-color reply (`ESC ]10;rgb:...` ST/BEL), THEME-13.
    /// Protocol output from our own terminal-palette query; it is never
    /// composer input. `index` is the queried color slot (10 foreground,
    /// 11 background) and `rgb` is the resolved opaque color.
    OscColorReply { index: u8, rgb: [u8; 4] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Ground,
    Escape,
    Csi,
    Ss3,
    DiscardCsi,
    DiscardSs3,
    ControlString {
        bell_terminated: bool,
        escape: bool,
        abandoned: bool,
    },
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
    string_deadline: Option<Instant>,
    events: [Option<InputEvent>; MAX_EVENTS],
    head: usize,
    count: usize,
    /// THEME-13: bounded payload capture for an in-flight OSC 10/11 reply.
    control_capture: Option<ControlCapture>,
}

/// Bounded capture of an OSC reply body (`kind;<payload>`). Payloads longer
/// than [`CONTROL_CAPTURE_MAX`] set `overflow` and never emit.
#[derive(Debug)]
struct ControlCapture {
    kind: u16,
    kind_done: bool,
    bytes: [u8; CONTROL_CAPTURE_MAX],
    len: usize,
    overflow: bool,
}

const CONTROL_CAPTURE_MAX: usize = 40;

impl Default for ControlCapture {
    fn default() -> Self {
        Self {
            kind: 0,
            kind_done: false,
            bytes: [0; CONTROL_CAPTURE_MAX],
            len: 0,
            overflow: false,
        }
    }
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
            control_capture: None,
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
            State::ControlString {
                bell_terminated,
                escape,
                abandoned,
            } => {
                if (bell_terminated && byte == 0x07) || (escape && byte == b'\\') {
                    let closed = self.control_capture.take();
                    self.reset();
                    if let Some(capture) = closed {
                        if !capture.overflow && !abandoned {
                            if let Some((index, rgb)) =
                                parse_osc_color_reply(capture.kind, &capture.bytes[..capture.len])
                            {
                                self.emit(InputEvent::OscColorReply { index, rgb });
                            }
                        }
                    }
                } else {
                    self.capture_control_byte(byte);
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
            if let State::ControlString {
                bell_terminated,
                escape,
                abandoned,
            } = self.state
            {
                if abandoned && escape {
                    self.reset();
                    self.emit(InputEvent::Escape);
                } else {
                    // An idle or overlong reply is abandoned, not converted
                    // into text. A new quiet ESC provides explicit recovery,
                    // as for an abandoned paste. A late ST can still close it.
                    self.state = State::ControlString {
                        bell_terminated,
                        escape,
                        abandoned: true,
                    };
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
        self.state = State::ControlString {
            bell_terminated,
            escape: false,
            abandoned: false,
        };
        self.string_deadline = Some(now + CONTROL_STRING_TIMEOUT);
        self.deadline = Some(now + ESCAPE_GRACE);
        // Retain no payload. Timing out abandons framing without releasing a
        // late reply tail into the composer. Ctrl-C/Ctrl-D keep exit priority.
        // THEME-13 exception: OSC frames may be our own 10/11 palette replies,
        // so a bounded body is captured for the terminator (parse at close).
        self.control_capture = bell_terminated.then(ControlCapture::default);
    }

    fn sequence_byte(&mut self, byte: u8, now: Instant) {
        self.deadline = Some(now + ESCAPE_GRACE);
        if (0x40..=0x7e).contains(&byte) {
            if self.state == State::Csi && byte == b'~' && &self.sequence[..self.len] == b"200" {
                self.begin_paste(now);
                return;
            }
            // Page movement (CSI 5~/6~) is the only `~` key the live select
            // consumes; every other tilde form stays protocol (DIALOG-08).
            if self.state == State::Csi
                && byte == b'~'
                && self.len == 1
                && matches!(self.sequence[0], b'1' | b'4' | b'5' | b'6')
            {
                self.emit(match self.sequence[0] {
                    b'1' => InputEvent::Home,
                    b'4' => InputEvent::End,
                    b'5' => InputEvent::PageUp,
                    _ => InputEvent::PageDown,
                });
            } else if self.state == State::Csi
                && self.len == 0
                && matches!(byte, b'H' | b'F')
            {
                self.emit(if byte == b'H' {
                    InputEvent::Home
                } else {
                    InputEvent::End
                });
            } else if matches!(byte, b'A' | b'B')
                && self.state == State::Csi
                && &self.sequence[..self.len] == b"1;3"
            {
                self.emit(if byte == b'A' {
                    InputEvent::AltArrowUp
                } else {
                    InputEvent::AltArrowDown
                });
            } else if matches!(byte, b'A' | b'B')
                && self.state == State::Csi
                && &self.sequence[..self.len] == b"1;4"
            {
                self.emit(if byte == b'A' {
                    InputEvent::AltShiftArrowUp
                } else {
                    InputEvent::AltShiftArrowDown
                });
            } else if matches!(byte, b'A' | b'B' | b'C' | b'D')
                && (self.len == 0
                    || (self.state == State::Csi && &self.sequence[..self.len] == b"1"))
            {
                self.emit(if byte == b'A' {
                    InputEvent::ArrowUp
                } else if byte == b'B' {
                    InputEvent::ArrowDown
                } else if byte == b'C' {
                    InputEvent::ArrowRight
                } else {
                    InputEvent::ArrowLeft
                });
            } else if self.state == State::Csi && self.len == 0 && byte == b'Z' {
                // Legacy back-tab ESC [ Z; upstream `agent.cycle`
                // (`config/keybind.ts:176`).
                self.emit(InputEvent::AgentCycle);
            } else if self.state == State::Csi && byte == b'u' {
                self.decode_kitty();
            } else if self.state == State::Csi
                && self.len == 0
                && matches!(byte, b'I' | b'O')
            {
                // Focus reports are matched exactly like upstream
                // (`chunk-node-51kpf0mz.js:8774-8789`): a parameterized or
                // SS3 form stays protocol.
                self.emit(if byte == b'I' {
                    InputEvent::FocusGained
                } else {
                    InputEvent::FocusLost
                });
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
        if modifier == 1 {
            match codepoint {
                57350 => {
                    self.emit(InputEvent::ArrowLeft);
                    return;
                }
                57351 => {
                    self.emit(InputEvent::ArrowRight);
                    return;
                }
                _ => {}
            }
        }
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
        // Kitty Return-family presses are distinct from raw CR. The upstream
        // keymap binds bare Return to submit and Shift/Ctrl/Alt+Return plus
        // Ctrl-J to newline (keybind.ts:208-212). Preserve the physical event
        // here so the caller's existing SubmitKeymap remains authoritative.
        const SHIFT: u32 = 0b001;
        const ALT: u32 = 0b010;
        const CTRL: u32 = 0b100;
        // A single combined value, NOT an or-pattern: inside a tuple pattern
        // `CTRL | SHIFT` parses as alternatives (which the compiler flags as an
        // unreachable pattern), so Ctrl+Shift never matched.
        const CTRL_SHIFT: u32 = CTRL | SHIFT;
        const CAPS_LOCK: u32 = 0b1_000_000;
        const NUM_LOCK: u32 = 0b10_000_000;
        // Tab navigation chords (`config/keybind.ts:113,114,118,152-161`). They
        // carry no printable meaning, so they are resolved to the bound
        // command instead of being dropped or leaking as C0 text. Only chords
        // upstream actually binds are decoded: `ctrl+tab`, `ctrl+shift+tab`,
        // `ctrl+1..9`,`ctrl+0`, `ctrl+shift+t`. `alt+backspace` is deliberately
        // absent - it is `input.delete.word.backward` (`:244`), not a tab chord.
        if modifier_bits & !(SHIFT | ALT | CTRL | CAPS_LOCK | NUM_LOCK) == 0 {
            match (codepoint, modifier_bits & (SHIFT | ALT | CTRL)) {
                (9, CTRL) => {
                    self.emit(InputEvent::TabNext);
                    return;
                }
                (9, CTRL_SHIFT) => {
                    self.emit(InputEvent::TabPrevious);
                    return;
                }
                // shift+tab is `agent.cycle` (`config/keybind.ts:176`),
                // not a session-tab chord.
                (9, SHIFT) => {
                    self.emit(InputEvent::AgentCycle);
                    return;
                }
                (49..=57, CTRL) => {
                    self.emit(InputEvent::TabSlot((codepoint - 49) as usize));
                    return;
                }
                (48, CTRL) => {
                    self.emit(InputEvent::TabSlot(9));
                    return;
                }
                (116 | 84, CTRL_SHIFT) => {
                    self.emit(InputEvent::TabReopen);
                    return;
                }
                _ => {}
            }
        }
        if codepoint == 0x0d
            && modifier_bits & !(SHIFT | ALT | CTRL | CAPS_LOCK | NUM_LOCK) == 0
        {
            match modifier_bits & (SHIFT | ALT | CTRL) {
                0 => self.emit(InputEvent::Byte(0x0d)),
                SHIFT => self.emit(InputEvent::ShiftEnter),
                ALT => self.emit(InputEvent::AltEnter),
                CTRL => self.emit(InputEvent::CtrlJ),
                _ => return,
            }
            return;
        }
        // Only the explicit Ctrl-letter mapping is text-safe.  In particular,
        // never turn modified punctuation into arbitrary C0 controls.
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
        self.string_deadline = None;
        self.control_capture = None;
    }

    /// THEME-13: retain a bounded OSC 10/11 reply body. Any other control
    /// string, or a body that is not `<digits>;`, drops the capture and keeps
    /// today's discard-only behaviour.
    fn capture_control_byte(&mut self, byte: u8) {
        let Some(capture) = self.control_capture.as_mut() else {
            return;
        };
        if byte == 0x1b {
            // ST prefix; framing completes on the next byte.
            return;
        }
        if capture.kind_done {
            if capture.overflow || capture.len == capture.bytes.len() {
                capture.overflow = true;
                return;
            }
            capture.bytes[capture.len] = byte;
            capture.len += 1;
        } else if byte.is_ascii_digit() {
            capture.kind = capture
                .kind
                .saturating_mul(10)
                .saturating_add(u16::from(byte - b'0'));
        } else if byte == b';' {
            capture.kind_done = true;
            if !matches!(capture.kind, 10 | 11) {
                self.control_capture = None;
            }
        } else {
            self.control_capture = None;
        }
    }
}

#[cfg(test)]
mod horizontal_cursor_regressions {
    use super::*;

    #[test]
    fn alt_vertical_arrows_decode_while_other_modifiers_stay_inert() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[1;3B\x1b[1;3A" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::AltArrowDown));
        assert_eq!(decoder.pop(), Some(InputEvent::AltArrowUp));
        // Alt+Shift is the unread-tab binding (`config/keybind.ts:115-116`).
        for byte in b"\x1b[1;4B\x1b[1;4A" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::AltShiftArrowDown));
        assert_eq!(decoder.pop(), Some(InputEvent::AltShiftArrowUp));
        // Ctrl/Shift (and any other modifier) forms remain protocol: they must
        // never become an ordinary arrow that a dialog or composer consumes.
        for byte in b"\x1b[1;5B\x1b[1;2A" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn tab_navigation_chords_decode_to_their_bound_commands() {
        // Kitty CSI u: codepoint;modifier, where modifier bits are
        // shift=1, alt=2, ctrl=4 plus one (`config/keybind.ts:113,114`).
        // Only chords upstream actually binds are decoded.
        let now = Instant::now();
        let mut decoder = TerminalInputDecoder::default();
        for byte in b"\x1b[9;5u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabNext), "ctrl+tab");
        for byte in b"\x1b[9;6u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabPrevious), "ctrl+shift+tab");
        // shift+tab is `agent.cycle` (`config/keybind.ts:176`), not a
        // session-tab chord: Kitty CSI 9;2u and legacy CSI Z both decode.
        for byte in b"\x1b[9;2u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::AgentCycle), "kitty shift+tab");
        for byte in b"\x1b[Z" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::AgentCycle), "legacy CSI Z backtab");
        // `alt+down`/`alt+up` are already `AltArrowDown`/`AltArrowUp`, and
        // upstream binds neither `alt+tab` nor `alt+backspace` to a tab
        // command: `alt+backspace` is `input.delete.word.backward`
        // (`config/keybind.ts:244`). Both must decode to no tab event.
        for byte in b"\x1b[9;3u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None, "alt+tab is not a tab binding");
        for byte in b"\x1b[8;3u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None, "alt+backspace is delete-word-backward");
        for byte in b"\x1b[49;5u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabSlot(0)), "ctrl+1");
        for byte in b"\x1b[53;5u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabSlot(4)), "ctrl+5");
        for byte in b"\x1b[48;5u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabSlot(9)), "ctrl+0");
        for byte in b"\x1b[116;6u\x1b[84;6u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::TabReopen), "ctrl+shift+t");
        assert_eq!(decoder.pop(), Some(InputEvent::TabReopen), "ctrl+shift+T");
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn modified_digits_and_release_events_never_leak_text() {
        let now = Instant::now();
        let mut decoder = TerminalInputDecoder::default();
        // Ctrl+digit release (event type 2 is a repeat; releases are >= 3).
        for byte in b"\x1b[49:3u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
        // Ctrl+Shift+digit is not a bound slot selector.
        for byte in b"\x1b[49;6u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
        // A plain tab still reaches the composer.
        for byte in b"\t" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Byte(b'\t')));
    }

    #[test]
    fn unmodified_horizontal_cursor_sequences_are_events_not_draft_bytes() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[D\x1b[1C\x1bOD\x1bOC" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowLeft));
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowRight));
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowLeft));
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowRight));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn focus_reports_decode_exactly_while_parameter_and_ss3_forms_stay_inert() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[I\x1b[O\x1b[I" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::FocusGained), "CSI I is focus gained");
        assert_eq!(decoder.pop(), Some(InputEvent::FocusLost), "CSI O is focus lost");
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::FocusGained),
            "every received report emits one raw event; the renderer's edge             dedup lives on the consumer side (chunk-node-51kpf0mz.js:8778-8786)"
        );
        assert_eq!(decoder.pop(), None);
        // SS3 application-key forms and parameterized/modifier variants are
        // not focus reports anywhere upstream.
        for byte in b"\x1bOI\x1b[1;3I\x1b[1;5O\x1b[?I" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
   }

    #[test]
    fn kitty_plain_horizontal_cursor_private_keys_decode_but_modified_stays_inert() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[57350;1u\x1b[57351;1u\x1b[57350;5u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowLeft));
        assert_eq!(decoder.pop(), Some(InputEvent::ArrowRight));
        assert_eq!(decoder.pop(), None);
    }
}

fn decode_mouse(sequence: &[u8], end: u8) -> Option<InputEvent> {
    let parameters = sequence.strip_prefix(b"<")?;
    let mut fields = parameters.split(|byte| *byte == b';');
    let button = parse_number(fields.next()?)?;
    let x = u16::try_from(parse_number(fields.next()?)?.checked_sub(1)?).ok()?;
    let y = u16::try_from(parse_number(fields.next()?)?.checked_sub(1)?).ok()?;
    if fields.next().is_some() {
        return None;
    }
    let modifiers = (button & 28) as u8;
    let kind = match (button & !28, end) {
        (0, b'M') => 0,
        (1, b'M') => 8,
        (32, b'M') => 1,
        (33, b'M') => 10,
        (35, b'M') => 7,
        (0 | 3, b'm') => 2,
        (1, b'm') => 9,
        (64, b'M') => 3,
        (65, b'M') => 4,
        (2, b'M') => 5,
        (2, b'm') => 6,
        _ => return None,
    };
    Some(InputEvent::Mouse { x, y, kind, modifiers })
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

/// THEME-13: one `rgb:h*/h*/h*` (xterm reply) or `#rgb`/`#rrggbb` channel
/// group, scaled to an opaque 8-bit color.
fn parse_osc_color_text(text: &str) -> Option<[u8; 4]> {
    fn component(value: &str) -> Option<u8> {
        if value.is_empty() || value.len() > 4 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let parsed = u32::from_str_radix(value, 16).ok()?;
        let max = u32::from_str_radix(&"f".repeat(value.len()), 16).ok()?;
        Some(((parsed * 255 + max / 2) / max) as u8)
    }
    // A color payload is a protocol string of ASCII digits and separators.
    // Reject non-ASCII up front: `str` indexing is by BYTE offset, so slicing a
    // multibyte char at a fixed index would panic in the live decoder loop
    // (terminal-driven DoS). No valid reply contains a non-ASCII byte, so this
    // is behavior-preserving for well-formed input.
    if !text.is_ascii() {
        return None;
    }
    if let Some(rest) = text.strip_prefix("rgb:") {
        let mut parts = rest.split('/');
        let r = component(parts.next()?)?;
        let g = component(parts.next()?)?;
        let b = component(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        return Some([r, g, b, 0xFF]);
    }
    if let Some(rest) = text.strip_prefix('#') {
        return match rest.len() {
            // #rgb expands each nibble: f -> ff (value * 17).
            3 => {
                let r = u8::from_str_radix(&rest[..1], 16).ok()?;
                let g = u8::from_str_radix(&rest[1..2], 16).ok()?;
                let b = u8::from_str_radix(&rest[2..3], 16).ok()?;
                let expand = |value: u8| value * 17;
                Some([expand(r), expand(g), expand(b), 0xFF])
            }
            6 => Some([
                u8::from_str_radix(&rest[0..2], 16).ok()?,
                u8::from_str_radix(&rest[2..4], 16).ok()?,
                u8::from_str_radix(&rest[4..6], 16).ok()?,
                0xFF,
            ]),
            _ => None,
        };
    }
    None
}

/// THEME-13: a closed OSC 10/11 body (`10;rgb:...`) becomes a palette reply.
fn parse_osc_color_reply(kind: u16, payload: &[u8]) -> Option<(u8, [u8; 4])> {
    let index = match kind {
        10 => 10u8,
        11 => 11u8,
        _ => return None,
    };
    let text = std::str::from_utf8(payload).ok()?;
    Some((index, parse_osc_color_text(text)?))
}

#[cfg(test)]
mod mouse_protocol_tests {
    use super::*;
    #[test]
    fn sgr_hover_is_a_pointer_event_without_inserting_composer_bytes() {
        let mut decoder = TerminalInputDecoder::default();
        for byte in b"\x1b[<35;43;8M" {
            decoder.push(*byte, Instant::now());
        }
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 42, y: 7, kind: 7, modifiers: 0 }));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn sgr_modified_hover_retains_modifiers_and_never_becomes_a_click() {
        for modifiers in [4, 8, 16, 28] {
            let sequence = format!("\x1b[<{};2;3M", 35 + modifiers);
            let mut decoder = TerminalInputDecoder::default();
            for byte in sequence.bytes() { decoder.push(byte, Instant::now()); }
            assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 1, y: 2, kind: 7, modifiers: modifiers as u8 }));
            assert_eq!(decoder.pop(), None);
        }
    }

    #[test]
    fn malformed_hover_and_unsupported_release_do_not_emit_clicks() {
        for sequence in [b"\x1b[<35;0;2M".as_slice(), b"\x1b[<35;1;0M", b"\x1b[<35;2;3m", b"\x1b[<35;2;3;4M"] {
            let mut decoder = TerminalInputDecoder::default();
            for byte in sequence { decoder.push(*byte, Instant::now()); }
            assert_eq!(decoder.pop(), None);
        }
    }

    #[test]
    fn sgr_drag_press_and_release_are_real_events_not_draft_bytes() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<0;3;5M\x1b[<32;9;5M\x1b[<0;9;5m" {
            decoder.push(*byte, now);
        }
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse {
                x: 2,
                y: 4,
                kind: 0,
                modifiers: 0
            })
        );
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse {
                x: 8,
                y: 4,
                kind: 1,
                modifiers: 0
            })
        );
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse {
                x: 8,
                y: 4,
                kind: 2,
                modifiers: 0
            })
        );
        assert_eq!(decoder.pop(), None);
    }
    #[test]
    fn malformed_mouse_coordinates_are_discarded_without_input_leak() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for frame in [
            b"\x1b[<0;0;4M".as_slice(),
            b"\x1b[<0;99999999999999999999;4M",
            b"\x1b[<0;2;3;4M",
        ] {
            for byte in frame {
                decoder.push(*byte, now);
            }
        }
        assert_eq!(decoder.pop(), None);
        decoder.push(b'a', now);
        assert_eq!(decoder.pop(), Some(InputEvent::Byte(b'a')));
    }

    #[test]
    fn sgr_mouse_preserves_modifier_bits_and_never_maps_right_click_to_left() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<10;4;2M\x1b[<2;4;2M\x1b[<2;4;2m" {
            decoder.push(*byte, now);
        }
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse { x: 3, y: 1, kind: 5, modifiers: 8 })
        );
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse { x: 3, y: 1, kind: 5, modifiers: 0 })
        );
        assert_eq!(
            decoder.pop(),
            Some(InputEvent::Mouse { x: 3, y: 1, kind: 6, modifiers: 0 })
        );
    }

    #[test]
    fn sgr_middle_press_decode_distinctly_from_left_and_drag() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<1;5;3M" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 4, y: 2, kind: 8, modifiers: 0 }));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn sgr_middle_release_is_not_silently_dropped() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<1;9;7m" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 8, y: 6, kind: 9, modifiers: 0 }));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn sgr_middle_drag_is_classified_not_confused_with_left_drag_or_release() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<33;4;2M" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 3, y: 1, kind: 10, modifiers: 0 }));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn sgr_middle_press_with_modifiers_retains_modifier_bits_and_kind() {
        for (cb, modbit) in [(5, 4), (9, 8), (17, 16), (29, 28)] {
            let sequence = format!("\x1b[<{};6;2M", cb);
            let mut decoder = TerminalInputDecoder::default();
            for byte in sequence.bytes() { decoder.push(byte, Instant::now()); }
            assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 5, y: 1, kind: 8, modifiers: modbit }));
            assert_eq!(decoder.pop(), None);
        }
    }

    #[test]
    fn sgr_interleaved_middle_down_up_does_not_corrupt_following_left_events() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[<1;2;2M\x1b[<1;2;2m\x1b[<0;3;3M\x1b[<0;3;3m" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 1, y: 1, kind: 8, modifiers: 0 }));
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 1, y: 1, kind: 9, modifiers: 0 }));
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 2, y: 2, kind: 0, modifiers: 0 }));
        assert_eq!(decoder.pop(), Some(InputEvent::Mouse { x: 2, y: 2, kind: 2, modifiers: 0 }));
        assert_eq!(decoder.pop(), None);
    }
}

#[cfg(test)]
mod kitty_printable_regressions {
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<InputEvent> {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        let mut events = Vec::new();
        for &byte in bytes {
            decoder.push(byte, now);
            while let Some(event) = decoder.pop() {
                events.push(event);
            }
        }
        events
    }

    #[test]
    fn unmodified_kitty_printable_is_not_dropped() {
        assert_eq!(decode(b"\x1b[99;1u"), vec![InputEvent::Byte(b'c')]);
    }

    #[test]
    fn unmodified_unicode_is_emitted_as_complete_utf8() {
        assert_eq!(
            decode(b"\x1b[233;1u\x1b[128512;1u"),
            "é😀".bytes().map(InputEvent::Byte).collect::<Vec<_>>()
        );
    }

    #[test]
    fn kitty_press_and_repeat_emit_but_release_does_not() {
        assert_eq!(
            decode(b"\x1b[99;1:1u\x1b[99;1:2u\x1b[99;1:3u"),
            vec![InputEvent::Byte(b'c'), InputEvent::Byte(b'c')]
        );
    }

    #[test]
    fn invalid_modifiers_scalars_and_protocol_keys_are_not_text() {
        assert!(decode(b"\x1b[99;0u\x1b[57344;1u\x1b[55296;1u\x1b[1114112;1u").is_empty());
        assert_eq!(
            decode(b"\x1b[57350;1u\x1b[57351;1u"),
            vec![InputEvent::ArrowLeft, InputEvent::ArrowRight]
        );
    }

    #[test]
    fn kitty_bytes_preserve_seed_without_protocol_reports() {
        let events = decode(b"native escape seed \x1b[12;34R\x1b[99;1u caf\xc3\xa9");
        assert_eq!(
            events,
            "native escape seed c café".bytes().map(InputEvent::Byte).collect::<Vec<_>>()
        );
    }

    #[test]
    fn return_family_is_distinct_and_resolves_via_caller_keymap() {
        assert_eq!(decode(b"\x1b[13;1u"), vec![InputEvent::Byte(13)]);
        assert_eq!(decode(b"\x1b[13;2u"), vec![InputEvent::ShiftEnter]);
        assert_eq!(decode(b"\x1b[13;3u"), vec![InputEvent::AltEnter]);
        assert_eq!(decode(b"\x1b[13;5u"), vec![InputEvent::CtrlJ]);
    }

    #[test]
    fn return_family_accepts_fragmented_input_and_lock_bits() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[13;" { decoder.push(*byte, now); }
        for byte in b"66u\x1b[13;69u" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::ShiftEnter));
        assert_eq!(decoder.pop(), Some(InputEvent::CtrlJ));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn paste_cr_lf_is_one_atomic_event_and_never_keyboard_enter() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[200~one\r\ntwo\r\x1b[201~" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Paste("one\ntwo\n".to_owned())));
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn invalid_utf8_paste_is_rejected_atomically_without_leaking_bytes() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[200~valid\xfftail\x1b[201~" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
        decoder.push(b'x', now);
        assert_eq!(decoder.pop(), Some(InputEvent::Byte(b'x')));
    }

    #[test]
    fn unterminated_paste_is_abandoned_and_late_body_never_becomes_keyboard_input() {
        let mut decoder = TerminalInputDecoder::default();
        let start = Instant::now();
        for byte in b"\x1b[200~partial\r" { decoder.push(*byte, start); }
        decoder.expire(start + PASTE_FRAME_TIMEOUT + ESCAPE_GRACE);
        assert_eq!(decoder.pop(), None);
        for byte in b"tail\r\n" { decoder.push(*byte, start + PASTE_FRAME_TIMEOUT + ESCAPE_GRACE); }
        assert_eq!(decoder.pop(), None);
    }

    #[test]
    fn oversized_paste_is_rejected_but_following_input_remains_atomic() {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        for byte in b"\x1b[200~" { decoder.push(*byte, now); }
        for _ in 0..=MAX_PASTE_BYTES { decoder.push(b'x', now); }
        for byte in b"\x1b[201~" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), None);
        for byte in b"ok" { decoder.push(*byte, now); }
        assert_eq!(decoder.pop(), Some(InputEvent::Byte(b'o')));
        assert_eq!(decoder.pop(), Some(InputEvent::Byte(b'k')));
    }
}

#[cfg(test)]
mod cp31_cp36_cp41_input_tests {
    //! CP-31 (IME/duplicate submit), CP-36 (paste normalization) and CP-41
    //! (renderer-safe offsets). Upstream: `component/prompt/index.tsx:1097-1477`.

    use super::*;

    fn decode(bytes: &[u8]) -> Vec<InputEvent> {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        let mut events = Vec::new();
        for &byte in bytes {
            decoder.push(byte, now);
            while let Some(event) = decoder.pop() {
                events.push(event);
            }
        }
        events
    }

    #[test]
    fn cp36_cp41_paste_normalizes_line_endings_and_preserves_unicode_clusters() {
        let mut frame = Vec::new();
        frame.extend_from_slice(b"\x1b[200~");
        frame.extend_from_slice("界\r\n👩‍👩‍👧‍👦\rதமிழ்\r\n".as_bytes());
        frame.extend_from_slice(b"\x1b[201~");
        assert_eq!(
            decode(&frame),
            vec![InputEvent::Paste(
                "界\n👩‍👩‍👧‍👦\nதமிழ்\n".to_owned()
            )],
            "one atomic paste; CRLF and lone CR become LF; no byte offsets"
        );
        // Framing bytes never reach ordinary keyboard dispatch.
        assert!(decode(b"\x1b[200~\x1b[201~")
            .iter()
            .all(|event| matches!(event, InputEvent::Paste(_))));
    }

    #[test]
    fn cp31_final_hangul_scalar_is_one_complete_ime_event() {
        // U+D55C (한) delivered as a Kitty key report stays a complete UTF-8
        // scalar; the caller's textarea flush sees the final composition.
        let bytes = "\u{d55c}".as_bytes();
        assert_eq!(
            decode(b"\x1b[54620;1u"),
            bytes.iter().copied().map(InputEvent::Byte).collect::<Vec<_>>()
        );
        assert_eq!(
            String::from_utf8(bytes.to_vec()).unwrap(),
            "한"
        );
    }

    #[test]
    fn cp31_rapid_kitty_enters_remain_two_press_events_for_the_submit_gate() {
        // The decoder never coalesces physical presses; deduplication belongs
        // to the composer's SubmitGate, so exactly one submission is sent.
        assert_eq!(
            decode(b"\x1b[13;1u\x1b[13;1u"),
            vec![InputEvent::Byte(13), InputEvent::Byte(13)]
        );
        // Shift/Ctrl/Alt Return stay distinct newline events even under a
        // rapid sequence.
        assert_eq!(
            decode(b"\x1b[13;2u\x1b[13;5u\x1b[13;3u"),
            vec![
                InputEvent::ShiftEnter,
                InputEvent::CtrlJ,
                InputEvent::AltEnter
            ]
        );
    }
}

#[cfg(test)]
mod page_key_regressions {
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<InputEvent> {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        let mut events = Vec::new();
        for &byte in bytes {
            decoder.push(byte, now);
            while let Some(event) = decoder.pop() {
                events.push(event);
            }
        }
        events
    }

    #[test]
    fn dialog08_page_keys_decode_and_other_tilde_keys_stay_protocol() {
        assert_eq!(
            decode(b"\x1b[5~\x1b[6~"),
            vec![InputEvent::PageUp, InputEvent::PageDown]
        );
        assert_eq!(
            decode(b"\x1b[H\x1b[1~\x1b[F\x1b[4~"),
            vec![InputEvent::Home, InputEvent::Home, InputEvent::End, InputEvent::End]
        );
        // Insert/Delete and modified forms are not select paging and must
        // never become typed text or an arrow move.
        assert!(decode(b"\x1b[2~\x1b[3~\x1b[5;2~").is_empty());
        assert!(decode(b"\x1b[5;2~").is_empty());
    }
}

#[cfg(test)]
mod theme13_review_r1_non_ascii_osc_color_tests {
    //! THEME-12/13 review R1: the OSC 10/11 color decoder must never panic on
    //! a hostile frame. `parse_osc_color_text` slices the `#RRGGBB` payload by
    //! BYTE offset, so a multibyte character at a slice boundary (for example
    //! `#a` + U+00E9, whose payload is 3 bytes) used to abort the live input
    //! loop with `byte index 2 is not a char boundary`. Every malformed payload
    //! must instead be abandoned (no `OscColorReply`), and valid replies must
    //! still decode.

    use super::*;

    fn decode(bytes: &[u8]) -> Vec<InputEvent> {
        let mut decoder = TerminalInputDecoder::default();
        let now = Instant::now();
        let mut events = Vec::new();
        for &byte in bytes {
            decoder.push(byte, now);
            while let Some(event) = decoder.pop() {
                events.push(event);
            }
        }
        events
    }

    #[test]
    fn non_ascii_hex_payload_never_panics_and_is_abandoned() {
        // `ESC ] 10 ; # a é BEL`: payload bytes `23 61 c3 a9` (`#aé`) are valid
        // UTF-8 but not ASCII, so `rest[1..2]` lands inside the two-byte scalar
        // and panicked before the ASCII guard.
        let frame = [27u8, 93, b'1', b'0', b';', b'#', b'a', 0xc3, 0xa9, 7];
        assert!(
            decode(&frame).is_empty(),
            "non-ASCII OSC 10 payload must be abandoned, never decoded or leaked as text"
        );

        // A six-byte non-ASCII payload takes the `#rrggbb` branch; it must be
        // rejected by the payload check rather than parsed.
        let long_non_ascii = [27u8, 93, b'1', b'1', b';', b'#', b'a', b'b', 0xc3, 0xa9, b'X', b'Y', 7];
        assert!(decode(&long_non_ascii).is_empty());

        // Non-ASCII inside an `rgb:` reply is likewise abandoned.
        let rgb_non_ascii = [27u8, 93, b'1', b'0', b';', b'r', b'g', b'b', b':', 0xc3, 0xa9, b'/', b'2', b'2', b'/', b'3', b'3', 7];
        assert!(decode(&rgb_non_ascii).is_empty());
    }

    #[test]
    fn overlong_and_malformed_hex_payloads_are_abandoned() {
        // Seven hex digits: no `#rgb`/`#rrggbb` length matches, so it is None.
        assert!(decode(b"\x1b]10;#1234567\x07").is_empty());
        // Five hex digits is not a valid length either.
        assert!(decode(b"\x1b]10;#12345\x07").is_empty());
        // A stray slash in the hex body is not a color.
        assert!(decode(b"\x1b]10;#12/45\x07").is_empty());
    }

    #[test]
    fn valid_rgb_reply_still_decodes_after_the_ascii_guard() {
        // Positive control: the fix must not break the happy path.
        assert_eq!(
            decode(b"\x1b]11;rgb:11/22/33\x07"),
            vec![InputEvent::OscColorReply {
                index: 11,
                rgb: [0x11, 0x22, 0x33, 0xFF]
            }]
        );
        // A valid `#rrggbb` payload continues to decode as opaque RGBA.
        assert_eq!(
            decode(b"\x1b]10;#0a1b2c\x07"),
            vec![InputEvent::OscColorReply {
                index: 10,
                rgb: [0x0A, 0x1B, 0x2C, 0xFF]
            }]
        );
    }
}

#[cfg(test)]
mod ffi11_shared_subset_equivalence_tests {
    //! FFI-11: pins the shared-subset framing equivalence between this
    //! decoder and `opentui_bridge::terminal_input::TerminalInput` by feeding
    //! identical byte frames to both adapters and asserting the same outcome:
    //! one atomic paste, one modifier decode, one isolated capability reply.
    //! The deliberate divergences (oversized/invalid-UTF8
    //! paste fail-closed, OSC 10/11 carrying an RGB payload, deadline states)
    //! are pinned as such rather than "fixed" toward the bridge. See the
    //! crate-header note "keep local, proved equivalent".

    use super::*;
    use opencode_rk_opentui_bridge::terminal_input::{
        CapabilityResponse, KeyCode, KeyEvent, KeyEventKind, TerminalEvent, TerminalInput,
    };
    // Alias avoids the glob collision with this decoder's own 32 KiB cap.
    use opencode_rk_opentui_bridge::terminal_input::MAX_PASTE_BYTES as BRIDGE_MAX_PASTE_BYTES;

    /// Feed the same chunks to both adapters byte-for-byte (the cli decoder
    /// is push-per-byte; the bridge buffers) and drain both queues.
    fn run_both(chunks: &[&[u8]]) -> (Vec<InputEvent>, Vec<TerminalEvent>) {
        let mut cli = TerminalInputDecoder::default();
        let mut bridge = TerminalInput::new();
        let now = Instant::now();
        let mut cli_events = Vec::new();
        for chunk in chunks {
            for &byte in *chunk {
                cli.push(byte, now);
            }
            bridge.feed(chunk);
        }
        while let Some(event) = cli.pop() {
            cli_events.push(event);
        }
        let mut bridge_events = Vec::new();
        while let Some(event) = bridge.next_event() {
            bridge_events.push(event);
        }
        (cli_events, bridge_events)
    }

    #[test]
    fn ffi11_paste_is_atomic_split_and_cr_normalized_identically_in_both_adapters() {
        // The frame is split mid-terminator, the hardest boundary both
        // adapters must survive (`prompt/index.tsx:1805` CRLF/CR -> LF).
        let (cli_events, bridge_events) = run_both(&[
            b"\x1b[200~one\r\ntwo\r\x1b[20",
            b"1~",
        ]);
        assert_eq!(cli_events, vec![InputEvent::Paste("one\ntwo\n".to_owned())]);
        assert_eq!(
            bridge_events,
            vec![TerminalEvent::Paste {
                bytes: b"one\r\ntwo\r".to_vec(),
                truncated: false,
            }]
        );
        // The normalization result is byte-identical on both sides.
        let bridge_text = bridge_events[0].paste_text().expect("bridge paste");
        assert_eq!(bridge_text, "one\ntwo\n");
        match &cli_events[0] {
            InputEvent::Paste(text) => assert_eq!(*text, bridge_text),
            other => panic!("expected cli paste, got {other:?}"),
        }
    }

    #[test]
    fn ffi11_kitty_modifier_parameters_agree_with_bridge_modifiers() {
        // (frame, cli event, bridge KeyCode, (shift, alt, ctrl)). Both
        // adapters must read `modifier - 1` bits: shift=1, alt=2, ctrl=4
        // (`terminal_input.rs` `Modifiers::from_protocol`).
        let cases: &[(&[u8], InputEvent, KeyCode, (bool, bool, bool))] = &[
            (b"\x1b[13;2u", InputEvent::ShiftEnter, KeyCode::Enter, (true, false, false)),
            (b"\x1b[13;5u", InputEvent::CtrlJ, KeyCode::Enter, (false, false, true)),
            (b"\x1b[13;3u", InputEvent::AltEnter, KeyCode::Enter, (false, true, false)),
            (b"\x1b[9;5u", InputEvent::TabNext, KeyCode::Tab, (false, false, true)),
            (b"\x1b[9;6u", InputEvent::TabPrevious, KeyCode::Tab, (true, false, true)),
            (b"\x1b[49;5u", InputEvent::TabSlot(0), KeyCode::Char('1'), (false, false, true)),
            (b"\x1b[116;6u", InputEvent::TabReopen, KeyCode::Char('t'), (true, false, true)),
        ];
        for (frame, cli_expected, bridge_code, (shift, alt, ctrl)) in cases {
            let (cli_events, bridge_events) = run_both(&[frame]);
            assert_eq!(cli_events, vec![cli_expected.clone()], "cli {frame:?}");
            assert_eq!(bridge_events.len(), 1, "bridge {frame:?}");
            let TerminalEvent::Key(KeyEvent { code, modifiers, kind }) = &bridge_events[0] else {
                panic!("bridge key expected for {frame:?}, got {:?}", bridge_events[0]);
            };
            assert_eq!(code, bridge_code, "bridge code for {frame:?}");
            assert_eq!((modifiers.shift, modifiers.alt, modifiers.ctrl), (*shift, *alt, *ctrl));
            assert_eq!(*kind, KeyEventKind::Press, "press mod {frame:?}");
        }
    }

    #[test]
    fn ffi11_capability_responses_stay_isolated_from_composer_input() {
        // The bridge models each reply; this decoder is the composer feed and
        // discards all of them (only OSC 10/11 is captured, see THEME-13).
        let frames: &[(&[u8], TerminalEvent)] = &[
            (
                b"\x1b[12;34R",
                TerminalEvent::Response(CapabilityResponse::CursorPosition { row: 12, col: 34 }),
            ),
            (
                b"\x1b[?62;22c",
                TerminalEvent::Response(CapabilityResponse::DeviceAttributes {
                    secondary: false,
                    values: vec![62, 22],
                }),
            ),
            (
                b"\x1b]0;opencode\x07",
                TerminalEvent::Response(CapabilityResponse::Opaque { code: 0 }),
            ),
            (
                b"\x1b]7777;unknown\x07",
                TerminalEvent::Response(CapabilityResponse::Opaque { code: 7777 }),
            ),
        ];
        for (frame, bridge_expected) in frames {
            let (cli_events, bridge_events) = run_both(&[frame]);
            assert!(cli_events.is_empty(), "cli must discard {frame:?}, got {cli_events:?}");
            assert_eq!(bridge_events, vec![bridge_expected.clone()], "bridge {frame:?}");
        }
    }

    #[test]
    fn ffi11_osc_10_11_replies_carry_rgb_here_but_only_opaque_code_in_the_bridge() {
        let frames: &[(&[u8], u8, [u8; 4])] = &[
            (b"\x1b]11;rgb:11/22/33\x07", 11, [0x11, 0x22, 0x33, 0xFF]),
            (b"\x1b]10;#0a1b2c\x07", 10, [0x0A, 0x1B, 0x2C, 0xFF]),
        ];
        for &(frame, index, rgb) in frames {
            let (cli_events, bridge_events) = run_both(&[frame]);
            assert_eq!(
                cli_events,
                vec![InputEvent::OscColorReply { index, rgb }],
                "cli THEME-13 payload for {frame:?}"
            );
            assert_eq!(
                bridge_events,
                vec![TerminalEvent::Response(CapabilityResponse::Opaque {
                    code: u16::from(index)
                })],
                "bridge keeps only the code for {frame:?}"
            );
        }
    }

    #[test]
    fn ffi11_oversized_paste_fails_closed_here_but_truncates_in_the_bridge() {
        assert_eq!(
            super::MAX_PASTE_BYTES,
            BRIDGE_MAX_PASTE_BYTES,
            "32 KiB cap is shared"
        );
        let mut frame = b"\x1b[200~".to_vec();
        frame.extend_from_slice(&vec![b'x'; super::MAX_PASTE_BYTES + 2000]);
        frame.extend_from_slice(b"\x1b[201~");
        let (cli_events, bridge_events) = run_both(&[&frame, b"ok"]);
        // cli: the oversized frame is dropped whole; following input stays
        // atomic (the divergent fail-closed pin).
        assert_eq!(
            cli_events,
            [InputEvent::Byte(b'o'), InputEvent::Byte(b'k')]
        );
        // bridge: truncated paste plus the tail text.
        assert_eq!(bridge_events.len(), 2);
        let TerminalEvent::Paste { bytes, truncated } = &bridge_events[0] else {
            panic!("bridge paste expected, got {:?}", bridge_events[0]);
        };
        assert!(*truncated);
        assert_eq!(bytes.len(), BRIDGE_MAX_PASTE_BYTES);
        assert_eq!(bridge_events[1], TerminalEvent::Text("ok".into()));
    }

    #[test]
    fn ffi11_invalid_utf8_paste_fails_closed_here_but_is_lossy_in_the_bridge() {
        let mut frame = b"\x1b[200~cat\xff\n".to_vec();
        frame.extend_from_slice(b"\x1b[201~");
        let (cli_events, bridge_events) = run_both(&[&frame, b"x"]);
        // cli: the whole frame is rejected; the composer stays clean.
        assert_eq!(cli_events, vec![InputEvent::Byte(b'x')]);
        // bridge: lossy replacement keeps the shape (paste_text contract).
        assert_eq!(bridge_events.len(), 2);
        assert_eq!(
            bridge_events[0].paste_text(),
            Some("cat\u{FFFD}\n".to_owned()),
            "bridge replaces the invalid scalar instead of dropping"
        );
        assert_eq!(bridge_events[1], TerminalEvent::Text("x".into()));
    }
}
