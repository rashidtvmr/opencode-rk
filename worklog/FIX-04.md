# FIX-04 scratchpad

- Claim: crates/opentui-bridge/src/caller_wire_full.rs
- Evidence:
  - render_loop_full.rs:26 `pub fn request`
  - page_router.rs:26 `pub fn show`
  - route_session.rs:39 `pub fn open`
  - run_footer.rs:78 `pub fn append`
  - run_stream.rs:45 `pub fn push`
  - scrollback_family.rs:91 `pub fn push` on ScrollbackWriter
  - sdk_stream.rs:27 `pub fn push` on SdkStream
  - theme_engine.rs:55 `pub fn apply`
  - plugin_runtime.rs:41 `pub fn register`
- Target: WireCall + WIRE_CALLS(9) + wire_count(), per-symbol CLI hook comment.
- Bounds: types only, std-only, forbid unsafe, <=90 lines, >=3 tests.
- Verification: rustfmt --check only (no cargo, no commit per lane).
- Status: written,未verified (rustfmt run by verifier/parent).
