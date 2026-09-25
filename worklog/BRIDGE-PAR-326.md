# BRIDGE-PAR-326 scratchpad

Claim: attempted via completion_claims.claim (ses_par326) -> ClaimError
"claims must be a bounded mapping": claims.json has 501 rows > MAX_ROWS 500
(load_ledger validates len <= 500). Repo-wide blocker, not lane fault.
Status NOT set (claim + update both call load_ledger, both fail).

Source evidence:
- packages/tui/src/component/dialog-retry-action.tsx:39-46 (selected/action, default action)
- crates/opentui-bridge/src/dialog_stack.rs:1-19 (lane pattern forbid unsafe, consts)

Target boundary: ONE new file crates/opentui-bridge/src/dialog_retry_full.rs.
No lib.rs / Cargo.toml / cargo / commit per scope.

Tests: 4 in-file (push_cap, push_rejects_bad, cursor_clamps, retry_bumps).
Verification: `rustfmt --check crates/opentui-bridge/src/dialog_retry_full.rs` PASS exit 0, 87 lines (<110).
Decisions: saturating_add for attempts; clamp cursor; push validates empty/oversize.
Unknowns: none. Unblocks when orchestrator prunes ledger below 500 rows.
