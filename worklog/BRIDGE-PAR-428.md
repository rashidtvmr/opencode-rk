# BRIDGE-PAR-428 (unclaimed, file-only per orchestrator)

Claim: no claims.json touch. Proceeding file-only.
Source: diff-viewer-file-tree.tsx:1-40 (files + selectedFileIndex props, flattenFileTree rows).
Target: crates/opentui-bridge/src/diff_tree_tsx_full.rs only.
Tests: 5 (new empty, first select, 512 cap, 64 evict, clamp).
Decisions: Vec + saturating_sub cursor on evict; clamp move_cursor; std-only; forbid unsafe; ponytail flat list.
Unknowns: none.
Verify: rustfmt --check PASS.
