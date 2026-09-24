# TOOL-SHELL-BATCH-IMMEDIATE-RED - scratchpad

- Task: TOOL-SHELL-BATCH-IMMEDIATE-RED (RED authoring, Rust dispatcher regression test author).
- Session: ses_f2b9f0cf6ffeeGf6eX3o5WJSuh.
- Branch/worktree: red/TOOL-SHELL-BATCH-IMMEDIATE.
- Owned file: crates/tools/tests/registry_batch_immediate_red.rs.
- Status: completed. Compiling RED confirmed fail-for-cause on 2026-09-24;
  frozen test SHA below. No source/test edits, no assert weakening.

## Authority / source evidence

- Defect location: crates/tools/src/registry_dispatch.rs `dispatch_batch`.
  - Spawn-extraction loop (base revision lines ~278-279):
    `if let Some(Ready::Spawn { tool, input, provenance }) = slot.take()`.
    `slot.take()` is applied to EVERY slot, so each non-`Spawn` (`Immediate`)
    variant is replaced with `None` before the rebuild.
  - Rebuild loop (~324-339) maps `None` without a spawn result to
    `_ => Err(DispatchError::Unknown(format!("slot-{idx}")))`.
  - Effect: unknown / disabled / policy-denied / oversized / empty-name
    fail-closed items and shell-denied `Ready::Immediate(Ok(..))` all resolve
    as misleading `Unknown("slot-i")`; breaks parity with single `dispatch`.
- Single-dispatch reference (`dispatch`, ~181-219): empty -> EmptyName; input
  over `max_input_bytes` -> InputTooLarge; registry miss -> Unknown(name);
  disabled -> Disabled(name); policy false -> Denied(name); shell alias ->
  `Ok(shell_denied_record(tool, provenance))` (never spawned, no store write).
- Fixed denial constant: crates/tools/src/registry_dispatch.rs:43-44
  `SHELL_DENIED_NO_BROKER = "shell execution denied: broker authorization required"`.
- Shell gate commit b66ee38 (TOOL-SHELL-BROKER-REGISTRY-GREEN).
- Verifier worklog (commit 2b7a6fa, worklog/TOOL-SHELL-BROKER-FULL-GREEN-VERIFY.md):
  reproduces `disc103_t05` failing with `Unknown("slot-0")`; classifies
  Immediate-loss as a separate implementation defect, not stale intent.
- Test-conflict review (commit 45095ae, worklog/TOOL-SHELL-REMAINING-TEST-CONFLICTS.md):
  same defect; repair direction = preserve Immediates + batch fail-closed parity
  test (each maps exactly, order kept, no store write, permits intact).
- docs/TDD.md sec 3 (RED compiles + fails for missing behavior; denial asserts
  absence of side effects), docs/SECURITY.md sec 4 (no-side-effect on denial).
