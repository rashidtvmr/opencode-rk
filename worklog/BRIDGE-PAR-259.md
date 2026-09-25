# BRIDGE-PAR-259 scratchpad

claim: ses_par259 via tools/completion_claims.py claim OK.
source evidence:
- crates/opentui-bridge/src/session_index.rs:16 SessionIndex {foreground_tasks, actions, kv_hide}
- crates/opentui-bridge/src/id8.rs:8 id8, :14 session_line
- crates/opentui-bridge/src/lib.rs:216 pub mod session_index, :300 pub mod id8
observed: TS truth read-only, no edits to shared files.
target boundary: ONE new file crates/opentui-bridge/src/session_index_full.rs only. No lib.rs/Cargo.toml edits, no cargo, no commit.
tests: 5 unit tests in-file (new_is_empty, set_id_stores, set_id_truncates_to_128, short_is_id8, line_delegates_to_session_line).
decisions: std-only, forbid(unsafe_code), char-safe truncation, delegate line/short to id8 fns, 84 lines < 110.
unknowns: none. Module wiring into lib.rs is orchestrator/integration job (out of scope per task).
verification: rustfmt --check crates/opentui-bridge/src/session_index_full.rs -> FMT_OK.
