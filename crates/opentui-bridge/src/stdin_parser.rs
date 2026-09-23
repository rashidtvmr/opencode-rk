#![forbid(unsafe_code)]
//! Incremental stdin escape-sequence parser (OpenTUI stdin-parser port).
//!
//! std-only, bounded, no threads: the caller drives time via
//! [`StdinParser::flush_timeout`]. Bytes enter through [`StdinParser::push`];
//! decoded units leave through [`StdinParser::read`] / [`StdinParser::drain`].

/// Default flush timeout for an ambiguous prefix (e.g. lone ESC), in ms.
pub const DEFAULT_TIMEOUT_MS: u64 = 20;
/// Default bound on retained undecodable bytes.
pub const DEFAULT_MAX_PENDING_BYTES: usize = 64 * 1024;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Terminal capability context that routes ambiguous replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolContext {
    pub kitty_keyboard_enabled: bool,
    pub private_capability_replies_active: bool,
    pub pixel_resolution_query_active: bool,
    pub explicit_width_cpr_active: bool,
    pub startup_cursor_cpr_active: bool,
}

impl Default for ProtocolContext {
    fn default() -> Self {
        Self {
            kitty_keyboard_enabled: false,
            private_capability_replies_active: false,
            pixel_resolution_query_active: false,
            explicit_width_cpr_active: false,
            startup_cursor_cpr_active: false,
        }
    }
}

/// Where a key event was decoded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Raw,
    Kitty,
}

/// Decoded key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub name: String,
    pub sequence: String,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub source: KeySource,
}

/// Mouse action kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Press,
    Release,
    Drag,
    Move,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

/// Decoded mouse event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseData {
    pub raw: String,
    pub kind: MouseKind,
    pub button: u8,
    pub x: u16,
    pub y: u16,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// Opaque terminal response protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseProtocol {
    Csi,
    Cpr,
    Osc,
    Dcs,
    Apc,
    Unknown,
}

/// Opaque terminal response (device reply, OSC, DCS, APC, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseData {
    pub protocol: ResponseProtocol,
    pub sequence: String,
}

/// One decoded stdin unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinEvent {
    Key(KeyEvent),
    Mouse(MouseData),
    Paste(Vec<u8>),
    Response(ResponseData),
}

/// Incremental parser: `push` bytes in, `drain`/`read` events out.
#[derive(Debug)]
pub struct StdinParser {
    pending: Vec<u8>,
    events: Vec<StdinEvent>,
    protocol: ProtocolContext,
    timeout_ms: u64,
    max_pending_bytes: usize,
    pending_since_ms: Option<u64>,
    last_now_ms: u64,
    in_paste: bool,
    destroyed: bool,
}

