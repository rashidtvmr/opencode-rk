# BRG wave status
- Gate: tools/bridge_lane_gate.py
- Integration branch: lane/bridge-parity (worktree /Users/mymac/Projects/opencode-rk-bridge)
- Policy: one owned file per lane; orchestrator integrates + commits + pushes. Lanes told No commit/push; orchestrator lands.
- capabilities.rs uses crate::color (Rgba::from_hex, ansi256_index_to_rgb) so standalone rustc fails; verify via cargo test --lib capabilities.
- clipboard.rs GREEN verified by orchestrator: 7/7 standalone.
- Pending completion: BUFFER (partial, truncated impl), STDIN (research only, no file), CAPS (file done, GREEN unverified via cargo), KEYS (INCOMPLETE), TEXTBUF (partial), GOLDEN (missing).
