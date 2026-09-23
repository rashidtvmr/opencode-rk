#![forbid(unsafe_code)]
//! Render-context types for the OpenTUI bridge.
//!
//! Maps the TypeScript surface from `packages/core/src/types.ts` (RenderContext,
//! TerminalCapabilities, WidthMethod) and `packages/core/src/renderer.ts`
//! (CliRendererConfig, CliRenderEvents) into Rust value types. std-only, no
//! unsafe. Strings for `from_str` use kebab-case matching the TS source.

use std::fmt;
use std::str::FromStr;

// ── WidthMethod ──────────────────────────────────────────────────────────

/// Mirrors `types.ts:105`: `"wcwidth" | "unicode" | "unicode-wide"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidthMethod {
    Wcwidth,
    Unicode,
    UnicodeWide,
}

impl WidthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wcwidth => "wcwidth",
            Self::Unicode => "unicode",
            Self::UnicodeWide => "unicode-wide",
        }
    }
}

impl FromStr for WidthMethod {
    type Err = ParseWidthMethodError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "wcwidth" => Self::Wcwidth,
            "unicode" => Self::Unicode,
            "unicode-wide" => Self::UnicodeWide,
            _ => return Err(ParseWidthMethodError(s.to_string())),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWidthMethodError(pub String);

impl fmt::Display for ParseWidthMethodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown WidthMethod: {}", self.0)
    }
}

impl std::error::Error for ParseWidthMethodError {}

// ── Terminal info enums ──────────────────────────────────────────────────

/// Mirrors `types.ts:106`: `"none" | "tmux" | "zellij" | "screen" | "unknown"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TerminalMultiplexer {
    None,
    Tmux,
    Zellij,
    Screen,
    Unknown,
}

impl TerminalMultiplexer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Tmux => "tmux",
            Self::Zellij => "zellij",
            Self::Screen => "screen",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for TerminalMultiplexer {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "none" => Self::None,
            "tmux" => Self::Tmux,
            "zellij" => Self::Zellij,
            "screen" => Self::Screen,
            "unknown" => Self::Unknown,
            _ => return Err(s.to_string()),
        })
    }
}

/// Mirrors `types.ts:107`: `"unknown" | "supported" | "unsupported"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TerminalCapabilityState {
    Unknown,
    Supported,
    Unsupported,
}

impl TerminalCapabilityState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
        }
    }
}

impl FromStr for TerminalCapabilityState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "unknown" => Self::Unknown,
            "supported" => Self::Supported,
            "unsupported" => Self::Unsupported,
            _ => return Err(s.to_string()),
        })
    }
}

/// Mirrors `types.ts:108`: `"auto" | "kitty" | "sixel" | "blocks"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageRenderProtocol {
    Auto,
    Kitty,
    Sixel,
    Blocks,
}

impl ImageRenderProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Kitty => "kitty",
            Self::Sixel => "sixel",
            Self::Blocks => "blocks",
        }
    }
}

impl FromStr for ImageRenderProtocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "auto" => Self::Auto,
            "kitty" => Self::Kitty,
            "sixel" => Self::Sixel,
            "blocks" => Self::Blocks,
            _ => return Err(s.to_string()),
        })
    }
}

// ── TerminalInfo ────────────────────────────────────────────────────────

/// Mirrors `types.ts:110-114`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalInfo {
    pub name: String,
    pub version: String,
    pub from_xtversion: bool,
}

// ── TerminalCapabilities ─────────────────────────────────────────────────

