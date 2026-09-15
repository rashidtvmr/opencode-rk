# SHARE-004

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007 (Session sharing).
Dependencies: none.
Test obligations: SHARE-004-T01, SHARE-004-T02, SHARE-004-T03, SHARE-004-T04, SHARE-004-T05.

## User-observable outcome

Local share-metadata lifecycle without secret persistence: create records share id plus public URL for one session, sync updates only the last-synced marker, delete removes the record; secrets are held call-locally and never stored. No network, no secret-bearing rows, no secret logging.

## Source evidence

- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`.
- `packages/core/src/share/sql.ts:1-13` (blob `a7a08d0c025436a1266cd642be984742925cd298`): upstream `session_share` row keyed by session id (this slice deliberately stores no secret column).
- `packages/opencode/src/share/share-next.ts:224-359` (blob `60112e10d964b3d9693727501e1e40dac6b1fdf1`): create/sync/delete caller that persists secret-bearing metadata upstream; this slice owns only the local secret-free lifecycle.
- `packages/opencode/test/share/share-next.test.ts:138-224` (blob `fe036be8c959b889516a894266bf4bb390ce1827`): direct lifecycle test matrix mirrored below.
- `specs/storage/remove-opencode-db.md:1-239` (blob `071e70c1841eef11efe1b1dda2792c77494530cb`): storage context, not a lifecycle contract.
- `sources/sharing-ownership-gap.json`: partition `share-metadata-persistence` (ownershipBlocker: secret-bearing user-DB mutation outside the no-user-DB lane); unresolved `local-share-metadata-and-secret-persistence`, `share-create-remove-deletion-and-secret-lifetime`.
- `sources/disc-003-reconciliation.json` finding `opencode.sharing` (partial): durable metadata/secret lifetime and deletion guarantees must be explicitly owned before persistence implementation.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/TOOL-016.md: task-card model (Status/Kind/contract/test obligations).
- Classification: deliberate safer deviation (secret-free local record; upstream persists the remote secret).

## Observable contract

- `ShareMeta { session: SessionId, share_id: ShareId, public_url: String, last_synced_seq: u64 }` (no secret field exists by construction).
- `ShareStore::create(session, share_id, public_url, secret: &ShareSecret) -> &ShareMeta`: secret used only for the call (e.g. handed to transport by caller), never stored; duplicate session => `Err(StoreError::AlreadyShared)`.
- `ShareStore::mark_synced(session, seq)` advances `last_synced_seq` monotonically (regressions rejected with `Err(StoreError::SeqRegression)`); `remove(session)` deletes the record; `get(session) -> Option<&ShareMeta>`.
- Session deletion cascades: `remove_session(session)` deletes the share record with it (foreign-key lifecycle parity, in-memory).
- Deterministic: same op sequence => identical store contents; no wall-clock stored.
- Suggested module boundary: `crates/share/src/store.rs` (crate `opencode-rk-share`); worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- Already shared: `Err(AlreadyShared)`; existing record unchanged.
- Unknown session on sync/remove: `Err(StoreError::NotFound)`; store unchanged.
- Seq regression: `Err(SeqRegression)`; marker unchanged.
- Invalid public URL (non-https or unparseable): `Err(StoreError::InvalidUrl)`; nothing stored.
- Secret safety: `ShareSecret` is `&`-borrowed, zeroized by owner, never cloned into the store, never logged; store debug output redacts share id suffix. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory fixtures only.

## Resource bounds

- One record per shared session, hard cap `MAX_SHARES=1024`; `create` beyond cap => `Err(StoreError::Full)`.
- Bounded strings: `public_url` max 2048 bytes; no unbounded retained output.
- Single-owner in-memory map; no thread/task/clock; drop frees all records; no detached state.
- Zero hidden cost when unused: empty store allocates nothing.

## Test obligations (frozen)

- SHARE-004-T01 (lifecycle happy path): `create` => `get` returns matching `share_id` + `public_url`; `mark_synced(seq=7)` => `assert_eq!(last_synced_seq, 7)`; `remove` => `get` returns `None`.
- SHARE-004-T02 (cascade + duplicate): duplicate `create` => `assert_eq!(err, AlreadyShared)`; `remove_session` => share record gone with session.
- SHARE-004-T03 (failure states): unknown session sync/remove => `assert_eq!(err, NotFound)`; seq regression => `assert_eq!(err, SeqRegression)`; `http://` URL => `assert_eq!(err, InvalidUrl)`; oversized store => `assert_eq!(err, Full)`.
- SHARE-004-T04 (no secret retention): after `create`, store memory/debug contains zero secret bytes (scan store + `format!("{:?}", store)`); secret buffer zeroized after call.
- SHARE-004-T05 (safety + determinism): same op sequence twice => identical stores; no test writes outside disposable fixture dir (assert DB/fixture-outside untouched); captured logs contain zero secret bytes and zero URL query/fragment bytes.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, gap files + disc-003 `opencode.sharing` finding (done, see evidence).
2. Contract: defined above.
3. Author tests SHARE-004-T01..T05; establish compiling RED (fail: no store module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust secret-free share store.
6. GREEN, refactor, rerun; negative tests (duplicate, unknown, regression, bad URL, full, secret scan).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/SHARE-004.md
cargo test -p opencode-rk-share store
cargo check --workspace
```
