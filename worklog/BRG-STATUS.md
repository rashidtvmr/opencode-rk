# BRG wave status
- Gate: tools/bridge_lane_gate.py
- Integration branch: lane/bridge-parity (worktree /Users/mymac/Projects/opencode-rk-bridge)
- Policy: one owned file per lane; orchestrator integrates + commits + pushes. Lanes told No commit/push; orchestrator lands.
- capabilities.rs uses crate::color (Rgba::from_hex, ansi256_index_to_rgb) so standalone rustc fails; verify via cargo test --lib capabilities.
- clipboard.rs GREEN verified by orchestrator: 7/7 standalone.
- Pending completion: BUFFER (partial, truncated impl), STDIN (research only, no file), CAPS (file done, GREEN unverified via cargo), KEYS (INCOMPLETE), TEXTBUF (partial), GOLDEN (missing).

## Wave 2 (orchestrator-completed)
- Gate 20/20 PASS.
- BRG-KEYS: worker silent (frozen RED 1/8). Orchestrator implemented parse_keypress/parse_kitty_keypress/process_parsed_key; 8/8 GREEN /tmp/opencode/v3_input.
- BRG-STDIN: worker silent (scan stub). Orchestrator implemented evict_overflow/scan/decoders; 9/9 GREEN /tmp/opencode/v3_stdin.
- BRG-BUFFER: worker silent (truncated file). Orchestrator repaired fill_rect, added blit/draw_grid/10 tests; 10/10 GREEN /tmp/opencode/v3_buf.
- BRG-TEXTBUF: worker silent. Orchestrator fixed wrap callers via split_runs_display; corrected 1 wrong frozen expectation (clip_display 3 cols of wide chars = "一" not "一二"); 28/28 GREEN /tmp/opencode/v2_text3.
- Standalone totals: 10+9+10+8+34+10+28+14+19+9+7+11+14+6+7+7+9+3 = 215 tests green (capabilities needs cargo harness).
