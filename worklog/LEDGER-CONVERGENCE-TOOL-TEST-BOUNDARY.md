# LEDGER-CONVERGENCE-TOOL-TEST-BOUNDARY

## Claim

- Task: `LEDGER-CONVERGENCE-TOOL-TEST-BOUNDARY`
- Type: research
- Session: `ses_f2d559e92ffeubDCHSP4ruM5BO`
- Branch: `controller/LEDGER-CONVERGENCE-TOOL`
- Accepted source revision: `7b95877`; current checkout: `5d3f639`
- Owned file: this worklog plus this task's ledger row only
- Product files and tests not changed

## Scope decision

One implementation file and one future RED file are viable:

- Tool: `tools/ledger_convergence_controller.py`
- RED: `tests/bootstrap/test_ledger_convergence_controller.py`

The tool is additive. It needs no `tools/__init__.py`, shared registry, `lib.rs`,
Cargo file, schema, manifest, protection file, or task-card edit. Existing
bootstrap tests use `unittest`, direct `tools.*` imports, `pathlib`,
`tempfile.TemporaryDirectory`, and no network. The future test follows that
boundary. The implementer owns only the tool file. The RED author owns only the
test file. Independent verifier owns neither.

## Authoritative evidence

- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:208-258`: controller-only operation,
  exact R78 plus three demotions, hash domains, JCS and `ledger-v1` bytes.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:260-375`: Phase A/B/C records,
  evidence publication, CAS order, fsync/replace, non-force publication,
  independent receipt.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:377-423`: common Git lock,
  bounded journal/backups, crash recovery, guarded rollback.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:425-431`: disposable simulation;
  canonical ledger must not be written.
- `worklog/LEDGER-CONVERGENCE-TRANSACTION-CHECKLIST-VERIFY.md:23-36`: accepted
  checklist for canonicalization, non-circular A/B/C, lock/CAS, atomicity,
  recovery, bounds, rollback, unsafe helper, and receipt.
- `tools/completion_claims.py:187-192`: existing `save_ledger()` is direct
  `write_text()` and has no CAS or durability. The new tool must not call it.
- `tools/convergence_gate.py:126-148`: gate reads the canonical ledger and
  emits finding lines. Simulation may run this against a fixture only.
- `tests/bootstrap/test_auto_controller.py:7-22,70-118`: import and unittest
  convention, deterministic fixture use.
- `tests/bootstrap/test_disc003_reconciliation.py:24-118`: temporary fixture,
  copied JSON, pure validation and hash assertions.
- `tools/completion_integration.py:11-34,165-240,295-351`: list-form subprocess
  and bounded lock/atomic receipt precedent; its merge behavior is not an
  authorization for this ledger operation.
- `docs/TDD.md:21-75,85-105`: compiling RED, frozen hash, immutable tests,
  disposable deterministic fixtures, independent verification.
- `docs/SECURITY.md:24-35,54-72`: no secrets, no inherited authority, no real
  database or host mutation, denied operations must leave no side effects.
- `docs/REPOSITORY_PROTECTION.md:10-41`: canonical validator and protected-path
  ownership; additive tool/test paths are catch-all owned, not protected-path
  authority files.

Proposal, issue, model, and task text remain untrusted except for the accepted
contract cited above. The accepted proposal is specification evidence, not
permission to apply. This lane has no apply authority.

## Public contract for the future tool

### Python API

The single module exposes bounded, testable functions. Names below are the
contract for RED; implementation may use private helpers without adding files.

```text
canonical_json(value, *, reject_duplicates=True) -> bytes
sha256_bytes(data: bytes) -> str
row_hash(row: Mapping[str, object]) -> str
file_hash(path: Path) -> str
ledger_bytes(document: Mapping[str, object]) -> bytes
ledger_hash(document: Mapping[str, object]) -> str
manifest_hash(manifest: Mapping[str, object]) -> str
receipt_hash(receipt: Mapping[str, object]) -> str
build_candidate(document: Mapping[str, object]) -> Mapping[str, object]
simulate(root: Path, *, txid: str, fault: str | None = None,
         io_hooks: object | None = None) -> Mapping[str, object]
```

