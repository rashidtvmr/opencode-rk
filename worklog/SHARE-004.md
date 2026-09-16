# SHARE-004 — secret-free share-metadata lifecycle

## Claim
Own `crates/sessions/src/share_store.rs` (lane-variant; task-card `crates/share/src/store.rs` never existed — no `opencode-rk-share` crate). In-memory `ShareStore`: create/sync/remove per session, secret borrowed-never-stored, session-delete cascade. No test/lib.rs edits this session.

## Source evidence (pin 95daf90670b7c039c436c85537da5fbfe2205b41)
- `packages/core/src/share/sql.ts:1-13` (blob `a7a08d0c…` per card): upstream `session_share` row keyed by session id; this slice deliberately stores NO secret column.
- `packages/opencode/src/share/share-next.ts:224-359`: upstream create/sync/delete persists secret-bearing metadata; this slice owns only local secret-free lifecycle.
- `packages/opencode/test/share/share-next.test.ts:138-224`: lifecycle matrix mirrored in T01..T03.
- `sharing-ownership-gap.json` `share-metadata-persistence` (ownershipBlocker: secret-bearing user-DB mutation); `disc-003 opencode.sharing` (partial): secret lifetime/deletion must be owned before persistence.
- Classification: deliberate safer deviation (secret-free local record).

## Observed scenario
- `src/share_store.rs` (`#![forbid(unsafe_code)]`, `#[path]`-included, header states NOT wired into `lib.rs`): `ShareMeta{session,share_id,public_url,last_synced_seq}` (no secret field by construction); `ShareStore::create(session,share_id,url,&secret)` borrows secret, `AlreadyShared` on dup; `mark_synced` monotonic (`SeqRegression`); `remove`/`remove_session` cascade; `get`. `ShareId` Debug redacts suffix; URL https-only.
- Frozen suites: `share004_t01_lifecycle_happy_path`, T02 cascade+duplicate, T03 failure states (NotFound/SeqRegression/InvalidUrl/Full), T04 no-secret-retention, T05 safety+determinism.

## Target boundary
- One record/session, `MAX_SHARES=1024` (`Full` beyond), `public_url` ≤2048B https-only; single-owner map, no thread/clock; drop frees all.
- `ShareSecret` `&`-borrowed, never cloned/stored/logged, zeroized by owner; store Debug redacted; no DB writes; disposable fixtures only.

## Tests (frozen, verify-only rerun)
- Suites: `tests/share_store.rs` + `tests/share_store_lane.rs`, 5 tests each.
- GREEN (2026-09-16, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, rev `248f519` + workdir dirt, serial, `timeout 120 rtk`):
  `share_store` 5/5, `share_store_lane` 5/5 (each `cargo test -p opencode-rk-sessions --test <suite>` EXIT=0).
- Frozen hashes (sha256): test `share_store.rs` `dd21adf7…`, lane `5a95a015…`; src `share_store.rs` `418be003…`, lane `7c32b321…`. No frozen test edited.
- RED history: SHARE-001.md:186-217 — cap mutation (`>=MAX_SHARES`→`>=MAX_SHARES+1`) gives `share_store` 4/5 (T03 RED only), restored byte-identical GREEN.

## Decisions
- No code change: implementation GREEN; dedicated worklog only.

## Remaining unknowns
- `crates/share` acceptance mapping pending controller allowlist. No other gaps observed.
