# RED-VALIDITY-AUTO4 (AUTO-004/005/006 verify-only, yA wave)

Rev pinned: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. VERIFY-ONLY: zero impl/test/tool edits by this lane. Worktree dirty (parallel QA race, 426 files `git status`), hashes = worktree-at-verify-time.

## Counts (serial JOBS=1 THREADS=1, `timeout 120 rtk`, `free -h` first each run)

| Target | Result | Log |
|---|---|---|
| `--test delegation_lane` | 5 passed / 0 failed | `/tmp/opencode/yA-delegation_lane.log` |
| `--test driver_lane` | 5 passed / 0 failed | `/tmp/opencode/yA-driver_lane.log` |
| `--test delegation_gated` | 5 passed / 0 failed | `/tmp/opencode/yA-delegation_gated.log` |
| full `-p opencode-rk-agents` | 35 passed / 0 failed (unit 15 + gated 5 + delegation 5 + driver 5 + turn_submission 5; doc 0) | `/tmp/opencode/yA-full-agents.log` |

Lane total: 15/15. Full crate: 35/35. No failures.

## AUTO-005 live tool (fixtures `/tmp/opencode/yA005/`, rev `auto005-rev-001`)

| Fixture | Exit | Reason |
|---|---|---|
| pass | 0 | `passed:true`, all 4 checks pass |
| mutated (1-byte frozen append) | 2 | `mutated-frozen-evidence`, names `tests/frozen_suite.rs` |
| self (worker `passes:true`, no verifier) | 2 | `self-report-not-evidence` |
| wrong-rev (verifier rev mismatch) | 2 | `verifier-revision-mismatch` |

Reports: `/tmp/opencode/yA-tool-{pass,mutated,self,wrongrev}.log`.

## sha256 (worktree at verify time)

- `crates/agents/src/delegation_lane.rs`: `51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681`
- `crates/agents/src/driver_lane.rs`: `c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd`
- `crates/agents/tests/delegation_lane.rs`: `3ca19e36aae23c85a0ee65085d7df62b8d793c88ed2bcc08d02e7e0355b1d971`
- `crates/agents/tests/driver_lane.rs`: `4d9a30067d2bbf038f132decb51e5ed6c8c5f159de94e0133513c20a27405a37`
- `crates/agents/tests/delegation_gated.rs`: `0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7`
- `tools/check_tdd_pipeline.py`: `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474` (matches AUTO-005 worklog frozen note)

Frozen/ralph.json/lib.rs untouched. No stubs created.

## Confirm-2 (bI wave, rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`, VERIFY-ONLY zero edits)

Serial `JOBS=1 THREADS=1`, `timeout 120 rtk`, `free -h` first each run:

| Target | Result | Log |
|---|---|---|
| `--test delegation_lane` | 5 passed / 0 failed | `/tmp/opencode/bI-delegation_lane.log` |
| `--test driver_lane` | 5 passed / 0 failed | `/tmp/opencode/bI-driver_lane.log` |
| `--test delegation_gated` | 5 passed / 0 failed | `/tmp/opencode/bI-delegation_gated.log` |
| full `-p opencode-rk-agents` | 35 passed / 0 failed (unit 15 + gated 5 + delegation 5 + driver 5 + turn_submission 5) | `/tmp/opencode/bI-agents.log` |

Lane total: 15/15. Full crate: 35/35. No failures.

AUTO-005 live tool, disposable fixtures `/tmp/opencode/auto005y/` (copied from yA005), rev `auto005-rev-001`:

| Fixture | Exit | Reason |
|---|---|---|
| pass | 0 | `passed:true`, all 4 checks pass |
| mutated (1-byte frozen append) | 2 | `mutated-frozen-evidence`, names `tests/frozen_suite.rs` |
| self (worker `passes:true`, no verifier) | 2 | `self-report-not-evidence` |
| wrong-rev (verifier rev mismatch) | 2 | `verifier-revision-mismatch` |

Reports: `/tmp/opencode/bI-tool-{pass,mutated,self,wrongrev}.json`.

sha256 re-recorded (worktree at verify time, identical to yA wave):

- `crates/agents/src/delegation_lane.rs`: `51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681`
- `crates/agents/src/driver_lane.rs`: `c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd`
- `crates/agents/tests/delegation_lane.rs`: `3ca19e36aae23c85a0ee65085d7df62b8d793c88ed2bcc08d02e7e0355b1d971`
- `crates/agents/tests/driver_lane.rs`: `4d9a30067d2bbf038f132decb51e5ed6c8c5f159de94e0133513c20a27405a37`
- `crates/agents/tests/delegation_gated.rs`: `0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7`
- `tools/check_tdd_pipeline.py`: `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474`

`git status --short -- crates/agents/src/lib.rs ralph.json tasks/AUTO-004.md tasks/AUTO-005.md tasks/AUTO-006.md` empty. Frozen/ralph.json/lib.rs untouched. No stubs created.
