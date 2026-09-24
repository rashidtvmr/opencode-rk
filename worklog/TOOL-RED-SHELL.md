# TOOL-RED-SHELL scratchpad

## Claim
- Task `TOOL-RED-SHELL`, session `ses_f2ddec05cffe9glpDG6jx1t5FO`, scratchpad `worklog/TOOL-RED-SHELL.md`.
- Role: RED-test author only. No production code, no manifest, no existing-test edits.

## Source evidence (base 5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b)
- `crates/tools/src/executor.rs:109-134` `ToolExecutor::execute`: `bash`/`shell` routed straight to `execute_shell`, no broker consult.
- `crates/tools/src/executor.rs:138-147` `execute_authorized`: non-`read` calls fall through to unbrokered `execute`; shell never sees a broker.
- `crates/tools/src/executor.rs:240-297` `execute_shell`: `Command::new("bash").arg("-c")` spawn + `tokio::time::timeout` on `output()` future; no authorization, no `kill_on_drop`.
- `crates/tools/src/registry_dispatch.rs:131-146` `RegistryDispatcher::new/with_policy`: default policy `AllowAll` (always true).
- `crates/tools/src/registry_dispatch.rs:168-208` `dispatch`: shell resolves via registry and spawns under `AllowAll`; no concrete broker capability required.
- `crates/tools/src/shell_tool.rs:164-211` `ShellTool::run`: `authz: None` = legacy allowlist-only spawn (documents bypass, not the RED target; RED targets public executor/dispatcher paths).
- `crates/tools/src/shell_tool.rs:536-565` existing unit tests prove broker-deny works only when a broker is explicitly injected into `ShellTool`; executor/dispatcher paths inject none.
- `crates/server/src/lib.rs:1572` live unbrokered `ToolExecutor::new()` (per independent verifier; confirmed present).
- Package name: `opencode-rk-tools` (`crates/tools/Cargo.toml:2`).

## Observable contract (frozen RED target)
1. Shell request without explicit broker authorization is denied before child spawn.
2. Denial leaves no marker file; captured output/error/record emit no secret sentinel.
3. Bounded timeout leaves no surviving child / no late marker (observable via public `ToolCall::with_timeout`; documents kill gap, no private access).
4. Fixture: disposable `tempfile::tempdir`, fixed harmless `touch <marker>` / `sleep 1; touch <marker>` commands. No network, no real secrets, no user DB, no `#[ignore]`.

## Tests (`crates/tools/tests/phase1_shell_broker.rs`)
- `t01_shell_without_broker_denied_before_spawn`: executor `bash touch marker` must fail denied, marker absent.
- `t02_denial_leaves_no_marker_and_no_secret`: same path, marker absent + sentinel absent from output/error.
- `t03_dispatcher_without_broker_denies_shell_no_store_write`: `RegistryDispatcher::new` (AllowAll) dispatch of `bash` must return `Denied`, marker absent, store stats `(0,0)`, permits reclaimed.
- `t04_timeout_leaves_no_late_marker`: `sleep 1; touch marker` with 50ms timeout must error timeout and marker must still be absent after 1.5s settle.

## Decisions
- Target public APIs only (`ToolExecutor::execute`, `RegistryDispatcher::dispatch`, `ToolOutput/OutputStore` stats).
- Cancellation subcase expressible via public timeout API, included as t04 (late-marker = surviving-child side effect).
- Sentinel is a fake constant; assertions never print its value, so failure output leaks nothing.

## Remaining unknowns
- GREEN design (broker-threaded executor API shape) belongs to implementer; not this lane.
- `ShellTool` allowlist-only `None`-broker path noted, not RED-covered (unit-covered bypass left for GREEN wiring).
