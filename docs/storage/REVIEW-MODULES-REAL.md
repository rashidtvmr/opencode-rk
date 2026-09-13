# Adversarial Review: Storage V2 Modules (NOW-IMPLEMENTED)

Reviewer: Michael Feathers-style adversarial pass, evidence only.
Scope: `crates/storage/src/{gc_v2,admission_v2,execution_v2,approvals_v2,snapshot_v2,import_v2,quota_v2}.rs`
against `crates/storage/schema/v2/workspace.sql`, `crates/storage/src/lib.rs`,
`crates/storage/src/{writer_v2,schema_v2}.rs`, the `crates/storage/tests/*.rs` suites, and the
Python contracts in `tests/bootstrap/test_storage_schema_v2.py`.
Repo commit reviewed: `7e00dcd1aaa80eea5806110cb8df7f63959559b6` (modules are untracked working-tree
files; commit is the parent tree).

Test status before rating (run by this reviewer):

```
$ rtk cargo test -p opencode-rk-storage 2>&1 | tail
cargo test: 123 passed (13 suites, 24.85s)
```

Raw breakdown (second run, per-suite): lib 60 passed, backup_v2 5, catalog_v2 11, import_v2 4,
integration_v2 2, perf_modules_v2 6, perf_v2 5, quota_v2 5, restart_v2 5, schema_v2 6, stress_v2 4,
writer_v2 10. 123 passed, 0 failed, 0 ignored. Suite is green; green does not mean safe (see below).

Ratings per criterion: CONFIRMED (implemented and evidenced), PARTIAL (present but with a real
hole), MISSING (absent).

## gc_v2.rs

Module: `crates/storage/src/gc_v2.rs:1-378` (non-test code `:13-154`).

- Unbounded growth: PARTIAL. Blob tombstoning and outbox pruning are bounded
  (`claim_unreferenced_for_deletion` clamps 1..=500 at `gc_v2.rs:85`, `MAX_CLAIM_ROWS` at
  `gc_v2.rs:19`; `prune_outbox_prefix` deletes a strict prefix, `gc_v2.rs:117-120`). But the
  reachability arms (`UNREFERENCED_ARMS`, `gc_v2.rs:24-64`) only make BLOBS collectible. Orphan
  `payloads` rows (inline payloads whose root rows were deleted, e.g. after session/message
  cascade) are never collected: no code path executes `DELETE FROM payloads` anywhere in
  `crates/storage/src/` (grep over the crate returns zero hits), and the `payload_roots` view
  (`workspace.sql:302-313`) is referenced only in the doc comment at `gc_v2.rs:7`. The payloads
  table grows without bound in the delete-heavy lifecycle the schema otherwise supports.
  Additionally there is no physical blob-file unlink: `finish_deletion` (`gc_v2.rs:94-105`)
  deletes only the `blobs` DB row; no v2 code path calls `remove_file`/unlink (grep over
  `gc_v2.rs`, `writer_v2.rs`, `schema_v2.rs` returns nothing). DB-row GC without file GC leaks
  disk once a v2 CAS writer exists; today no v2 blob writer exists, so this is latent.
- Missing ownership/lifetime: none. Stateless struct, `&Connection`/`&mut Connection` borrows
  correct; IMMEDIATE tx used where two statements must be atomic
  (`prune_outbox_prefix`, `gc_v2.rs:116-133`).
- Panic paths: MISSING (i.e. none found). Non-test code `gc_v2.rs:13-154` contains no
  `unwrap`/`expect`/indexing; every fallible path returns `StorageError`. `unwrap` occurrences
  start at `gc_v2.rs:163` inside `#[cfg(test)]` (`gc_v2.rs:156`).
- Silent data loss: MISSING as direct loss; PARTIAL as risk. `finish_deletion` re-verifies
  unreferenced (`gc_v2.rs:95-97`) and the DDL triggers `blob_gc_claim` and `blob_no_resurrection`
  (`workspace.sql:49-54`) fail a racing claim closed. But note the error taxonomy:
  `invalid_input` at `gc_v2.rs:152-154` smuggles a human message into
  `rusqlite::Error::InvalidParameterName`, so callers cannot programmatically distinguish
  "raced a new reference" from "bad input". Fail-closed but undiagnosable.
