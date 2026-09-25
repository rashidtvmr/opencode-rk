# BRIDGE-PAR-113 scratchpad

claim: BRIDGE-PAR-113 ses_par113 in-progress.
evidence: crates/cli/src/tui_entry.rs:452-509 Chat branch (tail window height-7, pad to height-3, rule, `> draft`, footer); crates/opentui-bridge/src/paint_full.rs:17-32 FrameInput, :50-86 paint_frame, :35 DEFAULT_FOOTER.
boundary: ONE new file crates/opentui-bridge/src/paint_adapter.rs. No lib.rs/Cargo.toml/paint_full.rs/tui_entry.rs edits. No cargo/commit.
tests: empty_placeholder, tail_window, height_clamp, width_clip, draft_row (in-file).
decision: cap title/status 256 chars; build_frame footer default (empty hints); frame_height height.max(8). ponytail: no styling spans.
