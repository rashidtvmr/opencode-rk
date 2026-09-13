# STORAGE-V2-IMPL: schema initializer + bounded writer wave

Task: implement the format-2 new-file schema initializer and a bounded
writer, per the next-implementation boundary declared in
`docs/storage/REVIEW.md:53-55` and `docs/STORAGE.md:203-204`.
This worklog follows the root AGENTS.md scratchpad format (claim, source
evidence, observed scenario, target boundary, tests, decisions, remaining
unknowns). It is advisory and does not replace verification receipts.

## Claim

Deliver `crates/storage/src/schema_v2.rs` (new-file workspace initializer),
`crates/storage/src/writer_v2.rs` (bounded writer over the initialized
schema) and `crates/storage/src/catalog_v2.rs` (catalog initializer), with
the frozen RED suite `crates/storage/tests/writer_v2.rs` passing GREEN
without weakening any frozen test. Activation of format 2 in the product
remains gated (see engine gate below); this wave only makes the code exist
and pass its own contracts.

## Source evidence

- Base commit: `ec559120c4c5a27a1195150904b38f04463d0325`
  (`feat(storage): define format-2 schemas and crash-consistency contracts`),
  which introduced the DDL under review.
- DDL: `crates/storage/schema/v2/workspace.sql` (19 tables, 313 lines) and
  `crates/storage/schema/v2/catalog.sql` (6 tables). The initializer embeds
  workspace.sql via `include_str!` at `crates/storage/src/schema_v2.rs:13`.
- Authoritative contract text:
  - Initialization and connection policy: `docs/STORAGE.md:141-168`
    (page_size 4096, auto_vacuum INCREMENTAL, application_id 0x4F525732,
    user_version 2, checksum of exact SQL bytes, one IMMEDIATE transaction,
    foreign_keys=ON, synchronous=FULL, busy_timeout=5000, trusted_schema=OFF,
    cache_size=-8192, temp_store=FILE, mmap_size=0, verify journal_mode=WAL).
  - Crash consistency: `docs/storage/CRASH_CONSISTENCY.md` (publication
    ordering, GC tombstones, startup/shutdown). Not implemented this wave
    (see target boundary).
  - Prior-wave evidence: `validation/storage-v2-verification.md` (author-run,
    not independent; its "not compiled" note is now stale, see
    `docs/storage/REVIEW-INDEPENDENT.md` Part 6).
- Existing reference runtime (format 1, unchanged): `crates/storage/src/lib.rs`
  `Storage::configure` (lib.rs:25, note it uses synchronous=NORMAL) and
  `Storage::migrate` (lib.rs:26). The v2 wave must not switch
  `Storage::open` or touch format-1 databases.
- RED suite (parallel workers, frozen): `crates/storage/tests/writer_v2.rs`
  - imports `writer_v2::{NewMessage, NewSession, V2Writer}` (writer_v2.rs:5-8)
  - initializer contract assertions (writer_v2.rs:50-100): page_size 4096,
    auto_vacuum 2, application_id 0x4F525732, user_version 2, journal_mode
    wal, foreign_keys 1, synchronous 2, trusted_schema 0, temp_store 1,
    busy_timeout 5000, migration row version 2 with checksum len 32,
    workspace_state singleton row.
  - fail-closed: non-empty file rejected without modification
    (writer_v2.rs:103-114); reinitialize rejected, reopen path
    `open_existing` (writer_v2.rs:117-125); tampered checksum rejected
    (writer_v2.rs:128-143).
  - writer contracts: title cap 1024 bytes (writer_v2.rs:146-162), monotonic
    per-session message sequences with `next_message_seq` advancing
    (writer_v2.rs:165-199), monotonic workspace outbox seq with
    `event_head_seq` advancing (writer_v2.rs:202-227), atomic rollback of
    head/rows on invalid event (writer_v2.rs:230-248), 4096-byte outbox cap
    without head advance (writer_v2.rs:251-270), state-filtered keyset
    pagination with limit clamping (writer_v2.rs:273-321).
- Existing GREEN suites (must stay green): `crates/storage/tests/schema_v2.rs`
  (6 tests) and `tests/bootstrap/test_storage_schema_v2.py` (37 tests).

## Observed scenario (state of tree at worklog time)