- Unsafe/unguarded state transitions: MISSING (none). Claim is `state = 0 -> 1` only
  (`gc_v2.rs:66-69`), finish is `state = 1 -> DELETE` with re-verification (`gc_v2.rs:96`),
  and the floor bump uses `MAX(event_floor_seq, ?)` so a stale floor cannot regress
  (`gc_v2.rs:124-127`). The DDL CHECK `event_floor_seq <= event_head_seq`
  (`workspace.sql:19`) makes a prune past head fail closed and roll back the prefix delete.
- Missing/weak tests: PARTIAL. In-module tests cover claim age/limit/references and
  floor-not-head (`gc_v2.rs:262-377`). Gaps: (1) the prune-past-head rollback path (keep_from_seq
  beyond head+1) is asserted only in Python (`test_storage_schema_v2.py:652-656`), not through
  `GcV2::prune_outbox_prefix` in Rust (the Rust test stops at the boundary case
  `gc_v2.rs:364`, keep=4, head=3, floor=3 which passes the CHECK); (2) no Rust test drives the
  `blob_gc_claim` trigger abort THROUGH the module (claim racing a concurrent reference insert);
  only the Python drop-trigger tests prove the trigger owns the invariant
  (`test_storage_schema_v2.py:564-573`); (3) `GcV2` has zero callers outside its own file: grep
  for `GcV2::` across `crates/storage/{src,tests}` returns only a stale comment
  `crates/storage/tests/perf_modules_v2.rs:14` claiming "gc_v2 is still a stub lane" while the
  perf test at `perf_modules_v2.rs:190-218` bypasses the module with raw SQL. The real claim API
  has no integration, stress, or perf coverage. (4) `docs/STORAGE.md:263` still declares
  "GC implementation | STUB | gc_v2.rs:1-3 is a placeholder", contradicting the implemented
  378-line file; stale doc evidence.

## admission_v2.rs

Module: `crates/storage/src/admission_v2.rs:1-510` (non-test code `:12-260`).

- Unbounded growth: PARTIAL, one CONFIRMED hole. (1) CONFIRMED: `operation_receipts` is
  insert-only. `receipt_store` (`admission_v2.rs:241-255`) only INSERTs; there is no
  `DELETE FROM operation_receipts` anywhere in the crate (grep over `crates/` returns nothing).
  The expiry index `receipts_expiry_idx` (`workspace.sql:292`) exists precisely for an expiry
  sweep that no code performs. Table grows without bound under retry-heavy workloads.
  (2) PARTIAL: pending `session_inputs` (state 0) have no cancel API; state 2 (cancelled) is
  declared in the DDL (`workspace.sql:118`) but no function ever sets it (grep: only `state=1`
  at `admission_v2.rs:179-183`). Unpromoted inputs accumulate; the partial index
  `inputs_pending_idx` (`workspace.sql:129`) implies a pending-input drain that does not exist.
- Missing ownership/lifetime: MISSING (none). `submit_input` and `promote_input` take
  `&mut Connection` and wrap IMMEDIATE transactions (`admission_v2.rs:52-53`,
  `admission_v2.rs:106-107`); seq allocation and insert are atomic; overflow is guarded by
  `checked_add` (`admission_v2.rs:62`, `:155`).
- Panic paths: MISSING (none). `admission_v2.rs:12-260` has no `unwrap`/`expect`/indexing;
  test-only `unwrap` starts at `:270` (in `workspace()`, `:268-271`) inside `#[cfg(test)]` (`:262`).
- Silent data loss: MISSING (none). Promotion shares payload rows, bodies never copied
  (`admission_v2.rs:167-173`), and the old parts rows are deleted only after the new ones are
  inserted in the same tx (`:174-177`). Rejected promotions leave the input pending.
- Unsafe/unguarded state transitions: MISSING (none). Promotion is a CAS on
  `state = 0` (`admission_v2.rs:178-183`, `changed != 1` errors at `:184-186`); double promote
  is rejected and tested (`admission_v2.rs:381-389`). Deferred FK on `promoted_message_pk`
  (`workspace.sql:124-125`) resolves at commit and is cross-session guarded (Python:
  `test_storage_schema_v2.py:696-702`).
