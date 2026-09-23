#![forbid(unsafe_code)]
//! Key/mouse input events + focus navigation. Pure, bounded, no IO/FFI.
//!
//! This module parses terminal keypress escape sequences into structured
//! [`ParsedKey`] values, including CSI sequences, the Kitty keyboard protocol,
//! and raw control characters. It also provides a [`KeyHandler`] that
//! dispatches parsed keys to handlers with global-first semantics.

/// Max focusable ids in one ring.
pub const MAX_FOCUS: usize = 64;

// ---------------------------------------------------------------------------
// Key
// ---------------------------------------------------------------------------

/// Key press: raw code (Unicode scalar) plus modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Key {
    #[must_use]
    pub const fn new(code: u32, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self { code, ctrl, alt, shift }
    }

    /// Plain key, no modifiers held.
    #[must_use]
    pub const fn plain(code: u32) -> Self {
        Self::new(code, false, false, false)
    }

    /// True when no modifier is held.
    #[must_use]
    pub const fn is_plain(self) -> bool {
        !self.ctrl && !self.alt && !self.shift
    }
}

// ---------------------------------------------------------------------------
// Mouse / InputEvent / FocusRing
// ---------------------------------------------------------------------------

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Input event. Cell coords/sizes, origin (0, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(Key),
    Mouse { x: u16, y: u16, button: MouseButton },
    Resize { w: u16, h: u16 },
    Focus(bool),
}

/// Fail-closed focus-ring error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusError {
    Empty,
    Full,
    Invalid,
}

/// Bounded ring of focusable ids. `next`/`prev` wrap around.
#[derive(Debug, Clone)]
pub struct FocusRing {
    ids: [u32; MAX_FOCUS],
    len: usize,
    current: usize,
}

impl FocusRing {
    #[must_use]
    pub const fn new() -> Self {
        Self { ids: [0; MAX_FOCUS], len: 0, current: 0 }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len >= MAX_FOCUS
    }

    /// Push a nonzero id. Fail-closed on 0 or a full ring.
    pub fn push(&mut self, id: u32) -> Result<(), FocusError> {
        if id == 0 {
            return Err(FocusError::Invalid);
        }
        if self.is_full() {
            return Err(FocusError::Full);
        }
        self.ids[self.len] = id;
        self.len += 1;
        Ok(())
    }

    /// Currently focused id, if any.
    #[must_use]
    pub const fn current(&self) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        Some(self.ids[self.current])
    }

    /// Advance with wrap, return newly focused id.
    pub fn next(&mut self) -> Result<u32, FocusError> {
        if self.is_empty() {
            return Err(FocusError::Empty);
        }
        self.current = (self.current + 1) % self.len;
        Ok(self.ids[self.current])
    }

    /// Retreat with wrap, return newly focused id.
    pub fn prev(&mut self) -> Result<u32, FocusError> {
        if self.is_empty() {
            return Err(FocusError::Empty);
        }
        self.current = (self.current + self.len - 1) % self.len;
        Ok(self.ids[self.current])
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.current = 0;
    }
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = assert!(MAX_FOCUS == 64);

// ---------------------------------------------------------------------------
// ParsedKey
// ---------------------------------------------------------------------------

/// Type of key event produced by the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Press,
    Repeat,
    Release,
}

/// Source protocol that produced this key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Raw,
    Kitty,
}

/// Parsed result of a keypress escape sequence.
///
/// Mirrors the ParsedKey interface from
/// `parse.keypress.ts` / `parse.keypress-kitty.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub name: String,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    pub option: bool,
    pub sequence: String,
    pub number: bool,
    pub raw: String,
    pub event_type: KeyEventType,
    pub source: KeySource,
    pub code: Option<String>,
    pub super_: bool,
    pub hyper_: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
    /// Kitty's base-layout codepoint as a Unicode number (e.g. 99 = `c`).
    pub base_code: Option<u32>,
    pub repeated: bool,
}

impl ParsedKey {
    #[must_use]
    pub fn new(name: &str, seq: &str) -> Self {
        Self {
            name: name.to_string(),
            ctrl: false,
            meta: false,
            shift: false,
            option: false,
            sequence: seq.to_string(),
            number: false,
            raw: seq.to_string(),
            event_type: KeyEventType::Press,
            source: KeySource::Raw,
            code: None,
            super_: false,
            hyper_: false,
            caps_lock: false,
            num_lock: false,
            base_code: None,
            repeated: false,
        }
    }
}

