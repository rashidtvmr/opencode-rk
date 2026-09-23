# BRG-TYPES worklog

## Claim
- Task: BRG-TYPES
- Session: ses_brg_types
- Owned file: crates/opentui-bridge/src/render_context.rs
- Status: completed

## Verification
- Command: `rustc --edition 2021 --test crates/opentui-bridge/src/render_context.rs -o /tmp/opencode/brg_types_test && /tmp/opencode/brg_types_test`
- Result: 10/10 tests pass, 0 failures
- `#![forbid(unsafe_code)]` at line 1
- 898 lines (exceeds 100 minimum)
- 10 tests written (exceeds 6 minimum)

## Source evidence
- types.ts:105 - `WidthMethod = "wcwidth" | "unicode" | "unicode-wide"`
- types.ts:106 - `TerminalMultiplexer = "none" | "tmux" | "zij" | "screen" | "unknown"`
- types.ts:107 - `TerminalCapabilityState = "unknown" | "supported" | "unsupported"`
- types.ts:108 - `ImageRenderProtocol = "auto" | "kitty" | "sixel" | "blocks"`
- types.ts:110-114 - `TerminalInfo` { name, version, from_xtversion }
- types.ts:116-139 - `TerminalCapabilities` (13 bool + image_protocol option + terminal)
- types.ts:153-195 - `RenderContext` interface (extends EventEmitter)
- renderer.ts:122-233 - `CliRendererConfig` interface (30 fields)
- renderer.ts:750-767 - `CliRenderEvents` enum (13 variants)
- render-geometry.ts:1-9 - `RenderGeometry` { effectiveFooterHeight, renderOffset, renderWidth, renderHeight }
- render-geometry.ts:1 - `RenderGeometryScreenMode = "alternate-screen" | "main-screen" | "split-footer"`

## Target boundary
- Create render_context.rs with markers:
  - `pub struct CliRendererConfig`
  - `pub struct TerminalCapabilities`
  - `pub enum WidthMethod`
  - `pub trait RenderContext`
  - `pub enum CliRenderEvent`
- Std-only, forbid(unsafe_code), >=100 lines, >=6 tests
- RED via `rustc --edition 2021 --test ...`

## Decisions
- WidthMethod: enum with Wcwidth, Unicode, UnicodeWide variants + FromStr
- TerminalMultiplexer: enum None, Tmux, Zellij, Screen, Unknown
- TerminalCapabilityState: enum Unknown, Supported, Unsupported
- ImageRenderProtocol: enum Auto, Kitty, Sixel, Blocks (Option<>)
- CliRendererConfig: struct with all 30 fields as Option<> defaults where possible
- CliRenderEvent: enum mirroring CliRenderEvents values + payloads
- RenderContext: trait with methods from interface (width, height, capabilities, etc.)
- TerminalCapabilities: struct mirrored from TS interface

## Tests (RED phase)
- T01: WidthMethod from_str roundtrip
- T02: TerminalMultiplexer from_str roundtrip
- T03: TerminalCapabilityState from_str roundtrip
- T04: CliRendererConfig default + build
- T05: TerminalCapabilities default + update
- T06: CliRenderEvent construction/matching
