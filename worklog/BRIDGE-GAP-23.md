# BRIDGE-GAP-23

- Claim: `BRIDGE-GAP-23`, session `ses_gap23`.
- Source evidence: `BRIDGE_MIGRATION_DETAIL.md:266-274` names the scrollback family, `RunScrollbackStream`, and `entryLook`; no vendored TS files exist in this checkout.
- Observed contract: retained rows are bounded at 2000, oldest rows evict, separator is `---`, freezing blocks writes, snapshots are independent copies.
- Owned path: `crates/opentui-bridge/src/run_scrollback.rs`; std-only; no `lib.rs` wiring or commit requested.
- Implementation: `Scrollback` exposes public `rows` and `frozen`, `push`, `commit`, `separator`, `freeze`, `snapshot`, `CAP`, and `MAX_ROWS`.
- Tests: five in-file tests cover empty state, eviction, separator, freeze, and clone independence.
- Verification pending: `rustfmt --edition 2021 --check`; standalone `rustc --test` (cargo/lib.rs out of scope).
- Unknowns: exact upstream TS stream argument types unavailable; compatibility is retained at the requested row-level boundary.