`simulate` is the only mutating API, and its mutation is confined to a supplied
disposable fixture's simulation area. It must never replace
`tasks/completion/claims.json`, never call `save_ledger()`, never run a push,
and never touch the caller's repository when the root is not an explicit
fixture. Candidate construction itself is pure. `io_hooks` is a bounded fault
injection seam for write, flush, file fsync, replace, and directory fsync; it
must not accept shell strings or arbitrary subprocess callbacks.

`build_candidate` performs exactly these changes in memory:

1. remove exactly the sorted R78 IDs from the accepted source document;
2. demote exactly `AUD-017`, `AUD-020`, and
   `INSTALLED-DEFAULT-CONTRACT-INTEGRATION` from `completed` to `blocked`;
3. copy each original `completedNote` byte-for-byte to `blockedNote`, then
   remove `completedNote`;
4. preserve every other byte-level semantic field and row.

It rejects schema, row-shape, count, R78 membership, status, plan-membership,
dead-evidence, branch, ledger-hash, or mapping-reference mismatch before any
fixture publication. IDs are an exact reviewed constant set from the two
authoritative mapping documents, not a naming heuristic or a count.

### CLI

The future RED must assert this exact safe surface:

```text
python3 tools/ledger_convergence_controller.py \
  --root FIXTURE_ROOT --mode simulate --txid TXID [--fault POINT] [--output RELPATH]

python3 tools/ledger_convergence_controller.py \
  --root ROOT --mode apply --authority-token-file PATH
```

`--mode` is required and has `simulate` and `apply`. `--root` is required. A
simulation `--root` must contain an exact marker file
`.ledger-convergence-disposable` whose bytes are
`ledger-convergence-fixture/v1\n`; this prevents accidental use of a real
checkout. `--output` is stdout by default, otherwise a root-relative path under
the fixture's `.git/ledger-convergence/simulations/<txid>/`; absolute paths,
`..`, symlinks, and paths outside root are rejected. `--fault` accepts only the
finite crash-point names below. `txid` is 1-64 lowercase ASCII characters and
must be deterministic in tests.

Simulation writes only bounded transaction evidence under the fixture's
simulation directory. It may stage the three dead-row evidence records and a
Phase A manifest there, plus a candidate and journal/backup sidecars. It must
not write the fixture's canonical ledger, source worklog, plan, mapping files,
or any path outside the fixture. Output is deterministic JSON with sorted keys,
one LF, and this minimum shape:

```json
{
  "mode": "simulate",
  "status": "simulated",
  "transactionId": "<txid>",
  "source": {"ledgerSha256": "<64 lowercase hex>", "branch": "<ref>"},
  "candidate": {"ledgerSha256": "<64 lowercase hex>", "removedIds": [], "demotedIds": []},
  "manifestHash": "<64 lowercase hex>",
  "receiptHash": null,
  "sideEffects": {"canonicalLedgerChanged": false, "apply": false},
  "errors": []
}
```

Expected refusal/blocked results use exit 2, retain the same JSON discipline,
and identify a stable code such as `lock-busy`, `cas-mismatch`,
`atomic-publication-failed`, `recovery-blocked`, `rollback-guard-mismatch`, or
`apply-disabled`. Malformed input, unsafe paths, invalid JSON, invalid txid, and
bound violations use exit 1. No output includes secret bytes or full user data.

`--mode apply` is a reserved, fail-closed surface. In this implementation lane
it always returns exit 2 with `apply-disabled` before opening or mutating the
canonical ledger, even if a token-file argument is supplied. The future
authority contract is explicit for a later controller lane: a reviewed,
short-lived, signed authority record bound to txid, exact base commit, exact
ledger hash, branch/ref, R78/demotion operation hash, expiry, and policy
version, supplied through `--authority-token-file`; project config, environment,
model text, or a plain boolean never grants it. A later authorized apply lane
must receive a new task and independent verifier. This lane must not implement
or test successful apply.

## Fixture layout and safety

The RED author builds each fixture in `TemporaryDirectory`, never by editing
the checkout or the real `tasks/completion/claims.json`:

