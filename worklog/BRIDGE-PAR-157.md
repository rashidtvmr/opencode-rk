# BRIDGE-PAR-157 scratchpad

Claim: BRIDGE-PAR-157 via cc.claim session ses_par157. Owned file: crates/opentui-bridge/src/thinking_ctx.rs.
Source evidence: /home/rashid/projects/opencode/packages/tui/src/context/thinking.ts (full read 67 lines): :4 ThinkingMode show|hide, :12-17 reasoningSummary bold-title split, :19-21 isThinkingMode, :24-27 nextThinkingMode show->hide->show, :29-67 useThinkingMode kv.signal thinking_mode default hide + legacy thinking_visibility bool migrate (:34-54) + minimal->hide (:56).
kv.tsx grep thinking: no matches (case-insensitive), migration lives in thinking.ts not kv.tsx.
Observed: task asks level 0..=2 off|low|high + migrated flag. Divergence from TS two-state show|hide: level 0=off maps hide default, 1=low, 2=high; migrated flag mirrors legacy thinking_visibility migration branch.
Target boundary: ONE new file thinking_ctx.rs only. No lib.rs, no Cargo.toml, no cargo, no commit.
Tests: 6 in-file cfg(test): default_off, set_valid, set_reject, labels, migrate_flag, reject_preserves.
Decisions: std-only, forbid(unsafe_code), <120 lines. ponytail: skipped reasoningSummary port, add when bridge parses markdown.
Remaining: rustfmt --check, flip completed.