- Missing/weak tests: PARTIAL. Eight in-module tests (`admission_v2.rs:303-509`) cover seq
  allocation, promotion sharing, double-promote, receipt roundtrip/expiry, inline size bound,
  and input validation; integration adds submit/promote in a full lifecycle
  (`tests/integration_v2.rs:62-71`). Gaps: (1) no test for a 256-part boundary submit
  (`MAX_INPUT_PARTS` `admission_v2.rs:17` is checked at `:39` but never exercised at the
  boundary; only empty and 1-part inputs are tested); (2) no test that promotion preserves
  original part ordinals - and inspection shows it does NOT: `promote_input` selects ordinal
  (`admission_v2.rs:126-127`) then discards it and renumbers with `enumerate`
  (`admission_v2.rs:167`). Benign today because `submit_input` always writes contiguous
  ordinals (`admission_v2.rs:80-91`), but a gap cannot be created through the API only by
  accident of no other writer existing; (3) no expiry sweep test for receipts because the sweep
  does not exist (see growth); (4) no concurrency test (two threads submitting to the same
  session) - `stress_v2.rs` covers outbox/message seq but not input seq.

## execution_v2.rs

Module: `crates/storage/src/execution_v2.rs:1-522` (non-test code `:11-218`).

- Unbounded growth: PARTIAL. `executions`, `provider_attempts`, `tool_calls` rows accumulate for
  the session's lifetime. Deletion only happens via session cascade
  (`workspace.sql:147`, `:173`, `:205-206` `ON DELETE CASCADE`). No archival/retention API.
  Bounded by sessions, which themselves have no archival sweep; acceptable as a design but
  unstated as an operator contract.
- Missing ownership/lifetime: MISSING (none). Single-owner is enforced by the partial UNIQUE
  index `executions_single_owner_idx` (`workspace.sql:162`) and exercised by the module test
  `duplicate_owner_rejected` (`execution_v2.rs:344-361`) plus the stress test
  `single_owner_race` (`crates/storage/tests/stress_v2.rs:212-...`).
- Panic paths: MISSING (none). `execution_v2.rs:11-218` has no `unwrap`/`expect`/indexing;
  test-only `unwrap` starts at `:239` inside `#[cfg(test)]` (`:220`).
- Silent data loss: MISSING (none).
- Unsafe/unguarded state transitions: PARTIAL, two CONFIRMED holes. `transition_execution` is a
  proper CAS (`WHERE pk=? AND state=?`, `changed == 0` errors, `execution_v2.rs:69-81`) with an
  explicit legal-transition table (`execution_v2.rs:209-214`). But:
  (1) CONFIRMED `finish_attempt` (`execution_v2.rs:108-130`) does an UNCONDITIONAL
  `UPDATE provider_attempts SET state=?1, finished_at_us=?2 WHERE pk=?3`
  (`execution_v2.rs:122-125`) - no state precondition, no CAS. A terminal attempt (state 2
  success) can be silently rewritten to 3 (failure) or 5; `changed == 0` only fires for a
  missing row. The DDL CHECK (`workspace.sql:183`) only keeps `finished_at_us` consistent with
  the NEW state; it does not constrain the transition. (2) CONFIRMED same pattern in
  `finish_tool` (`execution_v2.rs:170-202`): the UPDATE at `execution_v2.rs:192-197` has no
  `AND state = ?` guard, so a finished tool call can be re-finished or have its terminal state
  rewritten. (3) PARTIAL: `record_attempt` (`execution_v2.rs:85-104`) validates only
  ordinal/through_seq ranges; it does not check that the parent execution is still live, so
  attempts can be recorded against a terminal execution (no DDL CHECK ties attempt creation to
  execution state). (4) Minor: `executions.started_at_us` (`workspace.sql:156`) is never set
  by any API - the 0 -> 1 transition at `execution_v2.rs:56-82` touches only `state` and
  `finished_at_us`; the column is dead weight and a runbook analytics lie.
