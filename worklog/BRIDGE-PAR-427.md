# BRIDGE-PAR-427 scratchpad (UNCLAIMED - orchestrator owns claims.json, file-only lane)

- Claim: NOT claimed (per task: do NOT touch claims.json). Proceeded file-only.
- Source: packages/tui/src/feature-plugins/system/diff-viewer.tsx:38 ROUTE="diff"; dialog_export_full.rs pattern (struct + caps + saturating + tests).
- Target: crates/opentui-bridge/src/diff_viewer_tsx_full.rs, 77 lines, forbid(unsafe_code), std-only.
- API: DiffView {path: String (cap 512 on set_path), lines: u32} + set_path->bool + bump_lines (saturating_add) + summary->String (cap 512 via char-boundary truncate).
- Tests (3): path_and_summary, bump_saturates, summary_caps_512.
- Verify: rustfmt --check PASS (ran rustfmt once to fix 2 blank-line diffs, re-check FMT_OK). No cargo/commit per scope. Did NOT edit lib.rs/Cargo.toml/diff_viewer.rs.
- Unknowns: none. Module not wired into lib.rs (out of scope, orchestrator wires).
