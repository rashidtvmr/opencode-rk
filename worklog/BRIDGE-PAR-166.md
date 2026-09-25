# BRIDGE-PAR-166 scratchpad

Claim: ses_par166, status in-progress.
Source: TS truth permission.shared.ts (reply once/always/reject via permissionRun/permissionReply); sibling run_permission_shared.rs (PermQueue bounded, MAX_TOOL_CHARS 64); pattern model question_full.rs (trunc caps, truncate opts).
Target: crates/opentui-bridge/src/permission_full.rs only.
Tests: default_pending, allow_sets_true, deny_sets_false, overwrite_last_wins, trunc_caps.
Verify: rustfmt --check EXIT=0.
