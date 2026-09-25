# BRIDGE-GAP-48 scratchpad

Claim: BRIDGE-GAP-48, session ses_gap48.
Source evidence: crates/opentui-bridge/src/run_scrollback.rs (CAP=2000, frozen flag, push/commit/separator/freeze/snapshot).
Target boundary: ONE new file crates/opentui-bridge/src/scrollback_family.rs only. No lib.rs/Cargo.toml, no cargo, no commit/push.
Tests: 7 in-file #[cfg(test)] (text_passthrough, markdown_stays_raw, code_is_fenced, body_capped_at_8kib, cap_evicts_oldest, freeze_blocks_writes, snapshot_is_clone).
Decisions: Surface/Snapshot as type aliases over ScrollbackWriter/Vec<Renderable>; truncate on char boundary; code fenced with lang.
Unknowns: none.