/// Mirrors `types.ts:116-139`.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalCapabilities {
    pub kitty_keyboard: bool,
    pub kitty_graphics: bool,
    pub rgb: bool,
    pub ansi256: bool,
    pub unicode: WidthMethod,
    pub sgr_pixels: bool,
    pub color_scheme_updates: bool,
    pub explicit_width: bool,
    pub scaled_text: bool,
    pub sixel: bool,
    pub focus_tracking: bool,
    pub sync: bool,
    pub bracketed_paste: bool,
    pub hyperlinks: bool,
    pub osc52: bool,
    pub osc52_support: TerminalCapabilityState,
    pub notifications: bool,
    pub explicit_cursor_positioning: bool,
    pub remote: bool,
    pub multiplexer: TerminalMultiplexer,
    pub image_protocol: Option<ImageRenderProtocol>,
    pub terminal: TerminalInfo,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            kitty_keyboard: false,
            kitty_graphics: false,
            rgb: false,
            ansi256: false,
            unicode: WidthMethod::Unicode,
            sgr_pixels: false,
            color_scheme_updates: false,
            explicit_width: false,
            scaled_text: false,
            sixel: false,
            focus_tracking: false,
            sync: false,
            bracketed_paste: false,
            hyperlinks: false,
            osc52: false,
            osc52_support: TerminalCapabilityState::Unknown,
            notifications: false,
            explicit_cursor_positioning: false,
            remote: false,
            multiplexer: TerminalMultiplexer::None,
            image_protocol: None,
            terminal: TerminalInfo {
                name: String::new(),
                version: String::new(),
                from_xtversion: false,
            },
        }
    }
}

impl TerminalCapabilities {
    pub fn new(terminal: TerminalInfo) -> Self {
        let mut caps = Self::default();
        caps.terminal = terminal;
        caps
    }
}

// ── ScreenMode, ExternalOutputMode, ConsoleMode ────────────────────────

/// Mirrors `renderer.ts:242`: `"alternate-screen" | "main-screen" | "split-footer"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScreenMode {
    AlternateScreen,
    MainScreen,
    SplitFooter,
}

impl ScreenMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlternateScreen => "alternate-screen",
            Self::MainScreen => "main-screen",
            Self::SplitFooter => "split-footer",
        }
    }
}

impl FromStr for ScreenMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "alternate-screen" => Self::AlternateScreen,
            "main-screen" => Self::MainScreen,
            "split-footer" => Self::SplitFooter,
            _ => return Err(s.to_string()),
        })
    }
}

/// Mirrors `renderer.ts:250`: `"capture-stdout" | "passthrough"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExternalOutputMode {
    CaptureStdout,
    Passthrough,
}

impl FromStr for ExternalOutputMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "capture-stdout" => Self::CaptureStdout,
            "passthrough" => Self::Passthrough,
            _ => return Err(s.to_string()),
        })
    }
}

/// Mirrors `renderer.ts:265`: `"console-overlay" | "disabled"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConsoleMode {
    ConsoleOverlay,
    Disabled,
}

impl FromStr for ConsoleMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "console-overlay" => Self::ConsoleOverlay,
            "disabled" => Self::Disabled,
            _ => return Err(s.to_string()),
        })
    }
}

// ── CliRendererConfig ────────────────────────────────────────────────────

/// Mirrors `renderer.ts:750-849` `CliRenderEvents` enum.
///
/// Each variant corresponds to a `CliRenderEvents` key. Variants that carry
/// a payload encode the TS argument shape.
#[derive(Clone, Debug, PartialEq)]
pub enum CliRenderEvent {
    /// `resize(width, height, renderHeight)`
    Resize {
        width: u16,
        height: u16,
        render_height: u16,
    },
    /// `frame(frameId)`
    Frame { frame_id: u64 },
    /// `render:error({ error, renderable })`
    RenderError { error: String },
    /// `handler:error({ error, event })`
    HandlerError { error: String },
    /// `external_output(commit)`
    ExternalOutput,
    /// `focus`
    Focus,
    /// `blur`
    Blur,
    /// `focused_renderable(current, previous)`
    FocusedRenderable { current: Option<String>, previous: Option<String> },
    /// `focused_editor(current, previous)`
    FocusedEditor { current: Option<String>, previous: Option<String> },
    /// `theme_mode(mode)`
    ThemeMode { mode: ThemeMode },
    /// `palette(colors)`
    Palette,
    /// `capabilities(capabilities)`
    Capabilities(TerminalCapabilities),
    /// `selection(selection)`
    Selection,
    /// `debugOverlay:toggle(enabled)`
    DebugOverlayToggle { enabled: bool },
    /// `destroy`
    Destroy,
    /// `memory:snapshot(snapshot)`
    MemorySnapshot {
        heap_used: u64,
        heap_total: u64,
        array_buffers: u64,
    },
}

