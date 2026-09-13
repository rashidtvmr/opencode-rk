# Storage format 2: local verification

Base repository commit: `53aac452acfe64e5adab3fd4fcc9597f90e79cda`.

Scope: new SQL schema contracts only, not the existing full repository or Rust runtime.

- Command: `python3 -m unittest discover -s tests/bootstrap -v` in the isolated storage-wave directory.
- Result: **37 tests passed, zero skipped**. Full output: `storage-v2-tests.log`.
- Process-crash test: real `os._exit(17)` before/after COMMIT; reopens SQLite and checks state/outbox plus quick_check. This is not a power-loss test.
- Mutation: removing payload_ready from a disposable DB causes the expected negative test failure; `storage-v2-mutation.log`.
- Both exact DDL files load with foreign_keys ON and trusted_schema OFF. Four EXPLAIN paths use the intended indexes without temporary sort B-trees.
- Six Rust integration tests written against the same DDL; **not compiled or executed** (cargo/rustc absent).
- No filesystem fsync/no-replace/OS-lock race test, power-cut test, full-repository regression suite, upstream differential test or resource benchmark executed.
- No runtime migration activated; no original OpenCode database touched.
- This is author-run verification, not an independently frozen RED/GREEN acceptance receipt.

Environment: Python 3.13.5, SQLite 3.46.1.
The Python SQLite version is for DDL testing, not the production fixed-engine qualification.
Execution timestamp (UTC): 2026-09-13T11:12:52.617633+00:00.

## Exact tested/reviewed file hashes

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `crates/storage/schema/v2/catalog.sql` | 2813 | `764292c9e3b3548fd1dc95d80565851822b3441af43180895dc7398c45a16d97` |
| `crates/storage/schema/v2/workspace.sql` | 19127 | `f0356e2263d605cfa364d5ab7131701160d83133da6ce8858a8d5e31ac3cccd6` |
| `crates/storage/tests/schema_v2.rs` | 4303 | `bb5a496cc57de3a954027f16499cce72481ea351205623a853101abda9060ecd` |
| `docs/STORAGE.md` | 12803 | `343bfeeefdcba81734f31c13c5c588d5f920b2d217529595a0cc7de5d42f6293` |
| `docs/storage/CRASH_CONSISTENCY.md` | 14860 | `eaed76e715dddd77f520ce96c75dbcbf2437e062dd56ba8231cd611b0961e4dd` |
| `docs/storage/REVIEW.md` | 3724 | `8a95b163b5deb1496bdce69c79ff2be93fcd2329cccacf927e4d97f7869e8172` |
| `tests/bootstrap/test_storage_schema_v2.py` | 17762 | `96878665e642408b89fdd87f5c61b8ea8fd75c83aea44c691c8f19d132c6eefd` |
| `validation/storage-v2-mutation.log` | 1267 | `f6dcbe37300a0a74d131d81aeb0f162c3b8f154788f760215c8e5e43ec3849a9` |
| `validation/storage-v2-tests.log` | 5293 | `83cbc322ecc65f4a87b9a829d119777d6a8c9ba2770b3ed1c03c757f63c3edae` |
| `worklog/STORAGE-V2.md` | 1160 | `b1ac8cb7008ad0c30b4869ba55ce48c845509d51c400a652a728ee34be1be435` |