- Missing/weak tests: PARTIAL. Module tests cover start/transition legality, single-owner,
  uncertain release, attempt finish, tool success/failure payload requirements
  (`execution_v2.rs:325-521`); integration repeats the happy path and wrong-from-state
  (`tests/integration_v2.rs:82-94`, `:184`). Gaps: (1) no test that `finish_attempt`/`finish_tool`
  REJECT a rewrite of a terminal row - because they do not reject it (see above); (2) the legal
  `(0, 5)` queued-cancel transition (`execution_v2.rs:212`) is never exercised in any Rust test;
  (3) no test for `record_attempt` against a terminal execution; (4) `finish_tool` to state 4
  (cancelled, requires finished_at per DDL `workspace.sql:208`) is never tested - only 2, 3,
  and the module test asserts `finish_tool(.., 1, ..)` is invalid (`execution_v2.rs:520`).

## approvals_v2.rs

Module: `crates/storage/src/approvals_v2.rs:1-424` (non-test code `:10-174`).

- Unbounded growth: PARTIAL, one CONFIRMED hole. (1) CONFIRMED: no `DELETE FROM approvals`
  anywhere in the crate (grep returns zero hits). `expire_sweep` (`approvals_v2.rs:130-145`)
  flips pending rows to state 3 (expired) but never removes anything; resolved (1, 2, 4) and
  expired (3) rows accumulate forever. The sweep bounds the PENDING count, not the table size.
  (2) PARTIAL: there is no cap on pending approvals per session; a client loop can grow the
  pending set without bound between sweeps (sweep limit is clamped to 500 per call,
  `approvals_v2.rs:16`, `:135`, so back-to-back calls drain, but nothing schedules them).
- Missing ownership/lifetime: MISSING (none). Approvals are decision records, not capabilities
  (`approvals_v2.rs:18-23`, `workspace.sql:222`); the module never treats them as grants.
- Panic paths: MISSING (none). `approvals_v2.rs:10-174` has no `unwrap`/`expect`/indexing;
  test-only `unwrap` starts at `:183` inside `#[cfg(test)]` (`:176`).
- Silent data loss: MISSING (none). Single-winner resolve is guarded by
  `WHERE pk = ? AND state = 0` on the UPDATE (`approvals_v2.rs:109-121`): the mandatory-human
  SELECT at `approvals_v2.rs:92-98` and the UPDATE are separate statements (not one tx), but the
  UPDATE's own `state = 0` predicate makes the race fail safe (lost CAS -> `changed == 0` ->
  error at `:122-126`). Expired rows refuse resolve and are tested
  (`approvals_v2.rs:303-313`).
- Unsafe/unguarded state transitions: MISSING (none). Resolve targets limited to 1, 2, 4
  (`approvals_v2.rs:89-91`); 0 and 3 are reachable only via insert and sweep respectively;
  the DDL CHECK at `workspace.sql:239` independently forces `human_client_id` for
  mandatory-human resolves to 1/4, matching the module check at `approvals_v2.rs:104-108`.
  One note: `resolve` sets `human_client_id = COALESCE(?2, human_client_id)`
  (`approvals_v2.rs:112`), so a resolve cannot CLEAR a client id - fail-safe direction.
- Missing/weak tests: PARTIAL. Module tests cover request validation, double resolve, resolve
  target bounds, mandatory human, sweep-due-only, resource bounds, denied/consumed
  (`approvals_v2.rs:231-423`); integration adds request+resolve in the lifecycle
  (`tests/integration_v2.rs:97-110`) and the Python suite proves the state=0 guard and the
  DDL CHECK (`test_storage_schema_v2.py:780-815`). Gaps: (1) the `tool_call_pk = Some(..)`
  happy path (valid tool in the same session) is never tested through the module - the only
  Some(...) test is the mismatched-session rejection (`approvals_v2.rs:386-398`); the FK
  (`workspace.sql:237`) is unexercised via this API; (2) no sweep clamp/batch test (limit 0
  or > 500); (3) no concurrency test (two resolvers racing); (4) no growth/retention test
  because no retention exists; (5) the sweep query filters `state = 0 AND expires_at_us <= ?1
  ORDER BY expires_at_us` (`approvals_v2.rs:137-141`) but the only pending index is
  `approvals_pending_idx (session_pk, created_at_us, pk) WHERE state = 0`
  (`workspace.sql:242`) - no index serves expires_at_us ordering; a full scan per sweep at
  scale. Perf-only, but unmeasured.

## snapshot_v2.rs

Module: `crates/storage/src/snapshot_v2.rs:1-393` (non-test code `:14-210`).