/// Mirrors `types.ts:32`: `"dark" | "light"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl FromStr for ThemeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => return Err(s.to_string()),
        })
    }
}

/// Mirrors `renderer.ts:122-233` `CliRendererConfig` interface.
///
/// All numeric/bool fields carry sensible defaults matching the TS defaults
/// (width 80, height 24, debounce 100ms, targetFps 30, maxFps 60, etc.).
/// Optional handles (`stdin`, `stdout`, `bufferedOutput`) are represented as
/// opaque tokens here (the bridge never dereferences them directly).
#[derive(Clone, Debug, PartialEq)]
pub struct CliRendererConfig {
    pub stdin: Option<u64>,
    pub stdout: Option<u64>,
    pub width: u16,
    pub height: u16,
    pub remote: bool,
    pub kitty_image_transport: KittyImageTransport,
    pub buffered_output: Option<u64>,
    pub exit_on_ctrl_c: bool,
    pub exit_signals: Vec<String>,
    pub clear_on_shutdown: bool,
    pub forward_env_keys: Vec<String>,
    pub debounce_delay: u64,
    pub target_fps: u32,
    pub max_fps: u32,
    pub memory_snapshot_interval: u64,
    pub use_thread: bool,
    pub gather_stats: bool,
    pub max_stat_samples: usize,
    pub console_options: Option<()>,
    pub post_process_fns: usize,
    pub enable_mouse_movement: bool,
    pub use_mouse: bool,
    pub auto_focus: bool,
    pub screen_mode: ScreenMode,
    pub footer_height: u16,
    pub external_output_mode: ExternalOutputMode,
    pub console_mode: ConsoleMode,
    pub use_kitty_keyboard: Option<KittyKeyboardOptions>,
    pub background_color: Option<RgbaColor>,
    pub open_console_on_error: bool,
    pub prepend_input_handlers: usize,
    pub stdin_parser_max_buffer_bytes: usize,
    pub clock: Option<u64>,
    pub on_destroy: bool,
}

/// Mirrors `renderer.ts:110`: `"raw" | "zlib" | "file"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KittyImageTransport {
    Raw,
    Zlib,
    File,
}

impl FromStr for KittyImageTransport {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "raw" => Self::Raw,
            "zlib" => Self::Zlib,
            "file" => Self::File,
            _ => return Err(s.to_string()),
        })
    }
}

/// Placeholder for kitty keyboard flags (opaque in bridge).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KittyKeyboardOptions;

/// Simple RGBA color (mirrors `color::Rgba` semantics, local re-declaration).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for CliRendererConfig {
    fn default() -> Self {
        Self {
            stdin: None,
            stdout: None,
            width: 80,
            height: 24,
            remote: false,
            kitty_image_transport: KittyImageTransport::Raw,
            buffered_output: None,
            exit_on_ctrl_c: true,
            exit_signals: Vec::new(),
            clear_on_shutdown: true,
            forward_env_keys: Vec::new(),
            debounce_delay: 100,
            target_fps: 30,
            max_fps: 60,
            memory_snapshot_interval: 0,
            use_thread: false,
            gather_stats: false,
            max_stat_samples: 300,
            console_options: None,
            post_process_fns: 0,
            enable_mouse_movement: true,
            use_mouse: true,
            auto_focus: true,
            screen_mode: ScreenMode::AlternateScreen,
            footer_height: 12,
            external_output_mode: ExternalOutputMode::Passthrough,
            console_mode: ConsoleMode::ConsoleOverlay,
            use_kitty_keyboard: None,
            background_color: None,
            open_console_on_error: false,
            prepend_input_handlers: 0,
            stdin_parser_max_buffer_bytes: 64 * 1024 * 1024,
            clock: None,
            on_destroy: false,
        }
    }
}

// ── RenderContext trait ─────────────────────────────────────────────────

/// Mirrors `types.ts:153-195` `RenderContext` interface (subset relevant to
/// the bridge; the trait is a Rust-native mirror, not an FFI boundary).
pub trait RenderContext {
    // Hit grid ──
    fn add_to_hit_grid(&mut self, x: u16, y: u16, width: u16, height: u16, id: u32);
    fn push_hit_grid_scissor_rect(&mut self, x: u16, y: u16, width: u16, height: u16);
    fn pop_hit_grid_scissor_rect(&mut self);
    fn clear_hit_grid_scissor_rects(&mut self);

