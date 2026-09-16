# AUTO-005 worklog

## Claim

Trusted RED/GREEN pipeline enforcement fragment. Additive tool only. No product Rust port in this slice.

## Source evidence

- HEAD `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- `tasks/AUTO-005.md:9-11` outcome, `:24-31` contract, `:51-57` T01-T05.
- `PLAN.md:115-120` ADR-007 implementer cannot accept; `:162-180` RED/GREEN lifecycle; section 8 serial controller.
- `docs/TDD.md:9-20` authority, `:21-56` lifecycle/RED/freeze, `:58-75` GREEN/evidence.
- `docs/SECURITY.md:54-60` scope/verifier config not worker-writable.
- `requirements/user-requirements.json` REQ-002 + REQ-004.
- `tools/plan_model.py:44` AUTO rank 6.
- `ralph.json:303-319` AUTO-005 `in-progress`.
- `sources/backlog-exhaustion.json:51-73` AUTO-005 `explicit-blocker automation-ownership-undefined`, taskCard/worklog null — stale, controller acceptance external.

## Observed scenario

- Pre-change repo had no pipeline-enforcement fragment before `tools/check_tdd_pipeline.py` (`PARTITION=trusted-red-green-pipeline` `:31`): no manifest/receipt consumer enforcing propose->freeze->verify->accept, no edited-test/self-report/wrong-revision rejection path.
- Tool contract verified live 2026-09-16 against disposable fixtures at `/tmp/opencode/a005b/` (no repo writes, no network):
  - PASS fixture: manifest pins 1 file + red(compiled, exit 1, missing-behavior)/green(exit 0)/verifier(independent, exit 0) receipts on rev r1 => exit 0, `passed:true`, all 4 checks pass.
  - RED/edited-test fixture: same manifest, 1 byte mutated in frozen file => exit 2, `reason:mutated-frozen-evidence`, `failing_fixture` names the file.
- Tool SHA-256 `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474`.
- Agents-crate 30-pass log is unrelated scope, not AUTO-005 evidence. Broker-gated probe note (`submit_gated`, RED fail / GREEN pass, deleted temp probe) lives in AUTO-004 worklog; AUTO-005 records only the pipeline-role analogy, no freeze by design — freeze/rerun is controller decision.

## Target boundary

- Owned: `tools/check_tdd_pipeline.py` only (tracked, existing).
- No `crates/agents` changes for AUTO-005. No `state/` writes from worker. No edits to shared `lib.rs`/`Cargo.toml`/schemas.
- Tests: no dedicated Rust suite; declarative task, no invented product tests per TDD §3. Executable check = the two disposable-fixture runs above.

## Tests

- Tool SHA-256: `eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474` (on disk at verify time; not controller-frozen — no freeze exists).
- RED baseline: live edited-test rejection run (exit 2, `mutated-frozen-evidence`), not a compiled Rust RED. No frozen manifest, no frozen command manifest, no controller-frozen hash. No RED run history beyond the fixture runs above.
- Contract stats: `PARTITION=trusted-red-green-pipeline` (`tools/check_tdd_pipeline.py:31`); bounds 256 files / 8 MiB inputs / 64 KiB report; exits 0 pass / 2 assurance fail / 1 tool error; rejects edited tests (`mutated-frozen-evidence`), self-report (`self-report-not-evidence`), wrong revision (`verifier-revision-mismatch`), blocked-stop path.
- Sibling guard: `tools/check_release_tdd.py:6,103-119` REL-002 partition refuses AUTO-005 material (`duplicate-of-existing-owner:AUTO-005`); reverse guard in pipeline tool refuses non-`trusted-red-green-pipeline` with `duplicate-of-existing-owner:REL-002`.
- `tools/lane_gate.py:31-44` storage-only, no AUTO-005 lane. `tools/ralph_loop.py:58-61` mandatory verification = `validate_repository.py` + `lane_gate.py --run`; pipeline tool not wired into controller verification list.
- GREEN receipt: PASS-fixture run exit 0 (disposable dir only). Agents regression 30 passed is unrelated scope, not AUTO-005 evidence.

## Decisions

- Keep tool additive: reads manifest/receipts read-only, writes only `<out>`. Stdlib only, single process, no threads, no network.
- No Rust port: card suggests controller-owned `tools/` + `config/controller.settings.json` + `state/` ledger; prototype `ralph_loop.py`/`lane_gate.py`/`plan_model.py` already exist. Worker ships additive fragment only.
- No worker `state/` writes, no test-source edits, no verifier-config edits.

## Remaining unknowns

- No controller-frozen hash + command manifest, no compiled RED proof, no GREEN-on-frozen run, no independent verifier rerun receipt. Per TDD these are mandatory; verifier must reject acceptance until controller freezes and reruns.
- Wiring `check_tdd_pipeline.py` into `tools/ralph_loop.py` verification or `tools/lane_gate.py` lanes is integrator/controller decision, not worker scope.
- Ledger blocker vs on-disk tool conflict unresolved; acceptance external.

## Broker+tokio slice (2026-09-16, shared lane AUTO-004/005/006)

- Pipeline-role note: `submit_gated` is propose->freeze->verify in miniature — Deny/human (unfrozen/edited analog) blocks admission with zero state; only Allow (frozen-verified analog) proceeds. See AUTO-004 worklog for RED fail / GREEN pass / hashes; impl `crates/agents/src/delegation_lane.rs` +80/-0, frozen tests untouched.

## Sole-writer revalidation (2026-09-16, rev 248f519)
- Gated tests pre==post sha256 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7 (untouched).
- Declarative RED: broker_allows inverted. /tmp/opencode/wA-AUTO005-declarative-red.log 0/5 fail (t01..t05). GREEN 5/5.
- Live tool /tmp/opencode/wA005 rev auto005-rev-001: PASS exit 0; edited exit 2 mutated-frozen-evidence; self exit 2 self-report-not-evidence; wrong-rev exit 2 verifier-revision-mismatch.

## Sole-writer revalidation xA (2026-09-16, rev 248f519)
- Gated tests pre==post sha256 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7 (untouched); impl delegation_lane.rs pre==post 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681.
- Declarative RED: broker_allows inverted. /tmp/opencode/xA-AUTO-005-red.log 0/5 fail (t01 :18, t02 :34, t03 :44, t04 :55, t05 :77). GREEN /tmp/opencode/xA-AUTO-005-green.log 5/5.
- Live tool /tmp/opencode/xA005 rev auto005-rev-001: PASS exit 0; edited exit 2 mutated-frozen-evidence; self exit 2 self-report-not-evidence; wrong-rev exit 2 verifier-revision-mismatch. Tool sha256 eea1ac0f21d5d047f9455b6d9b77c5a8f3942ad4ad1ff34efcff9b723e288474.
