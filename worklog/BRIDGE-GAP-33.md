# BRIDGE-GAP-33

## Claim
- Task: BRIDGE-GAP-33
- Session: `ses_gap33`
- Scope: `crates/opentui-bridge/src/layout_calls.rs` only, plus this scratchpad and ledger row.

## Source evidence
- `crates/opentui-bridge/src/layout.rs:9-45`: `Rect` is Copy, exposes `is_empty`, and fail-closed zero-area behavior.
- `crates/opentui-bridge/src/layout.rs:242-266`: `split_row` and `split_col` are bounded pure callers returning `Result<Vec<Rect>, LayoutError>`.
- `crates/opentui-bridge/src/frame_layout.rs:14-50`: `shell_layout` owns the canonical terminal shell geometry, including compact/narrow sidebar collapse and zero-size empty regions.

## Target boundary
- `chat_regions(cols, rows)` exposes transcript, composer, and status rects from `shell_layout(..., false)`.
- `sidebar_regions(cols, rows)` returns `Some` only for a non-empty sidebar from `shell_layout(..., true)`.
- `palette_popup(cols, rows)` clamps a 60x12 rect to the viewport, centers with floor division, and uses fixed row/column splits.
- Zero viewport returns empty rects; malformed split results fail closed.

## Tests
In-file tests cover chat row-height sum, zero-size closure, narrow sidebar absence, popup clamping, and popup centering. An odd-dimension centering case guards rounding.

## Decisions
- Reuse `shell_layout` for canonical shell policy rather than duplicate thresholds.
- Use `split_row` and `split_col` for popup padding, with all fixed, bounded dimensions.
- No `lib.rs`, `Cargo.toml`, cargo command, commit, or push changes.

## Remaining unknowns
- Integrator must add the module declaration in `lib.rs` if the bridge caller surface is needed.
