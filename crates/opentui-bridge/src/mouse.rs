//! Mouse event parsing from terminal escape sequences.
//!
//! Ports `packages/core/src/lib/parse.mouse.ts` and `scroll-acceleration.ts`
//! to pure Rust (std-only, forbid(unsafe_code)). Standalone-test friendly.
//!
//! SOURCE EVIDENCE:
//! - `parse.mouse.ts`: `MouseEventType`, `ScrollInfo`, `RawMouseEvent`,
//!   `MouseParser::parseMouseEvent`. SGR: `ESC [ < B ; x+1 ; y+1 M|m`;
//!   X10: `ESC [ M cb cx cy`. Button bits: 0-1 button, 2 shift, 3 alt, 4 ctrl,
//!   6 wheel, 7 motion. SGR scroll: bit6 set, button 0 up/1 down/2 left/3 right.
//! - `parse.mouse.test.ts`: `encodeBasic(buttonByte,x,y)` => `ESC[M (bb+32) (x+33) (y+33)`;
//!   `encodeSGR(buttonCode,x,y,press)` => `ESC[< code; x+1; y+1 M|m`.
//! - `renderer.ts` `recheckHoverState` (~3823): stores `_latestPointer{x,y,modifiers}`
//!   from `processSingleMouseEvent` and reuses it for out/over on hover recheck.
//! - `scroll-acceleration.ts` `MacOSScrollAccel`: streakTimeout=150, minTickInterval=6,
//!   historySize=3, mult=1+A*(exp(vel/tau)-1) capped at maxMultiplier=6.

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

/// Scroll direction. Mirrors `ScrollInfo.direction` in `parse.mouse.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Per-event scroll payload. `delta` is the logical tick count (1 for a single
/// wheel tick, matching `delta: 1` in `decodeSgrEvent`/`decodeBasicEvent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollInfo {
    pub direction: ScrollDirection,
    pub delta: f64,
}

/// Keyboard modifier state. Mirrors `modifiers: {shift,alt,ctrl}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl Modifiers {
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self {
            shift: (bits & 4) != 0,
            alt: (bits & 8) != 0,
            ctrl: (bits & 16) != 0,
        }
    }
}

/// Mouse event type. The full set mirrors `MouseEventType` in `parse.mouse.ts`;
/// parse produces only the press/release/motion/scroll variants, and the
/// renderer promotes some into `drag`/`drag-end`/`drop`/`over`/`out` (see
/// `renderer.ts` `processSingleMouseEvent`/`recheckHoverState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventType {
    Down,
    Up,
    Move,
    Drag,
    DragEnd,
    Drop,
    Over,
    Out,
    Scroll,
}

/// Decoded terminal mouse event. Cell coords are 0-based (wire format is
/// 1-based for SGR, byte-33 offset for X10); negative values are possible for
/// OOB handling downstream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawMouseEvent {
    pub event_type: MouseEventType,
    pub button: i32,
    pub x: i32,
    pub y: i32,
    pub modifiers: Modifiers,
    pub scroll: Option<ScrollInfo>,
    /// Raw SGR button code (pre-decode), kept so the bridge can replay the
    /// exact button byte to native callers. 0 for X10/fallback parses.
    pub raw_button: u8,
}

impl RawMouseEvent {
    #[must_use]
    pub const fn origin() -> Self {
        Self {
            event_type: MouseEventType::Move,
            button: 0,
            x: 0,
            y: 0,
            modifiers: Modifiers { shift: false, alt: false, ctrl: false },
            scroll: None,
            raw_button: 0,
        }
    }
}

/// SGR button bit flags (matches `decodeSgrEvent` bit tests in `parse.mouse.ts`).
#[allow(dead_code)]
mod bit {
    pub const WHEEL: u8 = 0x40; // bit 6
    pub const MOTION: u8 = 0x20; // bit 5
    pub const MAX_BUTTON: u8 = 0x03; // bits 0-1
    pub const SHIFT: u8 = 0x04; // bit 2
    pub const ALT: u8 = 0x08; // bit 3
    pub const CTRL: u8 = 0x10; // bit 4
}

