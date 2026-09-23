# GUARD-OPS-004 reconciliation audit

## Claim and boundary

- Task: `GUARD-OPS-004`
- Session: `ses_f32482b5dffeiYzXcJ6BJmrwgJ`
- Branch: `lane/GUARD-OPS-004-20260923`
- Candidate revision before this receipt: `c8f6dcb39d98302f27f04a25897d3880e09e2c1b`
- Scope: read-only OPS-004 reconciliation audit.
- Owned writes: this scratchpad and the `GUARD-OPS-004` row in
  `tasks/completion/claims.json` only.
- Forbidden: canonical, product, test, validator, `ralph.json`,
  `ralph.completion.json`, `FEATURES.md`, or acceptance edits.

## Source evidence

- `ralph.json:1217-1228`: OPS-004 is `status: accepted`, generic DISC-002
  story, empty requirements/dependencies, five obligations.
- `tasks/OPS-004.md:1-15`: local card exists and declares `Status: NOT STARTED`.
  Contract scope is pure offline version/channel parsing plus path derivation
  from caller-supplied directories. No filesystem mutation, process-environment
  read, lock creation, or network.
- `tasks/OPS-004.md:20-49`: source pin is OpenCode
  `95daf90670b7c039c436c85537da5fbfe2205b41`; reviewed sources are
  `installation/version.ts:1-8`, `global.ts:10-85`, and `npm-config.ts:12-40`.
  Global/npm/process side effects are explicitly excluded.
- `tasks/OPS-004.md:51-115`: observable contract, failure states, caps, and
  frozen T01-T05 obligations.
- `worklog/OPS-004.md:7-34`: prior implementation receipt records source hash
  `1548fa81dc663f238a657a2164f4608ef6339b90f19c41b639847afda35a680b`, test
  hash `d613884087cfa8efd15db99ee2cb66f3ed75d38e8291742a8604a9a102b2b802`,
  5/5 GREEN, foundation check clean, and no RED re-break for that session.
- `crates/foundation/src/install_meta.rs:1-130`: pure bounded implementation;
  no filesystem, environment, network, clock, task, or global-state access.
- `crates/foundation/src/lib.rs:9`: `install_meta` is wired into the
  foundation module.
- `crates/foundation/tests/install_meta.rs:15-105`: five executable tests;
  T05 checks zero counters and disposable paths. This guard does not rerun
  Cargo or alter tests.
- `sources/operations-ownership-gap.json:109-113`: OPS-004 binding remains
  generic, with `taskCard: null`, `worklog: null`, and unresolved ownership.
- `sources/operations-ownership-gap.json:474-521`: OPS-004 remains in the
  `configuration-and-repository` equivalence group with OPS-008. Ownership
  requires direct task-specific source/caller/test/spec evidence, bounded
  authority, and explicit disposition of every unresolved partition.
- `sources/operations-ownership-gap.json:504-511`: unresolved partition
  `global-npm-installation-process-side-effects` remains separate from the
  pure metadata fragment.
- `sources/backlog-exhaustion.json:804-838`: OPS-004 remains
  `unresolved-decomposition`, `controllerStatus: not-started`, with no local
  evidence, task card, worklog, or implementation commits recorded there.
- `FEATURES.md:9-22,67,649`: controller sync records OPS-004 as accepted with
  a quiesced-tree re-run caveat. This is not independent acceptance authority.
- `tools/validate_backlog_exhaustion.py:973-1199`: operations guard requires
  Ralph `not-started`, unresolved/null task binding, absent task/worklog, and
  the fixed equivalence groups and ledger classification.
- `tools/validate_repository.py:21-28,99-112`: canonical validation runs
  backlog exhaustion before the plan check can pass.

## Attributable failures

Read-only command:

```text
rtk python3 tools/validate_backlog_exhaustion.py
validate_backlog_exhaustion: 51 error(s)
```

Exact OPS-004-specific errors:

```text
OPS-004: operations ownership-gap is stale after Ralph task semantics changed
OPS-004: task/worklog appeared; operations ownership gap needs deliberate review
```

Origins:

1. `tools/validate_backlog_exhaustion.py:1031-1033`: checked-in Ralph says
   `accepted`, while the ownership-gap contract requires `not-started`.
2. `tools/validate_backlog_exhaustion.py:1046-1047`: both
   `tasks/OPS-004.md` and `worklog/OPS-004.md` exist while the gap binding and
   ledger still require absent task/worklog evidence.

Not attributable to OPS-004:

- Other 49 backlog errors: shared accepted/classification drift plus routing,
  release, extensibility, sharing, and integrations ownership-gap rows.
- `rtk python3 tools/convergence_gate.py`: `CONVERGENCE BLOCKED`, total `80`,
  due to off-plan completed claims; no OPS-004-specific convergence line.
- `rtk python3 tools/validate_repository.py`: fails at backlog exhaustion after
  ruleset import, readback fixture, and protection checks pass.
- `FEATURES.md` status sync: controller-owned state, not a guard-local repair.
- Prior `worklog/OPS-VERIFY4.md:24-33` records OPS-004 `install_meta` 5/5,
  but test GREEN is not independent acceptance.

## Authority-safe disposition

**BLOCKED. No canonical reconciliation patch is authorized in this lane.**

Do not flip `ralph.json`, delete or rewrite the OPS-004 card, mutate product or
tests, edit the ownership-gap JSON, edit the backlog ledger, edit
`FEATURES.md`, or weaken the validator. Those are controller/integration
authority paths. Do not claim acceptance from the local 5/5 test receipt.

Controller/integrator disposition required:

1. Determine whether the pure installation fragment is genuinely task-specific
   under the gap closure criteria. Preserve the OPS-008 overlap and the
   excluded global/npm/process side-effect remainder.
2. If ownership is accepted, atomically reconcile the operations gap binding,
   ownership decision, backlog row/evidence, validator expectations, and Ralph/
   FEATURES semantics. Then obtain independent frozen-test and integrated
   revision evidence.
3. If ownership is rejected, controller must formally abandon or re-scope the
   local card/evidence and restore gap/ledger invariants. This guard must not
   perform that rollback.
4. Re-run canonical validation, convergence validation, and independent
   OPS-004 verification on the exact integrated revision before any acceptance
   claim.

## Verification

- `rtk python3 tools/validate_backlog_exhaustion.py`: exit 1, 51 errors; exactly
  the two OPS-004 lines above are attributable.
- `rtk python3 tools/convergence_gate.py`: exit 1, total 80; no OPS-004 line.
- `rtk python3 tools/validate_repository.py`: exit 1 at backlog exhaustion;
  preliminary ruleset/readback/protection checks pass.
- `rtk git diff --check`: exit 0.
- `rtk test -e worklog/GUARD-OPS-004.md`: exit 0 after recreation.
- No Cargo, network, browser, database, or heavy command run.

## Final status

Ledger status: `blocked`.

Scratchpad recreated after prior receipt was absent on disk. Commit must contain
only this file plus the `GUARD-OPS-004` ledger row. No acceptance claimed.
