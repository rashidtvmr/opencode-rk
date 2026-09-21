# FIX-SQLITE-GATE scratchpad

Claim: FIX-SQLITE-GATE, session ses_fix_sqlite, owned file crates/storage/src/lib.rs only.

## Source evidence
- `crates/storage/src/lib.rs:106-117` `Storage::open` -> `configure` (line 639) -> `migrate` (line 644 static CREATE TABLE). `MigrationRunner` (`migrations.rs:33`) never called from open. Out of lane.
- `crates/storage/src/lib.rs:639-643` `configure`: `synchronous=NORMAL`, `journal_mode WAL`, no version check, no WAL verification.
- Gate contract `docs/STORAGE.md:164-168`: bundle >= 3.51.3 or audited fixed backport 3.44.6 / 3.50.7, record sqlite_source_id. WAL-reset race fixed 3.51.3 (check-in 7168988acb), backports per `docs/storage/ENGINE_GATE.md`.
- Measured current build (this machine, not memory): `Cargo.lock` rusqlite 0.40.2 -> libsqlite3-sys 0.38.2; vendored header `SQLITE_VERSION "3.53.2"`, `SQLITE_VERSION_NUMBER 3053002`. Meets preferred path (>= 3051003).
- `docs/STORAGE.md:235-255` already records gate CLEARED on 3.51.3; `validation/storage-v2-verification.md:144-150` BLOCKED note is stale (written against 3.50.2 bundle). Runtime gate proposal at `docs/storage/ENGINE_GATE.md:90-138` wants init-time fail-closed check; string-compare caveat noted there -> use numeric `rusqlite::version_number()` instead.

## Observed scenario
`Storage::open` on a below-gate engine today: silently enters WAL anyway. No typed error, no durability boundary documented at call site.

## Target boundary
- Gate + honest error only, in `configure` (runs before any write in both `open` and `open_in_memory`).
- Typed `StorageError::UnsupportedSqliteEngine { version, source_id }`, fail-closed before PRAGMAs.
- Numeric predicate: `version_number >= 3051003`; all backport releases rejected because no audited `sqlite_source_id()` allowlist exists. This preserves fail-closed provenance behavior.
- Doc comment on `configure`: durability boundary (bootstrap synchronous=NORMAL vs v2 FULL, WAL same-host FS semantics, engine floor, local Python 3.46.1 not qualification).
- Do NOT touch migrations.rs, tests, Cargo.toml/lock, docs. No commit/push per lane orders.

## Tests
Frozen lib tests untouched. Verify: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-storage --lib`.

## Decisions
- `rusqlite::version_number()` over `SELECT sqlite_version()` string compare (ENGINE_GATE note 1 caveat).
- `sqlite_source_id()` queried only on refusal path for error context; `unknown` fallback so happy path gains no new failure mode. Backports intentionally remain refused until exact audited IDs are available.
- No journal_mode verification added: out of gate scope; filesystem WAL refusal is a separate lane.

## Remaining unknowns
- Tests pending; verifier decides acceptance.