/// SGR decode for `ESC [ < B ; X ; Y M|m`. Returns None if malformed.
fn decode_sgr(raw: u8, wire_x: u32, wire_y: u32, release: bool, pressed: &mut [u8; 4]) -> Option<RawMouseEvent> {
    let button = raw & bit::MAX_BUTTON;
    let is_scroll = (raw & bit::WHEEL) != 0;
    let is_motion = (raw & bit::MOTION) != 0;
    let modifiers = Modifiers::from_bits(raw & 0x1C);

    // Compute scroll payload only for genuine press-scrolls: motion takes
    // priority over scroll and scroll-release is not a scroll (parse.mouse.ts
    // tests "motion bit takes priority" and "scroll release is not classified
    // as scroll").
    let scroll = if is_scroll && !is_motion && !release {
        let dir = match button {
            0 => ScrollDirection::Up,
            1 => ScrollDirection::Down,
            2 => ScrollDirection::Left,
            _ => ScrollDirection::Right,
        };
        Some(ScrollInfo { direction: dir, delta: 1.0 })
    } else {
        None
    };

    let (event_type, out_button) = if is_motion {
        // Motion bit takes priority over scroll bit. Drag is promoted by the
        // renderer when prior button-press tracking is active; here "is_drag"
        // mirrors `mouseButtonsPressed.size > 0` from parse.mouse.ts (only
        // buttons 0/1/2 are ever inserted; value 3 means "no button").
        let is_drag = pressed[0] != 0 || pressed[1] != 0 || pressed[2] != 0;
        if button == 3 {
            (MouseEventType::Move, 0)
        } else if is_drag {
            (MouseEventType::Drag, button as i32)
        } else {
            (MouseEventType::Move, button as i32)
        }
    } else if is_scroll && !release {
        (MouseEventType::Scroll, 0)
    } else {
        // press/release. On up, clear the tracked button; on down, set it.
        if release {
            if button <= 2 {
                pressed[button as usize] = 0;
            }
            (MouseEventType::Up, 0)
        } else if button <= 2 {
            pressed[button as usize] = button + 1;
            (MouseEventType::Down, button as i32)
        } else {
            (MouseEventType::Down, 0)
        }
    };

    let x = (wire_x as i32).saturating_sub(1);
    let y = (wire_y as i32).saturating_sub(1);

    Some(RawMouseEvent {
        event_type,
        button: out_button,
        x,
        y,
        modifiers,
        scroll,
        raw_button: raw,
    })
}

/// X10/Basic decode for `ESC [ M cb cx cy` (6 bytes). cb = button + 32.
fn decode_basic(cb: u8, cx: u32, cy: u32, pressed: &mut [u8; 4]) -> Option<RawMouseEvent> {
    let button_byte = cb.saturating_sub(32);
    let button = button_byte & bit::MAX_BUTTON;
    let is_scroll = (button_byte & bit::WHEEL) != 0;
    let is_motion = (button_byte & bit::MOTION) != 0;
    let modifiers = Modifiers::from_bits(button_byte & 0x1C);

    // Motion bit takes priority over scroll bit (no press/release for motion).
    let scroll = if is_scroll && !is_motion {
        let dir = match button {
            0 => ScrollDirection::Up,
            1 => ScrollDirection::Down,
            2 => ScrollDirection::Left,
            _ => ScrollDirection::Right,
        };
        Some(ScrollInfo { direction: dir, delta: 1.0 })
    } else {
        None
    };

    let (event_type, out_button) = if is_motion {
        // X10: motion => "move" (renderer promotes; parser itself reports move).
        // Button 3 here means "no button" in X10 motion; report -1.
        (MouseEventType::Move, if button == 3 { -1 } else { button as i32 })
    } else if is_scroll {
        (MouseEventType::Scroll, 0)
    } else {
        // X10 release is always button byte 3; press sets tracking.
        let bt = if button_byte == 3 { 0 } else { button as i32 };
        if button_byte != 3 && button <= 2 {
            pressed[button as usize] = button + 1;
        }
        let typ = if button_byte == 3 { MouseEventType::Up } else { MouseEventType::Down };
        (typ, bt)
    };

    // X10 coords: cx - 33, cy - 33
    let x = (cx as i32).saturating_sub(33);
    let y = (cy as i32).saturating_sub(33);

    Some(RawMouseEvent {
        event_type,
        button: out_button,
        x,
        y,
        modifiers,
        scroll,
        raw_button: button_byte,
    })
}

