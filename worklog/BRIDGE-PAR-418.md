# BRIDGE-PAR-418 (unclaimed: orchestrator owns claims.json, file-only lane)

- Claim: not claimed (per task order, ledger untouched).
- Source: packages/tui/.../diff-viewer-file-tree-utils.ts:1-40 (FileTreeNode depth/name, FileTreeRow).
- Target: crates/opentui-bridge/src/diff_tree_util_full.rs only; lib.rs/Cargo.toml/diff_tree.rs/diff_viewer.rs untouched.
- Tests: indent_scales, indent_caps_at_16, label_trims, label_caps_at_128 (in-file, std-only).
- Decisions: depth.min(8)=16sp cap; trim+chars().take(128); forbid(unsafe_code); <60 lines.
- Unknowns: none. No cargo run (verification rustfmt only per task).
