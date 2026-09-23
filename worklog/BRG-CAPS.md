# BRG-CAPS: Terminal capability detection bridge

## Claim
- Claimed via completion_claims.py, task BRG-CAPS, session ses_brg_caps

## Source evidence
- packages/core/src/lib/terminal-capability-detection.ts:
  - isCapabilityResponse: DECRPM `\x1b[?...;...$y`, CPR width `\x1b[1;NR (N>=2)`, XTVersion `\x1bP>|...ESC \`, XTGETTCAP Ms, Kitty graphics `\x1b_G...ESC \`, Kitty keyboard `\x1b[?Nu`, DA1 `\x1b[?...c`, OSC 99/1337
  - isPixelResolutionResponse: `\x1b[4;h;wt`
  - parsePixelResolution: returns {width, height}
- packages/core/src/lib/terminal-palette.ts:
  - normalizeTerminalPalette: maps detected hex palette + fg/bg to RGBA[], falls back to ANSI-256
  - TerminalColors interface: palette: Hex[], defaultForeground, defaultBackground, cursorColor, mouseForeground, mouseBackground, tekForeground, tekBackground, highlightBackground, highlightForeground
- packages/core/src/lib/RGBA.ts:
  - RGBA class, fromHex, toInts, DEFAULT_FOREGROUND_RGB=[255,255,255], DEFAULT_BACKGROUND_RGB=[0,0,0]
  - ansi256IndexToRgb: ANSI16 + 6x6x6 cube + grayscale
- packages/core/src/renderer-theme-mode.ts:
  - handleSequence: `\x1b[?997;1n` or `\x1b[?997;2n` triggers query
  - inferThemeModeFromBackgroundColor: brightness = (r*299 + g*587 + b*114) / 1000, >128 => light else dark
  - ThemeMode = "dark" | "light"
- packages/core/src/types.ts:
  - ThemeMode = "dark" | "light"
- crates/opentui-bridge/src/color.rs (existing pattern):
  - Rgba {r,g,b,a:u8}, RgbaU16, pack_meta, pack_rgba8, INTENT_RGB/INDEXED/DEFAULT, ansi256_index_to_rgb
  - Default ThemeMode in crates/foundation/src/config.rs: Dark/Light/System

## Target boundary
Owned file: crates/opentui-bridge/src/capabilities.rs

## Markers required
- pub struct CapabilityProbe
- pub enum CapabilityState
- pub fn process_response
- pub struct TerminalPalette
- TIMEOUT

## Constraints
- Rust 2021 edition, std-only
- No wall-clock: inject tick/ms via traits or function params
- forbid(unsafe_code)
- Min 100 lines, >=6 tests
- TDD: tests RED first, then implement

## Design
1. CapabilityProbe: tracks pending capability responses, 5s timeout (injectable ms)
   - process_response(&mut self, response: &str, now_ms: u64) -> ProcessResult
   - State machine: Pending -> Resolved / Timeout
   - Detects DA1 `\x1b[?...c`, DECRPM `\x1b[?...;...$y`, XTVersion, KITTY_KBD, pixel resolution
2. TerminalPalette: normalize 256-color palette + fg/bg
   - normalize(&self, palette: Option<&[Option<String>]>, fg: Option<&str>, bg: Option<&str>) -> NormalizedPalette
   - Fallback to ANSI256
3. Theme mode: infer from bg brightness (same formula as TS)
4. Pixel resolution: parse `\x1b[4;h;wt`

## Tests (RED -> GREEN)
1. parse DA1 response
2. parse DECRPM response
3. parse pixel resolution response
4. timeout pending->timeout (inject ms)
5. resolve pending->resolved (inject ms)
6. theme mode inference from brightness
7. palette normalize with fallback
8. palette normalize with detected colors
