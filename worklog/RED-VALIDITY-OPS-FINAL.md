# RED-VALIDITY-OPS-FINAL — behavior-RED final matrix, 9 OPS impl files

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. This lane: zero product/test
edits, zero cargo runs. Read-only verification of `worklog/RED-VALIDITY-OPS2.md`,
`/tmp/opencode/uB-*-red.log` (all 9 present, none missing), sha256 of 9 OPS src
files, `git diff --stat` foundation paths.

Prior stub-RED (`RED-VALIDITY-OPS.md`, E0432/E0433 compile-fail) weak per
`docs/TDD.md` §3. `RED-VALIDITY-OPS2.md` redid RED as BEHAVIOR RED: signatures
kept, one forced wrong behavior per file, suite compiles, fails on assertions.
This file is the final verdict matrix on that evidence (re-read from disk, not
copied on trust).

## Verdict: BEHAVIOR RED 9/9 valid, restore identical 9/9

| ID | Impl file | Test target | Behavior-RED compiling + assertion-fail? | RED detail (from log on disk) | Restores identical? |
|----|-----------|-------------|------------------------------------------|-------------------------------|---------------------|
| OPS-001 | `crates/foundation/src/resource_ledger.rs` | `resource_ledger` 5 | y | `test result: FAILED. 3 passed; 2 failed` (t03 admit_release, t04 double_bound `unwrap()` on `Err(SlotsExhausted)`); 2 `panicked at`; 0 E0432/E0433/`could not compile` | y (`b72c90179ba9` pre==post==disk) |
| OPS-002 | `crates/foundation/src/repo_ref.rs` | `repo_ref` 5 | y | `FAILED. 0 passed; 5 failed` (t01–t05 `unwrap()`/`PoisonError`/assert); 5 `panicked at`; 0 compile-fail | y (`63c45deaec32`) |
| OPS-003 | `crates/foundation/src/repo_cache_store.rs` | `repo_cache_store` 5 | y | `FAILED. 2 passed; 3 failed` (t01 inspect, t02 sweep, t04 corrupt-guard assertions); 3 `panicked at`; 0 compile-fail | y (`5c017f1cc5be`) |
| OPS-004 | `crates/foundation/src/install_meta.rs` | `install_meta` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`1548fa81dc66`) |
| OPS-005 | `crates/foundation/src/config_overlay.rs` | `config_overlay` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`bb3579aeb89a`) |
| OPS-006 | `crates/foundation/src/repo_ref_ext.rs` | `repo_ref_ext` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`84d236e33160`) |
| OPS-007 | `crates/foundation/src/ops_budget.rs` | `ops_budget` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`56d2829b443f`) |
| OPS-008 | `crates/foundation/src/ops_repo_ref.rs` | `ops_repo_ref` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`ae3746e4896d`) |
| OPS-009 | `crates/foundation/src/ops_replay.rs` | `ops_replay` 5 | y | `FAILED. 0 passed; 5 failed`; 5 `panicked at`; 0 compile-fail | y (`c775514f8e9a`) |

Counts verified by grep on disk: `test result: FAILED` ×1 in each of the 9
logs; `panicked at` 2/5/3/5/5/5/5/5/5; `E0432|E0433|could not compile` ×0 in
all 9. Partial-fail rows (OPS-001 3/2, OPS-003 2/3) expected: non-admit /
non-`read_state` tests pass; core paths fail on assertions (see OPS2 §
partial-fail notes). Only warnings in logs are `unreachable_code` /
`unused_variables` from the forced early-return inserts — compiles, not
compile-fail.

Restore: `diff /tmp/opencode/uB-pre.sha /tmp/opencode/uB-post.sha` → identical
(`HASH_MATCH_ALL9`, re-verified this lane); current disk sha256 of all 9 files
matches both manifests (full hashes in shell output; prefixes in table).

## Frozen-hash continuity note (formatter drift)

- No frozen-test hash manifest exists in `/tmp/opencode/*.sha` (only
  `uB-pre.sha`/`uB-post.sha`, which cover the 9 impl files). Continuity claim
  is therefore scoped: 9 impl files byte-identical pre/post/now; `tests/*`,
  `src/lib.rs`, `ralph.json`, `tasks/*` never written by either OPS RED lane.
- `git diff --stat HEAD -- crates/foundation/`: 6 of the 9 src files dirty vs
  HEAD (`resource_ledger`, `repo_ref`, `repo_cache_store`, `repo_ref_ext`,
  `ops_repo_ref`, `install_meta`), 3 clean (`config_overlay`, `ops_budget`,
  `ops_replay`). Inspected hunks on the 6 are rustfmt-only (multiline struct
  literals, chain splits, array layouts; e.g. `install_meta.rs` array split,
  `repo_cache_store.rs` 1-line join) — formatter drift from another lane, not
  behavior change, and post-date both RED lanes (RED sha prefixes predate the
  drift yet match current disk, i.e. drift already present at RED time).
- Frozen `tests/` dirty vs HEAD (7 OPS test files + others) and `src/lib.rs`
  dirty — all from another lane, same caveat as OPS/OPS2: RED proves
  test→impl wiring on current content, not pristine-commit validity.

## Boundary kept

Wrote only this worklog. No `src`/`tests`/`lib.rs`/`ralph.json`/`tasks` edits.
No cargo runs. Serial read-only commands under `timeout 120`, `rtk` prefix.