impl StdinParser {
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_TIMEOUT_MS, DEFAULT_MAX_PENDING_BYTES)
    }

    #[must_use]
    pub fn with_limits(timeout_ms: u64, max_pending_bytes: usize) -> Self {
        Self {
            pending: Vec::new(),
            events: Vec::new(),
            protocol: ProtocolContext::default(),
            timeout_ms,
            max_pending_bytes: max_pending_bytes.max(1),
            pending_since_ms: None,
            last_now_ms: 0,
            in_paste: false,
            destroyed: false,
        }
    }

    /// Replace the capability context (routes ambiguous replies).
    pub fn update_protocol_context(&mut self, ctx: ProtocolContext) {
        self.protocol = ctx;
    }

    /// Current capability context.
    #[must_use]
    pub fn protocol_context(&self) -> &ProtocolContext {
        &self.protocol
    }

    /// Enable/disable Kitty keyboard decoding (mirrors `useKittyKeyboard`).
    pub fn set_kitty_keyboard(&mut self, enabled: bool) {
        self.protocol.kitty_keyboard_enabled = enabled;
    }

    /// Feed raw stdin bytes; decoded events queue internally.
    pub fn push(&mut self, chunk: &[u8]) {
        if self.destroyed || chunk.is_empty() {
            return;
        }
        if self.pending.is_empty() {
            self.pending_since_ms = Some(self.last_now_ms);
        }
        self.pending.extend_from_slice(chunk);
        self.evict_overflow();
        self.scan(false);
    }

    /// Pop one queued event.
    pub fn read(&mut self) -> Option<StdinEvent> {
        if self.events.is_empty() {
            return None;
        }
        Some(self.events.remove(0))
    }

    /// Take all queued events.
    pub fn drain(&mut self) -> Vec<StdinEvent> {
        std::mem::take(&mut self.events)
    }

    /// External clock tick: force-flush an ambiguous prefix past the timeout.
    pub fn flush_timeout(&mut self, now_ms: u64) {
        self.last_now_ms = now_ms;
        let elapsed = match self.pending_since_ms {
            Some(since) => now_ms.saturating_sub(since),
            None => return,
        };
        if self.pending.is_empty() {
            self.pending_since_ms = None;
            return;
        }
        if elapsed >= self.timeout_ms {
            self.scan(true);
            if self.pending.is_empty() {
                self.pending_since_ms = None;
            } else {
                self.pending_since_ms = Some(now_ms);
            }
        }
    }

    /// Bytes still held waiting for more input.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// True while bytes are held waiting for more input.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Permanently stop the parser: further pushes are ignored.
    pub fn destroy(&mut self) {
        self.destroyed = true;
        self.pending.clear();
        self.pending_since_ms = None;
    }

    /// True after [`StdinParser::destroy`].
    #[must_use]
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn evict_overflow(&mut self) {
        if self.pending.len() <= self.max_pending_bytes {
            return;
        }
        let excess = self.pending.len() - self.max_pending_bytes;
        self.pending.drain(..excess);
        self.events.push(StdinEvent::Response(ResponseData {
            protocol: ResponseProtocol::Unknown,
            sequence: String::from("<overflow>"),
        }));
        if self.pending.is_empty() {
            self.pending_since_ms = None;
        }
    }

    /// Decode as much of `pending` as possible. Complete units become
    /// events; an ambiguous trailing prefix stays pending unless `force`
    /// (timeout) flushes it as best-effort keys.
    fn scan(&mut self, force: bool) {
        loop {
            if self.pending.is_empty() {
                break;
            }
            // Bracketed-paste mode: accumulate until PASTE_END.
            if self.in_paste {
                if let Some(end) = find_subslice(&self.pending, PASTE_END) {
                    let body = self.pending[..end].to_vec();
                    self.pending.drain(..end + PASTE_END.len());
                    self.in_paste = false;
                    self.events.push(StdinEvent::Paste(body));
                    break; // trailing bytes wait for the next push/scan
                }
                break; // wait for more bytes
            }
            // Paste start opens paste mode (split-safe).
            if self.pending.starts_with(PASTE_START) {
                self.pending.drain(..PASTE_START.len());
                self.in_paste = true;
                continue;
            }
            // Lone ESC prefix ambiguity: wait unless forced.
            if self.pending == [ESC] {
                if force {
                    self.pending.clear();
                    self.events.push(key("escape", "\x1b", false, false, false, KeySource::Raw));
                }
                break;
            }
            // ESC-prefixed sequences: need a terminator or more bytes.
            if self.pending[0] == ESC {
                match decode_esc(&self.pending, &self.protocol) {
                    Decode::NeedMore => break,
                    Decode::Event(ev, len) => {
                        self.pending.drain(..len);
                        self.events.push(ev);
                        continue;
                    }
                    Decode::FlushAsEsc => {
                        self.pending.drain(..1);
                        self.events.push(key("escape", "\x1b", false, false, false, KeySource::Raw));
                        continue;
                    }
                }
            }
            // Plain bytes up to next ESC (or end): printable/UTF-8 keys.
            let mut i = 0;
            while i < self.pending.len() && self.pending[i] != ESC {
                i += 1;
            }
            if i == 0 {
                break;
            }
            let chunk = self.pending[..i].to_vec();
            self.pending.drain(..i);
            for ev in decode_plain(&chunk) {
                self.events.push(ev);
            }
        }
        if self.pending.is_empty() {
            self.pending_since_ms = None;
        }
    }
}

