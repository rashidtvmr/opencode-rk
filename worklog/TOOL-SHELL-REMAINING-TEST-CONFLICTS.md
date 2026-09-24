# TOOL-SHELL-REMAINING-TEST-CONFLICTS — verification

- Claim: `TOOL-SHELL-REMAINING-TEST-CONFLICTS`, session `ses_f2cc1eac6ffetxXzlo6C4mQoXt`.
- Route `9router-oc-muse-spark-1-3-contributor-free`, allowlist-confirmed.
- Branch `plan/TOOL-SHELL-REMAINING-TEST-CONFLICTS`. Owned file: this worklog only.
- Status: in-progress. Role: independent verifier. No source/test edits, no Cargo, no repair/refreeze/acceptance.

## Authority

1. Frozen broker suite `crates/tools/tests/phase1_shell_broker.rs` t01-t03, hash `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`. Immutable per AGENTS.md, docs/TDD.md. Deny-before-spawn authoritative (PLAN ADR-006, docs/SECURITY.md sec 1-4).
2. Git/source evidence below. In-file `disc103_t05`, `disc111_t04` carry no frozen hash.
3. Prior model summaries evidence only: legacy review `9613f53`, verifier `2b7a6fa` (via `git show`), executor verify `6a55ac5`.

## Provenance

- `disc103_t05_batch_respects_max_parallel_bound`, `crates/tools/src/registry_dispatch.rs:564`. `git blame` lines 564-573: commit `06a52b2` (DISC-103 registry dispatch wiring). Body registers `Tool::new("bash","bash","shell",...)`, `RegistryDispatcher::with_policy(max_parallel:2, AllowAll)`, 4x `bash` `sleep 0.2; echo done-{i}`, asserts 4 success + elapsed>=350ms + permits reclaimed + store `bash` len 4 (:581-607).
- Shell gate `b66ee38` added: `SHELL_DENIED_NO_BROKER` (:43-44), gate in `dispatch()` before permit/executor/store (:198-206), batch `Ready::Immediate(Ok(shell_denied_record))` (:261-265), helpers `is_shell_alias` + `shell_denied_record` (:360-384). `git diff 06a52b2 b66ee38` = +42/-0 in this file.
- `disc111_t04_crash_bounded_retry_no_silent_restart`, `crates/tools/src/mcp_spawn.rs:728`. `git blame` line 726-735: commit `c1866be` (integrate wave2). Fixture `McpEndpoint::Stdio { program: "/bin/false", ... }` (:733-743) with RED-phase NOTE (:734-738): was `/bin/sh -c "exit 3"`, broker maps shell `-c` to RequireHuman, so approved spawn could never run. Crash codes caller-observed literals via `note_crash` (:753-761), budget 2 -> Crashed x2 then RestartsExhausted, `is_running` false.

## Satisfiability matrix

| Test | Frozen? | Status under current gate | Classification |
|---|---|---|---|
| disc103_t05 | No frozen hash | FAILS: `batch item must dispatch: Unknown("slot-0")` at registry_dispatch.rs:597 (reproduced `2b7a6fa`) | Stale intent (unbrokered `bash` batch success/timing/store asserts contradict `b66ee38` deny-before-spawn) PLUS separate implementation defect (below). Valid concurrency intent preserved for future non-shell replacement. |
| disc111_t04 | No frozen hash | FAILS in full-lib run as `SpawnFailed` missing binary, environment (`2b7a6fa` matrix: 107 passed/4 failed incl. this) | Valid intent, environment-dependent fixture. Crash/retry/no-silent-restart assertions are security-compatible; only the `/bin/false` spawn precondition fails where binary absent. |

## Implementation-defect note (not stale intent, do not conflate)

- `dispatch_batch` Immediate-loss: `registry_dispatch.rs:278-279` spawn-extraction `slot.take()` drops every non-Spawn Immediate to None; rebuild `:324-339` matches None-without-result to `_ => Err(Unknown("slot-{idx}"))`. Every fail-closed batch item (Unknown/Disabled/Denied/InputTooLarge/EmptyName) and every shell-denied Immediate resolves as misleading `Unknown("slot-i")`, breaking single/batch parity. Reproduced by t05 at `b66ee38`. Requires its own RED/repair lane (preserve Immediates, batch fail-closed parity test: each maps exactly, order kept, no store write, permits intact). No repair authorized here.

## Correction / refreeze proposal (future test owner + controller only)

Path A — disc103_t05 (stale intent):
1. Leave file untouched pending authorization. Do not edit in place.
2. Owner retires `bash`+`AllowAll` success/timing assertions (contradict frozen deny-before-spawn; `AllowAll` boolean is not process authority per registry_dispatch.rs:201-203 comment).
3. Owner authors replacement concurrency test on non-shell tool (e.g. echo-like registered tool) preserving bound intent: max_parallel=2, 4 items, elapsed serialization bound, order, permits reclaimed, store write-back. Compiling RED first, freeze new hash.
4. Separately, batch fail-closed parity test for Immediate-loss defect (unknown/denied/shell-denied each exact, order, no store, permits intact) under defect-repair lane RED.

Path B — disc111_t04 (valid intent, env fixture):
1. Leave file untouched pending authorization.
2. Owner swaps `/bin/false` for explicitly authorized direct-process fixture portable across CI hosts (same argv-direct pattern as `stdio_echo` `/bin/echo` / t06 `/bin/sleep`), keeping `note_crash` literal-code assertions unchanged (codes independent of child binary per in-file NOTE).
3. Preserve coverage: Crashed x budget then RestartsExhausted, no silent restart (`is_running` false), reclaim safe. Compiling RED first if fixture semantics change, refreeze.

Refreeze order: (1) land batch Immediate-loss repair with its RED hash; (2) owner migrates t05-concurrency (non-shell) + t05-parity batch items and t04 fixture under new RED hashes; (3) controller refreezes; (4) verifier reruns frozen t01-t03 (hash must stay `ef56f63a...`) plus new suites. Never touch t01-t03. Never: `cfg(test)` bypass, `AllowAll` process authority, content allowlist, shell bypass, fake success, hidden missing binary.

## Validation

- `shasum -a 256 crates/tools/tests/phase1_shell_broker.rs` = `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (unchanged).
- `git diff --check`: clean expected at commit time.
- `git status --short --branch`: two paths only (this worklog + own claims.json row).
- No product test run (verification artifact landing; failures already reproduced in `2b7a6fa`).

## Gaps

- None blocking this review. Product remains blocked: batch Immediate-loss defect + stale/env conflicts pending owner lanes.