    // Dimensions ──
    fn width(&self) -> u16;
    fn height(&self) -> u16;
    fn set_width_height(&mut self, width: u16, height: u16);
    fn terminal_width(&self) -> Option<u16>;
    fn terminal_height(&self) -> Option<u16>;
    fn resolution(&self) -> Option<(u16, u16)>;

    // Frame control ──
    fn frame_id(&self) -> u64;
    fn request_render(&mut self);

    // Cursor ──
    fn set_cursor_position(&mut self, x: u16, y: u16, visible: bool);
    fn set_cursor_style(&mut self, options: &CursorStyleOptions);
    fn set_cursor_color(&mut self, color: RgbaColor);
    fn set_mouse_pointer(&mut self, shape: &str);

    // Capabilities ──
    fn width_method(&self) -> WidthMethod;
    fn capabilities(&self) -> Option<&TerminalCapabilities>;
    fn request_live(&mut self);
    fn drop_live(&mut self);
}

/// Mirrors `types.ts:85-90` `CursorStyleOptions`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CursorStyleOptions {
    pub style: Option<CursorStyle>,
    pub blinking: Option<bool>,
    pub color: Option<RgbaColor>,
    pub cursor: Option<MousePointerStyle>,
}

/// Mirrors `types.ts:34`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorStyle {
    Block,
    Line,
    Underline,
    Default,
}

/// Mirrors `types.ts:47-83`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MousePointerStyle {
    Auto,
    Default,
    None,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    AllScroll,
    ColResize,
    RowResize,
    NResize,
    EResize,
    SResize,
    WResize,
    NeResize,
    NwResize,
    SeResize,
    SwResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ZoomIn,
    ZoomOut,
}

impl MousePointerStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Default => "default",
            Self::None => "none",
            Self::ContextMenu => "context-menu",
            Self::Help => "help",
            Self::Pointer => "pointer",
            Self::Progress => "progress",
            Self::Wait => "wait",
            Self::Cell => "cell",
            Self::Crosshair => "crosshair",
            Self::Text => "text",
            Self::VerticalText => "vertical-text",
            Self::Alias => "alias",
            Self::Copy => "copy",
            Self::Move => "move",
            Self::NoDrop => "no-drop",
            Self::NotAllowed => "not-allowed",
            Self::Grab => "grab",
            Self::Grabbing => "grabbing",
            Self::AllScroll => "all-scroll",
            Self::ColResize => "col-resize",
            Self::RowResize => "row-resize",
            Self::NResize => "n-resize",
            Self::EResize => "e-resize",
            Self::SResize => "s-resize",
            Self::WResize => "w-resize",
            Self::NeResize => "ne-resize",
            Self::NwResize => "nw-resize",
            Self::SeResize => "se-resize",
            Self::SwResize => "sw-resize",
            Self::EwResize => "ew-resize",
            Self::NsResize => "ns-resize",
            Self::NeswResize => "nesw-resize",
            Self::NwseResize => "nwse-resize",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
        }
    }
}

// ── RenderGeometry (from lib/render-geometry.ts) ────────────────────────

/// Mirrors `render-geometry.ts:1`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderGeometryScreenMode {
    AlternateScreen,
    #[default]
    MainScreen,
    SplitFooter,
}

/// Mirrors `render-geometry.ts:3-8`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderGeometry {
    pub effective_footer_height: u16,
    pub render_offset: u16,
    pub render_width: u16,
    pub render_height: u16,
}