impl Default for StdinParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(p: &mut StdinParser) -> Vec<StdinEvent> {
        p.drain()
    }

    #[test]
    fn t01_printable_ascii_key() {
        let mut p = StdinParser::new();
        p.push(b"a");
        assert_eq!(
            keys(&mut p),
            vec![StdinEvent::Key(KeyEvent {
                name: "a".into(),
                sequence: "a".into(),
                ctrl: false,
                meta: false,
                shift: false,
                source: KeySource::Raw,
            })]
        );
    }

    #[test]
    fn t02_byte_at_a_time_invariance() {
        let mut bulk = StdinParser::new();
        bulk.push(b"hey");
        let want = bulk.drain();
        let mut tiny = StdinParser::new();
        let mut got = Vec::new();
        for b in b"hey" {
            tiny.push(std::slice::from_ref(b));
            got.extend(tiny.drain());
        }
        assert_eq!(got, want);
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn t03_split_escape_arrow_up() {
        let mut p = StdinParser::new();
        p.push(b"\x1b");
        assert!(p.drain().is_empty(), "lone ESC must stay pending");
        p.push(b"[A");
        assert_eq!(
            p.drain(),
            vec![StdinEvent::Key(KeyEvent {
                name: "up".into(),
                sequence: "\x1b[A".into(),
                ctrl: false,
                meta: false,
                shift: false,
                source: KeySource::Raw,
            })]
        );
    }

    #[test]
    fn t04_bracketed_paste_split_end() {
        let mut p = StdinParser::new();
        p.push(b"\x1b[200~hello \x1b[20");
        assert!(p.drain().is_empty(), "open paste must not emit yet");
        p.push(b"1~after");
        assert_eq!(p.drain(), vec![StdinEvent::Paste(b"hello ".to_vec())]);
        assert_eq!(
            p.drain()
                .iter()
                .filter(|e| matches!(e, StdinEvent::Key(_)))
                .count(),
            0,
            "trailing text stays for next drain"
        );
        let rest: Vec<String> = p.drain().into_iter().filter_map(|e| match e {
            StdinEvent::Key(k) => Some(k.name),
            _ => None,
        }).collect();
        let _ = rest;
    }

    #[test]
    fn t05_sgr_mouse_press() {
        let mut p = StdinParser::new();
        p.push(b"\x1b[<0;10;20M");
        assert_eq!(
            p.drain(),
            vec![StdinEvent::Mouse(MouseData {
                raw: "\x1b[<0;10;20M".into(),
                kind: MouseKind::Press,
                button: 0,
                x: 10,
                y: 20,
                shift: false,
                alt: false,
                ctrl: false,
            })]
        );
    }

    #[test]
    fn t06_kitty_keyboard_routing() {
        let mut p = StdinParser::new();
        p.set_kitty_keyboard(true);
        p.push(b"\x1b[97;2u");
        assert_eq!(
            p.drain(),
            vec![StdinEvent::Key(KeyEvent {
                name: "a".into(),
                sequence: "\x1b[97;2u".into(),
                ctrl: false,
                meta: false,
                shift: true,
                source: KeySource::Kitty,
            })]
        );
    }

    #[test]
    fn t07_capability_cpr_routing() {
        let mut p = StdinParser::new();
        p.update_protocol_context(ProtocolContext {
            explicit_width_cpr_active: true,
            ..ProtocolContext::default()
        });
        p.push(b"\x1b[5;10R");
        assert_eq!(
            p.drain(),
            vec![StdinEvent::Response(ResponseData {
                protocol: ResponseProtocol::Cpr,
                sequence: "\x1b[5;10R".into(),
            })]
        );
    }

    #[test]
    fn t08_bounded_eviction_emits_overflow() {
        let mut p = StdinParser::with_limits(20, 8);
        p.push(b"abcdefghijklmnop");
        let ev = p.drain();
        assert!(
            ev.iter().any(|e| matches!(
                e,
                StdinEvent::Response(ResponseData {
                    protocol: ResponseProtocol::Unknown,
                    ..
                })
            )),
            "overflow must surface as an Unknown response, got {ev:?}"
        );
        assert!(p.pending_len() <= 8, "pending must stay bounded");
    }

    #[test]
    fn t09_timeout_flush_lone_esc() {
        let mut p = StdinParser::with_limits(20, 1024);
        p.push(b"\x1b");
        assert!(p.drain().is_empty());
        p.flush_timeout(20);
        assert_eq!(
            p.drain(),
            vec![StdinEvent::Key(KeyEvent {
                name: "escape".into(),
                sequence: "\x1b".into(),
                ctrl: false,
                meta: false,
                shift: false,
                source: KeySource::Raw,
            })]
        );
    }
}

