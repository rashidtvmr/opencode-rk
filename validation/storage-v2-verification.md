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

## Verification wave: 2026-09-13

This entry records the current uncommitted storage-v2 implementation tree. The
earlier statement at line 12 that the Rust integration tests were not compiled
or executed is historical evidence for that earlier wave. It is superseded for
the current writer and schema integration targets below, without rewriting the
historical entry.

### Scope and tree counts

The reviewed current-wave files and `wc -l` counts are:

| File | Lines |
| --- | ---: |
| `crates/storage/src/catalog_v2.rs` | 330 |
| `crates/storage/src/schema_v2.rs` | 196 |
| `crates/storage/src/writer_v2.rs` | 221 |
| `crates/storage/tests/writer_v2.rs` | 321 |
| `crates/storage/tests/schema_v2.rs` | 134 |
| `crates/storage/src/lib.rs` | 38 |

`crates/storage/src/lib.rs` currently contains six additive v2 exports:
`catalog_v2`, `schema_v2`, `writer_v2`, `CatalogV2`, `SchemaV2`, and the
`NewMessage`, `NewSession`, `V2Writer` re-exports. Format 1 remains in place;
no format-1 migration or behavior change was made by this verification wave.

### Environment

- `rustc --version`: `rustc 1.96.0 (ac68faa20 2026-05-25)`.
- `python3 --version`: `Python 3.14.4`.
- `python3 -c "import sqlite3; print(sqlite3.sqlite_version)"`: `3.46.1`.
- Bundled Rust SQLite: `3.50.2`, from the vendored `libsqlite3-sys 0.35.0`
  header documented in `docs/storage/ENGINE_GATE.md`.

The Python SQLite version is local DDL-test evidence only. It is not production
engine qualification.

### Exact test commands and outcomes

1. Command:

   ```text
   cargo test -p opencode-rk-storage --test writer_v2 2>&1 | tail -3
   ```

   Tail:

   ```text
   test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
   ```

2. Command:

   ```text
   cargo test -p opencode-rk-storage --test schema_v2 2>&1 | tail -3
   ```

   Tail:

   ```text
   test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
   ```

3. Command:

   ```text
   cargo test -p opencode-rk-storage --lib 2>&1 | tail -3
   ```

   Tail:

   ```text
   test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
   error: test failed, to rerun pass `-p opencode-rk-storage --lib`
   ```

   The failing test is the existing format-1
   `tests::session_round_trip_and_archive` assertion at
   `crates/storage/src/lib.rs:38:475`. It compares the millisecond value
   `2026-09-13T13:44:41.707Z` with the nanosecond-preserving value
   `2026-09-13T13:44:41.707220795Z`. This is not a green library suite.

4. Command used to isolate that failure while preserving the working tree:

   ```text
   git stash -q && cargo test -p opencode-rk-storage --lib tests::session_round_trip_and_archive 2>&1 | tail -2; git stash pop -q
   ```

   Outcome: the targeted test failed with the same timestamp-precision
   assertion; `git stash pop -q` succeeded and restored the working tree.

5. Command:

   ```text
   python3 -m unittest tests.bootstrap.test_storage_schema_v2 2>&1 | tail -3
   ```

   Tail:

   ```text
   Ran 37 tests in 0.225s
   OK
   ```

The current writer and schema integration targets therefore compile and pass,
while the storage library target remains red on the existing timestamp
precision assertion.

### Engine gate

Status: **BLOCKED**. `docs/storage/ENGINE_GATE.md` records bundled SQLite
`3.50.2`, below the required preferred `3.51.3` and below the audited
backport floor `3.50.7` (or the alternate audited `3.44.6`). The current
bundle is in the documented affected range. Format-2 activation remains
fail-closed. Passing local Python or Rust tests does not override this gate.

### Formatting and restoration incident

The shell log records `cargo fmt -p opencode-rk-storage` at
`/home/rashid/.local/share/opencode/log/opencode.log:161596`, followed later by
`git checkout crates/storage/src/lib.rs` at line 161942 to restore the file
after its export edits were endangered. The current file was then checked: it
is 38 lines and contains the six exports listed above. This records the
observed formatting and restoration commands, not an unsupported claim about
unlogged intermediate contents.

### Explicit NOT-CLAIMED

- No release acceptance or completion certificate.
- No production durability, crash-consistency, or power-loss proof.
- No engine qualification or format-2 activation; the engine gate is BLOCKED.
- No full storage library suite, full-repository regression, or benchmark claim.
- No catalog-specific frozen test was run in this wave.
- No filesystem `fsync`, no-replace, OS-lock race, power-cut, upstream
  differential, or resource-bound test was run.
- No independent frozen RED/GREEN acceptance receipt.
- No format-1 migration or format-1 replacement. Format 1 was left untouched.
- No access to or modification of a user's existing OpenCode database. Tests
  used disposable or test fixtures only.
- No runtime migration was activated.
