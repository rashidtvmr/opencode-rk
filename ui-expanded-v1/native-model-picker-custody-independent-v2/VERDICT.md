# Independent custody-core evaluation — UIC-004/006

Disposition: **READY-AS-SCOPED-CONTROL** for the narrowly exercised stdlib controls; **not PREVERIFIED or ACCEPTED**. The original out-of-grant incident remains recorded and is not erased by this correctly scoped reproduction.

## Exact source custody

The evaluator copied, without modification, the preserved bytes:

- `source/custody.py`: SHA-256 `15bc09e4c218912c32cff486b49f829fb476914d2372af95ef0a05013e30d334`
- `source/test_custody.py`: SHA-256 `d95b59f81e0e23187e6677831608ba00099eb4706906e7e509ca3f0341e22496`

Both hashes match the preserved incident source exactly before and after the run. `hashes.txt` excludes itself and records all independent source and receipt hashes. No source edits were made.

The approved hardcoded test temporary base existed before execution as a directory owned by the current user (`uid=501`, mode `drwxr-xr-x`). Its immediate entries were not removed or inspected beyond the ownership/type check. The test suite creates per-test directories below that base and removes only its own per-test directory during teardown. No external process, network, socket, PTY, database, Cargo, Node, provider, or product binary was used.

## Independent reproduction

Command, executed from the independent `source/` directory with bytecode disabled:

```text
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest -v test_custody
```

Result: **22 tests, 22 passed, exit 0**. Exact stdout, command, and exit receipts are in `receipts/`; their hashes are in `hashes.txt`. This independently confirms the copied unit controls execute and pass, but does not establish picker E2E, PTY, cache, provider, model-history, daemon, or real-binary behavior.

The refused immutable SHA is tested only as a policy string; no preimage is created. Missing/malformed-related coverage includes missing binary, missing manifest, missing manifest SHA, path mismatch, and hash mismatch. The suite does not separately test malformed JSON or every malformed manifest type despite the implementation handling some of those cases.

## Source review findings

### Artifact validation

`validate_artifact` hashes the exact CLI path, rejects missing/non-regular/symlinked paths and symlink-resolved parents, parses JSON, requires a manifest SHA, rejects the refused SHA, and requires a manifest `binary` path (when present) to equal the absolute CLI path. `VerifiedArtifact.argv_identity()` returns the verified absolute path. This is a useful pre-launch custody control.

It is not a complete launch-time custody guarantee: after validation, a caller could replace or mutate the file before a future daemon/PTY opens it. The hash and path record must therefore be treated as a boundary requiring controlled launch/open semantics, not proof that later execution consumes unchanged bytes.

### Owned output

`create_owned_output` rejects absolute paths, empty/dot/dot-dot components, symlink roots/components/leaves, non-directory components, and existing leaves; it uses exclusive `os.mkdir` and rechecks containment. These controls are exercised by the 22 tests.

The docstring’s claim “race-free claim” is too broad. The implementation is not race-free against an adversary able to swap the grant root or an already-checked intermediate directory between checks and `mkdir`, `realpath`, or final inspection. In particular, path-based checks are not descriptor-anchored (`openat`/`mkdirat`-style), and a directory can be replaced after validation. The accurate scope is best-effort path-based symlink/collision rejection under a trusted, non-mutating grant-root owner; it is not a general TOCTOU defense.

### Result state classification

`finish_green` refuses any recorded reason, and `guarded_run` converts exceptions to `HARNESS_UNFINISHED` (exit 3), so an exception cannot produce GREEN. The test confirms this.

`CaseResult` intentionally gives RED exit-code precedence after a later unfinished mark while retaining both reasons. This preserves the original product failure but can classify a case as exit 1 even when a later harness cleanup/error occurred. That is acceptable only if downstream consumers inspect `reasons` and treat any `HARNESS_UNFINISHED` reason as a non-PASS condition. A consumer that treats exit 1 alone as a clean product result would be unsafe. The unit test confirms reason retention, not downstream consumer behavior.

## Remaining issues and boundaries

This module provides narrowly scoped controls and independently reproduces cleanly. It is not a full picker harness and has no product E2E evidence. The race limitation and RED-plus-unfinished interpretation require explicit integration-contract treatment before use in a production acceptance gate. No implementation change is proposed or made by this evaluator.

The incident package remains out-of-grant historical evidence. This evaluation is a fresh correctly scoped copy and does not authorize checkout cleanup, integration, PREVERIFIED status, or ACCEPTED status.