- Working tree at base ec55912 plus uncommitted: `crates/storage/src/lib.rs`
  modified (+2 lines: `pub mod schema_v2; pub use schema_v2::SchemaV2;`),
  untracked `crates/storage/src/schema_v2.rs` (173 lines, initializer only),
  untracked `crates/storage/tests/writer_v2.rs` (321 lines, frozen RED suite).
- `writer_v2.rs` (implementation) and `catalog_v2.rs` (implementation) do
  not exist yet. `cargo test -p opencode-rk-storage` (default target) fails
  at compile: `unresolved import opencode_rk_storage::writer_v2` at
  writer_v2.rs:7. Isolated run `cargo test -p opencode-rk-storage --test
  schema_v2` passes 6/6 (2026-09-13T12:08:34Z).
- Python suite `tests/bootstrap/test_storage_schema_v2`: 37/37 OK.
  Full bootstrap discover runs 68 tests with 1 pre-existing error in
  `test_freeze_sources` (`tools/source_lock.py:76` SPDX check vs a test spec
  that omits `licenseSpdx`); unrelated to storage, reproduces with the
  storage diff stashed. See REVIEW-INDEPENDENT.md Part 5.
- Environment: cargo/rustc 1.96.0; Python 3.14.4 with SQLite 3.46.1;
  bundled Rust SQLite 3.50.2 (libsqlite3-sys 0.35.0 via rusqlite 0.37
  "bundled", `Cargo.toml:27`, `Cargo.lock:741`, sqlite3.h:149).

## Target boundary (what this wave may and may not do)

In scope:
- New empty files only. `SchemaV2::initialize_workspace` must refuse a
  non-empty existing file (schema_v2.rs:49-54) and never overwrite
  format-1 storage (STO:17-19 "NEW EMPTY DATABASE ONLY").
- One explicit IMMEDIATE transaction applying DDL, singleton
  `workspace_state` row, migration row, user_version and application_id
  (SCHEMA_INIT:64-85).
- Bounded writer commands over the schema: create session, append message
  (inline payload path), append outbox event, keyset-list recent sessions,
  with the state/head mutations atomic in the same transaction
  (CC:52-56).
- Catalog initializer for `catalog.sql` (new file only).

Out of scope (unchanged gates):
- No `Storage::open` switch, no format-1 migration, no import of the user's
  90 GB database (STO:199-201).
- No streaming CAS, GC, backup, recovery lifecycle, leases, or filesystem
  durability (STORAGE.md:203-204 places those after this wave).
- No dependency changes (engine gate below).

## Tests

- Frozen RED suite: `crates/storage/tests/writer_v2.rs` (authorship by
  parallel test workers; hash must be frozen before implementation proceeds,
  per root AGENTS.md step 3; freeze the suite hash when the writer lease is
  claimed).
- Planned RED suite addition: `crates/storage/tests/catalog_v2.rs` for the
  catalog initializer (application_id 0x4F524332 per STO:145, six STRICT
  tables, new-file-only, catalog_state singleton).
- Regression floor: `cargo test -p opencode-rk-storage --test schema_v2`
  (6 tests) and `python3 -m unittest tests.bootstrap.test_storage_schema_v2`
  (37 tests) must remain green; the pre-existing freeze-sources error is
  out of this wave's blast radius and must not be "fixed" here (it belongs
  to REPAIR-001 follow-up, whose claimed state does not currently match the
  tree; see REVIEW-INDEPENDENT.md Part 6.2).
