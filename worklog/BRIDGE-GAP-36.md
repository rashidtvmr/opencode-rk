# BRIDGE-GAP-36

Claimed by `ses_gap36` for `ses_gap36` before product work.

Source evidence:
- `crates/opentui-bridge/src/widget_paint.rs:59-167` defines bounded `bar_calls`, `spark_calls`, `menu_calls`, and `card_calls` callers over `Rect` and return `Vec<PaintCall>`.
- `crates/opentui-bridge/src/widget_paint.rs:37-39` treats zero-width or zero-height rects as empty.
- `crates/opentui-bridge/src/widget_paint.rs:24-28` defines menu input as borrowed string slices plus selected index.
- `crates/opentui-bridge/src/widget_paint.rs:30-35` defines card input as borrowed title plus body row count.
- `crates/opentui-bridge/src/world.rs:158-163` defines `PaintCall` as a region and rect.

Observed scenario: typed widget callers need to adapt owned `WidgetKind` state to existing borrowed widget-paint APIs while enforcing the menu cap and empty-rect behavior.

Target boundary: one new `crates/opentui-bridge/src/widget_catalog.rs`; no `lib.rs` or `Cargo.toml` edits. `paint_widget` rejects empty rects, caps menu items at 32, and delegates each variant to the existing primitive.

Tests authored in the owned file:
- `bar_fill_delegates`
- `spark_peak_delegates`
- `menu_rows_delegate_and_cap`
- `card_clip_delegates`
- `empty_rect_paints_nothing`

Decisions: std-only; `#![forbid(unsafe_code)]`; no new dependencies. Owned `String`/`Vec` values are borrowed only for the call duration.

Verification: `rustfmt --check crates/opentui-bridge/src/widget_catalog.rs` (pending).

Remaining unknowns: module is intentionally not wired through `lib.rs` per scope; integration caller remains parent task responsibility.
