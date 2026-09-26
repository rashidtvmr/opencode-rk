# BRIDGE-PAR-407 (unclaimed, file-only)

Task: crates/opentui-bridge/src/util_signal_full.rs
Status: unclaimed per delegator override (no claims.json touch).

Claim: skipped (orchestrator owns ledger).
Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/util/signal.ts:1-51 (createSignal get/set pair; debounced + fade variants, no version counter)
- Pattern crates/opentui-bridge/src/signal_graph.rs:1-25 (forbid unsafe, doc-comment TS refs, SigCell analog via values vec)
Observed: TS signal = getter/setter pair; Rust port needs owned versioned cell.
Target boundary: ONE file util_signal_full.rs; lib.rs/Cargo.toml/signal_graph.rs untouched.
Tests: default_is_zero, set_bumps_version, same_value_still_bumps.
Decisions: saturating_add for ver (matches signal_graph run_effects idiom); Copy+Clone; 52 lines < 60.
Unknowns: none. Wiring into lib.rs left to orchestrator.
