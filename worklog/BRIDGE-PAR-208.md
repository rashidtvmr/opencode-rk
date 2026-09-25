# BRIDGE-PAR-208 scratchpad

claim: in-progress, session ses_par208.
evidence: crates/cli/src/tui_entry.rs:717 `transcript.push(format!("you: {text}"))`, :722 `format!("assistant: {reply}")`, :726 `"offline: turn not executed"`. Style ref: crates/opentui-bridge/src/toast_line.rs (forbid unsafe, in-file tests).
boundary: ONE new file crates/opentui-bridge/src/reply_lines.rs. No lib.rs, Cargo.toml, cargo, commit.
tests: 5 in-file (you_prefix, you_caps_4k, assistant_prefix, assistant_caps_64k, offline_exact).
decisions: char-count caps via chars().take; returned String owned except offline_line static str. ponytail: no ellipsis.
verify: rustfmt --edition 2021 --check reply_lines.rs FMT_OK exit0. 71 lines <90. std-only forbid(unsafe_code).
