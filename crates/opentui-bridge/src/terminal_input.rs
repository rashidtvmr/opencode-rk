#![forbid(unsafe_code)]
//! Raw terminal input adapter that preserves paste, modifiers, terminal
//! protocols, mouse movement and capability responses (FFI-10).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §3
//!   "Input and Solid" (refs [E1] `packages/core/src/lib/stdin-parser.ts:1-100`
//!   and [E2] `packages/core/src/renderer.ts:3420-3488` at OpenTUI pin
//!   `8d6065a3e60039d16c6ee697692dcfbc46d3c786`) states the stdin parser owns
//!   byte framing and protocol recognition and produces key, SGR/X10 mouse,
//!   paste and terminal-response events, and that renderer dispatch separates
//!   capability responses from keys and routes paste bytes separately.
//!   Reimplementing printable characters and arrows alone would leave those
//!   behaviours unmatched.
//! - The vendored library exports no input-*parser* symbol
//!   (`nm -gU …/libopentui.dylib` has no `stdin*`/`parseInput*` export), so
//!   this module owns framing in Rust. The artifact does export encoders
//!   (`embeddedTerminalEncodeKey/Mouse/Paste`) and terminal-mode toggles
//!   (`enableMouse`, `enableKittyKeyboard`), which are writer-side concerns and
//!   are not used here. This module does not duplicate the keymap: the caller
//!   resolves binds exactly as OpenCode does with `keybind.ts`
//!   (`ctrl`/`alt`/`shift`/`super`, `packages/tui/src/config/keybind.ts:8-14`,
//!   pin `450e7b9`).
//! - Protocol expectations are pinned from OpenCode's runtime:
//!   * Kitty keyboard is negotiated and enabled/disabled by the host
//!     (`packages/tui/src/app.tsx:256`, `mini/runtime.lifecycle.ts:239-242`);
//!   * mouse movement is a renderer option (`useMouse`,
//!     `app.tsx:259,631`) and hover/move/drag handlers consume it
//!     (`ui/dialog.tsx:296-297`, `ui/pane-resize.ts:47-73`);
//!   * pasted bytes are decoded and newline-normalised by the prompt before
//!     insertion (`component/prompt/index.tsx:1805,1447-1448`), so the
//!     adapter must hand over the raw bytes rather than a lossy string.

/// Bracketed-paste start/end markers.
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Paste body cap (matches the composer's 32 KiB decoder budget).
pub const MAX_PASTE_BYTES: usize = 32 * 1024;
/// Unparsed input buffer cap (a sequence that never terminates is dropped).
pub const MAX_PENDING_BYTES: usize = 64 * 1024;
/// Device-attribute parameter cap.
pub const MAX_RESPONSE_PARAMS: usize = 16;

/// Modifier set with the protocol bits used by both xterm and Kitty.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    /// Kitty/xterm "super" (Command on macOS, Win key).
    pub super_key: bool,
    pub hyper: bool,
    pub meta: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
}

impl Modifiers {
    /// Decode the xterm/Kitty modifier parameter, which is `1 + bitmask`.
    /// Bit 0 shift, 1 alt, 2 ctrl, 3 super, 4 hyper, 5 meta, 6 caps, 7 num.
    #[must_use]
    pub const fn from_protocol(parameter: u16) -> Self {
        let bits = parameter.saturating_sub(1);
        Self {
            shift: bits & 1 != 0,
            alt: bits & 2 != 0,
            ctrl: bits & 4 != 0,
            super_key: bits & 8 != 0,
            hyper: bits & 16 != 0,
            meta: bits & 32 != 0,
            caps_lock: bits & 64 != 0,
            num_lock: bits & 128 != 0,
        }
    }

    /// Encode back to the `1 + bitmask` parameter (round-trips
    /// [`Modifiers::from_protocol`]).
    #[must_use]
    pub const fn to_protocol(self) -> u16 {
        let mut bits = 0u16;
        if self.shift {
            bits |= 1;
        }
        if self.alt {
            bits |= 2;
        }
        if self.ctrl {
            bits |= 4;
        }
        if self.super_key {
            bits |= 8;
        }
        if self.hyper {
            bits |= 16;
        }
        if self.meta {
            bits |= 32;
        }
        if self.caps_lock {
            bits |= 64;
        }
        if self.num_lock {
            bits |= 128;
        }
        bits + 1
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.to_protocol() == 1
    }

    #[must_use]
    pub const fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Self::empty_set()
        }
    }

    const fn empty_set() -> Self {
        Self {
            shift: false,
            alt: false,
            ctrl: false,
            super_key: false,
            hyper: false,
            meta: false,
            caps_lock: false,
            num_lock: false,
        }
    }

    #[must_use]
    pub const fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    #[must_use]
    pub const fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    #[must_use]
    pub const fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
}

/// Logical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    /// Function key number (1-based).
    F(u8),
}

/// Key event kind: Kitty reports press/repeat/release distinctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

/// One key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub kind: KeyEventKind,
}

impl KeyEvent {
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: Modifiers, kind: KeyEventKind) -> Self {
        Self {
            code,
            modifiers,
            kind,
        }
    }
}

/// Mouse button; `None` for motion without a held button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    None,
    Left,
    Middle,
    Right,
    Wheel,
}

