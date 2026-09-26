# BRIDGE-PAR-352 (unclaimed, file-only per prompt)

Claim: skipped ledger (orchestrator owns claims.json, overflow). File-only lane.
Source: packages/tui/src/index.tsx:1 `export {run} from "./app"` (re-export entry).
Target: crates/opentui-bridge/src/tui_index_full.rs, TuiIndex latch.
Tests: start_bumps_once, stop_releases_restart_remounts, stop_idempotent_fresh_zero.
Decision: saturating_add for mounts; minimal struct, no lib.rs edit.
Status: file written, rustfmt check pending.
