# BRIDGE-PAR-317 scratchpad

Claim: FAILED via completion_claims.claim, ClaimError "claims must be a bounded mapping" (501 rows > MAX_ROWS 500). Task ABSENT from ledger, no fence, proceeded per scope.
Source: /home/rashid/projects/opencode/packages/tui/src/component/dialog-variant.tsx:10 options memo, :32 DialogSelect flat pick.
Target: crates/opentui-bridge/src/dialog_variant_full.rs, VariantDialog {opts cap16/128, cursor} + push/move_cursor/selected.
Tests: 6 in-file (push_ok, blank, truncate, cap, wrap, empty_noop).
Decisions: rem_euclid wrap matches flat select cycling; blank reject mirrors non-empty titles.
Unknowns: ledger claim impossible until orchestrator prunes rows below 500.
