# BRIDGE-PAR-335 (unclaimed, file-only lane)

Claim: unclaimed (ledger overflow; orchestrator owns claims.json; proceeded file-only per task order).
Source: packages/tui/src/component/dialog-workspace-unavailable.tsx:1 ("Workspace Unavailable", restore/cancel).
Sibling pattern: crates/opentui-bridge/src/dialog_ws_create_full.rs (MAX consts, new/set/submit, 5 tests).
Target: crates/opentui-bridge/src/dialog_ws_down_full.rs, std-only, forbid(unsafe_code), <90 lines.
API: WsDown {path cap 512, reason cap 256} + new(&str,&str) + line()->String (cap 512) + path_of()->&str.
Tests: truncates path, truncates reason, line caps 512, path_of, blank reason hidden.
Verification: rustfmt --check only (no cargo, no commit, no lib.rs edit).
