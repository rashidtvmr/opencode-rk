# LEDGER-CONVERGENCE-TOOL-GREEN-VERIFY

## Claim
- Task: `LEDGER-CONVERGENCE-TOOL-GREEN-VERIFY`
- Role: Independent transactional-controller security verifier
- Session: `ses_verifier_green_LEDGER_CONVERGENCE`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-ledger-convergence-tool-green`
- Branch: `verify/LEDGER-CONVERGENCE-TOOL-GREEN`
- Owned file: `worklog/LEDGER-CONVERGENCE-TOOL-GREEN-VERIFY.md`
- Candidate commit: `49ab00c4932793fd02562e1e848b6ffd3904e355`
- RED-verify baseline commit: `d62fac6`

## Goal
Independently verify commit `49ab00c` implements the frozen simulate-only controller
contract with 21/21 GREEN, no test edits or real repository mutation, and
unconditional apply refusal. No source/test repair, successful apply, real ledger
mutation, reconciliation, merge, or release acceptance.

## Source evidence (authoritative)
- `AGENTS.md` worker contract (authority/ownership, TDD, stop semantics, landing).
- `docs/TDD.md` (frozen RED hash binding; tests immutable to implementers).
- `docs/SECURITY.md` (capability broker, marker/path fail closed, no secret access).
- Frozen test suite: `tests/bootstrap/test_ledger_convergence_controller.py`
  - SHA-256: `056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe`
  - Identical at `0cd0fa0`, `d62fac6`, and `49ab00c` (confirmed: zero diff between commits).
- RED-verify worklog: `worklog/LEDGER-CONVERGENCE-TOOL-RED-VERIFY.md` (historical
  50 assertion failures against absent controller).
- Implementation diff `d62fac6..49ab00c`: single tool file
  `tools/ledger_convergence_controller.py` (+569 lines).

## Source/security matrix (T01-T21 path audit)

| Test | Contract | Evidence in source |
|------|----------|--------------------|
| T01 | JCS UTF-8 canonical JSON; rejects dup keys, non-finite; 1.0->1, -0.0->0 | `canonical_json()`, `_jcs_value()`, `_pairs()` raise `_DuplicateKey` |
| T02 | Fixed hash vectors for `row_hash`, `file_hash`, `ledger_hash` | `sha256_bytes`, `row_hash`, `file_hash`, `ledger_bytes`, `ledger_hash` |
| T03 | `manifest_hash`/`receipt_hash` exclude own member | `_without(member)`, `manifest_hash`, `receipt_hash` |
| T04 | Phase A manifest: sorted 78 RETIRE_IDS, no dependent fields | `manifest["operation"]["removeIds"] = sorted(RETIRE_IDS)`; fields absent |
| T05 | `build_candidate` removes exactly 78 IDs, three demotions (note-preserving) | `build_candidate()` |
| T06 | Phase hashes append-only; no parent acceptance; no `acceptance` key | `sideEffects.apply=False`, `receiptHash=None` |
| T07 | Deterministic; protected bytes unchanged | fixture temp dir, `_protected_bytes` equality |
| T08 | No `save_ledger` call; no `shell=True` | verified by monkeypatch + grep |
| T09 | apply refuses (exit 2) before opening canonical ledger | `main()`: `--mode apply` -> `apply-disabled`, exit 2, before `simulate` |
| T10 | Unsafe paths/marker/json/txid bounds/secret canary fail closed | `_safe_output`, marker `read_bytes!=MARKER`, `_invalid` |
| T11 | Git dir lock 0600, non-blocking CAS, fail closed | `_acquire_lock` O_EXCL 0o600, `lock-busy` |
| T12 | Initial CAS rejects detached/dirty/schema/status/membership/mapping | `_git_preconditions`/`_git_identity`, `_validate_source` |
| T13 | Final CAS injection: canonical bytes unchanged | `before_final_cas` hook, `cas-mismatch` |
| T14 | Remote advance after phase A: hard nonforce failure | `before_final_cas` / remote probe |
| T15 | Same-txid idempotent; sim dir bounded (<=8) | `_simulation_dir` rglob count, `_durable_write` |
| T16 | All durable hooks observable: write/flush/file_fsync/replace/directory_fsync | `IoHooks`, `_call_hook` |
| T17 | Each durable failure -> stable blocked code; ledger preserved | `IoHooks(fail=...)`, `_error` stable |
| T18 | Every documented crash point bounded, nonpublishing | `CRASH_POINTS` set (16 points), `--fault` |
| T19 | Recovery state-driven; rejects corrupt/changed state | `_state_valid`, `recovery-blocked` |
| T20 | Journal sidecars bounded (<=8 files, <=16 MiB); backups <=2 | transaction dir count/size, `backup` filter |
| T21 | Guarded rollback requires expected hash tip + lock | `rollback-guard-mismatch` on marker drift / state mutation |

## Implementation audit

### Counts
- `RETIRE_IDS`: 78 (matches "R78").
- `DEMOTION_IDS`: 3 (AUD-017, AUD-020, INSTALLED-DEFAULT-CONTRACT-INTEGRATION).
- `CRASH_POINTS`: 16 (matches test list exactly).

### Hash domains (frozen vectors verified by direct function call)
- `row_hash({"completedNote":"ok","session":"s","status":"completed"})` ->
  `92bca4c9c7cc53b53854ff687764fab4db0be9784a1a35bd4ba3a1d19923c39` (MATCH)
- `file_hash(vector.bin)` -> `bb157861a164e35cdde9d726b0af9ce2765a8f530c35d9e45732b94ee65e9557` (MATCH)
- `ledger_hash({"claims":{"x":{"status":"completed"}},"schemaVersion":1})` ->
  `ab97c8b7b1aa2188adb6b1c0cd66f2fd12340bff8ea0ed1fc05fb54464f86fd5` (MATCH)

### Security guards
- `apply` (mode="apply"): prints `_error("apply-disabled", mode="apply")`, returns
  exit 2, BEFORE any root inspection, file read, or git operation. Confirmed
  against nonexistent root and real-looking root+token-file inputs (both exit 2,
  `apply-disabled`, canonical bytes unchanged).
- No `shell=True`, no `os.system`, no `eval`/`exec`, no network (`urllib`/`requests`/
  `aiohttp`), no `subprocess.call` (only bounded `subprocess.run` with 10s timeout).
- No stubs/placeholders/todo!/unimplemented!/TODO/FIXME (grep: only match is the
  frozen string `FIX-SESSIONS-STUBS` inside `RETIRE_IDS` literal).
- `_safe_output` rejects absolute paths, `..` parts, symlinks in parent; resolves
  against simulation dir base.
- Marker check: `marker.read_bytes() != MARKER` -> invalid/blocked; marker
  `is_symlink()` rejected.
- Lock: `O_CREAT|O_EXCL` 0600, `FileExistsError` -> `lock-busy` (nonblocking).
- Canonical/protected files: `_protected_bytes` equality asserted in every test;
  on-disk `PLAN.md`, `ralph.json`, mapping worklogs byte-identical to HEAD
  (verified via `git diff HEAD`).

## Commands / results
```
shasum -a 256 tests/bootstrap/test_ledger_convergence_controller.py
056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe  -
```
- Frozen hash exact: matches expected.
```
python3 -m py_compile tools/ledger_convergence_controller.py
exit: 0
```
```
PYTHONHASHSEED=0 python3 -m unittest tests.bootstrap.test_ledger_convergence_controller -v
Ran 21 tests in 9.452s
OK
```
- Focused 21/21 GREEN.
```
PYTHONHASHSEED=0 python3 -m unittest discover -s tests/bootstrap -p 'test_*.py'
Ran 231 tests in 16.747s
FAILED (failures=19)
```
- 19 failures are ALL in `test_auto_controller`, `test_backlog_exhaustion`,
  `test_ci_enforcement`, `test_validate_plan` — pre-existing convergence/gap
  analysis failures. None are `test_ledger_convergence_controller` tests.
  No candidate-caused regression in the frozen controller suite.
```
git diff --check
(clean for controller test + tool; claims.json has only this verifier's row)
git status --short
 M tasks/completion/claims.json   (only the verifier claim row added)
```
- Protected files (PLAN.md, ralph.json, DISC-003 worklogs) byte-identical to HEAD.

### Independent apply refusal (manual reproduction)
- Nonexistent root + token: exit 2, `{"errors":["apply-disabled"],"mode":"apply","status":"blocked"}`.
- Real-looking disposable root + `authority.token` containing `Bearer sk-...`: exit 2, `apply-disabled`.
- Canonical ledger (claims.json) hash identical before/after each apply invocation.

## Hashes
- Frozen test SHA-256: `056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe`.
- Candidate commit: `49ab00c4932793fd02562e1e848b6ffd3904e355`.
- On-disk canonical ledger hash (unmutated by simulate/apply): `405042acbb2e64edd4e00dc9abf55cae296b01e4de23f37c823e050cc6705c3a`.

## Resource observations
- One bounded Python unittest process; subprocess Git calls timeout 10s.
- Simulation sidecars bounded (<=8 files, <=16 MiB per T20).
- No network, no daemon, no background process, no real ledger mutation.
- `git diff --check` clean for product files.

## Remaining unknowns / gaps
- Independent verifier must still run `tools/convergence_gate.py` and
  `tools/lane_gate.py`; candidate is GREEN for its own bounded contract only.
- The 19 pre-existing bootstrap failures are out of scope (convergence/gap layer),
  not caused by commit `49ab00c`.
- No real apply or acceptance claim made; apply is unconditionally disabled.

## Verdict
ACCEPT for the bounded simulate-only controller contract at commit `49ab00c`:
frozen test hash exact, py_compile pass, 21/21 GREEN with zero test edits, apply
disabled before any root inspection, canonical/protected files unmutated, no
stubs/unsafe/secret/network/push paths. Simulation-only; never apply authority.
