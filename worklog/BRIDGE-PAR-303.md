# BRIDGE-PAR-303 scratchpad

- Claim: BRIDGE-PAR-303 via cc.claim, session ses_par303. OK.
- Source: `packages/tui/src/util/revert-diff.ts:1-18` (getRevertDiffFiles, a/b strip, +/- counts); sibling `crates/opentui-bridge/src/revert_diff.rs:1-97` (unified-diff parser, not reused per scope).
- Target: new file only `crates/opentui-bridge/src/revert_diff_full.rs`. No lib.rs / Cargo.toml edits.
- API: diff_stat capped 64, is_revertable either>0, revert_line path cap 256 chars. std-only, forbid(unsafe_code).
- Tests: 4 unit tests in-file (format, cap, revertable, line cap).
- Verify: rustfmt --check only (no cargo per task).
- Unknowns: none. Stat cap displays saturated value (no "+" suffix) per terse spec.
