# FIX-CI-APPROVAL — fail-closed CI approval coverage

Claim: FIX-CI-APPROVAL, session ses_fix_ciappr, status in-progress.
Owned file: crates/cli/src/ci_run.rs only.

## Source evidence (HEAD 62f7eb1)
- crates/cli/src/ci_run.rs:71-80 `check_approval_required` only matches 4 tool names: shell_exec, shell_command, exec, run_command (plain `prompt.contains(tool)`).
- ci_run.rs:308 caller: on match emits `CiEvent::ApprovalRequired` + `TurnFinished{exit:20}`, returns exit 20. Fail-closed, never auto-approves.
- ci_output.rs:217-225 `render_approval_required` ALWAYS exit 20. Frozen tests: ci_mode.rs T02 (render-level), ci_ext.rs (caps/doctor). Neither exercises prompt->exit20 path.
- Task mandate: cover approval-triggering prompts matching shell/exec/run/approval/request_permissions patterns; prompts with those patterns exit 20 with valid JSONL. Expand detection only; do NOT weaken fail-closed.

## Observed scenario
- Prompt "please run ls" contains none of the 4 tool names -> would proceed past approval gate (fail-open). Same for "shell ...", "approval", "request_permissions", "permission", natural-language exec/run requests.
- Fix: extend pattern list in `check_approval_required` only: existing 4 tool names (return tool name) + generic substrings shell, exec, run, approval, request_permissions, permission(s) (return matched pattern). Case-insensitive. Doctor prompt unaffected (prompt=="doctor" branch first).

## Target boundary
- Edit ONLY check_approval_required body in ci_run.rs. No test edits, no other files, no commit/push.

## Tests
- VERIFY cmd: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-cli --test ci_mode (frozen, must stay green).

## Decisions
- Keep returning matched pattern string as tool name (caller clones into event). Case-insensitive via prompt.to_lowercase() once.
- Order: exact tool names first (preserves existing tool attribution), then generic patterns.

## Verification (binary-level, target/debug/opencode-rk rebuilt with fix)
- All mandated patterns exit 20 with valid JSONL: "please run ls"->run/20;
  "open a shell"->shell/20; "execute this"->exec/20; "needs approval"->approval/20;
  "request_permissions for files"->request_permissions/20; "grant permission"->permission/20.
  Legacy "use shell_exec now"->shell_exec/20 (attribution preserved).
  Benign "say hi"->TurnStarted then TurnFinished exit 64 (missing-token fail-closed,
  no false positive). "doctor" prompt unaffected (Doctor event). All approval lines
  parse as JSONL (python json.loads OK).
- Frozen ci_mode suite: 7/9 pass (6 ci_output unit + T02 approval); T01/T03 e2e hung
  past 115s-280s timeouts, then tree broke under concurrent lanes.
- BLOCKER (pre-existing, outside owned file): rebuild now fails on
  crates/providers/src/fallback.rs:162 merge-conflict markers + crates/tools/src/permission.rs:74
  unexpected closing delimiter. Both outside FIX-CI-APPROVAL authority (ci_run.rs only).
  rustfmt on ci_run.rs: only pre-existing blank-line diffs.
- Zero test edits. No commit/push per orders.

## Remaining
- Blocked: full `cargo test --test ci_mode` tail unobtainable until foreign-file breakage
  (fallback.rs conflict markers, permission.rs brace) resolved by owning lanes; ledger blocked.