- Unbounded growth: PARTIAL. `context_epochs` and `compaction_checkpoints` rows accumulate for
  the session lifetime (cascade-only deletion, `workspace.sql:252`, `:265`); no epoch/checkpoint
  retention. `retained_payloads` is caller-bounded via pin/unpin (`snapshot_v2.rs:89-123`).
  Reads are strictly bounded: `MAX_PAGE_SIZE = 500` with clamp (`snapshot_v2.rs:18`, `:133`,
  `:164`), keyset pagination (`seq > ?` cursors, `snapshot_v2.rs:135-137`, `:170-172`).
- Missing ownership/lifetime: MISSING (none). All writes are single statements so `&Connection`
  is sound (stated and honored, `snapshot_v2.rs:5-6`); `open_epoch` allocates `MAX(epoch)+1`
  inside the INSERT..SELECT (`snapshot_v2.rs:32-38`) so concurrent openers serialize on
  `UNIQUE(session_pk, epoch)`/`context_open_idx` (`workspace.sql:258`, `:260`).
- Panic paths: MISSING (none). `snapshot_v2.rs:14-210` has no `unwrap`/`expect`/indexing;
  test-only `unwrap` starts at `:219` inside `#[cfg(test)]` (`:212`).
- Silent data loss: MISSING (none). `close_epoch` refuses when nothing is open
  (`changed != 1` errors, `snapshot_v2.rs:58-61`); `unpin` refuses a missing pin
  (`:119-122`); `checkpoint` rejects boundary seq <= 0 in code (`:75-77`) and the deferred FK
  to `messages(session_pk, seq)` (`workspace.sql:271-272`) rejects a nonexistent boundary at
  statement commit, tested at `snapshot_v2.rs:306-319`.
- Unsafe/unguarded state transitions: MISSING (none). The one-open-epoch invariant is owned by
  `context_open_idx` (`workspace.sql:260`) and tested (`snapshot_v2.rs:280-291`); close is a
  guarded conditional UPDATE (`:53-57`).
- Missing/weak tests: PARTIAL. Tests cover open/close/reopen, checkpoint FK failure, pin/unpin
  roundtrip, empty watermark, keyset paging, outbox bounded by head and session
  (`snapshot_v2.rs:279-392`). Gaps: (1) WRITE/READ ASYMMETRY: the module writes epochs and
  checkpoints but exposes NO read API for them - `export_page` returns only `(seq, message_pk)`
  tuples (`snapshot_v2.rs:132-152`) with no parts/payloads export; a compaction consumer cannot
  read back what `checkpoint` stored. The read paths for the module's own data are MISSING.
  (2) No test for `outbox_page`/`export_page` limit clamping (0 or > 500). (3) No test for
  `pin` with owner_id of the wrong length - left to the STRICT CHECK `length(owner_id) = 16`
  (`workspace.sql:277`), untested through the API. (4) No concurrency test for racing
  `open_epoch`. (5) The `Some(id)`/`None` arms of `outbox_page` are near-duplicate code
  (`snapshot_v2.rs:167-203`) - maintenance smell, not a defect.

## import_v2.rs

Module: `crates/storage/src/import_v2.rs:1-392` (non-test code `:14-246`).

- Unbounded growth: MISSING (none). Source reads are bounded by `PAGE_BUDGET = 500`
  (`import_v2.rs:20`, `:124`); the loop terminates on no-progress (`:57-59`); destination
  writes are per-message transactions (`append_to_v2`, `:189-229`).
- Missing ownership/lifetime: MISSING (none). Source connection is strictly read-only in this
  module (only SELECTs at `:99-105`, `:121-137`); byte-identity of the source file is proven
  mid-run and after close (`tests/import_v2.rs:170-188`) and row identity in-module
  (`import_v2.rs:365-391`).
- Panic paths: MISSING (none). `import_v2.rs:14-246` has no `unwrap`/`expect`/indexing;
  test-only `unwrap` starts at `:272` (in `source_db()`, `:271-273`) inside `#[cfg(test)]` (`:248`).
