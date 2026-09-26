# BRIDGE-PAR-291

- Claim: `cc.claim(..., 'BRIDGE-PAR-291', 'ses_par291', 'worklog/BRIDGE-PAR-291.md')` OK.
- Source evidence: TS truth `packages/tui/src/util/transcript.ts:54-58` (user vs assistant branch), `:84-112` (formatPart text/reasoning/tool); bridge emitters `reply_lines.rs:16,22` (`you:`/`assistant:`), `err_line.rs:13,30` (`error:` prefix). No `you:`/`system:` line protocol in TS.
- Observed: no existing `role_of`/`body_of`/`is_user` in `crates/opentui-bridge/src` (grep none).
- Target boundary: ONE new file `crates/opentui-bridge/src/transcript_util_full.rs`; no lib.rs/Cargo.toml edits; std-only, forbid(unsafe_code), <90 lines.
- Tests: 4 tests in-file (roles, body_after_space, body_no_space_is_whole, detects_user).
- Decisions: `role_of` prefix-match `you:`/`assistant:`/`error:`, fallback `system:`; `body_of` after first space else whole; `is_user` = role == `you:`. `offline:` not a role (ponytail note in file).
- Remaining: rustfmt --check only per task; no cargo/commit.