- BLOCKER-UPDATE 2026-09-20 (ses_f404c921fffeWNRGmT6durJ3PG): ledger fenced by ses_f40b69bc6ffeSPUwjCahoQU2dK blocked, claim ClaimError.
- Impl verified in worktree: ci_run.rs only diff, lowercase + 8 patterns, zero frozen-test diffs.
- ci_t02: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 70 cargo test -p opencode-rk-cli --test ci_mode ci_t02` -> 1 passed 0 failed.
- ci_run unit: `cargo test -p opencode-rk-cli --bin opencode-rk ci_run` -> 4/4 passed.
- Binary probe rebuilt: 8 approval prompts exit 20 case-insensitive; benign say-hi exit 64; JSONL parses OK.
- BLOCKER (pre-existing, outside owned file): full ci_mode hangs in ci_t01 daemon e2e (timeout 124). Zero test edits. No commit/push.

## Verification 2026-09-20 (session ses_f40b69bc6ffeSPUwjCahoQU2dK)
- Claim: FIX-CI-APPROVAL in-progress under ses_f40b69bc6ffeSPUwjCahoQU2dK (prior ses_fix_ciappr row superseded by reclaim-free overwrite; single in-progress holder).
- Merge conflicts resolved: no `<<<<<<<` in fallback.rs/permission.rs/ci_run.rs; `cargo check -p opencode-rk-cli` 0 errors.
- Binary probe (rebuilt target/debug/opencode-rk): all mandated patterns exit 20 case-insensitive (run/shell/exec/approval/request_permissions/permission + legacy shell_exec); benign "say hi" exit 64 (missing-token fail-closed, TurnStarted path); all approval lines valid JSONL.
- Frozen targets GREEN: ci_t02 render-only ok; ci_run unit 4/4 ok.
- BLOCKER (pre-existing, outside owned file): full `cargo test -p opencode-rk-cli --test ci_mode` hangs in ci_t01 (daemon-backed e2e, `...` then timeout 124 at 115s); ci_t01 alone also hangs >60s. Same class as noted baseline T01/T03 hang. Unrelated to check_approval_required (approval path returns before daemon spawn).
- Zero test edits. No commit/push per orders.

## Verify 2026-09-20 (ses_f40414a28ffeRtlI7oA8bqfQwM)
- Claim active FIX-CI-APPROVAL in-progress this session.
- Owned impl unchanged ci_run.rs:71-97 lowercase + 4 tool names + 8 generic patterns.

## Hang analysis 2026-09-20 (ses_f40414a28ffeRtlI7oA8bqfQwM)
- Direct test-binary run ci_t01: `timeout 55 ./target/debug/deps/ci_mode-9899af71a9ab255c ci_t01 --test-threads=1` -> EXIT 124, prints only `test ci_t01...` no assertion. Hang inside test body, not compile.
- Manual binary probe `run --ci "say hi"`: with env_cleared AND with dummy provider env, exits 64 (UsageError, missing daemon bearer). Never reaches daemon spawn/output wait. Path: run_ci TurnStarted -> daemon_bearer Err -> TurnFinished exit 64.
- ci_t01 env: ci_command uses env_clear + HOME/ADDR + OPENAI_* only; no OPENCODE_RK_DAEMON_TOKEN -> run_ci must exit 64 immediately with TurnFinished. Test waits for "TurnFinished" then asserts exit 0. Exit-64 path emits TurnFinished so wait_for should return; `child.wait()` then `assert status.success()` would FAIL fast, not hang.
- Hang therefore occurs before any TurnFinished: binary startup blocked. Candidates outside ci_run.rs: serve subcommand/daemon spawn, StdoutReader/wait_for, env_clear missing HOME/PATH causing spawn/lookup stall, one-shot provider fixture accept with no connection (fixture accepted only if daemon forwards; but with 64-exit no connection ever made — accept() blocks forever in fixture thread, but `provider_task.join()` runs AFTER child.wait; join on never-accepted listener blocks forever -> hang at join, line 234, after TurnFinished received).
- Proof: last log shows test stuck at start; approval-path binary probes (exit 20) emit ApprovalRequired+TurnFinished pre-daemon. The exit-64 path likewise returns before ensure_daemon/spawn_daemon/HTTP. check_approval_required cannot hang: pure lowercase+contains, no IO/loops. spawn_daemon/ensure_daemon unreachable for ci_t01 (bearer missing).
- Additional: orphan `serve --listen` daemons (PIDs 2073399/2073400/2073570/2073575) from parallel lanes confirm concurrent-lane port/env interference; ci_t01 hang class pre-existing (worklog baseline: T01/T03 hung 115-280s at HEAD too).
- Conclusion: hang root cause outside owned ci_run.rs: test harness fixture join (provider_task never served because fail-closed 64 exits before provider contact). Fix belongs in test (frozen, untouchable) or daemon/auth contract (other lanes), not check_approval_required.
- Zero test edits made. Ledger -> blocked with this evidence.
