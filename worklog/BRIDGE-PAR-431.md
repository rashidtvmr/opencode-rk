# BRIDGE-PAR-431 (unclaimed, file-only per orchestrator)

Claim: skipped ledger (orchestrator owns claims.json). File-only lane.
Source: packages/tui/src/ui/dialog-confirm.tsx:9 DialogConfirmProps (title, onConfirm/onCancel), :17 DialogConfirmResult bool|undefined. Sibling: crates/opentui-bridge/src/dialog_confirm_full.rs:1-38 ConfirmDialog pattern.
Target: crates/opentui-bridge/src/dialog_confirm_tsx_full.rs ConfirmTsx {title cap 128, ok} + set_title/confirm/cancel/decided.
Tests: 4 inline (cap+pending, confirm true, cancel false, retitle resets).
Decisions: `done` bool tri-state preserves pub ok field; char-based trunc unicode-safe; Default new().
Verification: rustfmt --check PASS. No cargo per scope.
