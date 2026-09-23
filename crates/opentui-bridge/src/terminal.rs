#![forbid(unsafe_code)]
//! Terminal session state machine (RED skeleton).
//!
//! Upstream: `packages/core/src/renderer.ts` (`buildKittyKeyboardFlags` ~616,
//! `setupTerminal` ~3271, `enableMouse`/`disableMouse` ~3242, pixel query
//! ~3950 + response ~3570, `processResize` ~3959, `suspend`/`resume`
//! ~4228ff) and `packages/ssh/src/bridge.ts` (`clampPtyDimension`, 20-38).

pub const KITTY_FLAG_DISAMBIGUATE: u32 = 0b1;
pub const KITTY_FLAG_EVENT_TYPES: u32 = 0b10;
pub const KITTY_FLAG_ALTERNATE_KEYS: u32 = 0b100;
pub const KITTY_FLAG_ALL_KEYS_AS_ESCAPES: u32 = 0b1000;
pub const KITTY_FLAG_REPORT_TEXT: u32 = 0b10000;

pub const MAX_COLS: u32 = 1000;
pub const MAX_ROWS: u32 = 500;
pub const DEFAULT_COLS: u32 = 80;
pub const DEFAULT_ROWS: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KittyKeyboardOptions {
    pub disambiguate: Option<bool>,
    pub alternate_keys: Option<bool>,
    pub events: bool,
    pub all_keys_as_escapes: bool,
    pub report_text: bool,
}

pub fn kitty_keyboard_flags(config: Option<&KittyKeyboardOptions>) -> u32 {
    let Some(c) = config else { return 0 };
    let mut flags = 0u32;
    if c.disambiguate != Some(false) {
        flags |= KITTY_FLAG_DISAMBIGUATE;
    }
    if c.alternate_keys != Some(false) {
        flags |= KITTY_FLAG_ALTERNATE_KEYS;
    }
    if c.events {
        flags |= KITTY_FLAG_EVENT_TYPES;
    }
    if c.all_keys_as_escapes {
        flags |= KITTY_FLAG_ALL_KEYS_AS_ESCAPES;
    }
    if c.report_text {
        flags |= KITTY_FLAG_REPORT_TEXT;
    }
    flags
}