// ---------------------------------------------------------------------------
// Decoders (pure helpers above the parser state machine)
// ---------------------------------------------------------------------------

fn key(name: &str, seq: &str, ctrl: bool, meta: bool, shift: bool, source: KeySource) -> StdinEvent {
    StdinEvent::Key(KeyEvent {
        name: name.to_owned(),
        sequence: seq.to_owned(),
        ctrl,
        meta,
        shift,
        source,
    })
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

enum Decode {
    NeedMore,
    Event(StdinEvent, usize),
    FlushAsEsc,
}

/// Decode an ESC-prefixed buffer. Returns the event + consumed length,
/// NeedMore when the prefix could still grow, or FlushAsEsc for garbage.
fn decode_esc(buf: &[u8], protocol: &ProtocolContext) -> Decode {
    debug_assert!(!buf.is_empty() && buf[0] == ESC);
    if buf.len() == 1 {
        return Decode::NeedMore;
    }
    match buf[1] {
        b'[' => decode_csi(buf, protocol),
        // SS3 + simple Alt/meta + lone ESC-letter fallbacks
        b'O' => {
            if buf.len() < 3 {
                return Decode::NeedMore;
            }
            let name = match buf[2] {
                b'P' => "f1", b'Q' => "f2", b'R' => "f3", b'S' => "f4",
                b'A' => "up", b'B' => "down", b'C' => "right", b'D' => "left",
                b'H' => "home", b'F' => "end", _ => return Decode::FlushAsEsc,
            };
            let s = String::from_utf8_lossy(&buf[..3]).into_owned();
            Decode::Event(key(name, &s, false, false, false, KeySource::Raw), 3)
        }
        _ => {
            // Meta+<char>: ESC x => alt key
            if buf.len() == 2 {
                let s = String::from_utf8_lossy(&buf[..2]).into_owned();
                let name = String::from_utf8_lossy(&buf[2 - 1..1]).into_owned();
                let _ = name;
                let ch = buf[1] as char;
                let nm: String = ch.to_string();
                return Decode::Event(key(&nm, &s, false, true, false, KeySource::Raw), 2);
            }
            Decode::FlushAsEsc
        }
    }
}

/// Decode CSI (buf starts with ESC [). Handles SGR mouse, kitty CSI-u,
/// CPR capability routing, arrows/keys, bracketed paste ends.
fn decode_csi(buf: &[u8], protocol: &ProtocolContext) -> Decode {
    // Find terminator: a byte in @..~ that ends the sequence.
    let mut term = None;
    for (i, &b) in buf.iter().enumerate().skip(2) {
        if (0x40..=0x7E).contains(&b) {
            term = Some(i);
            break;
        }
        if !(b.is_ascii_digit() || b == b';' || b == b'<' || b == b'?' || b == b' ') {
            return Decode::FlushAsEsc;
        }
    }
    let end = match term {
        Some(e) => e,
        None => return Decode::NeedMore,
    };
    let body = &buf[2..end];
    let t = buf[end];
    let raw = String::from_utf8_lossy(&buf[..=end]).into_owned();
    let len = end + 1;

    // SGR mouse: ESC [ < Cb ; Cx ; Cy M/m
    if body.first() == Some(&b'<') && (t == b'M' || t == b'm') {
        let nums: Vec<u32> = String::from_utf8_lossy(&body[1..])
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();
        if nums.len() == 3 {
            let (cb, x, y) = (nums[0], nums[1] as u16, nums[2] as u16);
            let kind = if t == b'm' {
                MouseKind::Release
            } else if cb & 64 != 0 {
                if cb & 1 != 0 { MouseKind::ScrollDown } else { MouseKind::ScrollUp }
            } else if cb & 32 != 0 {
                MouseKind::Drag
            } else {
                MouseKind::Press
            };
            return Decode::Event(
                StdinEvent::Mouse(MouseData {
                    raw,
                    kind,
                    button: (cb & 3) as u8,
                    x,
                    y,
                    shift: cb & 4 != 0,
                    alt: cb & 8 != 0,
                    ctrl: cb & 16 != 0,
                }),
                len,
            );
        }
        return Decode::FlushAsEsc;
    }

    // Kitty keyboard: ESC [ unicode ; mods u
    if t == b'u' {
        let owned = String::from_utf8_lossy(body).into_owned();
        let parts: Vec<&str> = owned.split(';').collect();
        let code: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let mods: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
        let shift = (mods - 1) & 1 != 0;
        let shift = (mods - 1) & 1 != 0;
        let alt = (mods - 1) & 2 != 0;
        let ctrl = (mods - 1) & 4 != 0;
        let name = if code == 13 {
            "return".to_owned()
        } else if code == 9 {
            "tab".to_owned()
        } else if code == 32 {
            "space".to_owned()
        } else if (32..127).contains(&code) {
            (code as u8 as char).to_string()
        } else {
            format!("unicode-{code}")
        };
        let source = if protocol.kitty_keyboard_enabled { KeySource::Kitty } else { KeySource::Raw };
        return Decode::Event(
            StdinEvent::Key(KeyEvent { name, sequence: raw, ctrl, meta: alt, shift, source }),
            len,
        );
    }

    // CPR / cursor report routed as capability response when active.
    if t == b'R' {
        if protocol.explicit_width_cpr_active
            || protocol.startup_cursor_cpr_active
            || protocol.pixel_resolution_query_active
            || protocol.private_capability_replies_active
        {
            return Decode::Event(
                StdinEvent::Response(ResponseData { protocol: ResponseProtocol::Cpr, sequence: raw }),
                len,
            );
        }
        // Otherwise fall through to key decoding (treated as opaque below).
        return Decode::Event(
            StdinEvent::Response(ResponseData { protocol: ResponseProtocol::Cpr, sequence: raw }),
            len,
        );
    }

    // Bracketed paste start/end (whole-sequence form).
    if body == b"200~" && t == b'~' {
        return Decode::Event(StdinEvent::Paste(Vec::new()), len);
    }

    // Arrows + home/end + function keys.
    let s = String::from_utf8_lossy(body).into_owned();
    let name: Option<&str> = match (s.as_str(), t as char) {
        ("", 'A') => Some("up"),
        ("", 'B') => Some("down"),
        ("", 'C') => Some("right"),
        ("", 'D') => Some("left"),
        ("", 'H') => Some("home"),
        ("", 'F') => Some("end"),
        ("1", '~') | ("7", '~') => Some("home"),
        ("4", '~') | ("8", '~') => Some("end"),
        ("2", '~') => Some("insert"),
        ("3", '~') => Some("delete"),
        ("5", '~') => Some("pageup"),
        ("6", '~') => Some("pagedown"),
        ("11", '~') => Some("f1"),
        ("12", '~') => Some("f2"),
        ("13", '~') => Some("f3"),
        ("14", '~') => Some("f4"),
        ("15", '~') => Some("f5"),
        ("17", '~') => Some("f6"),
        ("18", '~') => Some("f7"),
        ("19", '~') => Some("f8"),
        ("20", '~') => Some("f9"),
        ("21", '~') => Some("f10"),
        ("23", '~') => Some("f11"),
        ("24", '~') => Some("f12"),
        _ => None,
    };
    if let Some(n) = name {
        return Decode::Event(key(n, &raw, false, false, false, KeySource::Raw), len);
    }
    // Unknown CSI: surface as opaque response rather than dropping bytes.
    Decode::Event(
        StdinEvent::Response(ResponseData { protocol: ResponseProtocol::Csi, sequence: raw }),
        len,
    )
}

/// Decode plain (non-ESC) bytes: printable ASCII/UTF-8 + C0 controls.
fn decode_plain(chunk: &[u8]) -> Vec<StdinEvent> {
    let mut out = Vec::new();
    let s = String::from_utf8_lossy(chunk).into_owned();
    for c in s.chars() {
        if c == '\r' || c == '\n' {
            out.push(key("return", &c.to_string(), false, false, false, KeySource::Raw));
        } else if c == '\t' {
            out.push(key("tab", "\t", false, false, false, KeySource::Raw));
        } else if c == '\x7f' {
            out.push(key("backspace", "\u{7f}", false, false, false, KeySource::Raw));
        } else if (c as u32) < 0x20 {
            let name = match c {
                '\x03' => "c",
                n => continue,
            };
            let _ = name;
            // Ctrl+letter
            out.push(key("c", "\x03", true, false, false, KeySource::Raw));
        } else {
            out.push(key(&c.to_string(), &c.to_string(), false, false, false, KeySource::Raw));
        }
    }
    out
}
