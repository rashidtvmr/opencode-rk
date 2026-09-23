# WEB-009 branch structured activity RED

## Claim

- Task: `WEB-009`
- Session: `ses_f31381759ffegPGvEjks41sScI`
- Branch: `lane/WEB-009-branch-activity-red-20260923`
- Base: `1e84b31`
- Owned test: `crates/sessions/tests/web009_branch_activity_red.rs`

## Source evidence

- `crates/sessions/src/lib.rs:492-541`, `SessionService::append_assistant_with_activity`, validates reasoning size, routes branch sessions to `SessionManager`, and currently rejects non-empty tool/reference vectors with `Contract("structured assistant activity is not yet available for branch sessions")` before append.
- `crates/sessions/src/lib.rs:656-667`, `SessionService::assistant_activity`, reads branch activity through `SessionManager::list_assistant_activity` and legacy activity through `Storage`.
- `crates/sessions/src/lib.rs:751-795`, `SessionService::branch_from_message`, synchronizes legacy history/activity into the v2 shadow before `ForkV2` branch creation.
- `crates/sessions/src/branch_v2.rs:145-235`, `SessionManager::synchronize_legacy_shadow`, copies legacy message IDs and currently passes only reasoning to `V2Writer::append_message_with_reasoning`.
- `crates/sessions/src/branch_v2.rs:296-338`, `SessionManager::list_assistant_activity`, projects branch reasoning rows but constructs empty `tool_calls` and `references`.
- `crates/storage/src/lib.rs:231-283`, legacy `Storage::append_message_with_activity` validates and atomically inserts message plus activity in one transaction.
- `crates/storage/src/lib.rs:385-412`, legacy activity reload decodes tool/reference JSON into `AssistantActivity`.
- `crates/storage/src/writer_v2.rs:64-76,129-210`, v2 writer currently exposes reasoning-only append and inserts message/parts transactionally.
- `crates/storage/src/fork_v2.rs:161-225`, fork copies message rows with fresh message IDs and reuses message payload parts.
- `crates/contracts/src/lib.rs:22-32,236-257`, public bounds and `AssistantActivity`, `AssistantToolCall`, `AssistantReference` contracts.
- Existing `crates/sessions/tests/web_activity_branch.rs` covers reasoning-only legacy-to-branch projection; this file adds tool/reference, direct branch append/reopen, and negative side-effect coverage without editing it.

## Observable contract

1. A legacy assistant activity record containing reasoning, completed tool activity, and an HTTPS reference, branched at that assistant, yields one child activity record associated with the copied child assistant message, same ordering and exact fields, without provider re-execution or duplicate activity.
2. A branch assistant append persists answer plus reasoning/tool/reference atomically. Dropping and reopening `SessionManager` and `SessionService` against the same file-backed branch DB reproduces message ordering and exact activity fields.
3. Malformed tool state, over-bound tool count, and over-bound reasoning fail through public contracts before adding a child assistant message. Existing valid child history remains unchanged. Negative guard is expected GREEN on current code.

## RED verification

Command:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 - <<'PY'
import os, subprocess, sys
cmd = ['cargo', 'test', '-p', 'opencode-rk-sessions', '--test', 'web009_branch_activity_red', '--', '--test-threads=1']
env = os.environ.copy()
env.update(CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1')
try:
    result = subprocess.run(cmd, timeout=180, env=env, text=True)
except subprocess.TimeoutExpired:
    print('TIMEOUT')
    raise SystemExit(124)
print(result.stdout)
print(result.stderr, file=sys.stderr)
raise SystemExit(result.returncode)
PY
```

Result: compiling RED, `1 passed; 2 failed`.

- `malformed_or_over_bound_child_activity_has_no_message_side_effect`: PASS. Invalid state, 129 tools, and oversized reasoning are rejected with no message side effect.
- `legacy_assistant_activity_is_copied_to_branch_without_reexecution`: FAIL at child activity assertion. Child assistant is copied, reasoning survives, `tool_calls` and `references` are empty.
- `child_activity_append_survives_manager_and_service_reopen`: FAIL at direct branch append with `Contract("structured assistant activity is not yet available for branch sessions")`.

Test SHA-256 after RED freeze: `23f591fcb7d2990a60aabc23a9b5f9677e9e46974ce7a89fccc1db88551a0872`.

## Boundary

Tests only. No product or existing-test edits. Lane remains `blocked` pending v2 durable tool/reference writer, legacy-shadow copy, branch projection, and branch append wiring.
