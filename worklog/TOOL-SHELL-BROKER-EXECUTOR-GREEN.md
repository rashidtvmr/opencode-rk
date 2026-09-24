# TOOL-SHELL-BROKER-EXECUTOR-GREEN

## Claim
- Task `TOOL-SHELL-BROKER-EXECUTOR-GREEN`; session `ses_f2d280f56ffekshDTZMD3LgU8V`.
- Route `9router-xk-gpt56-luna`, confirmed in supplied allowlist.
- Branch `lane/TOOL-SHELL-BROKER-GREEN`; owned product file `crates/tools/src/executor.rs`.

## Source evidence
- `crates/tools/src/executor.rs:108-134`: `execute` sends `bash`/`shell` directly to `execute_shell`.
- `crates/tools/src/executor.rs:136-147`: `execute_authorized` currently authorizes only `read`; all other calls fall back to unbrokered `execute`.
- `crates/tools/src/executor.rs:240-297`: shell creates `bash -c <command>` and awaits output without broker authorization.
- `crates/security/src/lib.rs:42-71,206-227,290-300`: `OperationIntent::Process` binds exact program, argv, cwd; only `Decision::Allow` permits execution; opaque shell `-c` receives `RequireHuman`.
- `crates/security/src/tool_authorize.rs:59-69`: shell intent uses `bash`, `-c`, exact command, supplied cwd.
- Frozen test `crates/tools/tests/phase1_shell_broker.rs` SHA-256: `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

## Contract
- `ToolExecutor::execute` remains deny-by-default for `bash`/`shell`; no broker, no `Command` construction.
- `execute_authorized` handles shell through the broker before spawn. `Deny` and `RequireHuman` return bounded fixed denial text without command, input, environment, or approval ID.
- Authorized shell intent binds actual `bash`, `-c`, command, and cwd. Spawn occurs only in `Decision::Allow`.
- Read authorization, timeout caps, IDs, and non-shell behavior remain unchanged.
- Existing `Decision::RequireHuman` for opaque `bash -c` is intentional; no shell allow is claimed for the default protected broker.

## Validation
- Baseline frozen shell suite: expected compiling RED 0/3.
- Post-implementation: t01/t02 GREEN; t03 remains registry-owned RED if dispatcher is unchanged.
- Commands: focused shell suite, executor unit tests, `git diff --check`, frozen hash.

## Remaining unknowns
- Dispatcher default policy remains a separate owned slice.
- Independent verifier must inspect exact tree and decide acceptance.
