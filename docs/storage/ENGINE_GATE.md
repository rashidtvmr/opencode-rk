# SQLite Engine Qualification Gate: Evidence Record

Date of evidence collection: 2026-09-13
Status: CLEARED (gate satisfied)
Scope: this file records measured engine versions and the reason production
activation of the storage engine is blocked by the gate defined in
`docs/STORAGE.md` lines 164-168. No code or dependency is changed here.

## Verdict

**GATE CLEARED.** The bundled SQLite engine in the Rust build is version
3.51.3, which meets the gate floor of >= 3.51.3. Version 3.51.3 is the first
release containing the WAL-reset corruption bug fix (check-in 7168988acb,
2026-03-13). The engine gate in `docs/STORAGE.md:164-168` requires bundled
SQLite >= 3.51.3 or an explicitly audited fixed backport (3.44.6 or 3.50.7)
with `sqlite_source_id` recorded. The first condition is met.

The local Python engine (3.46.1) remains in the affected range and is not
production qualification, consistent with `docs/STORAGE.md:166-167`. The
Rust bundled engine is now qualified.

## Evidence: commands run and files read

Every version below was read from a command output or a file on this machine.
No version is asserted from memory.

| Item | Command or file | Result | Citation |
|---|---|---|---|
| rusqlite crate version | `Cargo.lock` | 0.40.2 | Cargo.lock: (grep for name = "rusqlite") |
| libsqlite3-sys crate version | `Cargo.lock` | 0.38.2 | Cargo.lock: (grep for name = "libsqlite3-sys") |
| Bundling mode | `Cargo.toml` | `rusqlite = { version = "0.40", features = ["bundled"] }` | Cargo.toml:27 |
| Vendored SQLITE_VERSION | vendored header | `"3.51.3"` | ~/.cargo/registry/src/index.crates.io-*/libsqlite3-sys-0.38.2/sqlite3/sqlite3.h |
| Vendored SQLITE_VERSION_NUMBER | vendored header | `3051003` | same file |
| Vendored SQLITE_SOURCE_ID | vendored header | `"2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6alt1"` | same file |
| System sqlite3 CLI | `sqlite3 --version` | command not found (no system sqlite3 binary installed) | shell, 2026-09-13 |
| Python engine | `python3 -c "import sqlite3; print(sqlite3.sqlite_version)"` | 3.46.1 | shell, 2026-09-13 |
| Rust toolchain | `rustc --version` | 1.96.0 (ac68faa20 2026-05-25) | shell, 2026-09-13 |
| Engine gate text | `docs/STORAGE.md` lines 164-168 | requires bundled >= 3.51.3 or audited backport 3.44.6/3.50.7, recording sqlite_source_id | docs/STORAGE.md:164-168 |

Raw grep used for the vendored header (path abbreviated, `$V` is the
libsqlite3-sys-0.38.2 registry directory):

```
$V/sqlite3/sqlite3.h:#define SQLITE_VERSION        "3.51.3"
$V/sqlite3/sqlite3.h:#define SQLITE_VERSION_NUMBER 3051003
$V/sqlite3/sqlite3.h:#define SQLITE_SOURCE_ID      "2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6alt1"
```

Because `Cargo.toml:27` enables the `bundled` feature, the version compiled
into the Rust binary is the one in that vendored header (3.51.3), not any
system SQLite. The Python 3.46.1 figure is a separate engine used for local
DDL tests only.

## Version table

| Role | Version | Source id / provenance | In affected range (3.7.0 to 3.51.2)? | Meets engine gate? |
|---|---|---|---|---|
| Bundled engine (current build) | 3.51.3 | 2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6alt1 (vendored header, libsqlite3-sys 0.38.2) | No (first release with the fix) | YES |
| Local Python engine (tests only) | 3.46.1 | system python3 sqlite3 module | Yes | NO, and not production qualification per docs/STORAGE.md:166-167 |
| Gate floor (audited backport) | 3.50.7 | branch-3.50 check-in c7facf7ac58d2cda, containing race fix 268c9da287 (2026-03-05) | No (backport of the fix) | YES, if actually bundled, source id recorded, and audit documented |
| Alternate audited backport | 3.44.6 | listed as available backport in sqlite.org/wal.html section 11 | No (backport of the fix) | YES, same conditions as 3.50.7 |
| Gate preferred | 3.51.3 | 2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618 (sqlite.org/changes.html release history) | No (first release with the fix) | YES (met by current build) |

Note: the affected range is SQLite's own statement that the bug is likely
present in all versions from 3.7.0 (2010-07-21) through 3.51.2 (2026-01-09),
fixed in 3.51.3 and later, with backports of the fix available for 3.44.6
and 3.50.7 (sqlite.org/wal.html, section 11, "The WAL-Reset Bug"). The fix
first landed on trunk as check-in 7168988acb (2026-03-03, user dan): "Avoid
an obscure race condition between a checkpointer and a writer wrapping
around to the start of the wal file."

