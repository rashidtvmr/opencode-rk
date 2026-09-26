# BRIDGE-GAP-17

- Claim: `ses_gap17`, `in-progress`.
- Evidence: `crates/opentui-bridge/src/layout.rs:9-45` defines `Rect`; `:58-68` defines sidebar sizing; `:242-266` defines `split_row`/`split_col`. `crates/opentui-bridge/src/world.rs:169-251` defines the destination `ShellRegions`; `crates/opentui-bridge/src/native_frame.rs:12-15,45-55` establishes zero-dimension fail-closed behavior.
- Target: one new `frame_layout.rs`; no wiring changes.
- Implementation: reserve status, split body/composer with `split_col`, split body into transcript/sidebar with `split_row` when full and requested; narrow/short layouts collapse sidebar and use compact composer.
- Tests: six unit tests cover full, compact width, zero dimensions, hidden sidebar, and disjoint full/compact regions.
- Verification pending: `rustfmt --check crates/opentui-bridge/src/frame_layout.rs`.
- Unknown: `frame_layout` is intentionally not added to `lib.rs`; integration belongs to the parent lane.
