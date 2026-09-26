# BRIDGE-PAR-116 scratchpad

Claim: BRIDGE-PAR-116 via cc.claim, session ses_par116, scratchpad worklog/BRIDGE-PAR-116.md.

Source evidence:
- TS truth `/home/rashid/projects/opencode/packages/tui/src/routes/session/index.tsx:116` sessionBindingCommands list
- same file `:214` foregroundTasks memo (running non-background task tool parts)
- same file `:249-261` kv.signal visibility toggles (timestamps, tool_details, scrollbar, generic_tool_output)
- Style model `crates/opentui-bridge/src/kv_toggles.rs:1-94` forbid(unsafe_code), in-file tests

Target boundary: ONE new file `crates/opentui-bridge/src/session_index.rs`. No lib.rs, no Cargo.toml, no cargo, no commit.

Tests: 7 in-file #[cfg(test)] (gate pass, gate fail, dup false, cap 32, toggle flips, has missing false, trunc 64).

Decisions: truncate overlong ids to 64 chars before dedup/push (keeps cap invariant); gate is `foreground_tasks <= max`.

Remaining: none. rustfmt --check PASS, 123 lines (<170).