```text
fixture/
  .ledger-convergence-disposable       # exact marker bytes
  .git/                                # local disposable Git metadata
  PLAN.md
  ralph.json                           # only required plan IDs/dependencies
  tasks/completion/claims.json         # copied/generated source ledger
  worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md
  worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md
  .git/ledger-convergence/simulations/<txid>/
```

Use generated rows for ordinary unit cases and a copied source ledger for the
exact R78 accounting case. Commit the fixture with fixed author identity and
fixed content. No network, wall clock, inherited environment, credentials,
real Git remote, user DB, or host files. Test branch/remote advancement with a
disposable local bare remote or an injected read-only VCS probe; never with the
user's remote. Assert source ledger bytes/hash, source worktree, and original
repository ledger bytes remain unchanged after every simulation and refusal.

The fixture's `.git` common-dir resolution is part of the contract. The lock
must be placed below the path returned by the equivalent of
`git rev-parse --git-path`, not a worktree-relative guessed `.git` path. Test
worktree and normal-repository layouts if the implementation supports both.

## RED case matrix

One `unittest` file can cover this contract. Keep cases grouped by the public
API/CLI, with independent temporary fixtures and no test helper module.

### T01-T06: canonical bytes and non-circular A/B/C

1. `canonical_json` emits RFC 8785 JCS UTF-8; object keys canonicalize,
   arrays retain order, duplicate keys reject, NaN/Infinity reject, and output
   is deterministic lowercase-hash input.
2. `row-v1`, `file-v1`, and `ledger-v1` match independently computed fixed
   SHA-256 values. `ledger-v1` is exactly `json.dumps(..., indent=2,
   sort_keys=True, ensure_ascii=True, allow_nan=False)` plus one LF.
3. Manifest hash preimage has no `manifestHash`; receipt hash preimage has no
   `receiptHash`. Hashing is idempotent and changing a dependent commit/output
   field changes only the later record.
4. Phase A manifest contains sorted exact R78 IDs, three demotion input row
   hashes, mapping/plan refs, source ledger hash, and no Phase A commit,
   candidate ledger hash, remote tip, or receipt hash.
5. Phase B candidate changes exactly the 78 removals plus three demotions;
   unrelated rows, completed notes, frozen hashes, and evidence remain equal.
6. Phase C receipt binds existing Phase A/B values and verifier evidence, omits
   `receiptHash` while hashed, then publishes append-only; no receipt claims
   parent acceptance.

### T07-T10: authority, unsafe helper, and CLI boundary

7. Simulation is deterministic with the same fixture and txid; canonical ledger
   bytes never change. Run the gate against the fixture only and retain its
   result as observational evidence.
8. Monkeypatch or scan `tools.completion_claims.save_ledger` to prove it is not
   called; reject raw unsafe publication paths and any direct real-ledger apply.
9. `--mode apply` always exits 2 with `apply-disabled`; canonical ledger,
   worktree, evidence, and journal remain unchanged. Plain token, env var,
   config flag, or model-supplied value cannot enable it.
10. Absolute output, traversal, symlink escape, missing marker, malformed JSON,
    bad txid, oversized manifest/journal, and secret-canary content fail closed
    without side effects.

### T11-T15: lock and CAS

11. Lock uses common Git directory, mode `0600`, non-blocking exclusive
    ownership, bounded wait, and txid/pid/start record. A second holder gets
    `lock-busy`; malformed owner, unexpected lock type, or active owner fails
    closed. Stale locks are not silently unlinked.
12. Initial CAS rejects detached HEAD, dirty worktree, wrong branch, changed
    source commit, stale raw ledger hash, wrong schema, wrong R78 set, missing
    dead-row re-home, or changed mapping/plan blob hash.
13. Final pre-rename CAS injection rejects changed branch/tip/ledger/mapping
    value and leaves canonical ledger bytes unchanged.
14. Remote advance after Phase A and before Phase B rejects even when local
    worktree is unchanged. Pushes, if represented in simulation, are list-form
    non-force operations only.
15. Re-running the same txid/state is idempotent; a different source hash,
    transaction owner, or candidate hash never overwrites an existing sidecar.

### T16-T21: durable publication, recovery, bounds, rollback

