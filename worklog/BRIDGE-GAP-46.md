# BRIDGE-GAP-46 scratchpad (ses_gap46)

Claim: BRIDGE-GAP-46 via tools/completion_claims.py (ses_gap46).
Source evidence: crates/opentui-bridge/src/solid_host.rs:1-32 (reactivity stays TS; Rust dims/slot-id only); boundary names HostCaps/TerminalDimensions/RenderOptions/SlotId/TimeToFirstDraw.
Target boundary: ONE new file crates/opentui-bridge/src/solid_p0.rs; do NOT touch lib.rs/Cargo.toml/solid_host.rs.
Tests: in-file #[cfg(test)] 6 tests (signal_get_set, memo_recompute, effect_run, effect_dispose, batch_counter, stale_disposed_no_run).
Decisions: std-only Rc<RefCell>/Cell; thread_local batch depth counter; explicit Memo::recompute in creation order (topo note); Effect run_count + disposed guard; !Send documented not asserted.
Unknowns: ledger claim CLI output ambiguous ("claims: 0 total"); retry update completed with note.
