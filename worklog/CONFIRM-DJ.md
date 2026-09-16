# CONFIRM-DJ — VERIFY-ONLY agents+INT+OPS+AUTO005

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Workdir: `/home/rashid/projects/opencode-rk`.
Lease: VERIFY-ONLY. Zero product/test edits. No frozen/lib.rs/ralph.json touches.

## Serial protocol

`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 110/120`, `--test-threads=1`.
`free -h` checked before every run (6.2Gi total, ~1.1-1.2Gi avail each run).
Arrival: 478 dirty paths (pre-existing). Close: 481 (delta = this file only + status churn).
`git diff --name-only HEAD -- frozen/ ralph.json`: empty. No frozen/ralph.json in untracked.
`frozen/` dir absent (`ls frozen/` = no such file). Nothing touched.

## Results (all exits 0, 0 failed)

| lane | suites | tests | log |
|---|---|---|---|
| agents full (`opencode-rk-agents`: lib unit 15 + delegation_lane 5 + driver_lane 5 + delegation_gated 5 + turn_submission_state 5) | 5 (4 int + 1 unit + 1 doctest-empty) | 35/35 | /tmp/opencode/dJ-agents.log |
| INT (`opencode-rk-providers`: int_alias, int_backoff, int_catentry, int_client, int_connect, int_events, int_handler_lane, int_ledger) | 8 | 40/40 | /tmp/opencode/dJ-int.log |
| OPS (`opencode-rk-foundation`: ops_budget, ops_guard, ops_health, ops_lock, ops_metrics, ops_parser_lane, ops_ping, ops_replay, ops_repo_ref) | 9 | 45/45 | /tmp/opencode/dJ-ops.log |
| AUTO-005 tool (`tools/check_tdd_pipeline.py` sha256 `eea1ac0f…288474`, rev `auto005-rev-001`) | 4 exits | PASS 0 + edited 2 + self 2 + wrongrev 2 | /tmp/opencode/auto005z/pass.json,edited.json,self.json,wrongrev.json |

Every suite `5 passed / 0 failed` (unit 15/0). Tool reasons exact:
`mutated-frozen-evidence` (failing_fixture `fail-edited/tests/test_slice.py`),
`self-report-not-evidence`, `verifier-revision-mismatch`, PASS `passed:true` 4/4.
All tool runs disposable `/tmp/opencode/auto005z/`, no repo writes.

## Stub scan

`todo!`/`unimplemented!` over `crates/agents/src/delegation_lane.rs`,
`driver_lane.rs`, `crates/providers/src/int_*.rs`, `crates/foundation/src/ops_*.rs`:
zero hits.

## Counts

agents 35, INT 40, OPS 45, AUTO-005 4 exits.

Edits: this file only.
