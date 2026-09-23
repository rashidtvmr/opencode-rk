#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Terminal capability detection bridge.
//!
//! Mirrors the upstream TypeScript capability detection in
//! `packages/core/src/lib/terminal-capability-detection.ts`,
//! `packages/core/src/lib/terminal-palette.ts`,
//! `packages/core/src/renderer-theme-mode.ts`, and
//! `packages/core/src/lib/RGBA.ts`.
//!
//! All timeouts use an injectable millisecond clock -- no wall-clock calls.

use crate::color::{Rgba, ansi256_index_to_rgb};

/// Default capability probe timeout: 5000 ms (matches renderer.ts line 3312).
pub const TIMEOUT: u64 = 5000;

// ---------------------------------------------------------------------------
// TerminalPalette normalization
// ---------------------------------------------------------------------------

/// A normalized RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<Rgba> for NormalizedColor {
    fn from(c: Rgba) -> Self {
        NormalizedColor {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }
    }
}

/// Normalized terminal palette: 256-entry palette plus default fg/bg.
///
/// Mirrors `normalizeTerminalPalette` in terminal-palette.ts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTerminalPalette {
    pub palette: Vec<NormalizedColor>,
    pub default_foreground: NormalizedColor,
    pub default_background: NormalizedColor,
}

/// Terminal color detection result (mirrors TS `TerminalColors`).
/// `None` entries mean the color was not detected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalColors {
    pub palette: Vec<Option<String>>,
    pub default_foreground: Option<String>,
    pub default_background: Option<String>,
}

/// `TerminalPalette` normalizes detected terminal colors to RGBA,
/// falling back to ANSI-256 palette and default fg/bg when undetected.
///
/// Mirrors `normalizeTerminalPalette` in terminal-palette.ts.
#[derive(Debug, Clone)]
pub struct TerminalPalette;

fn ansi256_palette() -> Vec<Rgba> {
    (0..=255u8)
        .map(|i| {
            let (r, g, b) = ansi256_index_to_rgb(i);
            Rgba::rgb(r, g, b)
        })
        .collect()
}

fn default_fg() -> Rgba {
    Rgba::rgb(255, 255, 255)
}

fn default_bg() -> Rgba {
    Rgba::rgb(0, 0, 0)
}

impl TerminalPalette {
    /// Normalize detected terminal colors. If `colors` is `None` or entries
    /// are missing, fall back to ANSI-256 palette and default fg/bg.
    pub fn normalize(colors: Option<&TerminalColors>) -> NormalizedTerminalPalette {
        let fallback_palette = ansi256_palette();

        let palette = (0..256)
            .map(|i| {
                let detected = colors
                    .and_then(|c| c.palette.get(i))
                    .and_then(|h| h.as_deref())
                    .and_then(Rgba::from_hex);
                match detected {
                    Some(c) => NormalizedColor::from(c),
                    None => NormalizedColor::from(fallback_palette[i]),
                }
            })
            .collect();

        let fg = colors
            .and_then(|c| c.default_foreground.as_deref())
            .and_then(Rgba::from_hex);
        let bg = colors
            .and_then(|c| c.default_background.as_deref())
            .and_then(Rgba::from_hex);

        NormalizedTerminalPalette {
            palette,
            default_foreground: NormalizedColor::from(fg.unwrap_or_else(default_fg)),
            default_background: NormalizedColor::from(bg.unwrap_or_else(default_bg)),
        }
    }
}

// ---------------------------------------------------------------------------
// Capability probe state machine
// ---------------------------------------------------------------------------

/// State of the capability probe lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    /// Probe not started or reset.
    Idle,
    /// Probe sent, waiting for responses (within timeout).
    Pending,
    /// Probe completed successfully before the timeout.
    Resolved,
    /// Probe exceeded `TIMEOUT` without resolution.
    Timeout,
}

/// Result of processing a capability response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub state: CapabilityState,
    pub da1: bool,
    pub decrpm: bool,
    pub xtversion: bool,
    pub kitty_keyboard: bool,
    pub pixel_resolution: Option<(u32, u32)>,
    pub consumed: bool,
}