/// Mouse gesture kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Press,
    Release,
    /// Button held and moved.
    Drag,
    /// Motion without a button (hover).
    Move,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

/// One mouse event. Coordinates are 0-based cells, origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub kind: MouseKind,
    pub button: MouseButton,
    pub modifiers: Modifiers,
}

/// Terminal response to a host query. Kept distinguishable from keys so it is
/// never routed into the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityResponse {
    /// `CSI row;col R` (DSR/CPR).
    CursorPosition { row: u16, col: u16 },
    /// `CSI ? … c` / `CSI > … c`.
    DeviceAttributes {
        secondary: bool,
        values: Vec<u16>,
    },
    /// `CSI ? flags u` (Kitty keyboard mode report).
    KeyboardFlags { flags: u8 },
    /// `CSI 4 ; height ; width t` (pixel resolution query).
    PixelResolution { width: u16, height: u16 },
    /// `CSI ? mode ; status $ y` (DECRQM).
    ModeReport { mode: u16, status: u8 },
    /// OSC/DCS/APC reply, consumed with its code only.
    Opaque { code: u16 },
}

/// One decoded terminal event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Key(KeyEvent),
    /// Committed text (printable run or IME scalar sequence).
    Text(String),
    /// Bracketed paste bytes, exactly as received (minus the markers).
    Paste { bytes: Vec<u8>, truncated: bool },
    Mouse(MouseEvent),
    Focus(bool),
    Response(CapabilityResponse),
    /// A syntactically complete escape sequence with no modelled meaning.
    Unknown,
}

