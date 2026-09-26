# BRIDGE-PAR-130 scratchpad

Claim: BRIDGE-PAR-130 via ses_par130. Owned file: crates/opentui-bridge/src/data_projection.rs.
Source evidence: packages/tui/src/context/data.tsx lines 1-60 (Data store session info/message, message.update/prepend keyed by sessionID); event switch projects session.next.* into rows.
Observed: no existing data_projection.rs; context_session.rs pattern forbid(unsafe_code)+consts+tests.
Target boundary: new file only, std-only, <150 lines, DataKind+DataRow+project+row_key+filter_kind.
Tests: 7 in-file (id trunc, label trunc, passthrough, key, filter match, filter none, cap).
Decisions: chars().take for unicode-safe trunc; MAX_ROWS cap in filter_kind.
Unknowns: none.
