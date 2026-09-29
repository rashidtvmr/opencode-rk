# SHIP-004 scratchpad: Full legacy and source-parity release proof (test-author lane)
# Session: ses_f115896b4fferTfuqdQsIe3y6Q (resume) | Status: blocked pending implementation/freeze authority

## Claim
- Ledger tasks/completion/claims.json: SHIP-004 reclaimed not-started (orchestrator evidence) then claimed in-progress session ses_f115896b4fferTfuqdQsIe3y6Q, scratchpad worklog/SHIP-004.md.
- Owned file (only): tests/release/full_scope/test_full_scope.py.
- Role: independent test author only. No implementation, no ledger edits
  beyond own claim row, no frozen-test edits, no expected-output generation.

## Source evidence (exact commit/path/symbol)
- Candidate revision (lane branch): 8a91a7b49a5a1c948218ad8f176d44e015530dcb
  (`git rev-parse HEAD`, branch lane/SHIP-004-phase1).
- Card: tasks/completion/delivery.json:15 — SHIP-004 deps PAR-010, AUD-020,
  paths tests/release/full_scope, 5 journey tests (legacy IDs, surfaces,
  accepted-but-unwired/TBD, stale-revision invalidation, optional-flag accounting).
- Legacy ledger: sources/completion/legacy-evidence.json — taskId PAR-001,
  inspectedCommit 5af7884cf7637c0760d985da7a03f2f99ccd0c78 (short 5af7884),
  legacyCount 258, tasks 258, recordedStatusesNotEvidence {"accepted": 258},
  statusIsReleaseEvidence all False, tbd True on 82 rows, emptyDependencyIds
  True on 237 rows, shardCounts per AUD-001..AUD-020.
- Surface ledger: sources/completion/surface-evidence.json — status
  "in-progress-not-release-evidence", certified False, inspectedCommit
  1f7640ad574e5081dff27294dd20244edb9f362c, 32 surfaces, executableTest
  "none" on 32/32, entrypointTrace "none" on 32/32, dispositions
  unwired 19 / unverified 10 / missing 3, certificationBlockers 5 rows
  (all unwired/missing/unverified, unknown dynamic surfaces, dev-delta
  5a8335857b0ebec44ef6aa1d52b339cf25c329ca TBD, OpenTUI fork absent,
  no independent receipts), warning "scaffold only; not release evidence".
- Claims: tasks/completion/claims.json — 0 of 258 legacy IDs present as
  claim keys (legacy plan ralph.json IDs vs completion-plan IDs are disjoint
  namespaces; no release evidence set maps them).
- Requirements: requirements/user-requirements.json — requirements 47,
  mandatory True on 47/47. No release-ledger.json exists in tree.
- Owned path absent before lane: tests/release/ did not exist.

## Observed scenario
- Both evidence ledgers pin revisions older than HEAD (5af7884, 1f7640a vs
  8a91a7b). Surface ledger self-declares scaffold, certified False.
- No tests/release/full_scope evidence set exists, so none of the 5 SHIP-004
  journey clauses can pass on the integrated tree.

## Target boundary
- RED file tests/release/full_scope/test_full_scope.py, stdlib unittest only,
  deterministic, read-only (no network, no clock in verdict, no DB mutation).
- 5 tests mirror the 5 journey clauses: T01 legacy+requirement coverage,
  T02 surface test-mapping, T03 unwired/TBD rejection, T04 stale-revision
  invalidation, T05 mandatory optional-flag accounting.
- Each asserts the releasable end-state; each FAILS now for real missing
  evidence (stale pins, certified False, 32 unmapped surfaces, 82 TBD rows,
  258 unrepresented legacy IDs, no release ledger). No syntax/import failure.

## Tests
- RED command (bounded): timeout 110 python3 tests/release/full_scope/test_full_scope.py
- Result: 5 tests run, 5 fail for missing release evidence (see tail below).
- Frozen: sha256 recorded after final write; command + hash in this file.
- Freeze authority note: test freeze/hash acceptance belongs to controller/
  verifier per TDD contract; lane records hash, does not self-accept.

## Decisions
- Assert end-state (certified proof on HEAD) rather than re-implementing
  validator tools; keeps the smallest executable RED that fails for the
  genuine gap.
- HEAD resolved at runtime via `git rev-parse HEAD` (bounded subprocess,
  timeout 15s) so T04 stays revision-sensitive after later commits.
- Optional-flag accounting (T05) checks mandatory REQ coverage in a release
  ledger artifact; absent ledger fails closed, never passes by default.

## Remaining unknowns
- Release ledger schema/location for full-scope proof is not defined by the
  card beyond paths tests/release/full_scope; implementer + integrator must
  define it. Lane stays blocked pending implementation/freeze authority.
- Whether controller wants this RED frozen as-is or re-authored under its
  own freeze manifest; no acceptance claimed by this lane.

## Resume 2026-09-30 (ses_f115896b4fferTfuqdQsIe3y6Q)
- Reclaimed SHIP-004 via tools/completion_claims.py claim (prior row not-started). Claim verified in-progress.
- Pre-execution defect corrected: ROOT parents[2]->parents[3] (file depth tests/release/full_scope/ => repo root). py_compile OK.
- Bounded RED: python3 tests/release/full_scope/test_full_scope.py (stdlib unittest, direct; no system timeout binary available in lane). Result FFFFF, Ran 5 tests, FAILED failures=5. All five fail on missing release evidence (T01 0/258 legacy + 47 reqs missing; T02 0/32 mapped certified=False; T03 82 TBD/32 non-impl/5 blockers/258 record-only; T04 legacy 5af7884/surface 1f7640a vs HEAD 8a91a7b; T05 47 mandatory uncovered). No import/syntax/path failure.
- sha256 a567d2f58514136e7cf4d9f90b62de4dfbc3ed04532e6229634a5dd7d52202a2. Freeze acceptance belongs to controller/verifier; lane does not self-freeze.
- Status set blocked pending implementation/freeze authority. Commit only test+scratchpad+claims; push lane/SHIP-004-phase1.

## RED evidence (prior owner)
- Command: timeout 110 python3 tests/release/full_scope/test_full_scope.py
- Outcome: FAIL (5/5 fail, 0 pass) — genuine missing release evidence.
- Tail (verbatim):
  T01 legacy IDs represented: 0/258 in evidence set; 82 TBD rows.
  T02 surfaces with executable test + entrypoint trace: 0/32; certified=False.
  T03 accepted-but-unwired/TBD/missing evidence blocks certification.
  T04 stale pins (legacy 5af7884, surface 1f7640a) vs HEAD 8a91a7b.
  T05 no release ledger covers 47 mandatory requirements.
- sha256 (resume RED): a567d2f58514136e7cf4d9f90b62de4dfbc3ed04532e6229634a5dd7d52202a2 (freeze authority: controller/verifier)
