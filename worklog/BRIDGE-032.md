# BRIDGE-032 diff_viewer

Claim: diff presentation contracts, no render/scroll port.
Source: diff-viewer.tsx (DiffView:48, hunk scan:290-292, jumpRelativeHunk:282-315, jumpRelativePatchFile:270-280, toggle_view:663-673, toggleSelectedFileTreeRow:399-406), diff-viewer-file-tree-utils.ts (orderedPatchFileIndexes:178-180, movePatchFileIndex:186-191, toggleFileTreeDirectory:197-203), diff-viewer-file-tree.tsx:148 markers, diff-viewer-ui.tsx Panel/Separator (not ported, layout only).
Target: crates/opentui-bridge/src/diff_viewer.rs, forbid unsafe, std only.
Tests (RED first, logically GREEN, not run per scope): toggle_view, next/prev_file clamp, toggle_expand, next/prev_hunk roll, MAX_FILES=256, MAX_CHILDREN=64, count_hunks, from_diffs.
Decisions: reuse FileDiff, hunk counts stored per-file; scroll/renderable/scrollTop not modeled; single-patch/review/kv not modeled.
Unknowns: none blocking.