impl ProcessResult {
    fn idle_or(state: CapabilityState) -> Self {
        ProcessResult {
            state,
            da1: false,
            decrpm: false,
            xtversion: false,
            kitty_keyboard: false,
            pixel_resolution: None,
            consumed: false,
        }
    }
}

/// Capability probe: tracks terminal capability responses within a timeout.
///
/// Mirrors the 5-second capability timeout in renderer.ts (line 3294-3312).
/// All time is injected as milliseconds since probe start -- no wall-clock access.
#[derive(Debug)]
pub struct CapabilityProbe {
    state: CapabilityState,
    deadline_ms: u64,
}

impl Default for CapabilityProbe {
    fn default() -> Self {
        CapabilityProbe::new(TIMEOUT)
    }
}

impl CapabilityProbe {
    /// Create a probe with a custom timeout (in ms).
    pub fn new(timeout_ms: u64) -> Self {
        CapabilityProbe {
            state: CapabilityState::Idle,
            deadline_ms: timeout_ms,
        }
    }

    /// Start the probe at `start_ms` (absolute time, injected by caller).
    pub fn start(&mut self, start_ms: u64) -> CapabilityState {
        self.state = CapabilityState::Pending;
        self.deadline_ms = start_ms + TIMEOUT;
        self.state
    }

    /// Current state.
    pub fn state(&self) -> CapabilityState {
        self.state
    }

    /// Check if the probe has timed out at `now_ms`.
    /// If `now_ms >= deadline_ms`, transition to `Timeout`.
    pub fn check_timeout(&mut self, now_ms: u64) -> CapabilityState {
        if self.state == CapabilityState::Pending && now_ms >= self.deadline_ms {
            self.state = CapabilityState::Timeout;
        }
        self.state
    }

    /// Process a capability response sequence at `now_ms`.
    ///
    /// If the probe is `Pending` and `now_ms < deadline_ms`, the response
    /// is parsed and the probe transitions to `Resolved`.
    ///
    /// Returns details about what was detected.
    pub fn process_response(&mut self, response: &str, now_ms: u64) -> ProcessResult {
        if self.state == CapabilityState::Pending && now_ms >= self.deadline_ms {
            self.state = CapabilityState::Timeout;
            return ProcessResult::idle_or(CapabilityState::Timeout);
        }

        if self.state != CapabilityState::Pending {
            return ProcessResult::idle_or(self.state);
        }

        let da1 = is_da1(response);
        let decrpm = is_decrpm(response);
        let xtversion = is_xtversion(response);
        let kitty_keyboard = is_kitty_keyboard(response);
        let pixel_resolution = parse_pixel_resolution(response);
        let consumed = da1 || decrpm || xtversion || kitty_keyboard || pixel_resolution.is_some();

        if consumed {
            self.state = CapabilityState::Resolved;
        }

        ProcessResult {
            state: self.state,
            da1,
            decrpm,
            xtversion,
            kitty_keyboard,
            pixel_resolution,
            consumed,
        }
    }
}

// ---------------------------------------------------------------------------
// Response parsers (mirror terminal-capability-detection.ts regex tests)
// ---------------------------------------------------------------------------

/// DA1 (Device Attributes): `\x1b[?...c`
fn is_da1(seq: &str) -> bool {
    if !seq.starts_with("\x1b[?") {
        return false;
    }
    let rest = &seq[3..];
    if !rest.ends_with('c') {
        return false;
    }
    let digits = rest.trim_end_matches('c');
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == ';')
}

