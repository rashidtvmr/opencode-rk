# RED-VALIDITY-AUTO (AUTO-004 / AUTO-006 impl + AUTO-005 declarative tool)

Scope: subagent owned ONLY `crates/agents/src/delegation_lane.rs` (AUTO-004)
and `crates/agents/src/driver_lane.rs` (AUTO-006). Frozen tests, lib.rs,
ralph.json, FEATURES.md, other files untouched by this lane (test-file
modifications pre-exist in working tree, not from this lane).

Bounds: serial runs, CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, timeout 120,
rtk prefix every shell, free -h checked first (6.2Gi total, ~2.3-2.8Gi
avail), disposable /tmp/opencode only.

## AUTO-004 (crates/agents/src/delegation_lane.rs)

- pre sha256: 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681
- Stub: inverted `cancel` owner guard (`!=` -> `==`) in temp copy.
- RED log: /tmp/opencode/rA-AUTO004-red.log — compiles, 4 passed / 1 failed.
  - FAIL: auto_004_t04_owner_cancel_reclaims_within_bound
    (delegation_lane.rs:83 — non-owner cancel wrongly succeeded, returned
    Cancelled Status instead of NotOwner).
- Restore: cp back orig, post sha256 identical (pre==post), `git status`
  shows only pre-existing working-tree modification, no new lane diff.
- GREEN log: /tmp/opencode/rA-AUTO004-green.log — 5/5 pass.

## AUTO-006 (crates/agents/src/driver_lane.rs)

- pre sha256: c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd
- Stub: inverted `gate` marker guard (`!contains` -> `contains`) in temp copy.
- RED log: /tmp/opencode/rA-AUTO006-red.log — compiles, 4 passed / 1 failed.
  - FAIL: auto_006_t04_commit_per_milestone (driver_lane.rs:145 —
    order [] vs [M-LOW M-MID M-HIGH], marker bodies gated Incomplete).
- Restore: cp back orig, post sha256 identical (pre==post), no new lane diff.
- GREEN log: /tmp/opencode/rA-AUTO006-green.log — 5/5 pass.

## Full crate GREEN

- Log: /tmp/opencode/rA-agents-full-green.log
  (`cargo test -p opencode-rk-agents`, restored impls):
  - lib unittests: 15 passed
  - delegation_gated: 5 passed
  - delegation_lane: 5 passed
  - driver_lane: 5 passed
  - turn_submission_state: 5 passed
  - doc-tests: 0

## AUTO-005 (declarative tool, no Rust RED per instructions)

Live `tools/check_tdd_pipeline.py` runs, rev auto005-rev-001:

- PASS fixture: exit 0, checks red_proof/frozen_intact/green_on_frozen/
  verifier_rerun all pass. Report: /tmp/opencode/auto005/report-pass.json.
- Mutated-fixture (fail-edited, 1 byte flip GREEM): exit 2,
  frozen_intact=fail, green_on_frozen=fail, reason=mutated-frozen-evidence,
  failing_fixture=fixtures/tdd-pipeline/fail-edited/tests/test_slice.py.
  Report: /tmp/opencode/auto005/report-edited.json.
- Self-report fixture (fail-self-report, worker passes:true, no verifier):
  exit 2, verifier_rerun=fail, reason=self-report-not-evidence.
  Report: /tmp/opencode/auto005/report-self.json.
- Wrong-rev fixture (fail-wrong-rev): exit 2, verifier_rerun=fail,
  reason=verifier-revision-mismatch.
  Report: /tmp/opencode/auto005/report-rev.json.

Note: RED .log greps hit shell `==` quirk under rtk zsh wrapper; fail IDs
and counts confirmed via `read` of log files (AUTO-004 log lines 98-116).