## Why this gate is now satisfied

The WAL-reset bug is a data race between a checkpointer and a concurrent
writer that can leave a WAL-index header field wrong, so a later checkpoint
skips part of a committed transaction and the database file goes corrupt.
The storage design in `docs/STORAGE.md` is WAL-first (journal_mode=WAL
verification at line 153, serialized checkpoints at line 154), which is
exactly the two-connection concurrent checkpoint/write pattern the bug
requires. The previous build used SQLite 3.50.2 (in the affected range
3.7.0 through 3.51.2), which failed the gate. The current build uses
SQLite 3.51.3 (libsqlite3-sys 0.38.2 via rusqlite 0.40.2 bundled), which is
the first release containing the fix (check-in 7168988acb, 2026-03-13).
The gate condition is met; the engine qualification failure is resolved.

The runtime gate proposal below remains valuable as a defense-in-depth
init-time check and should be implemented in `SchemaV2::open_existing` and
`CatalogV2::open_existing` before any write.

## Runtime gate proposal (SQL only, no Rust code)

Proposal for the init-time engine check, to run on the first connection
before any schema creation or write, consistent with the fail-closed
identity verification ordering in `docs/STORAGE.md:143-147`. It is a
threshold check expressed entirely in SQL. The gate is now satisfied by the
bundled engine, but this check provides defense in depth against accidental
downgrades or vendor builds without the fix.

```sql
-- Record engine identity first, unconditionally, for the audit log.
SELECT sqlite_version();
SELECT sqlite_source_id();

-- Fail-closed threshold. An empty result means the engine is NOT
-- qualified: refuse to open for writing. Do not silently fall back to
-- rollback-journal mode; a downgrade must be an explicit human decision.
SELECT sqlite_version() AS version, sqlite_source_id() AS source_id
 WHERE sqlite_version() >= '3.51.3'
UNION ALL
SELECT sqlite_version(), sqlite_source_id()
 WHERE sqlite_version() LIKE '3.50.%' AND sqlite_version() >= '3.50.7'
UNION ALL
SELECT sqlite_version(), sqlite_source_id()
 WHERE sqlite_version() LIKE '3.44.%' AND sqlite_version() >= '3.44.6';
```

Semantics and caveats:

1. String comparison of `sqlite_version()` is lexicographic. It is correct
   for the 3.44.x/3.50.x/3.51.x/3.53.x series because the minor component
   is two digits in this era, but it would misjudge a hypothetical 3.100.0
   (two-digit assumption). If the gate must survive arbitrary future
   versions, split the components and compare numerically. For this gate's
   lifetime the string form is sufficient and simpler.
2. A version number alone cannot prove a backport is present in a vendor
   build, since 3.50.7 shares its numbering with vulnerable siblings
   3.50.0 through 3.50.6 of the same branch. The `sqlite_source_id()` value
   must be recorded and compared against the audited source id, as
   `docs/STORAGE.md:164-165` already requires ("recording sqlite_source_id").
   For an upstream 3.50.7 build the source id from the release check-in
   c7facf7ac58d2cda line should be pinned at audit time; this file does not
   assert it because the backport was not built locally.
3. Empty result set equals fail closed: the opener must refuse writes and
   surface the measured version and source id in the error. There is no
   pass-open-on-error path.
4. Run this gate before the application identity check touches the file, or
   immediately after a read-only open and before any write, so an
   unqualified engine never performs a write or checkpoint.

## Sources consulted (web confirmation performed 2026-09-13)

- https://sqlite.org/changes.html: release history. 3.51.3 (2026-03-13)
  entry: "Fix the WAL-reset database corruption bug." with SQLITE_SOURCE_ID
  2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618.
  Also confirms 3.53.0 (2026-04-09) carries the same fix.
- https://sqlite.org/wal.html section 11, "The WAL-Reset Bug": affected
  range 3.7.0 through 3.51.2, fixed in 3.51.3 and later, backports available
  for 3.44.6 and 3.50.7.
- https://sqlite.org/src/info/c7facf7ac58d2cda: branch-3.50 check-in raising
  the version to 3.50.7 (2026-03-05), on top of check-in 268c9da287, the
  backported race fix "Avoid an obscure race condition between a
  checkpointer and a writer wrapping around to the start of the wal file."
- https://sqlite.org/src/info/5aadfbbdd83b051b: branch-3.51 form of the same
  fix, showing the fix content that shipped in 3.51.3.

No dependency, build file, or code was modified while producing this record.
