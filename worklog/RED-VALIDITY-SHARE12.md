# RED-VALIDITY-SHARE12 — behavior-RED matrix, SHARE-001/002

Rev: `248f519` (short). Workdir `/home/rashid/projects/opencode-rk`.
Own: `crates/sessions/src/share_merge.rs` + `crates/sessions/src/share_queue.rs` only.
Read: `tasks/SHARE-001.md`, `tasks/SHARE-002.md`, `docs/TDD.md` §§2-5.

Method per file: temp behavior-stub (signatures kept, one wrong behavior),
compiling RED run, byte-identical restore (sha256 pre==post), GREEN + lane mirrors.
Bounds: `free -h` first (avail 2.8 GiB, OK), serial, `CARGO_BUILD_JOBS=1`,
`RUST_TEST_THREADS=1`, `timeout 120`, `rtk` prefix every shell,
`--test-threads=1`, `/tmp/opencode` only.
NEVER touched: frozen `tests/*`, `src/lib.rs`, `ralph.json`, `tasks/*`.

## Verdict: BEHAVIOR RED 2/2 valid, restore identical 2/2

| ID | Impl file | Test target | Behavior-RED compiling + assertion-fail? | RED detail (from log on disk) | Restores identical? |
|----|-----------|-------------|------------------------------------------|-------------------------------|---------------------|
| SHARE-001 | `crates/sessions/src/share_merge.rs` | `share_merge` 5 | y | `/tmp/opencode/zA-SHARE-001-red.log` (62 lines): `test result: FAILED. 4 passed; 1 failed` — fail `share_merge_t01_last_write_wins_sorted` (`panicked at crates/sessions/tests/share_merge.rs:50:5`); T02-T05 pass. `could not compile` ×0. | y (`c97d3aab…` pre==post==disk) |
| SHARE-002 | `crates/sessions/src/share_queue.rs` | `share_queue` 5 | y | `/tmp/opencode/zA-SHARE-002-red.log` (84 lines): `test result: FAILED. 4 passed; 1 failed` — fail `share_queue_t01_coalesce_and_drain` (`panicked at crates/sessions/tests/share_queue.rs:54:13`); T02-T05 pass. `could not compile` ×0. | y (`25057f59…` pre==post==disk) |

Mutations (both reverted):
- SHARE-001: `map.insert(...)` → `if !map.contains_key(...) { insert }` (first-write-wins stub; T01 expects later batch wins).
- SHARE-002: replace-path `remove_key` → early `return` (drop-coalesce stub; keeps stale first value, T01 expects latest `{"v":2}`).
- One false start: un-Rust `if not` edit failed compile; immediately restored from `.bak`, redone with valid Rust; final logs above are clean behavior-RED (compile OK, assertion fail only).

GREEN post-restore (serial, JOBS=1 THREADS=1, `--test-threads=1`): `share_merge` 5/5, `share_queue` 5/5, `share_merge_lane` 5/5, `share_queue_lane` 5/5.

## Hashes

- src `share_merge.rs` `c97d3aabaae7151f4c4babc85e1a80c800eb0b0f01cea724f61c634d1e910fba` (pre==post; `/tmp/opencode/zA-pre.sha` diff clean → `RESTORE-IDENTICAL`).
- src `share_queue.rs` `25057f590658ece1038e8ff59af9885412f9d780b1a04cc00b2071376dd000cd` (pre==post).
- tests (read-only, never written): `share_merge.rs` `aaa1e6fb…`, `share_queue.rs` `7b8ba795…`, `share_merge_lane.rs` `17285450…`, `share_queue_lane.rs` `43dba68b…`.

## Pre-existing dirt (not this lane)

`git diff --stat` shows rustfmt-only hunks in `tests/share_merge.rs` (import reorder, line join), `tests/share_queue.rs` (call-chain split), `src/lib.rs` (+3 `pub mod` lines) — all present before this session's first mutation (initial reads already showed reordered imports) and untouched by this lane (only the two `src` files were written, then restored). Same caveat as prior lanes: RED proves test→impl wiring on current content, not pristine-commit validity. `src/share_merge.rs` (+13/-2) and `src/share_queue.rs` (+13/-5 vs HEAD) hunks are the redaction-lane manual-`Debug` impls, pre-existing.

## Boundary kept

Wrote only this worklog + `/tmp/opencode/zA-*` logs/manifests. No `tests/`/`lib.rs`/`ralph.json`/`tasks/` edits. Lane mirrors (`share_merge_lane.rs`, `share_queue_lane.rs`, owned by parallel lane) never touched.
