# SHARE-001

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007 (Session sharing).
Dependencies: none.
Test obligations: SHARE-001-T01, SHARE-001-T02, SHARE-001-T03, SHARE-001-T04, SHARE-001-T05.

## User-observable outcome

Pure deterministic share-snapshot merge with secret validation: typed session/message/part/diff/model records from multiple batches merge into one key-sorted last-write-wins list; unknown share ids fail NotFound and mismatched secrets fail InvalidSecret before any merge output is returned. No network, no DB mutation, no secret logging.

## Source evidence

- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `packages/enterprise/src/core/share.ts:10-172` (blob `781bcd5cbeb86000cb76a50a247f4666b0b72f43`): deterministic key-based last-write replacement plus NotFound/InvalidSecret secret validation.
- `packages/enterprise/test/core/share.test.ts:25-263` (blob `35d52735e2efb5efd68c75e35b2667a0935ba51f`): direct merge/validation test matrix mirrored below.
- `packages/enterprise/src/routes/api/[...path].ts:29-140` (blob `4677d68d33c48c6a030ae66100d59823fc241aa9`): storage-backed caller that this slice explicitly does not own.
- `specs/storage/remove-opencode-db.md:1-239` (blob `071e70c1841eef11efe1b1dda2792c77494530cb`): storage context, not a merge contract.
- `sources/sharing-ownership-gap.json`: partition `deterministic-share-merge-and-secret-validation`, candidate fragment `deterministic-share-merge`; unresolved `deterministic-data-keying-merge-and-snapshot-size-policy`.
- `sources/disc-003-reconciliation.json` finding `opencode.sharing` (partial): pure merge/secret validation must be separated from DB persistence, legacy migration, remote HTTP transport, auth, subscription lifetime, and support-admin behavior.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-016.md: task-card model (Status/Kind/contract/test obligations).
- Classification: native V2 behavior (merge + secret validation), deliberate resource-bounded deviation (explicit input/record caps; upstream specifies no target size cap).

## Observable contract

- `ShareId`, `ShareSecret` (opaque, zeroized on drop, never `Display`/`Debug`-logged); `ShareRecord { kind: Session|Message|Part|SessionDiff|Model, key: String, payload: CanonicalJson }` with identity key `(kind, key)`.
- `validate(secret_stored, secret_present) -> Result<(), ShareError>`; `merge_share_records(batches: &[Vec<ShareRecord>]) -> Vec<ShareRecord>`: last-write-wins per identity key (later batch wins), output sorted by `(kind, key)`; deterministic: same batches => identical order; no wall-clock in ordering.
- `apply_sync(existing: &[ShareRecord], incoming: &[ShareRecord], share_id, secret_stored, secret_present) -> Result<Vec<ShareRecord>, ShareError>`: validates secret first, then merges.
- Suggested module boundary: `crates/share/src/merge.rs` (new crate `opencode-rk-share`); worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- Unknown share id: `Err(ShareError::NotFound)`; no merge output returned.
- Secret mismatch: `Err(ShareError::InvalidSecret)`; no merge output returned, no secret bytes in error value or logs.
- Schema-invalid record: skipped with `skipped: u32` count; valid records still merge; never abort whole merge.
- Over caps: `Err(ShareError::TooLarge)`; never exceed caps; never OOM on adversarial batches.
- Secret safety: secrets compared in constant time, never logged, never embedded in transcripts or error strings. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory fixtures only.

## Resource bounds

- Input caps: `MAX_BATCHES=16`, `MAX_RECORDS_PER_BATCH=10000`, `MAX_RECORD_BYTES=1_048_576`; `merge` checks caps before allocating the working map.
- Working set: `O(unique record keys)` entries in one call-local map; map dropped on return (no retained output, no unbounded queue).
- Owner/cancel path: caller owns all inputs/outputs; pure synchronous function, no thread/task/clock; optional `cancel: &AtomicBool` checked per batch => `Err(ShareError::Cancelled)`.
- Zero hidden cost when unused: no global state, no background work, no I/O.

## Test obligations (frozen)

- SHARE-001-T01 (merge happy path): batches with overlapping identity keys: `assert_eq!(out.len(), unique_keys)`, later batch value wins per key, output sorted by `(kind, key)`.
- SHARE-001-T02 (determinism): two `merge_share_records` runs on same batches: `assert_eq!(out1, out2)`; key order identical.
- SHARE-001-T03 (secret validation): unknown id => `assert_eq!(err, NotFound)`; wrong secret => `assert_eq!(err, InvalidSecret)`; correct secret => `Ok(merged)`; error values and captured logs contain zero secret bytes.
- SHARE-001-T04 (caps + invalid records): oversized batch => `assert_eq!(err, TooLarge)`; batch with 2 invalid records => valid records merged and `assert_eq!(skipped, 2)`.
- SHARE-001-T05 (safety + purity): no test writes outside disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain zero secret bytes and zero record-payload bytes beyond record keys; cancel flag pre-set => `assert_eq!(err, Cancelled)`.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, gap files + disc-003 `opencode.sharing` finding (done, see evidence).
2. Contract: defined above.
3. Author tests SHARE-001-T01..T05; establish compiling RED (fail: no merge module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust merge + secret validation.
6. GREEN, refactor, rerun; negative tests (wrong secret, unknown id, oversized batch, invalid records, pre-cancelled).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SHARE-001.md
cargo test -p opencode-rk-share merge
cargo check --workspace
```
