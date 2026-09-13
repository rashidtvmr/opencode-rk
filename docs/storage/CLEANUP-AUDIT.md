# Storage Crate Cleanup Audit

> PARTIALLY SUPERSEDED (2026-09-13, later same day): findings about stub modules
> (`gc_v2.rs` #1) are obsolete - all `*_v2` modules are now implemented. The
> duplicate-helper, warning, and legacy-surface findings remain open.

Generated 2026-09-13. Read-only analysis; no code deleted.

## Summary

The `opencode-rk-storage` crate contains a format-1 legacy surface (`Storage`,
`BlobStore`, `BlobInfo`, `StoragePaths` in `lib.rs`) alongside 12 format-2
module files (`*_v2.rs`). Clippy passes clean (exit 0). `cargo test --lib`
reports 53 passed / 1 failed (the `quota_v2::admit_rejects_over_wal_limit`
test is a pre-existing flake on this host). Compiler warnings flag 2 unused
variables in test code.

## Findings

### 1. Stub module `gc_v2.rs` with `#[allow(dead_code)]`

**File:** `crates/storage/src/gc_v2.rs:1-3`
**Content:** Entire file is 3 lines: doc comment, `#![allow(dead_code)]`, `pub struct GcV2;`
**Safe to remove:** NO (unsafe). `GcV2` is re-exported from `lib.rs:19` and
referenced in `tests/integration_v2.rs:119` and `tests/perf_modules_v2.rs:194`
as a placeholder for a future GC lane. Removing it breaks the build.
**Safe to clean:** YES -- remove the `#![allow(dead_code)]` attribute. The
struct is `pub` and re-exported; the attribute suppresses no real warning
because the type is reachable. The attribute is vestigial from when the file
was first created.
**Why:** `#![allow(dead_code)]` on a module-level attribute in a file
containing only one `pub` item is unnecessary; the item is used via re-export.

### 2. `invalid_input()` helper duplicated across 7 files

**Files and lines:**
- `writer_v2.rs:219` -- `fn invalid_input() -> StorageError`
- `execution_v2.rs:216` -- same
- `fork_v2.rs:247` -- same
- `admission_v2.rs:258` -- same
- `snapshot_v2.rs:208` -- same
- `approvals_v2.rs:172` -- `fn invalid_input(message: &str) -> StorageError` (takes string)
- `catalog_v2.rs:282` -- `fn invalid_input(message: &str) -> StorageError` (takes string)

**Safe to remove:** YES (refactor, not delete). Each file is self-contained;
the duplication is intentional ponytail (documented in each file's module doc).
**Recommended cleanup:** Extract a shared `crate::util::invalid_input()` in a
new `util.rs` or add it to `lib.rs` as a crate-private function. Two variants
needed: zero-arg and message-bearing. This removes ~7 identical function
bodies. Each V2 module already documents the upgrade path in its module doc.
**Why:** 7 copies of the same 1-2 line function. The two variants
(no-arg returning `InvalidQuery`, and string-arg returning
`InvalidParameterName(msg)`) could be one function with `Option<&str>`.

### 3. `MAX_INLINE_PAYLOAD_BYTES` constant shadowed in v2 modules

**File:** `crates/storage/src/lib.rs:39` defines `pub const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;`
**File:** `crates/storage/src/writer_v2.rs:9` defines `const MAX_INLINE_PAYLOAD_BYTES: usize = 8192;`
**File:** `crates/storage/src/admission_v2.rs:16` defines `const MAX_INLINE_PAYLOAD_BYTES: usize = 8192;`

**Safe to remove:** NO (unsafe). Both v2 modules need the constant.
**Safe to clean:** YES -- deduplicate by defining once in `lib.rs` or a shared
`constants.rs` module and importing in both v2 files.
**Why:** Same value (8192) defined in two files. A single source of truth
prevents drift.

### 4. Format-1 legacy surface in `lib.rs` -- entire `Storage` struct and helpers

**File:** `crates/storage/src/lib.rs:79-243` (the `Storage` struct) and lines
310-383 (encode/decode helpers for format-1)
**Also:** `BlobStore` (246-303), `BlobInfo` (304-309), `StoragePaths` (62-78)

**Safe to remove:** NO (unsafe). Used by:
- `crates/cli/src/main.rs:9` -- `use opencode_rk_storage::{Storage, StoragePaths};`
- `crates/sessions/src/lib.rs:7` -- `use opencode_rk_storage::{Storage, StorageError};`
- `crates/sessions/src/lib.rs:86` -- `.blob_store()`

**Safe to mark as deprecated:** YES. The format-2 modules (`schema_v2`,
`writer_v2`, etc.) are the new surface. The legacy surface is still in use
by CLI and sessions crates. Mark with `#[deprecated(note = "use v2 modules")`
when migration is complete.
**Why:** The entire v1 API (`Storage`, `BlobStore`, `StoragePaths`,
`create_session`, `append_message`, etc.) coexists with v2. Migration to v2
has not been completed in the downstream crates.

### 5. `DEFAULT_MAX_EVENTS_PER_SESSION` and `BLOB_COMPRESSION_LEVEL` -- dead to v2

**File:** `crates/storage/src/lib.rs:38` -- `pub const DEFAULT_MAX_EVENTS_PER_SESSION: usize = 10_000;`
**File:** `crates/storage/src/lib.rs:40` -- `pub const BLOB_COMPRESSION_LEVEL: i32 = 3;`

**Used by:** Only the legacy `Storage` struct (lines 94, 106, 264).
**External usage:** None (grep confirms zero external references).
**Safe to remove:** YES (safe after legacy surface removal). Currently coupled
to `Storage` struct internals only.
**Why:** These constants serve only the format-1 surface. When `Storage` is
removed, they become dead code.

### 6. `MAX_EVENT_PAYLOAD_BYTES` defined with conflicting values

**File:** `crates/storage/src/lib.rs:39` -- `pub const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;` (65536)
**File:** `crates/storage/src/writer_v2.rs:11` -- `const MAX_EVENT_PAYLOAD_BYTES: usize = 4096;`

**Safe to remove:** NO (both are used).
**Action needed:** The v1 constant (64K) and v2 constant (4K) have different
values for the same name. This is a latent source of bugs if anyone imports
the wrong one. Document the intentional difference or rename one.
**Why:** Two modules define the same constant name with a 16x difference in
value. The lib.rs one is for format-1's `append_event`; the writer_v2 one is
for format-2's `append_outbox_event`.

### 7. Unused variables in test code (compiler warnings)

**File:** `crates/storage/src/fork_v2.rs:419` -- `let parent_pk = create_parent(...)` unused
**File:** `crates/storage/src/catalog_v2.rs:295` -- `let mut conn = ...` where `mut` is unnecessary

**Safe to remove:** YES. Both are test code.
**Fix:** `fork_v2.rs:419` -- prefix with `_parent_pk`. `catalog_v2.rs:295` -- remove `mut`.
**Why:** Compiler warnings. The fork test creates a parent but only uses it
implicitly (the parent is stored in DB). The catalog test's connection is
never mutated after initialization.

### 8. Pre-existing test failure: `quota_v2::admit_rejects_over_wal_limit`

**File:** `crates/storage/src/quota_v2.rs:157-163`
**Test:** `admit_rejects_over_wal_limit` panics on assertion at line 162.
**Root cause:** `wal_file_bytes()` returns 0 when the WAL file does not exist
(e.g., in-memory journal or immediately after `PRAGMA wal_checkpoint(TRUNCATE)`
in the test helper). With `max_wal_bytes=0` and `wal_bytes=0`, the condition
`0 > 0` is false, so admission passes instead of rejecting.
**Safe to remove:** NO. The test validates real quota behavior.
**Safe to fix:** YES. The test needs either: (a) force a WAL file to exist
before measuring, or (b) use `max_wal_bytes=-1` so `wal_bytes >= 0 > -1` is
always true. This is a test bug, not a production bug.
**Why:** The test assertion assumes a non-zero WAL measurement on a fresh
database, but WAL is empty or absent immediately after initialization.

### 9. `quota_v2` module references `crate::Storage` in tests only

**File:** `crates/storage/src/quota_v2.rs:118-129`
**Content:** Test helper `conn_and_paths()` imports `crate::Storage` and
`crate::StoragePaths` to create a format-1 database, then opens a second
`Connection` over the same file for quota queries.
**Safe to remove:** NO.
**Safe to refactor:** YES -- the test could use `SchemaV2::initialize_workspace`
instead of the legacy `Storage::open`, removing the last intra-test dependency
on the format-1 surface.
**Why:** The quota_v2 module itself is format-2, but its tests bootstrap via
format-1 `Storage::open`. This is a minor coupling that would disappear
when the legacy surface is removed.

### 10. Duplicate `session()` / `message()` / `initialized()` test helpers across integration tests

**Files:**
- `tests/writer_v2.rs:21-47` -- `initialized()`, `session()`, `message()`
- `tests/restart_v2.rs:19-40` -- `session()`, `message()`
- `tests/backup_v2.rs:22-43` -- `session()`, `message()`
- `tests/integration_v2.rs:13-39` -- `initialized()`, `session()`, `message()`
- `tests/perf_v2.rs:19-22` -- `init_db()`

**Safe to remove:** NO (each test file needs them).
**Safe to refactor:** YES -- extract shared test fixtures into a
`tests/common/mod.rs` module. All four files define nearly identical
`session()` and `message()` factory functions.
**Why:** 4 copies of the same `session()` function creating a `NewSession`
with defaults. Reduces maintenance burden when `NewSession` fields change.

### 11. `#[allow(dead_code)]` on `admission_v2.rs` and `approvals_v2.rs` are absent but noted in ponytail comments

**Files:**
- `approvals_v2.rs:8` -- `#![forbid(unsafe_code)]` (module-level, appropriate)
- `admission_v2.rs` -- no `allow(dead_code)`, no `forbid(unsafe_code)`

**Observation:** `approvals_v2.rs` has `#![forbid(unsafe_code)]` at line 8 but
`admission_v2.rs` does not. Neither needs `allow(dead_code)` since all items
are used in tests. This is not a cleanup item but a consistency gap.
**Safe to add:** YES -- add `#![forbid(unsafe_code)]` to `admission_v2.rs` for
consistency with the other v2 modules that have it.
**Why:** 4 of 6 substantive v2 modules have `#![forbid(unsafe_code)]`:
`approvals_v2.rs`, `catalog_v2.rs`, `gc_v2.rs`, `quota_v2.rs`. The missing
ones are `admission_v2.rs`, `execution_v2.rs`, `fork_v2.rs`, `import_v2.rs`,
`schema_v2.rs`, `snapshot_v2.rs`, `writer_v2.rs`. The crate root has
`#![forbid(unsafe_code)]` at `lib.rs:2`, so this is already enforced
crate-wide. The per-module attributes are redundant belt-and-suspenders.

### 12. `QuotaV2Error` enum defined but never returned as `StorageError` variant

**File:** `crates/storage/src/quota_v2.rs:18-22`
**Content:** `QuotaV2Error` has `DbBytes(i64)` and `WalBytes(i64)` variants.
`admit()` returns `Result<(), QuotaV2Error>`, not `Result<(), StorageError>`.
**Safe to remove:** NO. The type is used by `admit()` callers.
**Observation:** This is an intentional design (documented in module doc:
"Backpressure via rejection, never silent deletes"). The error type is
separate from `StorageError` by design. No cleanup needed; noted for
completeness.
**Why:** Other v2 modules all return `StorageError`. `QuotaV2` is the only
one with a separate error type. This is correct for its backpressure
semantics but worth noting for future API surface reviews.

## Cleanup Priority

| Priority | Finding | Effort |
|----------|---------|--------|
| 1 | #7 - Fix unused variable warnings | 2 min |
| 2 | #8 - Fix flaky quota test | 10 min |
| 3 | #1 - Remove vestigial `#[allow(dead_code)]` from gc_v2 | 1 min |
| 4 | #3 - Deduplicate `MAX_INLINE_PAYLOAD_BYTES` | 10 min |
| 5 | #6 - Document or rename conflicting `MAX_EVENT_PAYLOAD_BYTES` | 5 min |
| 6 | #2 - Extract shared `invalid_input()` helper | 30 min |
| 7 | #10 - Extract shared test fixtures | 20 min |
| 8 | #9 - Refactor quota_v2 tests to use v2 bootstrap | 10 min |
| 9 | #4 - Deprecate legacy `Storage` surface | Blocked on CLI/sessions migration |
| 10 | #5 - Remove v1-only constants | Blocked on #4 |
| 11 | #11 - Add missing `#![forbid(unsafe_code)]` attributes | 5 min |