- Frozen broker suite `crates/tools/tests/phase1_shell_broker.rs` hash
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` untouched.

## Test contract authored (6 tests, one per fail-closed class + parity)

File: crates/tools/tests/registry_batch_immediate_red.rs
- tsbi_t01_batch_unknown_retains_requested_id
- tsbi_t02_batch_disabled_retains_exact_error
- tsbi_t03_batch_oversized_retains_input_too_large
- tsbi_t04_batch_shell_denied_retains_fixed_record
- tsbi_t05_batch_ordered_parity_with_single_dispatch
- tsbi_t06_batch_denied_and_empty_name_exact_no_side_effects

Assertions: exact error/record parity vs single `dispatch`; order preserved;
shell denial keeps `SHELL_DENIED_NO_BROKER`, `success=false`, empty output, no
fake unbrokered success; no store write on any fail-closed/denied item; all
permits reclaimed (`available_permits == max_permits`). No `bash`-success or
timing intent encoded (deliberately avoids the stale `disc103_t05` intent).

Expected RED behaviour on current buggy code (pre-implementation):
- t01 -> `Err(Unknown("slot-0"))` != `Unknown("missing-tool")` FAIL.
- t02 -> `Unknown("slot-0")` != `Disabled("disabled-echo")` FAIL.
- t03 -> `Unknown("slot-0")` != `InputTooLarge{..}` FAIL.
- t04 -> `Unknown("slot-0")`, `.expect(...)` panics FAIL.
- t05 -> parity mismatch at index 0 FAIL.
- t06 -> `Unknown("slot-0")`/`Unknown("slot-1")` mismatch FAIL.

## Validation done (no Cargo, authoring phase)

- `rustfmt --edition 2024 --check crates/tools/tests/registry_batch_immediate_red.rs`
  -> exit 0 (formatted; rustfmt 1.9.0-stable).
- Manual API cross-check against source: `DispatchError` derives
  `Debug, Error, PartialEq, Eq`; `DispatchRecord` all fields pub and derives
  `Debug, Clone, PartialEq, Eq`; all imported symbols (`AllowAll`,
  `DispatchConfig`, `DispatchError`, `DispatchPolicy`, `DispatchRecord`,
  `RegistryDispatcher`, `SHELL_DENIED_NO_BROKER`) are pub in
  `crates/tools/src/registry_dispatch.rs`; `ToolRegistry::default`/`register`/
  `disable` and `OutputStore::stats`/`get` signatures match usage;
  dev-deps `tempfile` + tokio rt-multi-thread/macros present (not needed here).
- No existing test-file name collision (`crates/tools/tests/` listing checked).

## RED confirmation (Cargo run, 2026-09-24, slot free)

Command (sequential, jobs2/threads2):

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 \
  cargo test -p opencode-rk-tools --test registry_batch_immediate_red -- --test-threads=2
```

Result: compiles clean; `test result: FAILED. 0 passed; 6 failed; 0 ignored`.
No compile/API fixes were required.

| Test | Panic line | Observed (buggy) | Expected | Category proven |
|---|---|---|---|---|
| tsbi_t01_batch_unknown_retains_requested_id | :120 | `Unknown("slot-0")` | `Unknown("missing-tool")` | unknown substitution |
| tsbi_t02_batch_disabled_retains_exact_error | :158 | `Unknown("slot-0")` | `Disabled("disabled-echo")` | disabled substitution |
| tsbi_t03_batch_oversized_retains_input_too_large | :200 | `Unknown("slot-0")` | `InputTooLarge{..}` | oversized substitution |
| tsbi_t04_batch_shell_denied_retains_fixed_record | :236 | `Unknown("slot-0")` | `Ok(shell_denied_record)` | shell-denied substitution |
| tsbi_t05_batch_ordered_parity_with_single_dispatch | :324 | `Unknown("slot-0")` vs `Unknown("missing-tool")` | parity | ordered parity break |
| tsbi_t06_batch_denied_and_empty_name_exact_no_side_effects | :370 | `Unknown("slot-0")` | `Denied("echo")` | policy-denied substitution |

Every intended `Ready::Immediate` category (unknown, disabled, oversized,
shell-denied, policy-denied, empty-name) demonstrates `Unknown("slot-i")`
substitution. Empty-name case is covered in t06 second request.

## Frozen artifacts

- RED test SHA-256: `aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af`
  (`crates/tools/tests/registry_batch_immediate_red.rs`).
- Broker frozen hash UNCHANGED:
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`
  (`crates/tools/tests/phase1_shell_broker.rs`).
- `git diff --check`: clean.
- `crates/tools/src` diff: empty (no source edits).
- No surviving child processes (`pgrep` empty post-run); memory stable
  (free ~238 MiB pages brief, host responsive).

## Remaining unknowns / blockers

- None. Compiling RED stable fail-for-cause, frozen SHA recorded. Withdrawn
  resource blocker: Cargo slot was released by the cancellation lane; focused
  target run completed within memory budget.
- No source or existing-test edits made (registry_dispatch.rs unchanged,
  phase1_shell_broker.rs hash unchanged). Test file frozen, not to be edited;
  installers must fix `dispatch_batch` to preserve Immediate slots.