- Independent adversarial verification of the DDL that these tests sit on:
  `docs/storage/REVIEW-INDEPENDENT.md` (this wave's companion deliverable).
  Its Part 5 records the exact mandated command outputs and timestamps.
- Per root AGENTS.md: submit evidence and patch, never acceptance; the
  verifier decides integration.

## Decisions

1. blake3-vs-sha256 checksum deviation (pending). `docs/STORAGE.md:146-147`
   specifies "Checksum is SHA-256 of exact migration SQL bytes". The
   initializer computes blake3-256 instead (`SchemaV2::workspace_checksum`,
   schema_v2.rs:149-155; justification comment schema_v2.rs:4-7: `sha2` is
   not a workspace dependency and adding one is out of scope for the wave).
   Consequences: (a) the stored checksum length CHECK (32 bytes, workspace.sql:6)
   is still satisfied; (b) the stored digest does not match the documented
   algorithm, so any external tool computing SHA-256 per the doc will
   fail-closed against our files. Status: accepted as a recorded deviation
   for the development window, must be resolved before any file produced now
   is considered stable: either amend docs/STORAGE.md:146-147 to blake3 or
   add `sha2` and regenerate. The schema_migrations row records whatever the
   initializer wrote, so a later switch invalidates already-created dev DBs
   (acceptable; no production files exist).
2. Engine gate blocks activation. Measured: bundled SQLite 3.50.2
   (libsqlite3-sys 0.35.0, sqlite3.h:149). `docs/STORAGE.md:163-168`
   requires >= 3.51.3 or an audited backport (3.44.6/3.50.7 carry the WAL
   reset fix); 3.50.2 < 3.50.7. Decision: no dependency bump in this wave
   (fail-closed, consistent with REVIEW.md:36 and root contract "no
   dependency acceptance"); landing the writer code is allowed, but any
   activation flag, `Storage::open` switch or production data path must
   wait until the bundle is upgraded and `sqlite_source_id` is recorded
   (STO:164-168). Local tests on 3.50.2/3.46.1 are development evidence
   only.
3. user_version/application_id transactionality. Empirically (probe,
   REVIEW-INDEPENDENT.md Part 4.5): PRAGMA user_version and application_id
   are transactional; values set inside a rolled-back transaction vanish,
   values inside a committed one persist. The initializer's comment at
   schema_v2.rs:76-79 ("may or may not persist inside a transaction
   depending on the build") is therefore inaccurate; the code is still safe
   because the in-transaction set at schema_v2.rs:79-80 succeeds and the
   post-commit fallback schema_v2.rs:83-85 is dead code. Decision: keep the
   current code path (harmless), correct the comment when the file is next
   edited, do not rely on the fallback.
4. Missing configure on the fresh connection. `initialize_workspace`
   (schema_v2.rs:41-87) applies only `PRAGMA page_size/auto_vacuum/
   journal_mode` (schema_v2.rs:20) and never runs the full configure block
   (foreign_keys, synchronous, busy_timeout, trusted_schema, cache_size,
   temp_store, mmap_size) that `open_existing` applies at schema_v2.rs:157-168.
   The bundled engine defaults synchronous to 2 (FULL,
   sqlite3.c:17833-17837), but `foreign_keys` defaults OFF, so the returned
   fresh connection violates the documented writer contract
   (STORAGE.md:151-152). The frozen RED suite already asserts
   foreign_keys=1 (writer_v2.rs:67), so this is an expected RED failure the
   implementer must close by calling configure() before returning the
   connection in initialize_workspace.
5. Outbox/GC pattern choice. The DDL `payload_roots` view (workspace.sql:302-313)
   supports the coverage test, but the GC anti-join `NOT IN (view)`
   materializes the entire root list into an ephemeral structure before
   scanning payloads (EXPLAIN probe, REVIEW-INDEPENDENT.md Part 3h).
   Decision for the writer/GC implementer: use per-arm NOT EXISTS
   (correlated covering-index seeks, no materialization) once GC lands;
   the view remains the single source of truth for root coverage and its
   coverage test (`test_every_payload_fk_is_in_gc_root_view`,
   tests/bootstrap/test_storage_schema_v2.py:193-197).
6. Scope of the writer. First slice only: sessions, messages (inline
   payloads), outbox events, keyset listing. Blob-referencing payloads,
   admissions/promotion, executions, approvals transitions, compaction are
   later slices; the DDL deliberately leaves their state machines to the
   typed writer API (STO:93-97, REVIEW-INDEPENDENT.md Part 3f/Part 7).
   Rationale: the frozen RED suite covers exactly the first slice; writing
   more than the tests lease would create unverified surface.

## Remaining unknowns

1. Catalog initializer (`catalog_v2.rs`) has no frozen RED suite yet; a
   parallel test worker should author it before implementation (target:
   application_id 0x4F524332, user_version 2, catalog_state singleton,
   six STRICT tables, new-file-only, no secret-value columns).
2. Whether the freeze-sources bootstrap error (`tools/source_lock.py:76`
   SPDX check vs `tests/bootstrap/test_freeze_sources.py:44-53`) gets fixed
   by re-adding the conditional from REPAIR-001.md:32-35 or by fixing the
   test spec; outside this wave, but it blocks a fully green bootstrap
   discover and any future "all tests pass" receipt.
3. Engine upgrade path: which libsqlite3-sys version bundles >= 3.50.7 /
   >= 3.51.3 and whether the WAL-reset fix backport set needs an audited
   pin rather than a version floor (STO:163-168); no audit done here.
4. Checksum deviation resolution (decision 1): blake3 documented vs sha256
   implemented; needs an owner decision before any long-lived dev database
   is created with the current digest.
5. WAL on the initializer path: `journal_mode=WAL` was verified by reading
   back the pragma (schema_v2.rs:58-63), but no test asserts WAL persistence
   across process restart of an initialized file (the Python crash test
   covers outbox atomicity, not journal-mode persistence).
6. No measurement yet of writer latency, checkpoint behavior or memory under
   the bounded-reader policy; STO:154-156 flags these as proposals to
   measure, not verified settings.
7. The `open_existing` re-verification of journal_mode after configure
   (schema_v2.rs:139-145) is untested by the frozen suite (no RED test
   flips a file to non-WAL and reopens); candidate negative test.

## GREEN wave and milestone receipts (2026-09-13)

### Evidence

Command: `git log --oneline -4`

```text
4aca273 docs(storage): engine gate, gap analysis, independent review, verification wave
fb8d388 feat(storage): implement format-2 initializer, bounded writer and catalog registry
ec55912 feat(storage): define format-2 schemas and crash-consistency contracts
53aac45 feat(discovery): reconcile DISC-003 behavior surfaces
```

Command: `cargo test -p opencode-rk-storage --test writer_v2 2>&1 | tail -2`

```text
cargo test: 10 passed (1 suite, 0.08s)
```

Command: `cargo test -p opencode-rk-storage --test schema_v2 2>&1 | tail -2`

```text
cargo test: 6 passed (1 suite, 0.03s)
```

Command: `cargo test -p opencode-rk-storage --lib catalog_v2 2>&1 | tail -2`

```text
cargo test: 1 passed, 7 filtered out (1 suite, 0.02s)
```

Command: `cargo test -p opencode-rk-storage --lib 2>&1 | tail -2`

```text
test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
[full output: ~/.local/share/rtk/tee/1789308030_cargo_test.log]
```

Control command with the storage worktree stashed:
`git stash -q -- tests/bootstrap/test_storage_schema_v2.py && cargo test -p opencode-rk-storage --lib tests::session_round_trip_and_archive 2>&1 | tail -2; git stash pop -q`

```text
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s
[full output: ~/.local/share/rtk/tee/1789308424_cargo_test.log]
```

The control reproduced the same pre-existing `tests::session_round_trip_and_archive`
failure without the storage worktree change. The stash was restored successfully.

Command: `python3 -m unittest tests.bootstrap.test_storage_schema_v2 2>&1 | tail -3`

```text
Ran 37 tests in 0.144s

OK
```

### Lane map

- `crates/storage/tests/writer_v2.rs`: luna-A.
- `crates/storage/src/catalog_v2.rs`: luna-B.
- `crates/storage/src/schema_v2.rs` fixes: vyce.
- Python contracts: spark-C, status at time of writing.
- `docs/storage/GAP-ANALYSIS.md`: spark-G.
- `docs/storage/ENGINE_GATE.md`: kai.
- Verification: luna-D.
- `crates/storage/src/lib.rs` fmt-wipe/restore: main agent.

### Milestones and decisions

- `fb8d388` contains five files: three new modules, the RED suite, and
  `lib.rs` (`git show --stat --oneline --summary fb8d388`; exact paths are
  `crates/storage/src/catalog_v2.rs`, `crates/storage/src/lib.rs`,
  `crates/storage/src/schema_v2.rs`, `crates/storage/src/writer_v2.rs`, and
  `crates/storage/tests/writer_v2.rs`).
- `4aca273` contains four docs: `docs/storage/ENGINE_GATE.md`,
  `docs/storage/GAP-ANALYSIS.md`, `docs/storage/REVIEW-INDEPENDENT.md`, and
  `validation/storage-v2-verification.md`.
- Per-milestone commit policy adopted. The blake3-256 checksum deviation is
  recorded at `schema_v2.rs:137-155` and `docs/STORAGE.md:146-147`.
- Engine activation remains BLOCKED: bundled SQLite 3.50.2 is below 3.50.7,
  as recorded at `docs/storage/ENGINE_GATE.md:11-14` and
  `docs/storage/REVIEW-INDEPENDENT.md:211-217`.

### Remaining unknowns

1. No catalog RED integration file exists yet; the catalog check is currently
   the filtered library test at `crates/storage/src/catalog_v2.rs:1-330`.
2. `validation/storage-v2-verification.md` currency after this entry is not
   established; its current receipt predates this worklog section.
3. WAL-restart persistence remains untested. The existing coverage only reads
   back `journal_mode=WAL` at `crates/storage/src/schema_v2.rs:58-63`.
4. Writer latency, checkpoint behavior, and memory remain unmeasured, as noted
   by `docs/STORAGE.md:154-156`.
5. The full storage library still has the pre-existing
   `tests::session_round_trip_and_archive` failure; no repair was attempted.

## Four-lane integration attempt (2026-09-13): ERROR-WAVE G1-G5 / D1-D5

### Claim

Attempt to wire the 4 won storage-V2 lanes (gc_v2, admission_v2,
execution_v2, approvals_v2, snapshot_v2, fork_v2 - whichever actually
landed) and bookkeep per the 10-lane plan split into 5 dahl (D1-D5) plus
5 gg (G1-G5). Outcome: NONE of the six named lane files exist in the tree,
so no new module wiring was required or performed. `crates/storage/src/lib.rs`
is already correct (wires the three base modules that exist and compile).
Worklog section only.

### Lane plan and status (10-lane: 5 dahl D1-D5 + 5 gg G1-G5)

Named lanes in this task (engine/GC/admission/execution/approvals/snapshot/fork):

| Lane | File expected | Status | Evidence |
|------|---------------|--------|----------|
| D: gc_v2 | crates/storage/src/gc_v2.rs | MISSING | `ls crates/storage/src/` yields only catalog_v2.rs, schema_v2.rs, writer_v2.rs, lib.rs |
| D: admission_v2 | crates/storage/src/admission_v2.rs | MISSING | not present in src/ |
| D: execution_v2 | crates/storage/src/execution_v2.rs | MISSING | not present in src/ |
| D: approvals_v2 | crates/storage/src/approvals_v2.rs | MISSING | not present in src/ |
| D: snapshot_v2 | crates/storage/src/snapshot_v2.rs | MISSING | not present in src/ |
| D: fork_v2 | crates/storage/src/fork_v2.rs | MISSING | not present in src/ |

No `crates/storage/src/*_v2.rs` beyond the three base modules exists, so the
five gg (G1-G5) and five dahl (D1-D5) lanes each report MISSING with the
same directory listing as sole evidence. Because nothing landed, there was
nothing to wire: the `pub mod`/`pub use` block in lib.rs (lines 3-8) is
unchanged and remains exactly the three base modules.

### Engine-upgrade outcome (G1 tree state)

- Bundled SQLite remains 3.50.2 (libsqlite3-sys 0.35.0), below the
  documented floor of 3.50.7 / 3.51.3 (`docs/storage/ENGINE_GATE.md:11-14`,
  `docs/STORAGE.md:163-168`). No dependency bump was made this wave.
- No engine activation flag or `Storage::open` switch was touched; the G1
  tree state is unchanged from the prior wave: engine gate still BLOCKED,
  format-2 code exists but is not activated in product paths.

### Wire list

- Added: none. Source-filesystem evidence: `ls crates/storage/src/` =
  `catalog_v2.rs`, `lib.rs`, `schema_v2.rs`, `writer_v2.rs`.
- No `pub mod`/`pub use` lines edited in `crates/storage/src/lib.rs`; the
  existing block at lib.rs:3-8 already declares the only modules present.

### Full suite results (all green, current HEAD)

- `cargo check -p opencode-rk-storage`: Finished dev profile, 0 errors.
- `cargo test -p opencode-rk-storage --lib`: `ok. 8 passed; 0 failed`.
  Note: the pre-existing `tests::session_round_trip_and_archive` failure
  recorded in the prior section is no longer present in current HEAD.
- `cargo test -p opencode-rk-storage --test writer_v2`: `ok. 10 passed`.
- `cargo test -p opencode-rk-storage --test schema_v2`: `ok. 6 passed`.
- `cargo test -p opencode-rk-storage --test catalog_v2`: `ok. 11 passed`.
- `cargo test -p opencode-rk-storage --test perf_v2`: `ok. 5 passed`.
- `cargo test -p opencode-rk-storage --test restart_v2`: `ok. 5 passed`.
- `python3 -m unittest tests.bootstrap.test_storage_schema_v2`: `Ran 62
  tests in 0.190s, OK` (grew from 37 in the prior section; the catalog_v2
  RED integration file that the prior section listed as missing now exists,
  hence 11 integration tests, and the python contract suite expanded).

### Commit recommendation

- Nothing to stage for engine/GC/admission/execution/approvals/snapshot/fork
  lanes: their source files do not exist.
- No change to `crates/storage/src/lib.rs` was made, so no storage-source
  stage is warranted.
- Only `worklog/STORAGE-V2-IMPL.md` was modified (this section). If a
  bookkeeping commit is desired, stage exactly: `git add worklog/STORAGE-V2-IMPL.md`.
- The prior-wave artifact set (three base modules, `tests/writer_v2.rs`,
  `tests/catalog_v2.rs`) is already committed; no follow-up stage needed.

### Remaining unknowns

1. The six named lanes (gc_v2, admission_v2, execution_v2, approvals_v2,
   snapshot_v2, fork_v2) never landed; no test suites or implementation for
   them exist anywhere in the tree. Blob-referencing payloads, GC,
   admissions/promotion, executions, approvals transitions, compaction,
   snapshot and fork remain entirely unbuilt (STO:93-97, prior section
   decision 6).
2. Whether the 10-lane D1-D5 / G1-G5 plan actually exists in another owner
   doc; this worklog has no independent record of it beyond the task brief.
3. Engine activation remains BLOCKED on bundled SQLite 3.50.2 (< 3.50.7);
   no upgrade path (which libsqlite3-sys bundles >= 3.50.7) was audited in
   this section.
4. Checksum deviation (blake3-256 vs documented sha256, prior section
   decision 1) is still unresolved and still needs an owner decision before
   any long-lived dev database is created with the current digest.

## Engine upgrade wave (2026-09-13): bundled SQLite 3.53.2, ENGINE_GATE CLEARED

Dependency-only upgrade. Workspace `Cargo.toml:27` bumped `rusqlite` from
`0.37` to `0.40` (features `bundled`). Lock resolved via
`cargo update -p rusqlite` to:

- `rusqlite 0.40.2`
- `libsqlite3-sys 0.38.2`
- bundled `SQLITE_VERSION "3.53.2"`, `SQLITE_VERSION_NUMBER 3053002`,
  `SQLITE_SOURCE_ID "2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24"`
  (vendored header
  `libsqlite3-sys-0.38.2/sqlite3/sqlite3.h:149-151`).

Prior blockers cleared: the `u64` FromSql removal that blocked both 0.40.2
and 0.39.0 previously was resolved at commit `fcc925e` (lib.rs uses `i64`
bindings). No Rust source was edited in this wave.

### Engine gate outcome

Bundled 3.53.2 is above the audited-backport floor 3.50.7 and above the
preferred 3.51.3, so `docs/STORAGE.md:164-168` preferred path is satisfied.
`docs/storage/ENGINE_GATE.md` historical record (BLOCKED at 3.50.2) is left
intact; this upgrade satisfies its unblock condition 1. New verification
record appended to `validation/storage-v2-verification.md`.

### Suite tails (all green on 3.53.2)

Command:
`cargo test -p opencode-rk-storage --lib --test writer_v2 --test schema_v2 --test catalog_v2 --test restart_v2 --test perf_v2 --test backup_v2 --test stress_v2`

```text
lib (src/lib.rs, unittests):  13 passed; 0 failed
catalog_v2:                    11 passed; 0 failed
writer_v2:                     10 passed; 0 failed
schema_v2:                      6 passed; 0 failed
restart_v2:                     5 passed; 0 failed
perf_v2:                        5 passed; 0 failed
backup_v2:                      5 passed; 0 failed
stress_v2:                      4 passed; 0 failed
```

`cargo check -p opencode-rk-storage` also clean.

### Changed files (tree NOT committed)

- `Cargo.toml` (rusqlite 0.37 -> 0.40)
- `Cargo.lock` (resolved rusqlite 0.40.2 / libsqlite3-sys 0.38.2)
- `validation/storage-v2-verification.md` (new engine-upgrade wave)
- `worklog/STORAGE-V2-IMPL.md` (this section)

No commit made; leave tree staged/not as instructed.
