# BRIDGE-PAR-122 scratchpad

Claim: BRIDGE-PAR-122, session ses_par122, scratchpad worklog/BRIDGE-PAR-122.md.

Source evidence:
- crates/opentui-bridge/src/solid_p0.rs:37-49 (Signal<T> P0 stand-in, Rc<RefCell>, !Send)
- crates/opentui-bridge/src/solid_boundary.rs:17-20,34-37 (createSignal/createMemo/createEffect/createStore allowlists)
- crates/opentui-bridge/src/solid_host.rs:6 (reactivity stays in TS; Rust keeps boundary)
- TS truth paths context/local.tsx, context/sync.tsx do not exist in this repo tree (glob + read failed); modeled dependency graph only per task spec.

Target boundary: ONE new file crates/opentui-bridge/src/signal_graph.rs. No lib.rs/Cargo.toml/solid_*.rs edits. No cargo. rustfmt --check only.

Tests: in-file #[cfg(test)] 6 tests: set/get roundtrip, missing none, memo deps kept, effects count, values cap, memos cap + key truncation.

Decisions: plain Vec storage, no HashMap (std-only minimal); saturating_add for counter; key cap via chars().take(64).

Unknowns: none.