- Silent data loss: CONFIRMED, two lanes, plus one attribution trap.
  (1) CONFIRMED: source messages with `blob_hash` (blob-backed bodies) are imported as
  ZERO-BYTE payloads with provenance JSON in `message_parts.metadata_json`
  (`import_v2.rs:160-167`, `{source_blob_hash, source_byte_len}`). The message imports, the
  count verifies, the body is empty. (2) CONFIRMED: inline text over the 8192-byte v2 ceiling
  is likewise dropped to a zero-byte payload with `{source_truncated: true}`
  (`import_v2.rs:149-158`). (3) The gate that should catch this does not:
  `verify_counts` compares only `COUNT(*)` of messages (`import_v2.rs:67-81`) - it cannot see
  body loss. The importer's return value (`u64` message count, `:30-63`) carries no
  dropped-body signal. Callers learn of the loss only by inspecting metadata_json after the
  fact. The module doc admits the limitation (`import_v2.rs:9-13`) and defers a blob-copy
  lane, but the CURRENT public surface reports success while discarding content.
  (4) CONFIRMED trap: `import_is_resumable` picks "the next source session encountered at the
  cursor" (`import_v2.rs:99-109`) because the signature carries no source session id. On a
  multi-session source, resuming can land session B's messages into session A's destination
  session_pk. The doc warns (`:88-92`) and every test uses a single-session source
  (`tests/import_v2.rs:191-223`), so the misattribution lane is untested and unguarded.
- Unsafe/unguarded state transitions: PARTIAL. No state machine here, but two unguarded
  properties: (1) NO IDEMPOTENCY - re-running `import_session` duplicates the conversation
  (fresh `MessageId` per row at `import_v2.rs:205`, seq continues from `next_message_seq`);
  only an explicit caller-side `verify_counts` against a pre-known expected count would catch
  it, and after a crash mid-import the caller has no persisted manifest to know the expected
  count. (2) A malformed `created_at` aborts mid-import with `InvalidQuery`
  (`parse_us`, `:232-236`), leaving a partial import committed (per-message txs) and no
  recorded cursor; resumption is possible only if the caller persisted the last cursor
  itself, which no API does.
- Missing/weak tests: PARTIAL. Good coverage for: role/order/body preservation
  (`tests/import_v2.rs:114-167`), source byte-identity (`:169-188`), resumable paging with
  exact 500/200/0 pages (`:190-223`), count-mismatch rejection (`:225-250`), and in-module
  copies (`import_v2.rs:320-391`). Gaps: (1) NO test for either data-loss lane (blob-backed
  source row, oversize inline row) - the two most dangerous behaviors in the module have zero
  coverage; (2) no multi-session `import_is_resumable` test proving or refuting
  cross-session attribution; (3) no double-import (idempotency) test; (4) no malformed
  `created_at` test for `parse_us`'s error path; (5) no unknown-role test for
  `encode_role` returning None (`:238-246`).

## quota_v2.rs

Module: `crates/storage/src/quota_v2.rs:1-172` (non-test code `:3-113`).

- Unbounded growth: MISSING (none). `measure` reads PRAGMAs and file sizes
  (`quota_v2.rs:29-47`); `reclaim` issues a bounded `PRAGMA incremental_vacuum(N)`
  (`:70-77`); nothing accumulates in-process.
- Missing ownership/lifetime: PARTIAL, one CONFIRMED orphan. The gate itself is fine, but it
  is WIRED TO NOTHING: grep for `admit`/`QuotaV2` across `writer_v2.rs` and `lib.rs` returns
  no call site; `V2Writer::append_message` (`writer_v2.rs:52-108`),
  `append_outbox_event` (`:110-138`), `AdmissionV2::submit_input`, and `ImportV2` never
  consult `QuotaV2::admit`. The admission gate exists (`quota_v2.rs:51-66`) but no write path
  in the crate is gated. Backpressure is a public function, not a property of the system.
- Panic paths: MISSING (none). `quota_v2.rs:3-113` has no `unwrap`/`expect`; the one
  conversion uses `unwrap_or(i64::MAX)` saturation (`:109`) and page_size read uses
  `unwrap_or(4096)` (`:60`). Test-only `unwrap` starts at `:127` inside `#[cfg(test)]`
  (`:115`). `format!("PRAGMA incremental_vacuum({pages});")` (`:74`) interpolates a u32 - no
  injection surface.
