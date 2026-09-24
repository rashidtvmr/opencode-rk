# TOOL-SHELL-BROKER-REGISTRY-GREEN

## Claim
- Task `TOOL-SHELL-BROKER-REGISTRY-GREEN`; session `ses_f2cf68244ffek0gyTpqxcEewHa`.
- Route `9router-th-dsv41-flash-free`, confirmed in supplied allowlist.
- Branch `lane/TOOL-SHELL-BROKER-GREEN`; owned product file
  `crates/tools/src/registry_dispatch.rs`.

## Source evidence
- Frozen RED `crates/tools/tests/phase1_shell_broker.rs` t03:
  `RegistryDispatcher::new()` registers `bash`, dispatches with no broker
  context, expects `Ok(record)` with `success=false`, denial text, no marker,
  store `(0,0)`, all permits available.
- RED cause: `registry_dispatch.rs:203` calls generic `executor.execute`, which
  now denies shell (`executor.rs:134-141`), but `:206 store_result` pushed
  unconditionally and `:207` returned `Ok` regardless of `success`. Store stats
  `(1,4)`.
- Verifier `6a55ac5` + executor `ed4164db` established t01/t02 GREEN and t03
  registry-owned RED.
- Frozen SHA-256 `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

## Contract
- A boolean/`AllowAll` dispatch policy is not process authority. A registered
  `bash`/`shell` alias reached without a concrete broker verdict is denied in
  the registry before semaphore acquisition, executor call, or store write.
- Denial is a bounded `Ok(DispatchRecord)` (`success=false`,
  `error=Some(SHELL_DENIED_NO_BROKER)`) preserving tool id/name/provenance. No
  command, input, environment, or identity echo.
- Batch dispatch applies the same gate, so batch cannot bypass single dispatch.
- Non-shell tool semantics (echo, unknown, disabled, oversized, denied-by-policy)
  and permit accounting unchanged.

## Implementation
- Added `SHELL_DENIED_NO_BROKER` fixed denial constant.
- Added `is_shell_alias(&Tool)`: id in {bash,shell,/bin/bash,/bin/sh} or name in
  {bash,shell}. Id-or-name because registry keys by id while executor switches on
  name.
- Added `shell_denied_record(&Tool,&str)` bounded failed record.
- `dispatch`: after policy, `if is_shell_alias(tool) { return Ok(shell_denied_record(...)) }`
  before `semaphore.acquire`.
- `dispatch_batch`: matching arm emits `Ready::Immediate(Ok(shell_denied_record))`
  so no permit is taken and the immediate path never touches the store.

## Validation
- Frozen shell suite: pre-change 2 passed/1 failed (t03 store `(1,4)` vs `(0,0)`)
  -> post-change `3 passed; 0 failed`.
- Frozen hash re-checked before/after: `ef56f63a...` byte-identical.
- Tools lib pre vs post failure set identical (4 pre-existing conflicts):
  `executor::tests::execute_success`, `executor::tests::execute_timeout`,
  `mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart`,
  `registry_dispatch::tests::disc103_t05_batch_respects_max_parallel_bound`.
  107 passed both sides; no new failure introduced.
- `git diff --check` clean.

## Immutable conflict (outside this lane)
- `disc103_t05_batch_respects_max_parallel_bound` (lib, line 597) registers a
  `bash` tool and asserts batch bash success/record. The executor `ed4164db`
  gate makes `execute("bash")` deny by default, so spillover bash batching now
  denies. Pre-existing since `ed4164db`, unchanged by this lane, and not
  satisfiable without weakening the executor gate or editing the frozen lib test.
  Reported, not edited.

## Remaining unknowns
- Legacy `executor::tests::execute_success`/`execute_timeout` and
  `mcp_spawn::tests::disc111_t04` remain pre-existing conflicts owned elsewhere.
- Independent verifier must rerun the complete frozen suite on the integrated
  revision.