fn clamp_dim(value: u32, current: u32, max: u32) -> u32 {
    if value == 0 {
        return current;
    }
    value.clamp(1, max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseMode {
    #[default]
    Off,
    Basic,
    WithMovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlState {
    #[default]
    Idle,
    Started,
    Paused,
    Suspended,
    Stopped,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelResolution {
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug)]
pub struct TerminalSession {
    pub cols: u32,
    pub rows: u32,
    pub setup: bool,
    pub destroyed: bool,
    pub control: ControlState,
    pub prev_control: ControlState,
    pub mouse: MouseMode,
    pub saved_mouse: MouseMode,
    pub kitty_flags: u32,
    pub waiting_pixel: bool,
    pub pixel_requery_pending: bool,
    pub resolution: Option<PixelResolution>,
    event_log: Vec<&'static str>,
}

impl TerminalSession {
    pub fn new(cols: u32, rows: u32) -> Self {
        Self {
            cols: cols.clamp(1, MAX_COLS),
            rows: rows.clamp(1, MAX_ROWS),
            setup: false,
            destroyed: false,
            control: ControlState::Idle,
            prev_control: ControlState::Idle,
            mouse: MouseMode::Off,
            saved_mouse: MouseMode::Off,
            kitty_flags: 0,
            waiting_pixel: false,
            pixel_requery_pending: false,
            resolution: None,
            event_log: Vec::new(),
        }
    }
    /// Idempotent setup (renderer `setupTerminal` guard). Returns true on first setup.
    pub fn setup_terminal(&mut self) -> bool {
        if self.destroyed || self.setup {
            return false;
        }
        self.setup = true;
        true
    }
    pub fn kitty_keyboard_flags(&self) -> u32 {
        self.kitty_flags
    }
    pub fn enable_kitty(&mut self, config: Option<&KittyKeyboardOptions>) {
        self.kitty_flags = kitty_keyboard_flags(config);
    }
    /// Resize with per-axis clamp 1..max; zero/oversized per ssh `clampPtyDimension`.
    /// Frozen after close: returns current dims.
    pub fn resize(&mut self, cols: u32, rows: u32) -> (u32, u32) {
        if self.destroyed {
            return (self.cols, self.rows);
        }
        self.cols = clamp_dim(cols, self.cols, MAX_COLS);
        self.rows = clamp_dim(rows, self.rows, MAX_ROWS);
        self.resolution = None;
        (self.cols, self.rows)
    }
    pub fn enable_mouse(&mut self, movement: bool) {
        if self.destroyed {
            return;
        }
        self.mouse = if movement { MouseMode::WithMovement } else { MouseMode::Basic };
    }
    pub fn disable_mouse(&mut self) {
        self.mouse = MouseMode::Off;
    }
    /// Pixel query (renderer `queryPixelResolution`): defers while suspended/waiting.
    /// Returns true when a fresh native query is issued.
    pub fn query_pixel_resolution(&mut self) -> bool {
        if self.destroyed
            || self.control == ControlState::Suspended
            || self.waiting_pixel
        {
            self.pixel_requery_pending = true;
            return false;
        }
        self.pixel_requery_pending = false;
        self.waiting_pixel = true;
        true
    }
    /// Pixel response (renderer stdin handler): false when none outstanding.
    pub fn on_pixel_response(&mut self, w: u32, h: u32) -> bool {
        if !self.waiting_pixel {
            return false;
        }
        self.waiting_pixel = false;
        if self.pixel_requery_pending {
            self.pixel_requery_pending = false;
            self.waiting_pixel = true;
            return true;
        }
        if w > 0 && h > 0 {
            self.resolution = Some(PixelResolution { width_px: w, height_px: h });
        }
        true
    }
    /// Suspend order mirrors renderer `suspend()`: save control, flag, pause,
    /// save+disable mouse, drop listeners, reset parser, suspend native,
    /// raw-mode off, stdin pause.
    pub fn suspend(&mut self) {
        if self.destroyed || self.control == ControlState::Suspended {
            return;
        }
        self.prev_control = self.control;
        self.control = ControlState::Suspended;
        // Geometry may change while suspended; pixel state is stale and must
        // be re-queried on resume (renderer `resume()` requery branch).
        if self.setup {
            self.pixel_requery_pending = true;
        }
        self.event_log.push("save_control");
        self.event_log.push("set_suspended");
        self.event_log.push("pause");
        self.saved_mouse = self.mouse;
        self.event_log.push("save_mouse");
        self.disable_mouse();
        self.event_log.push("disable_mouse");
        self.event_log.push("remove_listeners");
        self.event_log.push("reset_parser");
        self.event_log.push("suspend_renderer");
        self.event_log.push("raw_mode_off");
        self.event_log.push("stdin_pause");
    }
    /// Resume order mirrors renderer `resume()`: raw on, drain, reset parser,
    /// re-listen, resume native, restore mouse, restore control, requery pixel.
    pub fn resume(&mut self) {
        if self.destroyed || self.control != ControlState::Suspended {
            return;
        }
        self.event_log.clear();
        self.event_log.push("raw_mode_on");
        self.event_log.push("drain_input");
        self.event_log.push("reset_parser");
        self.event_log.push("add_listener");
        self.event_log.push("resume_renderer");
        if self.saved_mouse != MouseMode::Off {
            self.mouse = self.saved_mouse;
            self.event_log.push("restore_mouse");
        }
        self.saved_mouse = MouseMode::Off;
        self.control = self.prev_control;
        self.event_log.push("restore_control");
        if self.pixel_requery_pending {
            self.pixel_requery_pending = false;
            self.waiting_pixel = true;
            self.event_log.push("requery_pixel");
        }
        self.event_log.push("start_or_render");
    }
    /// Idempotent close handshake: restore terminal, teardown native once,
    /// then close transport. Returns true on first close.
    pub fn close(&mut self) -> bool {
        if self.destroyed {
            return false;
        }
        self.destroyed = true;
        self.control = ControlState::Destroyed;
        self.disable_mouse();
        self.event_log.clear();
        self.event_log.push("restore_terminal");
        self.event_log.push("teardown_native");
        self.event_log.push("close_transport");
        true
    }
    pub fn event_log(&self) -> &[&'static str] {
        &self.event_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_none_is_zero() {
        assert_eq!(kitty_keyboard_flags(None), 0);
        assert_eq!(TerminalSession::new(80, 24).kitty_keyboard_flags(), 0);
    }

    #[test]
    fn kitty_defaults_5() {
        let opts = KittyKeyboardOptions::default();
        assert_eq!(kitty_keyboard_flags(Some(&opts)), 0b101);
    }

    #[test]
    fn kitty_all_31() {
        let opts = KittyKeyboardOptions {
            disambiguate: Some(true),
            alternate_keys: Some(true),
            events: true,
            all_keys_as_escapes: true,
            report_text: true,
        };
        assert_eq!(kitty_keyboard_flags(Some(&opts)), 31);
    }

    #[test]
    fn kitty_progressive_enhancement() {
        let opts = KittyKeyboardOptions {
            disambiguate: Some(false),
            alternate_keys: Some(false),
            events: true,
            ..KittyKeyboardOptions::default()
        };
        assert_eq!(kitty_keyboard_flags(Some(&opts)), 0b10);
    }

    #[test]
    fn resize_clamp_max() {
        let mut s = TerminalSession::new(80, 24);
        assert_eq!(s.resize(5000, 9000), (1000, 500));
    }

    #[test]
    fn resize_zero_rejected_per_axis() {
        let mut s = TerminalSession::new(80, 24);
        assert_eq!(s.resize(0, 0), (80, 24));
        assert_eq!(s.resize(0, 100), (80, 100));
        assert_eq!(s.resize(120, 0), (120, 100));
    }

    #[test]
    fn resize_min_floor() {
        let mut s = TerminalSession::new(80, 24);
        assert_eq!(s.resize(1, 1), (1, 1));
    }

    #[test]
    fn mouse_levels() {
        let mut s = TerminalSession::new(80, 24);
        assert_eq!(s.mouse, MouseMode::Off);
        s.enable_mouse(false);
        assert_eq!(s.mouse, MouseMode::Basic);
        s.enable_mouse(true);
        assert_eq!(s.mouse, MouseMode::WithMovement);
        s.disable_mouse();
        assert_eq!(s.mouse, MouseMode::Off);
    }

    #[test]
    fn pixel_suspend_defers() {
        let mut s = TerminalSession::new(80, 24);
        s.setup_terminal();
        assert!(s.query_pixel_resolution());
        assert!(s.waiting_pixel);
        s.suspend();
        assert!(!s.query_pixel_resolution());
        assert!(s.pixel_requery_pending);
        s.resume();
        assert!(s.waiting_pixel);
        assert!(!s.pixel_requery_pending);
    }

    #[test]
    fn suspend_resume_order() {
        let mut s = TerminalSession::new(80, 24);
        s.setup_terminal();
        s.enable_mouse(false);
        s.suspend();
        assert_eq!(
            s.event_log(),
            &[
                "save_control",
                "set_suspended",
                "pause",
                "save_mouse",
                "disable_mouse",
                "remove_listeners",
                "reset_parser",
                "suspend_renderer",
                "raw_mode_off",
                "stdin_pause"
            ]
        );
        s.resume();
        assert_eq!(
            s.event_log(),
            &[
                "raw_mode_on",
                "drain_input",
                "reset_parser",
                "add_listener",
                "resume_renderer",
                "restore_mouse",
                "restore_control",
                "requery_pixel",
                "start_or_render"
            ]
        );
    }

    #[test]
    fn close_handshake_idempotent() {
        let mut s = TerminalSession::new(80, 24);
        s.setup_terminal();
        assert!(s.close());
        assert!(s.destroyed);
        assert!(!s.close());
        assert_eq!(
            s.event_log(),
            &["restore_terminal", "teardown_native", "close_transport"]
        );
        assert_eq!(s.resize(100, 30), (80, 24));
    }
}