impl Default for ParsedKey {
    fn default() -> Self {
        Self::new("", "")
    }
}

// ---------------------------------------------------------------------------
// Kitty modifier decoding
// ---------------------------------------------------------------------------

/// Decode a Kitty modifier mask into individual flags.
///
/// Kitty sends `modifiers - 1` as the mask value (so 2 = no modifier, 3 = shift,
/// etc.). The raw bit layout is:
///   1 = Shift, 2 = Alt, 4 = Ctrl, 8 = Super, 16 = Hyper,
///   32 = Meta, 64 = CapsLock, 128 = NumLock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct KittyMods {
    shift: bool,
    alt: bool,
    ctrl: bool,
    super_: bool,
    hyper_: bool,
    meta: bool,
    caps_lock: bool,
    num_lock: bool,
}

impl KittyMods {
    const fn from_mask(mask: u32) -> Self {
        Self {
            shift: mask & 1 != 0,
            alt: mask & 2 != 0,
            ctrl: mask & 4 != 0,
            super_: mask & 8 != 0,
            hyper_: mask & 16 != 0,
            meta: mask & 32 != 0,
            caps_lock: mask & 64 != 0,
            num_lock: mask & 128 != 0,
        }
    }
}

/// Apply a Kitty modifier mask to a ParsedKey (mutating fields in place).
fn apply_kitty_mods(key: &mut ParsedKey, mask: u32) {
    if mask == 0 {
        return;
    }
    let m = KittyMods::from_mask(mask);
    key.shift = m.shift;
    key.ctrl = m.ctrl;
    key.meta = m.alt || m.meta;
    key.option = m.alt;
    key.super_ = m.super_;
    key.hyper_ = m.hyper_;
    key.caps_lock = m.caps_lock;
    key.num_lock = m.num_lock;
}

// ---------------------------------------------------------------------------
// Ctrl-name resolution
// ---------------------------------------------------------------------------

