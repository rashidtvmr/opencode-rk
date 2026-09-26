# BRIDGE-PAR-138 — toast_center.rs

Claim: ses_par138, in-progress (ledger fence confirmed own session).
Source evidence:
- packages/tui/src/ui/toast.tsx:54-56 (currentToast single-slot store), :60-67 (show replaces + timeout reset)
- crates/opentui-bridge/src/toast_single.rs (controller truth, replace-on-new; read only)
Target boundary: ONE new file crates/opentui-bridge/src/toast_center.rs. No edits to lib.rs, Cargo.toml, toast.rs, toast_view.rs, toast_single.rs.
Tests: 6 in-file #[cfg(test)] (show sets, queues when busy, overflow evicts oldest, dismiss promotes, empty dismiss none, trunc 512 char-boundary-safe).
Decisions: String messages, MAX_MSG 512 chars, MAX_QUEUE 8, show fills empty slot else queues with oldest evict, dismiss promotes front else None. std-only, forbid(unsafe_code).
Verification: rustfmt --check PASS. No cargo per task scope.
Unknowns: none.