impl TerminalEvent {
    /// Paste text with the prompt's newline normalisation applied
    /// (`prompt/index.tsx:1805`: `\r\n` and `\r` become `\n`). Invalid UTF-8
    /// is replaced rather than dropped, matching `decodePasteBytes`.
    #[must_use]
    pub fn paste_text(&self) -> Option<String> {
        match self {
            Self::Paste { bytes, .. } => {
                Some(String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n"))
            }
            _ => None,
        }
    }
}

enum Parsed {
    Event(TerminalEvent, usize),
    NeedMore,
}

/// Incremental byte-framing input adapter.
///
/// Bytes may arrive split at any boundary: a sequence that is not yet complete
/// yields no event and is retried on the next [`TerminalInput::feed`].
#[derive(Debug)]
pub struct TerminalInput {
    buffer: Vec<u8>,
    /// Paste bytes dropped because the frame exceeded [`MAX_PASTE_BYTES`].
    dropped: usize,
}

impl Default for TerminalInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalInput {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            dropped: 0,
        }
    }

    /// Bytes retained awaiting a complete sequence.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// Append raw bytes read from the terminal.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
        // Bracketed paste has its own bound (body cap + terminator tail), so
        // trim its middle instead of decapitating the opening marker. Never
        // trim once the terminator is present: the completion path truncates
        // the body itself and must see every terminator byte.
        if self.buffer.starts_with(PASTE_START)
            && find(&self.buffer[PASTE_START.len()..], PASTE_END).is_none()
        {
            self.trim_paste_overflow();
        }
        // A stream of malformed input must never grow the buffer without
        // bound; drop the oldest bytes (they can no longer form a sequence
        // that starts here) and keep the tail.
        if self.buffer.len() > MAX_PENDING_BYTES {
            let excess = self.buffer.len() - MAX_PENDING_BYTES;
            self.buffer.drain(..excess);
        }
    }

    /// Bound a paste body that has not seen its terminator yet: keep the first
    /// [`MAX_PASTE_BYTES`] body bytes plus the terminator-length tail so a
    /// split `ESC[201~` is still recognized. Skipped bytes are reported as
    /// truncation when the paste completes.
    fn trim_paste_overflow(&mut self) {
        let body_start = PASTE_START.len();
        if self.buffer.len() <= body_start + MAX_PASTE_BYTES + PASTE_END.len() {
            return;
        }
        let keep_tail = PASTE_END.len() - 1;
        let drop_from = body_start + MAX_PASTE_BYTES;
        let drop_to = self.buffer.len() - keep_tail;
        if drop_to > drop_from {
            self.dropped += drop_to - drop_from;
            self.buffer.drain(drop_from..drop_to);
        }
    }

    /// Next complete event, or `None` while a sequence is incomplete.
    pub fn next_event(&mut self) -> Option<TerminalEvent> {
        if self.buffer.is_empty() {
            return None;
        }
        let parsed = self.parse();
        match parsed {
            Parsed::NeedMore => None,
            Parsed::Event(event, len) => {
                self.buffer.drain(..len);
                Some(event)
            }
        }
    }

    /// Resolve a lone `ESC` as Escape after the host's escape-grace timeout.
    /// Returns `None` when the buffer holds a longer, still-incomplete
    /// sequence (the host should keep waiting or flush the frame itself).
    pub fn flush_escape(&mut self) -> Option<TerminalEvent> {
        if self.buffer == [0x1b] {
            self.buffer.clear();
            return Some(TerminalEvent::Key(KeyEvent::new(
                KeyCode::Escape,
                Modifiers::default(),
                KeyEventKind::Press,
            )));
        }
        None
    }

    fn parse(&mut self) -> Parsed {
        if self.buffer.starts_with(PASTE_START) {
            return self.parse_paste();
        }
        match self.buffer[0] {
            0x1b => self.parse_escape(),
            0x0d | 0x0a => Parsed::Event(
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Enter,
                    Modifiers::default(),
                    KeyEventKind::Press,
                )),
                1,
            ),
            0x09 => Parsed::Event(
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Tab,
                    Modifiers::default(),
                    KeyEventKind::Press,
                )),
                1,
            ),
            0x7f => Parsed::Event(
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Backspace,
                    Modifiers::default(),
                    KeyEventKind::Press,
                )),
                1,
            ),
            byte @ 0x00..=0x1f => {
                let code = match byte {
                    0x00 => KeyCode::Char(' '),
                    0x1c => KeyCode::Char('\\'),
                    0x1d => KeyCode::Char(']'),
                    0x1e => KeyCode::Char('^'),
                    0x1f => KeyCode::Char('_'),
                    _ => KeyCode::Char((b'a' + byte - 1) as char),
                };
                Parsed::Event(
                    TerminalEvent::Key(KeyEvent::new(
                        code,
                        Modifiers::ctrl(),
                        KeyEventKind::Press,
                    )),
                    1,
                )
            }
            _ => self.parse_text(),
        }
    }

    fn parse_text(&self) -> Parsed {
        let mut text = String::new();
        let mut index = 0;
        let bytes = &self.buffer;
        while index < bytes.len() {
            let byte = bytes[index];
            if byte == 0x1b || byte < 0x20 || byte == 0x7f {
                break;
            }
            let Some(ch) = next_char(&bytes[index..]) else {
                if text.is_empty() {
                    // A valid multi-byte prefix with too few bytes waits for
                    // the rest; an invalid lead byte is consumed so the
                    // decoder always makes progress.
                    let lead = bytes[index];
                    if (0xc0..=0xf7).contains(&lead) {
                        return Parsed::NeedMore;
                    }
                    return Parsed::Event(TerminalEvent::Unknown, 1);
                }
                break;
            };
            text.push(ch);
            index += ch.len_utf8();
        }
        if index == 0 {
            return Parsed::NeedMore;
        }
        Parsed::Event(TerminalEvent::Text(text), index)
    }

    fn parse_paste(&mut self) -> Parsed {
        let body_start = PASTE_START.len();
        if let Some(rel) = find(&self.buffer[body_start..], PASTE_END) {
            let body_end = body_start + rel;
            let mut body = self.buffer[body_start..body_end].to_vec();
            let mut truncated = self.dropped > 0;
            if body.len() > MAX_PASTE_BYTES {
                body.truncate(char_boundary(&body, MAX_PASTE_BYTES));
                truncated = true;
            }
            self.dropped = 0;
            return Parsed::Event(
                TerminalEvent::Paste {
                    bytes: body,
                    truncated,
                },
                body_end + PASTE_END.len(),
            );
        }
        // No terminator yet. Bound the retained body to the paste cap while
        // keeping the last few bytes so a split terminator is still detected;
        // the skipped middle is reported as truncation.
        self.trim_paste_overflow();
        Parsed::NeedMore
    }

    fn parse_escape(&self) -> Parsed {
        let bytes = &self.buffer;
        if bytes.len() == 1 {
            return Parsed::NeedMore;
        }
        match bytes[1] {
            b'[' => self.parse_csi(),
            b'O' => self.parse_ss3(),
            b']' => self.parse_control_string(2),
            b'P' => self.parse_control_string(2),
            b'_' => self.parse_control_string(2),
            // Alt+<key>: a single ESC prefix over a printable scalar.
            _ => {
                let Some(ch) = next_char(&bytes[1..]) else {
                    return Parsed::NeedMore;
                };
                let code = match ch {
                    '\r' | '\n' => KeyCode::Enter,
                    '\t' => KeyCode::Tab,
                    other => KeyCode::Char(other),
                };
                Parsed::Event(
                    TerminalEvent::Key(KeyEvent::new(
                        code,
                        Modifiers::default().with_alt(),
                        KeyEventKind::Press,
                    )),
                    1 + ch.len_utf8(),
                )
            }
        }
    }

    fn parse_ss3(&self) -> Parsed {
        let bytes = &self.buffer;
        if bytes.len() < 3 {
            return Parsed::NeedMore;
        }
        let code = match bytes[2] {
            b'A' => KeyCode::Up,
            b'B' => KeyCode::Down,
            b'C' => KeyCode::Right,
            b'D' => KeyCode::Left,
            b'H' => KeyCode::Home,
            b'F' => KeyCode::End,
            b'P' => KeyCode::F(1),
            b'Q' => KeyCode::F(2),
            b'R' => KeyCode::F(3),
            b'S' => KeyCode::F(4),
            _ => return Parsed::Event(TerminalEvent::Unknown, 3),
        };
        Parsed::Event(
            TerminalEvent::Key(KeyEvent::new(
                code,
                Modifiers::default(),
                KeyEventKind::Press,
            )),
            3,
        )
    }

    fn parse_csi(&self) -> Parsed {
        let bytes = &self.buffer;
        // Find the final byte: 0x40..=0x7e after the introducer.
        let Some(offset) = bytes[2..]
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
        else {
            return Parsed::NeedMore;
        };
        let final_index = 2 + offset;
        let final_byte = bytes[final_index];
        let params = &bytes[2..final_index];
        let params = std::str::from_utf8(params).unwrap_or("");
        let consumed = final_index + 1;

        // X10 mouse: `CSI M` followed by exactly three bytes.
        if final_byte == b'M' && params.is_empty() {
            if bytes.len() < consumed + 3 {
                return Parsed::NeedMore;
            }
            let raw = bytes[consumed].wrapping_sub(32);
            let x = u16::from(bytes[consumed + 1].wrapping_sub(32));
            let y = u16::from(bytes[consumed + 2].wrapping_sub(32));
            let (kind, button) = decode_mouse(raw, true);
            // Modifier bits already live in the button byte (bits 2..4).
            let modifiers =
                Modifiers::from_protocol(u16::from((raw & 0b0001_1100) >> 2) + 1);
            return Parsed::Event(
                TerminalEvent::Mouse(MouseEvent {
                    x: x.saturating_sub(1),
                    y: y.saturating_sub(1),
                    kind,
                    button,
                    modifiers,
                }),
                consumed + 3,
            );
        }

        if final_byte == b'M' || final_byte == b'm' {
            // SGR mouse: `<b;x;y`.
            let Some(rest) = params.strip_prefix('<') else {
                return Parsed::Event(TerminalEvent::Unknown, consumed);
            };
            let parts: Vec<&str> = rest.split(';').collect();
            if parts.len() != 3 {
                return Parsed::Event(TerminalEvent::Unknown, consumed);
            }
            let (Some(button), Some(x), Some(y)) = (
                parts[0].parse::<u16>().ok(),
                parts[1].parse::<u16>().ok(),
                parts[2].parse::<u16>().ok(),
            ) else {
                return Parsed::Event(TerminalEvent::Unknown, consumed);
            };
            let release = final_byte == b'm';
            let (kind, decoded) = if release {
                (MouseKind::Release, MouseButton::None)
            } else {
                decode_mouse(button as u8, false)
            };
            let modifiers = Modifiers::from_protocol(((button & 0b0001_1100) >> 2) + 1);
            return Parsed::Event(
                TerminalEvent::Mouse(MouseEvent {
                    x: x.saturating_sub(1),
                    y: y.saturating_sub(1),
                    kind,
                    button: decoded,
                    modifiers,
                }),
                consumed,
            );
        }

        match final_byte {
            b'A' | b'B' | b'C' | b'D' | b'H' | b'F' => {
                let (code, modifiers) = arrow(params, final_byte);
                Parsed::Event(
                    TerminalEvent::Key(KeyEvent::new(
                        code,
                        modifiers,
                        KeyEventKind::Press,
                    )),
                    consumed,
                )
            }
            b'Z' => Parsed::Event(
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Tab,
                    Modifiers::default().with_shift(),
                    KeyEventKind::Press,
                )),
                consumed,
            ),
            b'~' => self.parse_tilde(params, consumed),
            b'u' => self.parse_kitty(params, consumed),
            b'R' => {
                let parts: Vec<u16> = params
                    .split(';')
                    .filter_map(|part| part.parse().ok())
                    .collect();
                if parts.len() == 2 {
                    Parsed::Event(
                        TerminalEvent::Response(CapabilityResponse::CursorPosition {
                            row: parts[0],
                            col: parts[1],
                        }),
                        consumed,
                    )
                } else {
                    Parsed::Event(TerminalEvent::Unknown, consumed)
                }
            }
            b'c' => {
                let (secondary, rest) = match params
                    .strip_prefix('>')
                    .map(|rest| (true, rest))
                    .or_else(|| params.strip_prefix('?').map(|rest| (false, rest)))
                {
                    Some(pair) => pair,
                    None => return Parsed::Event(TerminalEvent::Unknown, consumed),
                };
                let values: Vec<u16> = rest
                    .split(';')
                    .filter_map(|part| part.parse().ok())
                    .take(MAX_RESPONSE_PARAMS)
                    .collect();
                Parsed::Event(
                    TerminalEvent::Response(CapabilityResponse::DeviceAttributes {
                        secondary,
                        values,
                    }),
                    consumed,
                )
            }
            b'y' => {
                // DECRQM: `?mode;status$y`.
                let body = params.trim_end_matches('$');
                let body = body.strip_prefix('?').unwrap_or(body);
                let parts: Vec<u16> = body.split(';').filter_map(|part| part.parse().ok()).collect();
                if parts.len() == 2 {
                    Parsed::Event(
                        TerminalEvent::Response(CapabilityResponse::ModeReport {
                            mode: parts[0],
                            status: parts[1] as u8,
                        }),
                        consumed,
                    )
                } else {
                    Parsed::Event(TerminalEvent::Unknown, consumed)
                }
            }
            b't' => {
                // Pixel resolution: `4;height;widtht`.
                let parts: Vec<u16> = params.split(';').filter_map(|part| part.parse().ok()).collect();
                if parts.len() == 3 && parts[0] == 4 {
                    Parsed::Event(
                        TerminalEvent::Response(CapabilityResponse::PixelResolution {
                            height: parts[1],
                            width: parts[2],
                        }),
                        consumed,
                    )
                } else {
                    Parsed::Event(TerminalEvent::Unknown, consumed)
                }
            }
            b'I' if params.is_empty() => {
                Parsed::Event(TerminalEvent::Focus(true), consumed)
            }
            b'O' if params.is_empty() => {
                Parsed::Event(TerminalEvent::Focus(false), consumed)
            }
            _ => Parsed::Event(TerminalEvent::Unknown, consumed),
        }
    }

    fn parse_tilde(&self, params: &str, consumed: usize) -> Parsed {
        let mut parts = params.split(';');
        let Some(number) = parts.next().and_then(|part| part.parse::<u16>().ok()) else {
            return Parsed::Event(TerminalEvent::Unknown, consumed);
        };
        let modifiers = parts
            .next()
            .and_then(|part| part.parse::<u16>().ok())
            .map(Modifiers::from_protocol)
            .unwrap_or_default();
        let code = match number {
            1 | 7 => KeyCode::Home,
            2 => KeyCode::Insert,
            3 => KeyCode::Delete,
            4 | 8 => KeyCode::End,
            5 => KeyCode::PageUp,
            6 => KeyCode::PageDown,
            11 => KeyCode::F(1),
            12 => KeyCode::F(2),
            13 => KeyCode::F(3),
            14 => KeyCode::F(4),
            15 => KeyCode::F(5),
            17 => KeyCode::F(6),
            18 => KeyCode::F(7),
            19 => KeyCode::F(8),
            20 => KeyCode::F(9),
            21 => KeyCode::F(10),
            23 => KeyCode::F(11),
            24 => KeyCode::F(12),
            _ => return Parsed::Event(TerminalEvent::Unknown, consumed),
        };
        Parsed::Event(
            TerminalEvent::Key(KeyEvent::new(
                code,
                modifiers,
                KeyEventKind::Press,
            )),
            consumed,
        )
    }

    fn parse_kitty(&self, params: &str, consumed: usize) -> Parsed {
        // `CSI ? flags u` is the keyboard mode report, not a key.
        if let Some(flags) = params.strip_prefix('?') {
            let Ok(flags) = flags.parse::<u8>() else {
                return Parsed::Event(TerminalEvent::Unknown, consumed);
            };
            return Parsed::Event(
                TerminalEvent::Response(CapabilityResponse::KeyboardFlags { flags }),
                consumed,
            );
        }
        let parts: Vec<&str> = params.split(';').collect();
        let Some(code) = parts.first().and_then(|part| part.parse::<u32>().ok()) else {
            return Parsed::Event(TerminalEvent::Unknown, consumed);
        };
        let modifiers = parts
            .get(1)
            .and_then(|part| part.parse::<u16>().ok())
            .map(Modifiers::from_protocol)
            .unwrap_or_default();
        let kind = match parts.get(2).and_then(|part| part.parse::<u8>().ok()) {
            Some(2) => KeyEventKind::Repeat,
            Some(3) => KeyEventKind::Release,
            _ => KeyEventKind::Press,
        };
        let code = match code {
            13 => KeyCode::Enter,
            9 => KeyCode::Tab,
            127 => KeyCode::Backspace,
            27 => KeyCode::Escape,
            57344 => KeyCode::Escape,
            57345 => KeyCode::Enter,
            57346 => KeyCode::Tab,
            57347 => KeyCode::Backspace,
            57348 => KeyCode::Insert,
            57349 => KeyCode::Delete,
            57350 => KeyCode::Left,
            57351 => KeyCode::Right,
            57352 => KeyCode::Up,
            57353 => KeyCode::Down,
            57354 => KeyCode::PageUp,
            57355 => KeyCode::PageDown,
            57356 => KeyCode::Home,
            57357 => KeyCode::End,
            57364..=57398 => KeyCode::F((code - 57363) as u8),
            code => match char::from_u32(code) {
                Some(ch) => KeyCode::Char(ch),
                None => return Parsed::Event(TerminalEvent::Unknown, consumed),
            },
        };
        Parsed::Event(
            TerminalEvent::Key(KeyEvent::new(code, modifiers, kind)),
            consumed,
        )
    }

    fn parse_control_string(&self, intro_len: usize) -> Parsed {
        let bytes = &self.buffer[intro_len..];
        // BEL terminates OSC; ESC \ (ST) terminates all control strings.
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == 0x07 {
                let code = if intro_len == 2 && self.buffer[1] == b']' {
                    parse_osc_code(&self.buffer[intro_len..intro_len + index])
                } else {
                    0
                };
                return Parsed::Event(
                    TerminalEvent::Response(CapabilityResponse::Opaque { code }),
                    intro_len + index + 1,
                );
            }
            if bytes[index] == 0x1b {
                if index + 1 >= bytes.len() {
                    return Parsed::NeedMore;
                }
                if bytes[index + 1] == b'\\' {
                    let code = if intro_len == 2 && self.buffer[1] == b']' {
                        parse_osc_code(&self.buffer[intro_len..intro_len + index])
                    } else {
                        0
                    };
                    return Parsed::Event(
                        TerminalEvent::Response(CapabilityResponse::Opaque { code }),
                        intro_len + index + 2,
                    );
                }
            }
            index += 1;
        }
        Parsed::NeedMore
    }
}

