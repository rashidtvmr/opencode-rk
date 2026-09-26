# BRIDGE-PAR-297 scratchpad

Claim: BRIDGE-PAR-297, session ses_par297, in-progress.
Source: TS `packages/tui/src/util/selection.ts:1-79` (copy/ctrl-c/escape key handling, no numeric range type). Sibling `crates/opentui-bridge/src/selection.rs:1-146` covers copy semantics.
Target: NEW `crates/opentui-bridge/src/selection_util_full.rs`, ordered index-range companion, no overlap.
Tests: 5 planned (order, empty len, contains edges, select_all, select_all zero).
Decisions: `end` exclusive (`len=end-start`); `new` orders via min/max; `contains` = start<=i<end; `select_all` = 0..total.
