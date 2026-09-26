# BRIDGE-GAP-15 terminal_mode.rs

- Claim: `BRIDGE-GAP-15`
- Session: `ses_f28c0def2ffeSBWRgUUiwsnX90`
- Owned file: `crates/opentui-bridge/src/terminal_mode.rs`
- Status: in progress
- Scope: std-only terminal theme color math. No `lib.rs` or theme edits.

## Source evidence

- Upstream checkout `/home/rashid/projects/opencode`, commit `a0d9b6c`, `packages/tui/src/theme/index.ts:353-358`: `terminalMode` returns undefined without `defaultBackground`; normalized luminance `0.299*r + 0.587*g + 0.114*b > 0.5` selects light, otherwise dark.
- `packages/tui/src/theme/index.ts:360-395`: system theme uses `isDark = mode == "dark"`, arbitrary-background gray/muted helpers, and diff alpha `0.22` dark, `0.14` light.
- `packages/tui/src/theme/index.ts:471-523`: gray ramp extreme branches plus arbitrary-background ratio path; TS floors output bytes.
- `packages/tui/src/theme/index.ts:525-553`: muted text dark/light branches, with mid-range scaling.
- Existing `crates/opentui-bridge/src/theme_registry.rs:44-80`: `tint` and extreme-background `generate_gray_scale` / `muted_text_color`; this file complements, not duplicates, its documented arbitrary-bg gap.
- Existing `crates/opentui-bridge/src/syntax_style.rs:215-220`: local luminance calculation uses byte-scaled threshold; API contract here returns an explicit optional mode bool.

## Observable contract

- `luminance(r,g,b)` returns normalized `[0,1]` std float math.
- `terminal_mode(None)` returns `None`; present black/white/mid RGB values return deterministic mode.
- `gray_scale(bg, ratio)` implements the arbitrary-background ratio branch with bounded/floored RGB output.
- `muted_text(is_dark)` returns mid scaling from the same TS formulas and endpoint values.
- `diff_alpha(is_dark)` returns `0.22` dark, `0.14` light.

## Tests

In-file tests cover absent background, black, white, mid gray, arbitrary gray ratio, muted dark/light, and diff alpha. No cargo available in lane; direct `rustc --test` if practical.

## Decisions

- Use `Option<bool>` mode where `Some(true)` means dark, matching TS `mode == "dark"` and `isDark`; None means undefined.
- Use `f32` std-only math. Byte outputs floor like TS; all ratios are constrained to valid RGB bytes.
- Keep helper names explicit and public for later integration; integrator may prewire `lib.rs` separately.

## Verification

- `rtk rustfmt --edition 2021 --check crates/opentui-bridge/src/terminal_mode.rs`: pass.
- `rtk rustc --edition=2021 --test crates/opentui-bridge/src/terminal_mode.rs -o /home/rashid/.cache/bun-tmp/opencode/terminal_mode_tests && /home/rashid/.cache/bun-tmp/opencode/terminal_mode_tests`: 6 passed, 0 failed.
- Implementation SHA-256: `3255ae7488c7cd6e27d4885efc3ac4cea998af976b57c73d7879ab6f3d3d7a40`.
- Cargo not run per lane constraint. `lib.rs` intentionally untouched; integrator wiring remains separate.