/// DECRPM (DEC Request Mode reply): `\x1b[?...;N$y`
fn is_decrpm(seq: &str) -> bool {
    if !seq.starts_with("\x1b[?") {
        return false;
    }
    let rest = &seq[3..];
    if !rest.ends_with("$y") {
        return false;
    }
    let body = rest.trim_end_matches("$y");
    let parts: Vec<&str> = body.split(';').collect();
    if parts.is_empty() {
        return false;
    }
    parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// XTVersion: `\x1bP>|...ESC \`
fn is_xtversion(seq: &str) -> bool {
    if !seq.starts_with("\x1bP>|") {
        return false;
    }
    if !seq.ends_with("\x1b\\") {
        return false;
    }
    let inner = &seq[4..seq.len() - 2];
    !inner.is_empty()
}

/// Kitty keyboard query response: `\x1b[?Nu` or `\x1b[?N;Mu`
fn is_kitty_keyboard(seq: &str) -> bool {
    if !seq.starts_with("\x1b[?") {
        return false;
    }
    let rest = &seq[3..];
    if rest.ends_with("u") {
        let body = rest.trim_end_matches('u');
        let parts: Vec<&str> = body.split(';').collect();
        !parts.is_empty()
            && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    } else if rest.ends_with(";u") {
        let body = rest.trim_end_matches(";u");
        body.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Pixel resolution response: `\x1b[4;height;widtht`
/// Returns `Some((width, height))`.
pub fn parse_pixel_resolution(seq: &str) -> Option<(u32, u32)> {
    if !seq.starts_with("\x1b[4;") || !seq.ends_with('t') {
        return None;
    }
    let inner = &seq[4..seq.len() - 1];
    let parts: Vec<&str> = inner.split(';').collect();
    if parts.len() != 2 {
        return None;
    }
    let height = parts[0].parse::<u32>().ok()?;
    let width = parts[1].parse::<u32>().ok()?;
    if width > 0x7fffffff || height > 0x7fffffff {
        return None;
    }
    Some((width, height))
}

// ---------------------------------------------------------------------------
// Theme mode inference
// ---------------------------------------------------------------------------

/// Theme mode: dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::Dark
    }
}

/// Infer theme mode from a background color using the ITU-R BT.601 luma formula.
/// brightness = (r*299 + g*587 + b*114) / 1000
/// brightness > 128 => light, else dark.
///
/// Mirrors `inferThemeModeFromBackgroundColor` in renderer-theme-mode.ts.
pub fn infer_theme_mode(color: &Rgba) -> ThemeMode {
    let brightness = (color.r as u32 * 299 + color.g as u32 * 587 + color.b as u32 * 114) / 1000;
    if brightness > 128 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- DA1 / DECRPM parsing ---

    #[test]
    fn parse_da1_response() {
        assert!(is_da1("\x1b[?62;c"));
        assert!(is_da1("\x1b[?62;22c"));
        assert!(is_da1("\x1b[?1;2;4c"));
        assert!(is_da1("\x1b[?6c"));
        assert!(!is_da1("\x1b[10;5R"));
        assert!(!is_da1("\x1b[?62;2$y"));
        assert!(!is_da1("a"));
    }

    #[test]
    fn parse_decrpm_response() {
        assert!(is_decrpm("\x1b[?1016;2$y"));
        assert!(is_decrpm("\x1b[?2027;0$y"));
        assert!(is_decrpm("\x1b[?1004;1$y"));
        assert!(!is_decrpm("\x1b[?62;c"));
        assert!(!is_decrpm("\x1b[A"));
    }

    #[test]
    fn parse_pixel_resolution_response() {
        assert_eq!(parse_pixel_resolution("\x1b[4;720;1280t"), Some((1280, 720)));
        assert_eq!(parse_pixel_resolution("\x1b[4;1080;1920t"), Some((1920, 1080)));
        assert_eq!(parse_pixel_resolution("\x1b[4;0;0t"), Some((0, 0)));
        assert_eq!(parse_pixel_resolution("a"), None);
        assert_eq!(parse_pixel_resolution("\x1b[A"), None);
        assert_eq!(parse_pixel_resolution("\x1b[4;4294967295;4294967295t"), None);
    }

    // --- Timeout / resolution ---

    #[test]
    fn timeout_pending_to_timeout() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        let start = 1000u64;
        assert_eq!(probe.start(start), CapabilityState::Pending);
        // Before deadline: still pending
        assert_eq!(probe.check_timeout(start + TIMEOUT - 1), CapabilityState::Pending);
        // At deadline: timeout
        assert_eq!(probe.check_timeout(start + TIMEOUT), CapabilityState::Timeout);
        assert_eq!(probe.state(), CapabilityState::Timeout);
    }

    #[test]
    fn pending_to_resolved_on_response() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        let start = 0u64;
        probe.start(start);
        let result = probe.process_response("\x1b[?62;c", start + 100);
        assert_eq!(result.state, CapabilityState::Resolved);
        assert!(result.da1);
        assert!(result.consumed);
        assert_eq!(probe.state(), CapabilityState::Resolved);
    }

    #[test]
    fn response_after_timeout_transitions_to_timeout() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        let start = 0u64;
        probe.start(start);
        let result = probe.process_response("\x1b[?62;c", start + TIMEOUT + 1);
        assert_eq!(result.state, CapabilityState::Timeout);
        assert!(!result.consumed);
    }

    // --- Theme mode ---

    #[test]
    fn theme_mode_inference_brightness() {
        // White background => light
        let white = Rgba::rgb(255, 255, 255);
        assert_eq!(infer_theme_mode(&white), ThemeMode::Light);
        // Black background => dark
        let black = Rgba::rgb(0, 0, 0);
        assert_eq!(infer_theme_mode(&black), ThemeMode::Dark);
        // Mid-gray (128,128,128) => brightness = 128, not > 128 => dark
        let gray = Rgba::rgb(128, 128, 128);
        assert_eq!(infer_theme_mode(&gray), ThemeMode::Dark);
        // Brightness 129 => light
        let near_white = Rgba::rgb(129, 129, 129);
        assert_eq!(infer_theme_mode(&near_white), ThemeMode::Light);
    }

    // --- Palette normalization ---

    #[test]
    fn palette_normalize_fallback() {
        let result = TerminalPalette::normalize(None);
        assert_eq!(result.palette.len(), 256);
        // Fallback: index 0 is black
        assert_eq!(result.palette[0], NormalizedColor::from(Rgba::rgb(0, 0, 0)));
        // Fallback: index 15 is white
        assert_eq!(result.palette[15], NormalizedColor::from(Rgba::rgb(255, 255, 255)));
        // Default fg is white, bg is black
        assert_eq!(result.default_foreground, NormalizedColor::from(Rgba::rgb(255, 255, 255)));
        assert_eq!(result.default_background, NormalizedColor::from(Rgba::rgb(0, 0, 0)));
    }

    #[test]
    fn palette_normalize_with_detected_colors() {
        let colors = TerminalColors {
            palette: vec![Some("#ff0000".to_string()); 256],
            default_foreground: Some("#00ff00".to_string()),
            default_background: Some("#0000ff".to_string()),
        };
        let result = TerminalPalette::normalize(Some(&colors));
        // All palette entries are red
        assert_eq!(result.palette[0], NormalizedColor::from(Rgba::rgb(255, 0, 0)));
        assert_eq!(result.palette[255], NormalizedColor::from(Rgba::rgb(255, 0, 0)));
        // Detected fg/bg
        assert_eq!(result.default_foreground, NormalizedColor::from(Rgba::rgb(0, 255, 0)));
        assert_eq!(result.default_background, NormalizedColor::from(Rgba::rgb(0, 0, 255)));
    }

    #[test]
    fn palette_normalize_partial_detection() {
        let mut palette = vec![None; 256];
        palette[0] = Some("#ff0000".to_string());
        let colors = TerminalColors {
            palette,
            default_foreground: None,
            default_background: None,
        };
        let result = TerminalPalette::normalize(Some(&colors));
        // Index 0 detected
        assert_eq!(result.palette[0], NormalizedColor::from(Rgba::rgb(255, 0, 0)));
        // Index 1 falls back to ANSI-256 index 1
        assert_eq!(result.palette[1], NormalizedColor::from(Rgba::from_ansi256(1)));
    }

    // --- ProcessResult integration ---

    #[test]
    fn process_response_decrpm() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        probe.start(0);
        let result = probe.process_response("\x1b[?1016;2$y", 100);
        assert_eq!(result.state, CapabilityState::Resolved);
        assert!(result.decrpm);
        assert!(result.consumed);
    }

    #[test]
    fn process_response_pixel_resolution() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        probe.start(0);
        let result = probe.process_response("\x1b[4;720;1280t", 100);
        assert_eq!(result.state, CapabilityState::Resolved);
        assert_eq!(result.pixel_resolution, Some((1280, 720)));
        assert!(result.consumed);
    }

    #[test]
    fn process_response_non_capability_not_consumed() {
        let mut probe = CapabilityProbe::new(TIMEOUT);
        probe.start(0);
        let result = probe.process_response("a", 100);
        assert_eq!(result.state, CapabilityState::Pending);
        assert!(!result.consumed);
    }
}