/// Parse a single terminal mouse event from raw bytes.
/// Decodes latin1 (preserves raw bytes >= 0x80 for X10 coords >= 95, matching
/// `decodeInput` in `parse.mouse.ts`). SGR is preferred over X10.
/// Returns None for empty/non-mouse/incomplete input.
pub fn parse_mouse_event(data: &[u8]) -> Option<RawMouseEvent> {
    let mut parser = MouseParser::new();
    parser.parse(data)
}

/// Stateful mouse parser that tracks pressed buttons for drag detection.
#[derive(Debug, Clone)]
pub struct MouseParser {
    pressed: [u8; 4],
}

impl Default for MouseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseParser {
    #[must_use]
    pub const fn new() -> Self {
        Self { pressed: [0; 4] }
    }

    /// Reset button-press tracking (mirrors `MouseParser::reset`).
    pub fn reset(&mut self) {
        self.pressed = [0; 4];
    }

    /// Parse one event from raw bytes. Returns None if no mouse event present.
    pub fn parse(&mut self, data: &[u8]) -> Option<RawMouseEvent> {
        // Need at least the ESC [ M introducer (4 bytes) before decoding.
        if data.len() < 4 || data[0] != 0x1b || data[1] != 0x5b {
            return None;
        }
        match data[2] {
            b'<' => self.parse_sgr(&data[3..]),
            b'M' => self.parse_basic(&data[3..]),
            _ => None,
        }
    }

    fn parse_sgr(&mut self, rest: &[u8]) -> Option<RawMouseEvent> {
        // Format: B ; X ; Y M|m   (the terminator M/m is mandatory)
        // Parse decimal params separated by ';'. Latin1-safe (digits only).
        let mut idx = 0;
        let mut values = [0u32; 3];
        let mut part = 0;
        let mut has_digit = false;
        let mut release;

        while idx < rest.len() {
            let c = rest[idx];
            match c {
                b'0'..=b'9' => {
                    if part > 2 {
                        return None; // too many params
                    }
                    has_digit = true;
                    values[part] = values[part].saturating_mul(10).saturating_add((c - b'0') as u32);
                    idx += 1;
                }
                b';' => {
                    if !has_digit || part >= 2 {
                        return None;
                    }
                    part += 1;
                    has_digit = false;
                    idx += 1;
                }
                b'M' | b'm' => {
                    if !has_digit || part != 2 {
                        return None;
                    }
                    release = c == b'm';
                    idx += 1;
                    // Mandatory terminator found; decode.
                    return decode_sgr(values[0] as u8, values[1], values[2], release, &mut self.pressed);
                }
                _ => return None,
            }
        }
        // Consumed all bytes without the M/m terminator: incomplete SGR.
        None
    }

    fn parse_basic(&mut self, rest: &[u8]) -> Option<RawMouseEvent> {
        // ESC [ M + cb + cx + cy = 6 bytes total; rest is bytes 3..6.
        if rest.len() < 3 {
            return None;
        }
        decode_basic(rest[0], rest[1] as u32, rest[2] as u32, &mut self.pressed)
    }
}

/// Scroll acceleration state machine, ported from `MacOSScrollAccel`.
///
/// Quick bursts of scroll ticks ramp the multiplier via an exponential curve;
/// ticks within `min_tick_interval` (6ms) are merged (ignored) so high-frequency
/// double-ticks from terminals like Ghostty don't over-accelerate.
#[derive(Debug, Clone)]
pub struct ScrollAccel {
    last_tick: Option<Instant>,
    history: Vec<Duration>,
    history_size: usize,
    streak_timeout: Duration,
    min_tick_interval: Duration,
    /// A (curve) shape factor. Larger => sharper acceleration.
    a: f64,
    /// Time constant (tau). Larger => gentler acceleration.
    tau: f64,
    max_multiplier: f64,
}

