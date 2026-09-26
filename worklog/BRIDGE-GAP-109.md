# BRIDGE-GAP-109

Claim: session ses_gap109, ledger in-progress confirmed.
Source evidence:
- crates/opentui-bridge/src/run_permission.rs:1-87 (Stage, PermissionReq, advance, tests mod 6 tests)
- TS truth: packages/opencode/src/cli/cmd/run/footer.permission.tsx:1-120 (three-stage permission/always/reject, Allow once / Always / Reject buttons)
Target boundary: append only; existing items byte-identical.
Tests: new #[cfg(test)] mod tests2, 7 tests written (unrun per scope, no cargo).
Decisions: PermPrompt::new truncates tool to 64 chars and detail to 512 chars before empty check; decide overwrites; decision_label pending/allow/always/reject; std-only.
Remaining: verifier runs cargo test.
