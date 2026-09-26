# Claim: BRIDGE-PAR-209 ses_par209
# Source evidence:
# - crates/cli/src/tui_entry.rs:502 `snapshot.session_id.chars().take(8).collect()`
# - crates/cli/src/tui_entry.rs:503-506 `format!("session {id8} • {} • {} msgs", state, count)`
# - crates/opentui-bridge/src/session_shared_full.rs:50 `self.id.chars().take(8)`
# - crates/opentui-bridge/src/route_session.rs:58 `self.session_id.chars().take(8)`
# Observed: id8 truncation repeated inline in 4 sites, no shared helper.
# Target boundary: ONE file crates/opentui-bridge/src/id8.rs, no lib.rs edit.
# Tests: 5 in-file (truncate, short, empty, line format, 256 cap).
# Decisions: space-joined session_line per task spec (TS uses bullets, spec wins);
# char-safe take(8)/take(256); std-only forbid(unsafe_code).
# Unknowns: none.
