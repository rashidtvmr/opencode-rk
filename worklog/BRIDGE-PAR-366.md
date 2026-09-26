# BRIDGE-PAR-366 (unclaimed, file-only per orchestrator)

Claim: skipped ledger (orchestrator owns claims.json). Proceeded file-only.
Source: packages/tui/src/context/local.tsx:1-49 (LocalTheme, parseModel, recentModels, solid store keys).
Target: crates/opentui-bridge/src/ctx_local_full.rs, std-only, forbid(unsafe_code).
Tests: 4 (add_has, dup, overlong, cap). Decisions: dedupe returns false; len>128 reject; 64 cap.
Unknowns: exact upstream key set; using generic string keys.
