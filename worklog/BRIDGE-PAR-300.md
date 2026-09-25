# BRIDGE-PAR-300 scratchpad

Claim: BRIDGE-PAR-300, session ses_par300. Owned file: crates/opentui-bridge/src/present_util_full.rs.
Source evidence: TS truth /home/rashid/projects/opencode/packages/tui/src/util/presentation.ts (38 lines, sessionEpilogue + wordmark, no truncate/pad/rule units; generic equivalents implemented). Pattern ref: crates/opentui-bridge/src/format_util_full.rs (forbid unsafe, must_use, in-file tests).
Target boundary: truncate_middle(s,max)->String + pad_right(s,w)->String + header_rule(title,width)->String. std-only, forbid(unsafe_code), <110 lines.
Tests: 5 in-file (short_passthrough, middle_truncates_char_safe incl multibyte, tiny_max_no_ellipsis_room, pad_right_pads_and_keeps incl multibyte, rule_shapes).
Decisions: head-biased split (head=(inner+1)/2); max<=3 hard-cuts chars no ellipsis; pad_right counts chars not bytes; header_rule long prefix falls back to truncate_middle; width 0 -> empty.
Unknowns: none. No lib.rs/Cargo.toml touch. No cargo per scope. No commit per scope (deviation from WORKER.md s5 noted).
