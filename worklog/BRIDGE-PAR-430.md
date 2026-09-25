# BRIDGE-PAR-430 (unclaimed, file-only)

Claim: skipped per orchestrator order (no claims.json touch). Unclaimed lane.
Evidence: frecency.tsx:1, history.tsx:1, stash.tsx:1 (each 1-line re-export). Style ref prompt_frecency_full.rs:1.
Target: crates/opentui-bridge/src/prompt_reexp_full.rs only. No lib.rs/Cargo.toml edits.
Tests: names, unknown_empty, trims (frozen in-file).
Status: implemented, rustfmt pending.