/// Map a raw control byte to the key names that bindings expect.
///
/// Terminals send Ctrl+A..Ctrl+Z as 0x01..0x1a, and Ctrl+\..Ctrl+_ as
/// 0x1c..0x1f. Returns the base character name (without modifier flags).
fn ctrl_key_name(char_code: u32) -> Option<String> {
    if char_code == 0 {
        return Some("space".to_string());
    }
    if (1..=26).contains(&char_code) {
        // Ctrl+A (1) -> 'a', etc.
        let ch = char::from_u32(b'a' as u32 + char_code - 1)?;
        return Some(ch.to_string());
    }
    if (28..=31).contains(&char_code) {
        // 0x1c -> '@', 0x1d -> ']', 0x1e -> '^', 0x1f -> '_'
        let ch = char::from_u32(char_code + 64)?;
        return Some(ch.to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// Key name tables
// ---------------------------------------------------------------------------

/// Kitty codepoint -> named key.
fn kitty_named_key(codepoint: u32) -> Option<String> {
    let name = match codepoint {
        27 => "escape",
        9 => "tab",
        13 => "return",
        127 => "backspace",
        57344 => "escape",
        57345 => "return",
        57346 => "tab",
        57347 => "backspace",
        57348 => "insert",
        57349 => "delete",
        57350 => "left",
        57351 => "right",
        57352 => "up",
        57353 => "down",
        57354 => "pageup",
        57355 => "pagedown",
        57356 => "home",
        57357 => "end",
        57358 => "capslock",
        57359 => "scrolllock",
        57360 => "numlock",
        57361 => "printscreen",
        57362 => "pause",
        57363 => "menu",
        57364 => "f1",
        57365 => "f2",
        57366 => "f3",
        57367 => "f4",
        57368 => "f5",
        57369 => "f6",
        57370 => "f7",
        57371 => "f8",
        57372 => "f9",
        57373 => "f10",
        57374 => "f11",
        57375 => "f12",
        57376 => "f13",
        57377 => "f14",
        57378 => "f15",
        57379 => "f16",
        57380 => "f17",
        57381 => "f18",
        57382 => "f19",
        57383 => "f20",
        57384 => "f21",
        57385 => "f22",
        57386 => "f23",
        57387 => "f24",
        57388 => "f25",
        57389 => "f26",
        57390 => "f27",
        57391 => "f28",
        57392 => "f29",
        57393 => "f30",
        57394 => "f31",
        57395 => "f32",
        57396 => "f33",
        57397 => "f34",
        57398 => "f35",
        57399 => "kp0",
        57400 => "kp1",
        57401 => "kp2",
        57402 => "kp3",
        57403 => "kp4",
        57404 => "kp5",
        57405 => "kp6",
        57406 => "kp7",
        57407 => "kp8",
        57408 => "kp9",
        57409 => "kpdecimal",
        57410 => "kpdivide",
        57411 => "kpmultiply",
        57412 => "kpminus",
        57413 => "kpplus",
        57414 => "kpenter",
        57415 => "kpequal",
        57416 => "kpseparator",
        57417 => "kpleft",
        57418 => "kpright",
        57419 => "kpup",
        57420 => "kpdown",
        57421 => "kppageup",
        57422 => "kppagedown",
        57423 => "kphome",
        57424 => "kpend",
        57425 => "kpinsert",
        57426 => "kpdelete",
        57427 => "clear",
        _ => return None,
    };
    Some(name.to_string())
}

/// Arrow / function / misc name for a single-letter CSI (or SS3) final byte.
fn csi_key_name(code: &str) -> Option<String> {
    let name = match code {
        "A" => "up",
        "B" => "down",
        "C" => "right",
        "D" => "left",
        "E" => "clear",
        "F" => "end",
        "H" => "home",
        "P" => "f1",
        "Q" => "f2",
        "R" => "f3",
        "S" => "f4",
        "Z" => "tab",
        _ => return None,
    };
    Some(name.to_string())
}

/// Name for a `CSI <number> ~` tilde sequence.
///
/// Mirrors the xterm legacy table in `parse.keypress.ts` (insert/delete/
/// home/end/pageup/pagedown plus F1..F20). 200/201 are bracketed-paste
/// markers and are intentionally absent: the parser returns `None` for them.
fn tilde_key_name(num: u32) -> Option<String> {
    let name = match num {
        1 => "home",
        2 => "insert",
        3 => "delete",
        4 => "end",
        5 => "pageup",
        6 => "pagedown",
        7 => "home",
        8 => "end",
        11 => "f1",
        12 => "f2",
        13 => "f3",
        14 => "f4",
        15 => "f5",
        17 => "f6",
        18 => "f7",
        19 => "f8",
        20 => "f9",
        21 => "f10",
        23 => "f11",
        24 => "f12",
        25 => "f13",
        26 => "f14",
        28 => "f15",
        29 => "f16",
        31 => "f17",
        32 => "f18",
        33 => "f19",
        34 => "f20",
        _ => return None,
    };
    Some(name.to_string())
}

// ---------------------------------------------------------------------------
// Parsers (RED stubs: fail-closed None / NotHandled until GREEN)
// ---------------------------------------------------------------------------

/// Parse a Kitty keyboard-protocol sequence (`CSI ... u`, `CSI ... ~` with
/// Kitty modifiers, or `CSI 1 ; mods LETTER`).
///
/// `data` is the raw input bytes, e.g. `b"\x1b[99;5u"` (Ctrl+C under Kitty).
/// Returns `None` when the bytes are not a Kitty key sequence.
pub fn parse_kitty_keypress(data: &[u8]) -> Option<ParsedKey> {
    if data.len() < 4 || data[0] != 0x1b || data[1] != b'[' {
        return None;
    }
    let tail = data[data.len() - 1];
    if tail != b'u' && tail != b'~' && !(tail.is_ascii_alphabetic()) {
        return None;
    }
    let inner = std::str::from_utf8(&data[2..data.len() - 1]).ok()?;
    let mut parts = inner.split(';');
    let first = parts.next()?;
    let second = parts.next();
    // Kitty CSI-u requires a numeric codepoint (or empty for modifier-only).
    let codepoint: u32 = if first.is_empty() { return None } else { first.parse().ok()? };
    let mods_raw: u32 = second.and_then(|s| s.split(':').next()).and_then(|s| s.parse().ok()).unwrap_or(1);
    if mods_raw == 0 {
        return None;
    }
    let mask = mods_raw - 1;
    let seq = String::from_utf8_lossy(data).into_owned();
    let name = if (32..127).contains(&codepoint) {
        (codepoint as u8 as char).to_string()
    } else {
        kitty_named_key(codepoint)?
    };
    let mut k = ParsedKey::new(&name, &seq);
    k.source = KeySource::Kitty;
    k.base_code = Some(codepoint);
    apply_kitty_mods(&mut k, mask);
    Some(k)
}

/// Parse one legacy/xterm keypress: CSI/SS3 sequences, control characters,
/// ESC-prefixed meta keys, and single printable (UTF-8) characters.
///
/// Returns `None` for empty input and for non-key sequences such as
/// bracketed-paste markers (`ESC[200~` / `ESC[201~`) and focus reports
/// (`ESC[I` / `ESC[O`), mirroring `parse.keypress.ts` returning null.
pub fn parse_keypress(data: &[u8]) -> Option<ParsedKey> {
    if data.is_empty() {
        return None;
    }
    let seq = String::from_utf8_lossy(data).into_owned();
    // Bracketed paste + focus reports are not keys.
    if data == b"\x1b[200~" || data == b"\x1b[201~" {
        return None;
    }
    if data == b"\x1b[I" || data == b"\x1b[O" {
        return None;
    }
    // Kitty sequences route to the kitty parser when they look like CSI-u.
    if data.len() >= 4 && data[0] == 0x1b && data[1] == b'[' && data[data.len() - 1] == b'u' {
        return parse_kitty_keypress(data);
    }
    // ESC-prefixed: CSI / SS3 / meta.
    if data[0] == 0x1b {
        if data.len() == 1 {
            let mut k = ParsedKey::new("escape", &seq);
            return Some(k);
        }
        if data.len() >= 3 && data[1] == b'[' {
            let tail = data[data.len() - 1];
            let inner = std::str::from_utf8(&data[2..data.len() - 1]).ok()?;
            if tail == b'~' {
                if let Ok(num) = inner.split(';').next().unwrap_or("").parse::<u32>() {
                    if let Some(name) = tilde_key_name(num) {
                        return Some(ParsedKey::new(&name, &seq));
                    }
                    return None;
                }
                return None;
            }
            if tail.is_ascii_alphabetic() {
                // CSI 1 ; mods LETTER carries modifiers.
                let mut it = inner.split(';');
                let _first = it.next().unwrap_or("");
                if let Some(mods) = it.next() {
                    if let Ok(m) = mods.parse::<u32>() {
                        let mut k = ParsedKey::new(csi_key_name(core::str::from_utf8(&[tail]).ok()?).as_deref()?, &seq);
                        apply_kitty_mods(&mut k, m - 1);
                        return Some(k);
                    }
                }
                if let Some(name) = csi_key_name(core::str::from_utf8(&[tail]).ok()?) {
                    return Some(ParsedKey::new(&name, &seq));
                }
                return None;
            }
            // CSI with numeric params but non-alpha tail: not a key.
            return None;
        }
        if data.len() == 3 && data[1] == b'O' {
            if let Some(name) = csi_key_name(core::str::from_utf8(&data[2..3]).ok()?) {
                return Some(ParsedKey::new(&name, &seq));
            }
            return None;
        }
        // Meta+<char>: ESC x.
        if let Ok(rest) = std::str::from_utf8(&data[1..]) {
            let mut chars = rest.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                let mut k = ParsedKey::new(&c.to_string(), &seq);
                k.meta = true;
                k.option = true;
                return Some(k);
            }
        }
        return None;
    }
    // Single control bytes.
    if data.len() == 1 {
        let b = data[0];
        if b == b'\r' || b == b'\n' {
            return Some(ParsedKey::new("return", &seq));
        }
        if b == b'\t' {
            let mut k = ParsedKey::new("tab", &seq);
            return Some(k);
        }
        if b == 0x7f {
            return Some(ParsedKey::new("backspace", &seq));
        }
        if b < 0x20 {
            if let Some(name) = ctrl_key_name(b as u32) {
                let mut k = ParsedKey::new(&name, &seq);
                k.ctrl = true;
                return Some(k);
            }
            return None;
        }
        if let Ok(s) = std::str::from_utf8(data) {
            let mut chars = s.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                return Some(ParsedKey::new(&c.to_string(), &seq));
            }
        }
        return None;
    }
    // Multi-byte UTF-8 printable.
    if let Ok(s) = std::str::from_utf8(data) {
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if !c.is_control() {
                return Some(ParsedKey::new(&c.to_string(), &seq));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Outcome of offering a parsed key to one handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDecision {
    /// No handler claimed the key; fall through to default handling.
    NotHandled,
    /// A handler consumed the key.
    Handled,
    /// A handler consumed the key and stops further propagation.
    StopPropagation,
}

/// Dispatches parsed keys to registered handlers, global handlers first.
///
/// Mirrors `KeyHandler.ts` (`processParsedKey`): global sinks see every key
/// before the focused/renderable handler, and the first non-`NotHandled`
/// decision wins.
pub struct KeyHandler {
    global: Vec<Box<dyn Fn(&ParsedKey) -> KeyDecision>>,
    local: Vec<Box<dyn Fn(&ParsedKey) -> KeyDecision>>,
}

impl KeyHandler {
    #[must_use]
    pub fn new() -> Self {
        Self { global: Vec::new(), local: Vec::new() }
    }

    /// Register a global handler (runs before local handlers).
    pub fn on_global<F>(&mut self, f: F)
    where
        F: Fn(&ParsedKey) -> KeyDecision + 'static,
    {
        self.global.push(Box::new(f));
    }

    /// Register a focused/renderable-local handler.
    pub fn on_key<F>(&mut self, f: F)
    where
        F: Fn(&ParsedKey) -> KeyDecision + 'static,
    {
        self.local.push(Box::new(f));
    }

    /// Offer `key` to globals first, then locals. First non-`NotHandled`
    /// decision wins; `NotHandled` when nobody claims it.
    pub fn process_parsed_key(&self, key: &ParsedKey) -> KeyDecision {
        for g in &self.global {
            let d = g(key);
            if d != KeyDecision::NotHandled {
                return d;
            }
        }
        for l in &self.local {
            let d = l(key);
            if d != KeyDecision::NotHandled {
                return d;
            }
        }
        KeyDecision::NotHandled
    }
}

impl Default for KeyHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests (frozen RED: must fail pre-implementation, pass post, no edits)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t01_printable_a() {
        let k = parse_keypress(b"a").expect("printable 'a' must parse");
        assert_eq!(k.name, "a");
        assert!(!k.ctrl && !k.meta);
    }

    #[test]
    fn t02_ctrl_c() {
        let k = parse_keypress(b"\x03").expect("0x03 must parse as ctrl+c");
        assert_eq!(k.name, "c");
        assert!(k.ctrl);
    }

    #[test]
    fn t03_arrow_up() {
        let k = parse_keypress(b"\x1b[A").expect("CSI A must parse as up");
        assert_eq!(k.name, "up");
    }

    #[test]
    fn t04_bracketed_paste_is_none() {
        assert!(parse_keypress(b"\x1b[200~").is_none());
        assert!(parse_keypress(b"\x1b[201~").is_none());
    }

    #[test]
    fn t05_kitty_printable() {
        let k = parse_kitty_keypress(b"\x1b[97u").expect("CSI 97 u must parse as 'a'");
        assert_eq!(k.name, "a");
        assert_eq!(k.source, KeySource::Kitty);
    }

    #[test]
    fn t06_kitty_ctrl_c() {
        let k = parse_kitty_keypress(b"\x1b[99;5u").expect("CSI 99 ; 5 u must parse");
        assert_eq!(k.name, "c");
        assert!(k.ctrl);
    }

    #[test]
    fn t07_handler_global_first() {
        let mut h = KeyHandler::new();
        h.on_global(|_| KeyDecision::Handled);
        h.on_key(|_| KeyDecision::StopPropagation);
        let key = ParsedKey::new("a", "a");
        assert_eq!(h.process_parsed_key(&key), KeyDecision::Handled);
    }

    #[test]
    fn t08_enter_key() {
        let k = parse_keypress(b"\r").expect("CR must parse as return");
        assert_eq!(k.name, "return");
    }
}
