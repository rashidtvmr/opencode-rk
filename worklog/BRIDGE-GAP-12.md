# BRIDGE-GAP-12 — render_focus.rs

Claim: ledger `BRIDGE-GAP-12` held by `ses_gap12` (in-progress, pre-existing claim reused).
Owned file: `crates/opentui-bridge/src/render_focus.rs` (new). No edits to `renderables.rs` or `lib.rs`.

Source evidence:
- `crates/opentui-bridge/src/renderables.rs:31-33` — homeless fields `focusedTextColor`/`textColor` (`ui/dialog-select.tsx:580,582`), `minHeight`/`maxHeight`, `height` (`ui/dialog-export-options.tsx:108`).
- `crates/opentui-bridge/src/renderables.rs:42-45,175-181` — `Focusable { focusable, focused }`, `focusedBackgroundColor` (`ui/dialog-select.tsx:580`).
- `crates/opentui-bridge/src/renderables.rs:294-305` — `InputStyle` base colors (`placeholder_color`/`text_color`/`cursor_color`), no focused overrides.
- `crates/opentui-bridge/src/color.rs:137-152` — `Rgba { r,g,b,a }`, `const fn new/rgb/transparent`, `Copy`.

Target boundary: `FocusedStyle { focused_text_color, focused_background_color: Option<Rgba>, height: Option<u16> }` + `apply(base_text, base_bg, focused) -> (Rgba,Rgba)` + `effective_height(default)` helper for `height: None` case.

Tests (in-file, 4): unfocused passthrough, focused override, partial None fallback, height none/default.
Decisions: `apply` is `const fn`; `None` = keep base; height resolved via `effective_height` (task lists `height: Option<u16>` field; resolution helper keeps `apply` signature exact).
Remaining: `cargo test -p opentui-bridge render_focus` NOT run (role has no cargo); needs verifier run. `lib.rs` `pub mod render_focus;` wiring left to orchestrator (shared file, out of lane authority).

## Final verification (2026-09-25 lane close)

- `rustfmt --edition 2021 --check crates/opentui-bridge/src/render_focus.rs` clean, no changes needed.
- In-file tests: 4 (`unfocused_passthrough`, `focused_override`, `partial_none_falls_back_to_base`, `height_none_uses_default`). Impl + tests present, no cargo run per role (no cargo).
- Status: done, ledger flip to completed.