/// Mirrors `render-geometry.ts:10-35` `calculateRenderGeometry`.
pub fn calculate_render_geometry(
    screen_mode: RenderGeometryScreenMode,
    terminal_width: u16,
    terminal_height: u16,
    footer_height: u16,
) -> RenderGeometry {
    if screen_mode != RenderGeometryScreenMode::SplitFooter {
        return RenderGeometry {
            effective_footer_height: 0,
            render_offset: 0,
            render_width: terminal_width,
            render_height: terminal_height,
        };
    }
    let effective_footer_height = footer_height.min(terminal_height);
    RenderGeometry {
        effective_footer_height,
        render_offset: terminal_height.saturating_sub(effective_footer_height),
        render_width: terminal_width,
        render_height: effective_footer_height,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// T01: WidthMethod from_str roundtrip (wcwidth | unicode | unicode-wide).
    #[test]
    fn t01_width_method_roundtrip() {
        assert_eq!(WidthMethod::from_str("wcwidth").unwrap(), WidthMethod::Wcwidth);
        assert_eq!(WidthMethod::from_str("unicode").unwrap(), WidthMethod::Unicode);
        assert_eq!(WidthMethod::from_str("unicode-wide").unwrap(), WidthMethod::UnicodeWide);
        assert_eq!(WidthMethod::Wcwidth.as_str(), "wcwidth");
        assert_eq!(WidthMethod::Unicode.as_str(), "unicode");
        assert_eq!(WidthMethod::UnicodeWide.as_str(), "unicode-wide");
        assert!(WidthMethod::from_str("bogus").is_err());
    }

    /// T02: TerminalMultiplexer from_str roundtrip.
    #[test]
    fn t02_terminal_multiplexer_roundtrip() {
        assert_eq!(TerminalMultiplexer::from_str("none").unwrap(), TerminalMultiplexer::None);
        assert_eq!(TerminalMultiplexer::from_str("tmux").unwrap(), TerminalMultiplexer::Tmux);
        assert_eq!(TerminalMultiplexer::from_str("zellij").unwrap(), TerminalMultiplexer::Zellij);
        assert_eq!(TerminalMultiplexer::from_str("screen").unwrap(), TerminalMultiplexer::Screen);
        assert_eq!(TerminalMultiplexer::from_str("unknown").unwrap(), TerminalMultiplexer::Unknown);
        assert_eq!(TerminalMultiplexer::Tmux.as_str(), "tmux");
        assert!(TerminalMultiplexer::from_str("weird").is_err());
    }

    /// T03: TerminalCapabilityState from_str roundtrip.
    #[test]
    fn t03_capability_state_roundtrip() {
        assert_eq!(TerminalCapabilityState::from_str("unknown").unwrap(), TerminalCapabilityState::Unknown);
        assert_eq!(TerminalCapabilityState::from_str("supported").unwrap(), TerminalCapabilityState::Supported);
        assert_eq!(TerminalCapabilityState::from_str("unsupported").unwrap(), TerminalCapabilityState::Unsupported);
        assert_eq!(TerminalCapabilityState::Supported.as_str(), "supported");
        assert!(TerminalCapabilityState::from_str("maybe").is_err());
    }

    /// T04: CliRendererConfig default matches documented TS defaults.
    #[test]
    fn t04_cli_renderer_config_defaults() {
        let cfg = CliRendererConfig::default();
        assert_eq!(cfg.width, 80);
        assert_eq!(cfg.height, 24);
        assert!(!cfg.remote);
        assert_eq!(cfg.kitty_image_transport, KittyImageTransport::Raw);
        assert!(cfg.exit_on_ctrl_c);
        assert!(cfg.clear_on_shutdown);
        assert_eq!(cfg.debounce_delay, 100);
        assert_eq!(cfg.target_fps, 30);
        assert_eq!(cfg.max_fps, 60);
        assert_eq!(cfg.memory_snapshot_interval, 0);
        assert!(!cfg.use_thread);
        assert!(!cfg.gather_stats);
        assert_eq!(cfg.max_stat_samples, 300);
        assert!(cfg.enable_mouse_movement);
        assert!(cfg.use_mouse);
        assert!(cfg.auto_focus);
        assert_eq!(cfg.screen_mode, ScreenMode::AlternateScreen);
        assert_eq!(cfg.footer_height, 12);
        assert_eq!(cfg.external_output_mode, ExternalOutputMode::Passthrough);
        assert_eq!(cfg.stdin_parser_max_buffer_bytes, 64 * 1024 * 1024);
    }

    /// T05: TerminalCapabilities default + new + field set.
    #[test]
    fn t05_terminal_capabilities_default_and_new() {
        let caps = TerminalCapabilities::default();
        assert!(!caps.kitty_keyboard);
        assert!(!caps.rgb);
        assert_eq!(caps.unicode, WidthMethod::Unicode);
        assert_eq!(caps.osc52_support, TerminalCapabilityState::Unknown);
        assert_eq!(caps.multiplexer, TerminalMultiplexer::None);
        assert!(caps.image_protocol.is_none());
        assert_eq!(caps.terminal.name, "");

        let mut caps = TerminalCapabilities::new(TerminalInfo {
            name: "xterm".into(),
            version: "360".into(),
            from_xtversion: false,
        });
        caps.kitty_keyboard = true;
        caps.rgb = true;
        caps.image_protocol = Some(ImageRenderProtocol::Kitty);
        assert!(caps.kitty_keyboard);
        assert!(caps.rgb);
        assert_eq!(caps.terminal.name, "xterm");
        assert_eq!(caps.image_protocol, Some(ImageRenderProtocol::Kitty));
    }

    /// T06: CliRenderEvent construction and variant matching.
    #[test]
    fn t06_cli_render_event_variants() {
        let resize = CliRenderEvent::Resize { width: 80, height: 24, render_height: 24 };
        assert!(matches!(resize, CliRenderEvent::Resize { width: 80, .. }));
        let frame = CliRenderEvent::Frame { frame_id: 42 };
        assert!(matches!(frame, CliRenderEvent::Frame { frame_id: 42 }));
        let err = CliRenderEvent::RenderError { error: "boom".into() };
        assert!(matches!(err, CliRenderEvent::RenderError { error: _ }));
        let focus = CliRenderEvent::Focus;
        assert!(matches!(focus, CliRenderEvent::Focus));
        let blur = CliRenderEvent::Blur;
        assert!(matches!(blur, CliRenderEvent::Blur));
        let themed = CliRenderEvent::ThemeMode { mode: ThemeMode::Dark };
        assert!(matches!(themed, CliRenderEvent::ThemeMode { mode: ThemeMode::Dark }));
        let caps = CliRenderEvent::Capabilities(TerminalCapabilities::default());
        assert!(matches!(caps, CliRenderEvent::Capabilities(_)));
        let mem = CliRenderEvent::MemorySnapshot { heap_used: 100, heap_total: 200, array_buffers: 50 };
        assert!(matches!(mem, CliRenderEvent::MemorySnapshot { heap_used: 100, .. }));
    }

    /// T07: RenderGeometry + calculate_render_geometry mirrors TS logic.
    #[test]
    fn t07_render_geometry_calculation() {
        // Non-split-footer: zero footer, offset 0, full dims.
        let geom = calculate_render_geometry(
            RenderGeometryScreenMode::AlternateScreen, 100, 40, 12,
        );
        assert_eq!(geom.effective_footer_height, 0);
        assert_eq!(geom.render_offset, 0);
        assert_eq!(geom.render_width, 100);
        assert_eq!(geom.render_height, 40);

        // Split-footer with footer 12.
        let geom = calculate_render_geometry(
            RenderGeometryScreenMode::SplitFooter, 100, 40, 12,
        );
        assert_eq!(geom.effective_footer_height, 12);
        assert_eq!(geom.render_offset, 28); // 40 - 12
        assert_eq!(geom.render_width, 100);
        assert_eq!(geom.render_height, 12);

        // Split-footer where footer exceeds terminal height.
        let geom = calculate_render_geometry(
            RenderGeometryScreenMode::SplitFooter, 100, 10, 50,
        );
        assert_eq!(geom.effective_footer_height, 10); // min(50, 10)
        assert_eq!(geom.render_height, 10);
        assert_eq!(geom.render_offset, 0); // 10 - 10
    }

    /// T08: ScreenMode, ExternalOutputMode, ConsoleMode FromStr roundtrips.
    #[test]
    fn t08_mode_strings_roundtrip() {
        assert_eq!(ScreenMode::from_str("alternate-screen").unwrap(), ScreenMode::AlternateScreen);
        assert_eq!(ScreenMode::from_str("main-screen").unwrap(), ScreenMode::MainScreen);
        assert_eq!(ScreenMode::from_str("split-footer").unwrap(), ScreenMode::SplitFooter);
        assert!(ScreenMode::from_str("bad").is_err());

        assert_eq!(ExternalOutputMode::from_str("capture-stdout").unwrap(), ExternalOutputMode::CaptureStdout);
        assert_eq!(ExternalOutputMode::from_str("passthrough").unwrap(), ExternalOutputMode::Passthrough);
        assert!(ExternalOutputMode::from_str("nope").is_err());

        assert_eq!(ConsoleMode::from_str("console-overlay").unwrap(), ConsoleMode::ConsoleOverlay);
        assert_eq!(ConsoleMode::from_str("disabled").unwrap(), ConsoleMode::Disabled);
        assert!(ConsoleMode::from_str("x").is_err());

        assert_eq!(KittyImageTransport::from_str("zlib").unwrap(), KittyImageTransport::Zlib);
        assert_eq!(KittyImageTransport::from_str("file").unwrap(), KittyImageTransport::File);
        assert!(KittyImageTransport::from_str("nope").is_err());

        assert_eq!(ThemeMode::from_str("dark").unwrap(), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_str("light").unwrap(), ThemeMode::Light);
        assert!(ThemeMode::from_str("red").is_err());
    }

    /// T09: RenderContext trait implementable by a struct.
    #[test]
    fn t09_render_context_trait_usable() {
        struct TestCtx {
            w: u16,
            h: u16,
            frame: u64,
            caps: Option<TerminalCapabilities>,
        }
        impl RenderContext for TestCtx {
            fn add_to_hit_grid(&mut self, _x: u16, _y: u16, _w: u16, _h: u16, _id: u32) {}
            fn push_hit_grid_scissor_rect(&mut self, _x: u16, _y: u16, _w: u16, _h: u16) {}
            fn pop_hit_grid_scissor_rect(&mut self) {}
            fn clear_hit_grid_scissor_rects(&mut self) {}
            fn width(&self) -> u16 { self.w }
            fn height(&self) -> u16 { self.h }
            fn set_width_height(&mut self, w: u16, h: u16) { self.w = w; self.h = h; }
            fn terminal_width(&self) -> Option<u16> { Some(self.w) }
            fn terminal_height(&self) -> Option<u16> { Some(self.h) }
            fn resolution(&self) -> Option<(u16, u16)> { Some((self.w, self.h)) }
            fn frame_id(&self) -> u64 { self.frame }
            fn request_render(&mut self) { self.frame += 1; }
            fn set_cursor_position(&mut self, _x: u16, _y: u16, _v: bool) {}
            fn set_cursor_style(&mut self, _opts: &CursorStyleOptions) {}
            fn set_cursor_color(&mut self, _color: RgbaColor) {}
            fn set_mouse_pointer(&mut self, _shape: &str) {}
            fn width_method(&self) -> WidthMethod { WidthMethod::Wcwidth }
            fn capabilities(&self) -> Option<&TerminalCapabilities> { self.caps.as_ref() }
            fn request_live(&mut self) {}
            fn drop_live(&mut self) {}
        }
        let mut ctx = TestCtx {
            w: 80,
            h: 24,
            frame: 0,
            caps: Some(TerminalCapabilities::default()),
        };
        assert_eq!(ctx.width(), 80);
        assert_eq!(ctx.height(), 24);
        assert_eq!(ctx.frame_id(), 0);
        ctx.request_render();
        assert_eq!(ctx.frame_id(), 1);
        ctx.set_width_height(120, 40);
        assert_eq!(ctx.width(), 120);
        assert_eq!(ctx.height(), 40);
        assert!(ctx.capabilities().is_some());
        assert_eq!(ctx.width_method(), WidthMethod::Wcwidth);
    }

    /// T10: MousePointerStyle as_str covers all variants.
    #[test]
    fn t10_mouse_pointer_as_str() {
        assert_eq!(MousePointerStyle::Pointer.as_str(), "pointer");
        assert_eq!(MousePointerStyle::NwseResize.as_str(), "nwse-resize");
        assert_eq!(MousePointerStyle::ZoomIn.as_str(), "zoom-in");
        assert_eq!(MousePointerStyle::Default.as_str(), "default");
    }
}

