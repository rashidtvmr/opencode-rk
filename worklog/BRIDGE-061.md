# BRIDGE-061 model_dialog

Claim: Rust ranking helpers mirroring dialog-model.tsx:23-183.
Source: TS checkout a0d9b6c dialog-model.tsx:23-183,53-59,43,81,44,82,122,129,186-197; local.tsx:35-49; index.tsx:1461,1549; dialog-debug.tsx:32; model_ref.rs reuse.
Target: crates/opentui-bridge/src/model_dialog.rs only. No cargo run per scope.
Tests: 7 tests (truncate, fav-first, recent-desc, name-tiebreak, deny+nano, free+footer, qualified). Logically green, not executed.
Decisions: std-only sort, fuzzysort stays TS-side; NANO_MARKER not NANO_MODEL (TS uses includes, no single model); is_denied new fail-closed guard.
Unknowns: wiring into lib.rs left to integrator (scope forbids touching it).
