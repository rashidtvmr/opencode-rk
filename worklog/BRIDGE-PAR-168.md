# BRIDGE-PAR-168

Claim: cc.claim BRIDGE-PAR-168, session ses_par168, scratchpad worklog/BRIDGE-PAR-168.md.
Source evidence:
- TS truth: packages/opencode/src/cli/cmd/run/session.shared.ts (createSession/resolveSession keyed by session id, 196 lines).
- Sibling: crates/opentui-bridge/src/run_session_shared.rs:1-159 (SessionRef caps 64/128, SessionTable cap 256; read only, not edited).
Target boundary: ONE new file crates/opentui-bridge/src/session_shared_full.rs, std-only, forbid(unsafe_code), <130 lines.
Tests: 7 in-file #[cfg(test)]: caps, rename empty false, rename ok, rename truncates, bump increments, bump saturates, header parts.
Decisions: title cap 256 per task (sibling uses 128, noted divergence); header format "id8 title (N)" with id truncated to 8 chars.
Unknowns: none; lib.rs wiring explicitly out of scope per task.
