# BRIDGE-PAR-325 scratchpad

claim: ledger claim BLOCKED (claims.json has 501 rows > MAX_ROWS 500, `load_ledger` raises ClaimError). File created without claim; orchestrator must claim/reclaim. No lib.rs/Cargo edits, no cargo run, no commit per lane scope.
source: TS truth packages/tui/src/component/dialog-move-session.tsx:34 DialogMoveSession (directory selection); pattern dialog_confirm_full.rs:16 ConfirmDialog capped title + answer.
observed: dialog_move_full.rs absent. Created new file.
target: crates/opentui-bridge/src/dialog_move_full.rs, MoveDialog {dest cap 512, confirmed}, set_dest/confirm/cancel, std-only, forbid unsafe, <90 lines, >=4 tests.
tests: 5 in-file (store+reset, cap512, confirm-true, confirm-false, cancel-clears). rustfmt --check PASS. No cargo per scope.
decisions: ponytail: no path normalization, add when wiring real fs checks.
remaining: ledger claim + completed flip by orchestrator (needs ledger prune to <=500 rows).
