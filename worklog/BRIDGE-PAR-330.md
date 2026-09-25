# BRIDGE-PAR-330 startup_loading_full

- Claim: FAILED via API (`claims must be a bounded mapping`; ledger has 501 rows > MAX_ROWS 500). Task `BRIDGE-PAR-330` absent from ledger (no foreign owner). Wrote owned file anyway, deviation noted.
- Source evidence: `/home/rashid/projects/opencode/packages/tui/src/component/startup-loading.tsx:8` (`props.ready() ? "Finishing startup..." : "Loading plugins..."`); style ref `crates/opentui-bridge/src/collapse.rs` (forbid unsafe, in-file tests).
- Target boundary: ONE new file `crates/opentui-bridge/src/startup_loading_full.rs`. No lib.rs/Cargo.toml edits, no cargo, no commit.
- API: `StartupLoad { msg: String cap 256 chars, done: bool }` + `new()`/`set_msg`/`finish`/`line` + Default. `line()` returns `"ready"` when done else msg.
- Tests: 4 in-file (default empty, roundtrip, 256-char cap, finish->ready).
- Decisions: char (not byte) truncation as edge-case-correct; literal `"ready"` per task spec (TS finishing text differs, noted).
- Unknowns: ledger infra bug blocks all claims until rows pruned or MAX_ROWS raised.