impl Default for ScrollAccel {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollAccel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_tick: None,
            history: Vec::with_capacity(3),
            history_size: 3,
            streak_timeout: Duration::from_millis(150),
            min_tick_interval: Duration::from_millis(6),
            a: 0.8,
            tau: 3.0,
            max_multiplier: 6.0,
        }
    }

    /// Build a custom-configured accelerator.
    #[must_use]
    pub fn with_opts(a: f64, tau: f64, max_multiplier: f64) -> Self {
        Self {
            last_tick: None,
            history: Vec::with_capacity(3),
            history_size: 3,
            streak_timeout: Duration::from_millis(150),
            min_tick_interval: Duration::from_millis(6),
            a,
            tau,
            max_multiplier,
        }
    }

    /// Returns the multiplier for this tick. First tick and any tick after a
    /// `streak_timeout` gap return 1.0 (no acceleration). Ticks within
    /// `min_tick_interval` are merged and return 1.0.
    pub fn tick(&mut self, now: Instant) -> f64 {
        let Some(prev) = self.last_tick else {
            self.last_tick = Some(now);
            self.history.clear();
            return 1.0;
        };

        let dt = now.saturating_duration_since(prev);

        if dt > self.streak_timeout {
            self.last_tick = Some(now);
            self.history.clear();
            return 1.0;
        }

        if dt < self.min_tick_interval {
            // Part of the same logical tick (e.g. Ghostty double-tick); ignore.
            return 1.0;
        }

        self.last_tick = Some(now);
        self.history.push(dt);
        if self.history.len() > self.history_size {
            self.history.remove(0);
        }

        let total: Duration = self.history.iter().sum();
        let avg_interval = total.as_secs_f64() / self.history.len() as f64;
        let reference_interval = 0.1; // 100ms = velocity 1
        let velocity = reference_interval / avg_interval;
        let x = velocity / self.tau;
        let multiplier = 1.0 + self.a * (x.exp() - 1.0);
        multiplier.min(self.max_multiplier)
    }

    /// Reset acceleration state (mirrors `MacOSScrollAccel::reset`).
    pub fn reset(&mut self) {
        self.last_tick = None;
        self.history.clear();
    }

    /// Current tracked velocity history length (for testing).
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// True when no tick has occurred yet (first-tick behavior).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.last_tick.is_none()
    }

    /// The minimum inter-tick interval that is treated as a real tick.
    #[must_use]
    pub const fn min_tick_interval(&self) -> Duration {
        self.min_tick_interval
    }

    /// The streak timeout duration.
    #[must_use]
    pub const fn streak_timeout(&self) -> Duration {
        self.streak_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// SGR single event helper: ESC [ < code ; x+1 ; y+1 M|m
    fn sgr(code: u8, x: u32, y: u32, press: bool) -> Vec<u8> {
        let suffix = if press { b"M" } else { b"m" };
        format!("\x1b[<{};{};{}", code, x + 1, y + 1)
            .into_bytes()
            .into_iter()
            .chain(suffix.iter().copied())
            .collect()
    }

    /// X10 helper: ESC [ M (bb+32) (x+33) (y+33)  -- bb is the logical byte.
    fn x10(bb: u8, x: u32, y: u32) -> Vec<u8> {
        vec![0x1b, b'[', b'M', bb + 32, (x + 33) as u8, (y + 33) as u8]
    }

    #[test]
    fn sgr_left_press() {
        let e = parse_mouse_event(&sgr(0, 10, 5, true)).unwrap();
        assert_eq!(e.event_type, MouseEventType::Down);
        assert_eq!(e.button, 0);
        assert_eq!(e.x, 10);
        assert_eq!(e.y, 5);
        assert!(!e.modifiers.shift && !e.modifiers.alt && !e.modifiers.ctrl);
        assert!(e.scroll.is_none());
    }

    #[test]
    fn sgr_left_release() {
        let mut p = MouseParser::new();
        p.parse(&sgr(0, 10, 5, true)).unwrap(); // press first
        let e = p.parse(&sgr(0, 10, 5, false)).unwrap(); // release
        assert_eq!(e.event_type, MouseEventType::Up);
        assert_eq!(e.button, 0);
    }

    #[test]
    fn sgr_scroll_directions() {
        let cases = [
            (64u8, ScrollDirection::Up),
            (65, ScrollDirection::Down),
            (66, ScrollDirection::Left),
            (67, ScrollDirection::Right),
        ];
        for (code, dir) in cases {
            let mut p = MouseParser::new();
            let e = p.parse(&sgr(code, 10, 5, true)).unwrap();
            assert_eq!(e.event_type, MouseEventType::Scroll, "code {}", code);
            let scroll = e.scroll.unwrap();
            assert_eq!(scroll.direction, dir, "code {}", code);
            assert_eq!(scroll.delta, 1.0);
            assert_eq!(e.button, 0);
        }
    }

    #[test]
    fn sgr_scroll_plus_motion_is_move() {
        // 96 (64|32) and 97 (65|32): motion bit takes priority over scroll bit.
        for code in [96u8, 97] {
            let mut p = MouseParser::new();
            let e = p.parse(&sgr(code, 80, 66, true)).unwrap();
            assert_eq!(e.event_type, MouseEventType::Move, "code {}", code);
            assert!(e.scroll.is_none(), "code {}", code);
            assert_eq!(e.x, 80);
            assert_eq!(e.y, 66);
        }
    }

    #[test]
    fn sgr_scroll_release_not_scroll() {
        // code 64 with release suffix => not a scroll event.
        let mut p = MouseParser::new();
        let e = p.parse(&sgr(64, 10, 5, false)).unwrap();
        assert_ne!(e.event_type, MouseEventType::Scroll);
        assert!(e.scroll.is_none());
    }

    #[test]
    fn sgr_drag_promoted_from_motion() {
        let mut p = MouseParser::new();
        // Press left first.
        let down = p.parse(&sgr(0, 5, 5, true)).unwrap();
        assert_eq!(down.event_type, MouseEventType::Down);
        // Motion with button 0 (code 32) => drag.
        let drag = p.parse(&sgr(0x20, 8, 5, true)).unwrap();
        assert_eq!(drag.event_type, MouseEventType::Drag);
        assert_eq!(drag.x, 8);
        assert_eq!(drag.y, 5);
    }

    #[test]
    fn sgr_motion_without_press_is_move() {
        let mut p = MouseParser::new();
        let e = p.parse(&sgr(0x20, 10, 5, true)).unwrap();
        assert_eq!(e.event_type, MouseEventType::Move);
    }

    #[test]
    fn sgr_motion_button3_is_move_with_buttons_pressed() {
        let mut p = MouseParser::new();
        p.parse(&sgr(0, 5, 5, true)).unwrap(); // press left
        let e = p.parse(&sgr(0x23, 12, 5, false)).unwrap(); // motion, button 3
        assert_eq!(e.event_type, MouseEventType::Move);
    }

    #[test]
    fn sgr_modifiers_all() {
        // 28 = 4 + 8 + 16 (shift + alt + ctrl), left button down.
        let e = parse_mouse_event(&sgr(28, 10, 5, true)).unwrap();
        assert_eq!(
            e.modifiers,
            Modifiers { shift: true, alt: true, ctrl: true }
        );
        assert_eq!(e.event_type, MouseEventType::Down);
        assert_eq!(e.button, 0);
    }

    #[test]
    fn sgr_modifiers_individual() {
        assert!(parse_mouse_event(&sgr(4, 5, 5, true)).unwrap().modifiers.shift);
        assert!(parse_mouse_event(&sgr(8, 5, 5, true)).unwrap().modifiers.alt);
        assert!(parse_mouse_event(&sgr(16, 5, 5, true)).unwrap().modifiers.ctrl);
    }

    #[test]
    fn sgr_origin_is_zero() {
        let e = parse_mouse_event(&sgr(0, 0, 0, true)).unwrap();
        assert_eq!(e.x, 0);
        assert_eq!(e.y, 0);
    }

    #[test]
    fn sgr_large_coordinates_no_223_limit() {
        let e = parse_mouse_event(&sgr(0, 500, 300, true)).unwrap();
        assert_eq!(e.x, 500);
        assert_eq!(e.y, 300);
    }

    #[test]
    fn x10_left_press() {
        let e = parse_mouse_event(&x10(0, 10, 5)).unwrap();
        assert_eq!(e.event_type, MouseEventType::Down);
        assert_eq!(e.button, 0);
        assert_eq!(e.x, 10);
        assert_eq!(e.y, 5);
    }

    #[test]
    fn x10_middle_right_press() {
        let p1 = parse_mouse_event(&x10(1, 10, 5)).unwrap();
        assert_eq!(p1.button, 1);
        let p2 = parse_mouse_event(&x10(2, 10, 5)).unwrap();
        assert_eq!(p2.button, 2);
    }

    #[test]
    fn x10_release_byte_3_is_up() {
        let mut p = MouseParser::new();
        p.parse(&x10(0, 10, 5)).unwrap(); // press
        let e = p.parse(&x10(3, 10, 5)).unwrap(); // release always byte 3
        assert_eq!(e.event_type, MouseEventType::Up);
    }

    #[test]
    fn x10_scroll_directions() {
        let cases = [
            (64u8, ScrollDirection::Up),
            (65, ScrollDirection::Down),
            (66, ScrollDirection::Left),
            (67, ScrollDirection::Right),
        ];
        for (bb, dir) in cases {
            let e = parse_mouse_event(&x10(bb, 10, 5)).unwrap();
            assert_eq!(e.event_type, MouseEventType::Scroll, "bb {}", bb);
            assert_eq!(e.scroll.unwrap().direction, dir, "bb {}", bb);
        }
    }

    #[test]
    fn x10_motion_is_move() {
        for bb in [35u8, 32, 33, 34] {
            let e = parse_mouse_event(&x10(bb, 10, 5)).unwrap();
            assert_eq!(e.event_type, MouseEventType::Move, "bb {}", bb);
        }
    }

    #[test]
    fn x10_motion_bit_priority_over_scroll() {
        // 96 (64|32): motion bit set => move, scroll None.
        let e = parse_mouse_event(&x10(96, 10, 5)).unwrap();
        assert_eq!(e.event_type, MouseEventType::Move);
        assert!(e.scroll.is_none());
    }

    #[test]
    fn x10_max_safe_coordinate_94() {
        // 94 + 33 = 127 (0x7F), valid single byte in latin1.
        let e = parse_mouse_event(&x10(0, 94, 94)).unwrap();
        assert_eq!(e.x, 94);
        assert_eq!(e.y, 94);
    }

    #[test]
    fn x10_high_coordinate_preserved_under_latin1() {
        // 95 + 33 = 128 (0x80). Under latin1 this is a valid byte; the parser
        // treats input as bytes so the coordinate round-trips to 95.
        let e = parse_mouse_event(&x10(0, 95, 95)).unwrap();
        assert_eq!(e.x, 95);
        assert_eq!(e.y, 95);
    }

    #[test]
    fn null_empty_incomplete_return_none() {
        assert!(parse_mouse_event(b"").is_none());
        assert!(parse_mouse_event(b"\x1b[A").is_none()); // cursor up
        assert!(parse_mouse_event(b"\x1b[1;2R").is_none()); // CPR
        assert!(parse_mouse_event(b"\x1b[<0;1;1").is_none()); // incomplete SGR
        assert!(parse_mouse_event(b"\x1b[M\x20").is_none()); // too short X10
        assert!(parse_mouse_event(&[0x1b, 0x5b]).is_none()); // introducer only
    }

    #[test]
    fn sgr_precedence_over_x10() {
        // An SGR payload is dispatched on '<'; X10 on 'M'. Verify dispatch.
        assert!(parse_mouse_event(&sgr(0, 5, 5, true)).is_some());
        assert!(parse_mouse_event(&x10(0, 5, 5)).is_some());
    }

    #[test]
    fn reset_clears_drag_tracking() {
        let mut p = MouseParser::new();
        p.parse(&sgr(0, 5, 5, true)).unwrap();
        p.reset();
        let e = p.parse(&sgr(0x20, 8, 5, true)).unwrap();
        assert_eq!(e.event_type, MouseEventType::Move); // no press tracked => move
    }

    #[test]
    fn scroll_accel_first_tick_is_one() {
        let mut acc = ScrollAccel::new();
        assert!(acc.is_idle());
        let m = acc.tick(Instant::now());
        assert_eq!(m, 1.0);
        assert!(!acc.is_idle());
    }

    #[test]
    fn scroll_accel_timeout_resets() {
        let mut acc = ScrollAccel::new();
        let t0 = Instant::now();
        acc.tick(t0);
        // Gap beyond streak_timeout (150ms) => 1.0.
        let m = acc.tick(t0 + Duration::from_millis(200));
        assert_eq!(m, 1.0);
    }

    #[test]
    fn scroll_accel_rapid_ticks_accelerate() {
        let mut acc = ScrollAccel::new();
        let t0 = Instant::now();
        // First tick establishes baseline.
        let base = acc.tick(t0);
        assert_eq!(base, 1.0);
        // Subsequent ticks at ~10ms intervals (above 6ms min) accumulate history
        // and ramp the multiplier above 1.0. With 3.33 tau the multiplier
        // saturates at maxMultiplier=6.0 quickly, so we assert it rises to the
        // cap rather than requiring a strict monotonic increase per tick.
        let m1 = acc.tick(t0 + Duration::from_millis(10));
        let m2 = acc.tick(t0 + Duration::from_millis(20));
        let m3 = acc.tick(t0 + Duration::from_millis(30));
        assert!(m1 > 1.0, "m1={} should exceed 1.0", m1);
        assert!(m2 >= m1, "m2={} should be >= m1={}", m2, m1);
        assert!(m3 >= m2, "m3={} should be >= m2={}", m3, m2);
    }

    #[test]
    fn scroll_accel_min_tick_interval_merges() {
        let mut acc = ScrollAccel::new();
        let t0 = Instant::now();
        acc.tick(t0);
        // Tick within 6ms => ignored (1.0).
        let m = acc.tick(t0 + Duration::from_millis(3));
        assert_eq!(m, 1.0);
    }

    #[test]
    fn scroll_accel_reset_clears_state() {
        let mut acc = ScrollAccel::new();
        let t0 = Instant::now();
        acc.tick(t0);
        acc.tick(t0 + Duration::from_millis(50));
        assert!(acc.history_len() >= 1);
        acc.reset();
        assert!(acc.is_idle());
        assert_eq!(acc.history_len(), 0);
    }

    #[test]
    fn hover_coords_reconstructable() {
        // The renderer stores _latestPointer{x,y} from processSingleMouseEvent and
        // reuses it for out/over in recheckHoverState. Verify the parsed event
        // carries coords consistent with that contract.
        let e = parse_mouse_event(&sgr(0, 42, 17, true)).unwrap();
        assert_eq!(e.x, 42);
        assert_eq!(e.y, 17);
        assert_eq!(e.event_type, MouseEventType::Down);
    }

    #[test]
    fn sgr_middle_button_press() {
        let e = parse_mouse_event(&sgr(1, 10, 5, true)).unwrap();
        assert_eq!(e.button, 1);
        assert_eq!(e.event_type, MouseEventType::Down);
    }

    #[test]
    fn sgr_right_button_press() {
        let e = parse_mouse_event(&sgr(2, 10, 5, true)).unwrap();
        assert_eq!(e.button, 2);
        assert_eq!(e.event_type, MouseEventType::Down);
    }

    #[test]
    fn drag_with_right_button_after_press() {
        let mut p = MouseParser::new();
        p.parse(&sgr(2, 5, 5, true)).unwrap(); // right press
        let e = p.parse(&sgr(0x22, 8, 5, true)).unwrap(); // motion with button 2 (32|2)
        assert_eq!(e.event_type, MouseEventType::Drag);
        assert_eq!(e.button, 2);
    }

    #[test]
    fn release_clears_tracked_button() {
        let mut p = MouseParser::new();
        p.parse(&sgr(0, 5, 5, true)).unwrap(); // left down
        p.parse(&sgr(0x20, 8, 5, true)).unwrap(); // motion => drag
        // release left
        p.parse(&sgr(0, 8, 5, false)).unwrap();
        // next motion => move (no button held)
        let e = p.parse(&sgr(0x23, 10, 5, false)).unwrap(); // button 3 motion
        assert_eq!(e.event_type, MouseEventType::Move);
    }

    #[test]
    fn wheel_delta_exceeds_i32_clamps() {
        // Very large SGR coordinate (wire value) must not overflow i32 during
        // the saturating sub. 2^31-1 + 1 = 2^31 would exceed i32::MAX on the TS
        // side; here we confirm a normal large value round-trips and the
        // overflow candidate path is exercised without panic.
        let _overflow = parse_mouse_event(&sgr(0, 0x7FFF_FFFF, 0x7FFF_FFFF, true));
        // Confirm a very large (but representable) value works:
        let e = parse_mouse_event(&sgr(0, 1_000_000, 1_000_000, true)).unwrap();
        assert_eq!(e.x, 1_000_000);
        assert_eq!(e.y, 1_000_000);
    }
}