---
name: SQLite Safety
description: Safe SQLite access for storage/sessions crates — WAL, FK, bounded read-only queries, disposable DBs only
---

## Rules

- Writer: `foreign_keys=ON, synchronous=FULL, busy_timeout=5000, cache_size=-8192, mmap_size=0, trusted_schema=OFF`.
- Verify `journal_mode=WAL` returns `wal`; `page_size=4096`, one blocking writer, max 4 handles.
- Readers: `foreign_keys=ON, query_only=ON, trusted_schema=OFF`, small cache, bounded txn.
- Queries: EXPLAIN-gated, max 4 bound params, always `LIMIT`, byte caps (inline <=8192).
- Migrations: check `application_id`, `user_version=2`, SHA-256 checksum drift fails closed.
- Tests: disposable fixtures only (`:memory:`/tmp). Never open/mutate user 90GB DB.
- Engine: rusqlite 0.40 bundled SQLite 3.51.3; no silent downgrade.