fn next_char(bytes: &[u8]) -> Option<char> {
    let len = match bytes.first()? {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return None,
    };
    if bytes.len() < len {
        return None;
    }
    std::str::from_utf8(&bytes[..len]).ok()?.chars().next()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn char_boundary(bytes: &[u8], limit: usize) -> usize {
    let mut end = limit.min(bytes.len());
    while end > 0 && (bytes[end] & 0xc0) == 0x80 {
        end -= 1;
    }
    end
}

fn parse_osc_code(body: &[u8]) -> u16 {
    let text = std::str::from_utf8(body).unwrap_or("");
    text.split(';')
        .next()
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

fn arrow(params: &str, final_byte: u8) -> (KeyCode, Modifiers) {
    let mut parts = params.split(';');
    let modifiers = parts
        .nth(1)
        .and_then(|part| part.parse::<u16>().ok())
        .map(Modifiers::from_protocol)
        .unwrap_or_default();
    let code = match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        _ => KeyCode::End,
    };
    (code, modifiers)
}

/// Decode a mouse button byte into a gesture.
///
/// Bit 2 = shift, 3 = alt/meta, 4 = ctrl, 5 = motion, 6 = wheel, 7 = wheel
/// horizontal (xterm tracking). `release_wins` marks X10, where button code 3
/// means release rather than "no button".
fn decode_mouse(button: u8, release_wins: bool) -> (MouseKind, MouseButton) {
    let motion = button & 32 != 0;
    let wheel = button & 64 != 0;
    let low = button & 3;
    if release_wins && low == 3 {
        return (MouseKind::Release, MouseButton::None);
    }
    if wheel {
        let kind = match low {
            0 => MouseKind::WheelUp,
            1 => MouseKind::WheelDown,
            2 => MouseKind::WheelLeft,
            _ => MouseKind::WheelRight,
        };
        return (kind, MouseButton::Wheel);
    }
    let held = match low {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::None,
    };
    let kind = if motion {
        if held == MouseButton::None {
            MouseKind::Move
        } else {
            MouseKind::Drag
        }
    } else {
        MouseKind::Press
    };
    (kind, held)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(input: &mut TerminalInput) -> Vec<TerminalEvent> {
        let mut out = Vec::new();
        while let Some(event) = input.next_event() {
            out.push(event);
        }
        out
    }

    fn key(code: KeyCode, modifiers: Modifiers) -> TerminalEvent {
        TerminalEvent::Key(KeyEvent::new(code, modifiers, KeyEventKind::Press))
    }

    #[test]
    fn plain_text_and_utf8_survive_split_reads() {
        let mut input = TerminalInput::new();
        // "héllo" split in the middle of the two-byte scalar.
        input.feed(b"h\xc3");
        assert_eq!(events(&mut input), [TerminalEvent::Text("h".into())]);
        assert_eq!(input.pending(), 1);
        input.feed(b"\xa9llo");
        assert_eq!(events(&mut input), [TerminalEvent::Text("éllo".into())]);
        assert_eq!(input.pending(), 0);
    }

    #[test]
    fn control_bytes_become_ctrl_modified_keys() {
        let mut input = TerminalInput::new();
        input.feed(b"\x01\x03\r\n\t\x7f\x1d");
        let events = events(&mut input);
        assert_eq!(
            events,
            [
                key(KeyCode::Char('a'), Modifiers::ctrl()),
                key(KeyCode::Char('c'), Modifiers::ctrl()),
                key(KeyCode::Enter, Modifiers::default()),
                key(KeyCode::Enter, Modifiers::default()),
                key(KeyCode::Tab, Modifiers::default()),
                key(KeyCode::Backspace, Modifiers::default()),
                key(KeyCode::Char(']'), Modifiers::ctrl()),
            ]
        );
    }

    #[test]
    fn modifiers_round_trip_through_protocol_encoding() {
        for parameter in 1..=255u16 {
            assert_eq!(Modifiers::from_protocol(parameter).to_protocol(), parameter);
        }
        let all = Modifiers::from_protocol(256);
        assert!(all.shift && all.alt && all.ctrl && all.super_key && all.hyper);
        assert!(all.meta && all.caps_lock && all.num_lock);
        assert!(!all.is_empty());
        assert!(Modifiers::default().is_empty());
        assert_eq!(Modifiers::default().with_alt().to_protocol(), 3);
    }

    #[test]
    fn legacy_arrows_carry_modifiers() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[A\x1b[1;5C\x1b[1;3B\x1b[Z\x1bOH");
        assert_eq!(
            events(&mut input),
            [
                key(KeyCode::Up, Modifiers::default()),
                key(KeyCode::Right, Modifiers::ctrl()),
                key(KeyCode::Down, Modifiers::default().with_alt()),
                key(KeyCode::Tab, Modifiers::default().with_shift()),
                key(KeyCode::Home, Modifiers::default()),
            ]
        );
    }

    #[test]
    fn navigation_and_function_keys() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[2~\x1b[3~\x1b[5~\x1b[6~\x1b[15;2~\x1bOP");
        assert_eq!(
            events(&mut input),
            [
                key(KeyCode::Insert, Modifiers::default()),
                key(KeyCode::Delete, Modifiers::default()),
                key(KeyCode::PageUp, Modifiers::default()),
                key(KeyCode::PageDown, Modifiers::default()),
                key(KeyCode::F(5), Modifiers::default().with_shift()),
                key(KeyCode::F(1), Modifiers::default()),
            ]
        );
    }

    #[test]
    fn kitty_keyboard_reports_modifiers_and_event_kind() {
        let mut input = TerminalInput::new();
        // Ctrl+Shift+A press, repeat and release; then Kitty Enter.
        input.feed(b"\x1b[97;6;1u\x1b[97;6;2u\x1b[97;6;3u\x1b[13;1;1u");
        assert_eq!(
            events(&mut input),
            [
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Char('a'),
                    Modifiers::default().with_shift().with_ctrl(),
                    KeyEventKind::Press,
                )),
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Char('a'),
                    Modifiers::default().with_shift().with_ctrl(),
                    KeyEventKind::Repeat,
                )),
                TerminalEvent::Key(KeyEvent::new(
                    KeyCode::Char('a'),
                    Modifiers::default().with_shift().with_ctrl(),
                    KeyEventKind::Release,
                )),
                key(KeyCode::Enter, Modifiers::default()),
            ]
        );
        // Kitty modifier protocol-keys (page up, F1) use the private range.
        input.feed(b"\x1b[57354;1u\x1b[57364;1u");
        assert_eq!(
            events(&mut input),
            [
                key(KeyCode::PageUp, Modifiers::default()),
                key(KeyCode::F(1), Modifiers::default()),
            ]
        );
    }

    #[test]
    fn modify_other_keys_sequence_is_preserved() {
        let mut input = TerminalInput::new();
        // `CSI 27;…~` (modifyOtherKeys) and `SS3`-final forms outside the
        // audited set are consumed as Unknown rather than leaking as text.
        input.feed(b"\x1b[27;5;97~\x1b[1;5P");
        let events = events(&mut input);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], TerminalEvent::Unknown);
        assert_eq!(events[1], TerminalEvent::Unknown);
    }

    #[test]
    fn bracket_paste_is_atomic_and_preserves_bytes() {
        let mut input = TerminalInput::new();
        input.feed(b"ab\x1b[200~line1\r\nline2\x1b[0m\x1b[201~c");
        let events = events(&mut input);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], TerminalEvent::Text("ab".into()));
        let TerminalEvent::Paste { bytes, truncated } = &events[1] else {
            panic!("expected paste, got {:?}", events[1]);
        };
        // Escape sequences inside a paste are data, not protocols.
        assert_eq!(bytes, b"line1\r\nline2\x1b[0m");
        assert!(!truncated);
        assert_eq!(
            events[1].paste_text().unwrap(),
            "line1\nline2\u{1b}[0m",
            "prompt normalisation applies CRLF->LF only"
        );
        assert_eq!(events[2], TerminalEvent::Text("c".into()));
    }

    #[test]
    fn paste_split_across_reads_is_reassembled() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[20");
        assert!(events(&mut input).is_empty());
        input.feed(b"0~hello");
        assert!(events(&mut input).is_empty(), "terminator not yet seen");
        input.feed(b" world\x1b[20");
        assert!(events(&mut input).is_empty());
        input.feed(b"1~");
        assert_eq!(
            events(&mut input),
            [TerminalEvent::Paste {
                bytes: b"hello world".to_vec(),
                truncated: false,
            }]
        );
    }

    #[test]
    fn oversized_paste_is_truncated_and_consumed() {
        let mut input = TerminalInput::new();
        input.feed(PASTE_START);
        let chunk = vec![b'x'; MAX_PASTE_BYTES];
        input.feed(&chunk);
        input.feed(&chunk);
        input.feed(PASTE_END);
        input.feed(b"tail");
        let events = events(&mut input);
        assert_eq!(events.len(), 2);
        let TerminalEvent::Paste { bytes, truncated } = &events[0] else {
            panic!("expected paste");
        };
        assert!(truncated);
        assert!(bytes.len() <= MAX_PASTE_BYTES);
        assert_eq!(events[1], TerminalEvent::Text("tail".into()));
    }

    #[test]
    fn sgr_mouse_preserves_button_motion_and_modifiers() {
        let mut input = TerminalInput::new();
        // Press left at 10,5; drag; release; move without a button.
        input.feed(b"\x1b[<0;10;5M\x1b[<32;11;5M\x1b[<0;11;5m\x1b[<35;12;6M");
        let first = events(&mut input);
        assert_eq!(
            first,
            [
                TerminalEvent::Mouse(MouseEvent {
                    x: 9,
                    y: 4,
                    kind: MouseKind::Press,
                    button: MouseButton::Left,
                    modifiers: Modifiers::default(),
                }),
                TerminalEvent::Mouse(MouseEvent {
                    x: 10,
                    y: 4,
                    kind: MouseKind::Drag,
                    button: MouseButton::Left,
                    modifiers: Modifiers::default(),
                }),
                TerminalEvent::Mouse(MouseEvent {
                    x: 10,
                    y: 4,
                    kind: MouseKind::Release,
                    button: MouseButton::None,
                    modifiers: Modifiers::default(),
                }),
                TerminalEvent::Mouse(MouseEvent {
                    x: 11,
                    y: 5,
                    kind: MouseKind::Move,
                    button: MouseButton::None,
                    modifiers: Modifiers::default(),
                }),
            ]
        );
        // Ctrl+wheel up and shift+right press (SGR bit 2 = shift).
        input.feed(b"\x1b[<80;1;1M\x1b[<6;2;2M");
        let more = events(&mut input);
        assert_eq!(
            more[0],
            TerminalEvent::Mouse(MouseEvent {
                x: 0,
                y: 0,
                kind: MouseKind::WheelUp,
                button: MouseButton::Wheel,
                modifiers: Modifiers::ctrl(),
            })
        );
        let TerminalEvent::Mouse(right) = more[1] else {
            panic!("expected mouse");
        };
        assert_eq!(right.kind, MouseKind::Press);
        assert_eq!(right.button, MouseButton::Right);
        assert!(right.modifiers.shift);
    }

    #[test]
    fn x10_mouse_is_decoded() {
        let mut input = TerminalInput::new();
        // `CSI M` + (32+0) left button, x=5, y=3 (1-based encoded).
        input.feed(b"\x1b[M\x20\x25\x23");
        assert_eq!(
            events(&mut input),
            [TerminalEvent::Mouse(MouseEvent {
                x: 4,
                y: 2,
                kind: MouseKind::Press,
                button: MouseButton::Left,
                modifiers: Modifiers::default(),
            })]
        );
        // Encoded release (32+3) and wheel down (32+64+1).
        input.feed(b"\x1b[M\x23\x21\x21\x1b[M\x61\x21\x21");
        let events = events(&mut input);
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1],
            TerminalEvent::Mouse(MouseEvent {
                x: 0,
                y: 0,
                kind: MouseKind::WheelDown,
                button: MouseButton::Wheel,
                modifiers: Modifiers::default(),
            })
        );
    }

    #[test]
    fn capability_responses_are_not_keys() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[12;40R\x1b[?62;22c\x1b[>41;354;0c\x1b[?1u\x1b[4;800;1200t\x1b[?2026;1$y");
        assert_eq!(
            events(&mut input),
            [
                TerminalEvent::Response(CapabilityResponse::CursorPosition { row: 12, col: 40 }),
                TerminalEvent::Response(CapabilityResponse::DeviceAttributes {
                    secondary: false,
                    values: vec![62, 22],
                }),
                TerminalEvent::Response(CapabilityResponse::DeviceAttributes {
                    secondary: true,
                    values: vec![41, 354, 0],
                }),
                TerminalEvent::Response(CapabilityResponse::KeyboardFlags { flags: 1 }),
                TerminalEvent::Response(CapabilityResponse::PixelResolution {
                    width: 1200,
                    height: 800,
                }),
                TerminalEvent::Response(CapabilityResponse::ModeReport {
                    mode: 2026,
                    status: 1,
                }),
            ]
        );
    }

    #[test]
    fn osc_replies_are_consumed_with_their_code() {
        let mut input = TerminalInput::new();
        // BEL-terminated OSC 11 (background colour) then ST-terminated OSC 10.
        input.feed(b"\x1b]11;rgb:0000/0000/0000\x07x\x1b]10;rgb:ffff/ffff/ffff\x1b\\y");
        let events = events(&mut input);
        assert_eq!(events.len(), 4);
        assert_eq!(
            events[0],
            TerminalEvent::Response(CapabilityResponse::Opaque { code: 11 })
        );
        assert_eq!(events[1], TerminalEvent::Text("x".into()));
        assert_eq!(
            events[2],
            TerminalEvent::Response(CapabilityResponse::Opaque { code: 10 })
        );
        assert_eq!(events[3], TerminalEvent::Text("y".into()));
    }

    #[test]
    fn focus_reports_are_distinct() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[I\x1b[O");
        assert_eq!(
            events(&mut input),
            [TerminalEvent::Focus(true), TerminalEvent::Focus(false)]
        );
    }

    #[test]
    fn incomplete_sequences_wait_without_losing_bytes() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b");
        assert_eq!(input.next_event(), None);
        input.feed(b"[1;");
        assert_eq!(input.next_event(), None);
        input.feed(b"5A");
        assert_eq!(input.next_event(), Some(key(KeyCode::Up, Modifiers::ctrl())));
        assert_eq!(input.pending(), 0);
    }

    #[test]
    fn lone_escape_is_flushed_only_by_explicit_timeout() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b");
        assert!(events(&mut input).is_empty());
        assert_eq!(
            input.flush_escape(),
            Some(key(KeyCode::Escape, Modifiers::default()))
        );
        assert_eq!(input.pending(), 0);
        // A longer incomplete sequence is not turned into Escape.
        input.feed(b"\x1b[1");
        assert_eq!(input.flush_escape(), None);
        assert_eq!(input.pending(), 3);
    }

    #[test]
    fn alt_prefixed_printable_keeps_the_modifier() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1bx\x1b\r");
        assert_eq!(
            events(&mut input),
            [
                key(KeyCode::Char('x'), Modifiers::default().with_alt()),
                key(KeyCode::Enter, Modifiers::default().with_alt()),
            ]
        );
    }

    #[test]
    fn unknown_csi_is_consumed_without_swallowing_text() {
        let mut input = TerminalInput::new();
        input.feed(b"\x1b[99;99zok");
        assert_eq!(
            events(&mut input),
            [TerminalEvent::Unknown, TerminalEvent::Text("ok".into())]
        );
    }

    #[test]
    fn pending_buffer_is_bounded() {
        let mut input = TerminalInput::new();
        // An unterminated OSC grows the buffer; it must stay bounded.
        input.feed(b"\x1b]0;");
        for _ in 0..(MAX_PENDING_BYTES / 1024 + 4) {
            input.feed(&[b'x'; 1024]);
        }
        assert!(input.pending() <= MAX_PENDING_BYTES);
    }
}
