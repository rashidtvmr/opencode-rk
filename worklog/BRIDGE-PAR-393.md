# BRIDGE-PAR-393 (unclaimed, file-only lane)

Claim: no claims.json touch per orchestrator override; proceeding file-only.
Source: packages/tui/src/ui/dialog.tsx:11 Dialog, :22 width (xlarge 116, large 88, else 60), :31 dismiss=!!getSelection().
Target: crates/opentui-bridge/src/dialog_tsx_full.rs, forbid(unsafe_code), std-only.
Tests: xlarge_width, large_width, default_width, dismiss_passthrough, title_trims_and_caps (5 tests).
Decisions: free fns, no struct (YAGNI); trim+chars().take(128) std-only; is_dismissed trivial passthrough mirroring JS !!.
Verify: rustfmt --check only (no cargo per scope). Pending run.