- Silent data loss: MISSING by intent - the design rejects rather than deletes
  (`quota_v2.rs:2`, `:49-51`), and `reclaim` cannot drop rows (vacuum only releases free
  pages; verified behaviorally by `tests/quota_v2.rs:57-85` asserting message count unchanged
  after reclaim).
- Unsafe/unguarded state transitions: PARTIAL. (1) TOCTOU is inherent: `measure` then `admit`
  on a caller-supplied snapshot is not atomic; the DB can pass the limit between the two
  (`quota_v2.rs:29-66`). Acceptable for a backpressure signal, but no doc at the call-site
  level states the staleness window. (2) `reclaim` on a database whose `auto_vacuum` is not
  INCREMENTAL is a silent no-op in SQLite; the function returns Ok without verifying the
  freelist decreased (`:70-77`). A caller believes pages were reclaimed. (3) `wal_file_bytes`
  returns 0 for in-memory connections (`:102-104`), so in-memory DBs always pass the WAL gate;
  fine, but untested-by-anyone behavior. (4) The `-wal` path is derived by string suffix
  (`:96-100`): correct for `workspace.db` -> `workspace.db-wal`, fragile if the path is a
  URI or has odd characters; `file_size` treats NotFound as 0 (`:110`), so a renamed WAL
  silently reads as "no WAL".
- Missing/weak tests: PARTIAL. Five integration tests on a real file-backed workspace
  (`tests/quota_v2.rs:16-85`) plus five in-module (`quota_v2.rs:132-171`) and lifecycle usage
  (`tests/integration_v2.rs:127-130`). Gaps: (1) the WAL-rejection path uses a SYNTHETIC
  snapshot (`tests/quota_v2.rs:44-54`, `quota_v2.rs:158-165` both construct wal_pages by
  hand); no test grows a real WAL past a limit; (2) no test proves `reclaim` actually reduces
  `freelist_count` or detects a non-incremental DB; (3) no test that `admit` blocks a
  subsequent write in any writer path - because it is not wired (see ownership); (4) no test
  for `page_size` failure fallback (`unwrap_or(4096)` at `:60`).

## Cross-cutting observations

- Panic discipline is uniformly good: across all seven modules, `unwrap`/`expect` appear only
  inside `#[cfg(test)]` blocks (gc `:156`, admission `:262`, execution `:220`, approvals
  `:176`, snapshot `:212`, import `:248`, quota `:115`). No indexing panics, no slicing.
- `#![forbid(unsafe_code)]` present in `gc_v2.rs:13`, `approvals_v2.rs:8`, `quota_v2.rs:3`,
  and `lib.rs:2`; absent (but unneeded - no unsafe) in admission, execution, snapshot, import.
- Error taxonomy is deliberately impoverished: admission/execution/snapshot reuse
  `Sqlite(InvalidQuery)` for every validation failure (admission `:258-260`, execution
  `:216-218`, snapshot `:208-210`); gc/approvals smuggle human messages into
  `InvalidParameterName` (gc `:152-154`, approvals `:172-174`). Callers cannot distinguish
  raced CAS from bad arguments without string matching. Acknowledged as ponytail in
  `admission_v2.rs:7-10`, `execution_v2.rs:7-9`, `snapshot_v2.rs:9-12`.
- Stale evidence in tree: `docs/STORAGE.md:263` ("GC implementation | STUB") and
  `tests/perf_modules_v2.rs:11-14` ("gc_v2 is still a stub lane") both contradict the
  implemented `gc_v2.rs`; the perf lane still bypasses the module
  (`perf_modules_v2.rs:190-218`). Anyone auditing from docs will mis-rank GC readiness.
- Python contracts and Rust behavior agree on every shared invariant I cross-checked:
  trigger-owned GC fail-closed (`test_storage_schema_v2.py:527-547` vs gc arms),
  state=0 resolve guard (`:780-791` vs approvals `:109-121`), single-owner partial UNIQUE
  (`:729-736` vs executions index), deferred-FK promotion and cross-session rejection
  (`:684-702` vs admission promote), terminal finished_at consistency (`:738-766` vs
  execution transitions). No drift found between the SQL contract suite and the Rust modules.

## Top-5 risks with owners

