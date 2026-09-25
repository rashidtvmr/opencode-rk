# BRIDGE-PAR-321 scratchpad
- Claim FAILED (ledger blocked). `cc.claim(BRIDGE-PAR-321/ses_par321)` -> ClaimError "claims must be a bounded mapping" (claims.json 501 rows, MAX_ROWS=500, schemaVersion present). No ledger write possible; orchestrator must prune. No shared files touched.
- Source: packages/tui/src/component/workspace-label.tsx:5-18 `{name} ({type})` + status/icon props. Prior Rust: crates/opentui-bridge/src/small_widgets.rs:249-296 WorkspaceLabel struct - new file additive free fns, no overlap.
- Target: crates/opentui-bridge/src/workspace_label_full.rs, std-only, forbid(unsafe_code), 91 lines.
- Tests: 5 in-file, zero edits (home_itself, subpath_tilde, outside_and_cap, basename_and_cap, current_matches_slashes).
- Decisions: norm() trims trailing / \; empty->"/". label: exact home->"~", "home/"-prefix->"~/rest", else raw; char-cap 128. short: rsplit basename, cap 64. is_current: norm equality.
- Verify: `rustfmt --edition 2021 --check crates/opentui-bridge/src/workspace_label_full.rs` -> FMT_OK. No cargo per scope.
- Status: blocked-on-ledger (file+tests exist, formatted; ledger update impossible until claims.json pruned below 500 rows).
