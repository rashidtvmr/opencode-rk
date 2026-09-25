# BRIDGE-PAR-141 fork_route

Claim: ses_par141, in-progress.
Evidence: TS `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12` DialogForkFromTimeline pick+fork+navigate; Rust `crates/opentui-bridge/src/session_fork_dialog.rs:14` ForkDialog pick-then-confirm gate (read-only).
Target: ONE file `crates/opentui-bridge/src/fork_route.rs`, forbid(unsafe_code), <140L.
Tests: 6 in-file (empty, blank, before-plan, roundtrip, reset, trunc).
Decisions: Default construct; plan clears stale target; complete guards plan+empty; char-wise trunc 64.
Unknowns: none. Verify: rustfmt --check only, no cargo/commit per scope.