1. Import silently drops message bodies (blob-backed and oversize inline become zero-byte
   payloads) and `verify_counts` cannot detect it; `import_is_resumable` can misattribute
   sessions on multi-session sources. Evidence: `import_v2.rs:149-167`, `:67-81`, `:99-109`;
   zero tests for these lanes.
   Owner: WRITER CODE. The importer must either fail closed on unverifiable bodies or return a
   loss report; `verify_counts` must compare payload bytes, not row counts. No SQL constraint
   can own this - the destination schema is satisfied by empty payloads.
2. `finish_attempt` and `finish_tool` perform unguarded terminal-state rewrites (no CAS, no
   state precondition); `record_attempt` accepts attempts on terminal executions. Evidence:
   `execution_v2.rs:122-125`, `:192-197`, `:85-104`.
   Owner: WRITER CODE (add `AND state = ?` guards mirroring `transition_execution`). The DDL
   CHECKs (`workspace.sql:183`, `:208`) only constrain finished_at consistency with the NEW
   state; they cannot express transition legality.
3. Retention debt: `operation_receipts` (insert-only), terminal `approvals` rows, orphan
   inline `payloads` after root deletion, and epoch/checkpoint history all grow without any
   sweep; GC reaches only blobs. Evidence: no `DELETE FROM operation_receipts`/`payloads`/
   `approvals` anywhere in the crate (grep); `receipts_expiry_idx` (`workspace.sql:292`)
   built for a sweep that does not exist; `UNREFERENCED_ARMS` joins only blob-bearing payloads
   (`gc_v2.rs:24-64`).
   Owner: WRITER CODE for the sweeps (SQL cannot own time-based retention policy), plus
   OPERATOR/RUNBOOK for disk-growth monitoring and VACUUM cadence until sweeps land.
4. The quota admission gate is orphaned: no write path in the crate calls
   `QuotaV2::admit`. Evidence: grep for `admit`/`QuotaV2` in `writer_v2.rs`/`lib.rs` returns
   nothing; `docs/STORAGE.md` "What is still NOT done" concedes native wiring is NOT DONE.
   Owner: WRITER CODE (wire admit into `V2Writer::append_message`, `append_outbox_event`,
   `AdmissionV2::submit_input`, and `ImportV2::append_to_v2`); OPERATOR/RUNBOOK sets the byte
   budgets. A SQL constraint cannot own an admission policy.
5. GC correctness is trigger-defended but untested through the module: the racing-claim
   abort and the prune-past-head rollback are proven only in Python
   (`test_storage_schema_v2.py:564-573`, `:652-656`); the Rust module has zero integration
   callers and its perf lane bypasses it (`tests/perf_modules_v2.rs:14`, `:190-218`); no
   physical blob unlink exists for when a CAS writer lands.
   Owner: split. SQL owns the fail-closed triggers (already in `workspace.sql:43-56`); WRITER
   CODE owns Rust-level racing tests routed through `GcV2::claim_unreferenced_for_deletion`
   and the eventual file-unlink step; OPERATOR/RUNBOOK owns the stale-doc risk
   (`docs/STORAGE.md:263`) until docs are updated.

## Integration-ready verdict (one line per module)

- gc_v2: PARTIAL - core lifecycle and fail-closed triggers are sound, but orphan payload
  sweep, blob-file unlink, Rust-level race tests, and any integration caller are missing.
- admission_v2: PARTIAL - submit/promote/receipts are correct and well tested, but receipts
  never expire off disk and the pending-cancel state is unreachable.
- execution_v2: NOT READY - CAS exists only for executions; attempts and tool calls accept
  terminal-state rewrites, and attempts can attach to finished executions.
- approvals_v2: PARTIAL - transitions are single-winner and human-gated, but terminal rows
  are never reclaimed and the sweep lacks a matching index.
- snapshot_v2: PARTIAL - bounded, guarded reads and epoch/checkpoint writes are solid, but
  there is no read API for the epochs/checkpoints it writes (write/read asymmetry).
- import_v2: NOT READY - two confirmed silent-body-loss lanes, a count verifier blind to
  them, a multi-session attribution trap, and no idempotency.
- quota_v2: NOT READY AS A SYSTEM GATE - the math and rejection semantics are correct, but
  nothing calls `admit`, so as deployed the quota provides zero backpressure.