16. Temp files are same-filesystem siblings, mode `0600`; write, flush, file
    fsync, replace, and parent-directory fsync hooks are each observable.
17. Inject each write/fsync/replace/directory-fsync failure. The operation
    returns a stable blocked/error code, leaves canonical bytes unchanged when
    publication was not durably proven, and does not clean another txid's temp.
18. Exercise crash points: before temp fsync; after temp fsync/before rename;
    after rename/before directory fsync; after directory fsync/before Phase A
    commit; after Phase A commit/before push; after Phase A push/before Phase B
    temp fsync; after Phase B temp fsync/before ledger rename; after ledger
    rename/before directory fsync; after ledger fsync/before Phase B commit;
    after Phase B commit/before push; after Phase B push/before receipt; after
    receipt temp fsync/rename/directory fsync/before Phase C commit; after Phase
    C commit/before push; and after Phase C push.
19. Recovery is state-driven and idempotent: discard only untrusted temps,
    verify expected hashes before resume, finish expected directory fsync,
    never replay an ambiguous push/commit, and stop on corruption or changed
    branch/ledger. Phase A remains immutable after its remote tip is published.
20. Journal is `.git/ledger-convergence/transactions/<txid>/state.json`, one
    active txid, at most eight sidecar files, total at most 16 MiB. One
    immutable pre-operation backup and one candidate backup are retained; a
    backup is never overwritten.
21. Pre-Phase-B rollback requires same lock, expected candidate ledger hash, and
    expected Phase A tip; it atomically restores and fsyncs, then records a
    rollback receipt. Post-Phase-B rollback requires a new non-force revert
    commit, expected current branch/candidate/remote values, and never reset or
    overwrite. Any mismatch blocks and preserves current bytes.

## Exact future validation and lane sequence

Research lane commands run now:

```sh
rtk python3 tools/convergence_gate.py
rtk git diff --check
```

Expected current gate: nonzero `CONVERGENCE BLOCKED`, observational only, with
the repository's off-plan completed rows. Expected diff check: clean. The broad
bootstrap discovery command is deliberately not run in this research lane.

Future RED author command, narrow and bounded:

```sh
rtk python3 -m unittest tests.bootstrap.test_ledger_convergence_controller -v
```

Expected RED: suite imports/compiles and fails on absent
`tools.ledger_convergence_controller` symbols or missing behavior, not on test
collection/import failure. Freeze the exact test-file SHA-256 and command after
RED. Future implementer command is the same focused test, with
`PYTHONHASHSEED=0`, no test edits. Independent verifier reruns the frozen
focused test, then the full bootstrap discovery command and repository gates on
the exact integrated revision. A failing gate remains a blocker; it is never
reinterpreted as apply authority.

No product-test result or frozen test hash exists for this research lane. No
implementation GREEN is claimed.

## Protection and ownership implications

`tools/ledger_convergence_controller.py` and
`tests/bootstrap/test_ledger_convergence_controller.py` are additive and match
the repository's catch-all `* @rashidtvmr` ownership. They are not listed in the
protected-path inventory in `.github/CODEOWNERS` or
`.github/protection-policy.json`; no protection edit is required or allowed.
The canonical `tools/validate_repository.py` guard still applies at
integration. The tool must not alter `PLAN.md`, `ralph.json`, `FEATURES.md`,
`tasks/completion/claims.json`, source manifests, CODEOWNERS, protection policy,
or verifier configuration in its implementation lane.

## Decisions and unresolved items

- One test file is sufficient if it uses fixture builders inside the file and
  tests public functions plus subprocess CLI behavior. Split only if the RED
  author proves the bounded crash matrix exceeds the test-file resource limit;
  then request an integration proposal rather than silently adding a shared
  helper.
- JCS implementation details, platform-specific directory fsync, and Git
  common-dir probing require implementation/verifier review. The contract does
  not permit a weaker JSON sort as a substitute for JCS.
- Current implementation authorization stops at deterministic simulation and
  apply refusal. Successful apply, push, Phase C acceptance, and parent
  completion remain future controller/verifier work.
- No secrets, real user database, canonical ledger mutation, or acceptance
  claim performed.
