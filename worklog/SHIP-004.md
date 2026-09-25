# SHIP-004

## Claim

- Task: SHIP-004
- Session: `ses_f27cc582effeFCqhzJJpml0RBy`
- Owned product/test file: `tests/release/full_scope/test_full_scope.py`
- Status: completed as frozen RED artifact; release acceptance remains external

## Source evidence

- `tasks/completion/delivery.json:15` defines SHIP-004 and five obligations.
- `sources/completion/legacy-evidence.json:5-12,36-44` records 258 legacy rows, accepted status as non-release evidence, and per-row `tbd`/`emptyDependencyIds` fields. Rows lack original requirement/test mappings.
- `sources/completion/surface-evidence.json:2-9,25,53-55` marks certification false; all 32 surfaces are unwired, missing, or unverified, with no executable entrypoint traces.
- `sources/completion/audits/AUD-020.json:5,13,35-60` binds audit evidence to an older revision, records 82 TBD stories, 258 required revalidations, missing per-story verification, and `audit-complete; no acceptance claimed`.
- `sources/disc-003-reconciliation.manifest.json:17-21` marks the reconciliation `in-progress-not-release-evidence` with 79 unresolved findings.
- `ralph.completion.json:8-35` makes legacy accepted flags non-release evidence and requires exact integrated release evidence.
- `ralph.json:5-19` is the canonical legacy story set. `requirements/user-requirements.json:2-11` defines mandatory original requirements.
- Mandatory card audit: 165 `tasks/*.md` cards state `Mandatory for full declared release: yes`; eight are absent from canonical plan IDs (`DISC-003`, `REL-005..008`, `TOOL-021..023`).

## Observable contract

The five unittest cases fail closed against the real repository snapshot:

1. Legacy IDs plus original requirement/test mapping must be represented.
2. Every source surface must have `implemented` disposition, executable test, and entrypoint trace.
3. Accepted flags/TBD/unwired/missing evidence must be absent before certification.
4. Revision-bound evidence must match current `git rev-parse HEAD`.
5. Every mandatory card, including runtime-optional cards, must remain in canonical accounting.

No fixtures, mocks, network, Cargo, or product code. Git revision lookup uses an argv list and a two-second timeout.

Observed exact current candidate: `2d04c1c925595b566f617b073129ecee24593eb9`.

## RED observations

Expected current failures:

- T01: legacy evidence rows have no `requirementIds`/`testObligations` mapping.
- T02: 32 source surfaces have `entrypointTrace: none` and `executableTest: none`; certification is false.
- T03: accepted-but-not-release-evidence rows, TBD rows, unresolved dispositions, and blockers remain.
- T04: evidence `inspectedCommit` values predate current HEAD.
- T05: eight mandatory task cards are absent from canonical plan accounting.

## Verification

- Focused command: `python3 -m unittest tests/release/full_scope/test_full_scope.py`
- Required: compile/run RED twice, identical source hash and deterministic failure set.
- No Cargo.

Results:

- `python3 -m py_compile tests/release/full_scope/test_full_scope.py` -> exit 0.
- `python3 -m unittest tests/release/full_scope/test_full_scope.py` -> RED, `5` failures.
- Deterministic second run of the same unittest command -> RED, `5` failures with the same five test IDs and failure categories.
- Frozen SHA-256: `647ca2a2b9deb361be4a14d7ca56b9509ce6f4190085b97323139c257aa7f3f9`.

## Remaining unknowns

- Independent verifier decides whether this RED artifact is accepted and whether release gaps are repaired.
- No production readiness or release acceptance claim.
