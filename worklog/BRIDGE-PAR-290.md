# BRIDGE-PAR-290 scratchpad

claim: BRIDGE-PAR-290 ses_par290 in-progress.
source evidence:
- TS truth `/home/rashid/projects/opencode/packages/tui/src/util/session.ts:1-3` exports only `isDefaultTitle(title)` regex `^(New session - |Child session - )\d{4}-...Z$`.
- sibling pattern `crates/opentui-bridge/src/session_header_full.rs:1-35` HeaderFlow std-only forbid(unsafe_code).
observed scenario: no existing session_title/age_label/msg_count util; new isolated `_full.rs` lane, no lib.rs wiring allowed.
target boundary: ONE file `crates/opentui-bridge/src/session_util_full.rs`, std-only, forbid(unsafe_code), <100 lines, 3 fns + >=4 tests. No lib.rs/Cargo.toml edits, no cargo, no commit.
tests: 5 in-file (title-passthrough, title-empty-falls-back-id8, title-cap-128, age-buckets, msg-count).
decisions: title = trimmed non-empty ? truncate 128 chars : id[..8]; age buckets s/m/h/d floor division; msg label `{n} msgs` (no singular special-case, matches spec).
remaining: rustfmt --check only per scope.
