# RED-VALIDITY OPS5 — behavior-RED OPS-001..009 (rev 248f519)

Lease: OPS-001..009 only. Owned impls: `crates/foundation/src/{resource_ledger,repo_ref,repo_cache_store,install_meta,config_overlay,repo_ref_ext,ops_budget,ops_repo_ref,ops_replay}.rs`. NO lib.rs edits (pre-existing other-lane `M crates/foundation/src/lib.rs` left alone; no E0432 encountered in this lane, so nothing blocked, nothing fixed). NEVER touched frozen `ralph.json` (`git status --short -- ralph.json` empty; no `frozen/` dir exists).

Method per ID: signatures kept, one compiling wrong-value probe injected (marked `RED-PROBE-zH-<ID>`), single failing test run serial (`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `free -h` before each run), rc=101 + assertion-fail confirmed (not compile error), probe removed, file restored byte-identical vs `/tmp/opencode/zH-bak/*.orig` (`cmp` identical), full 5/5 GREEN rc=0 re-run serial per target.

Backups: `/tmp/opencode/zH-bak/<name>.rs.orig` (9 files). RED logs: `/tmp/opencode/zH-OPS-<001..009>-red.log` (all rc=101).

| id | impl (sha256 restored = pre = post) | RED probe (signature kept) | RED log / rc / fail | GREEN 5/5 |
|---|---|---|---|---|
| OPS-001 | resource_ledger.rs b72c9017…fae5ccf | `admit` early `return Err(SlotsExhausted)` | zH-OPS-001-red.log rc=101 `FAILED 3 passed; 2 failed`; 2 `panicked at` tests/resource_ledger.rs:71 `unwrap()` on `Err(SlotsExhausted{SessionInput})`, :118 `unwrap()` on `Err(SlotsExhausted{AgentSlot})`; 0 E0432/E0433/compile | 5/5 rc=0 |
| OPS-002 | repo_ref.rs 63c45dea…af1ba00 | skip `validate_branch`, explicit branch forced `main` | zH-OPS-002-red.log rc=101 `FAILED 0 passed; 5 failed`; 5 `panicked at` t01 :28, t02 :61, t03 :85, t04 :126, t05 :144; 0 compile-fail | 5/5 rc=0 |
| OPS-003 | repo_cache_store.rs 5c017f1c…011273e | `read_state` early `return Fresh` | zH-OPS-003-red.log rc=101 `FAILED 2 passed; 3 failed`; 3 `panicked at` t01 :49, t02 :88, t04 :180; 0 compile-fail | 5/5 rc=0 |
| OPS-004 | install_meta.rs 1548fa81…47afda35 | unknown channel falls back `Ok(raw/Stable)` not `UnknownChannel` | zH-OPS-004-red.log rc=101 `FAILED 4 passed; 1 failed`; 1 `panicked at` tests/install_meta.rs:72 t04; 0 compile-fail | 5/5 rc=0 |
| OPS-005 | config_overlay.rs bb3579ae…af6b29e1 | empty docs `Ok(empty)` not `NoDocs` | zH-OPS-005-red.log rc=101 `FAILED 4 passed; 1 failed`; 1 `panicked at` tests/config_overlay.rs:87 t04; 0 compile-fail | 5/5 rc=0 |
| OPS-006 | repo_ref_ext.rs 84d236e3…23b64928b | empty-raw accept-anything stub (`canonical:stub`, 64x`0` id) | zH-OPS-006-red.log rc=101 `FAILED 4 passed; 1 failed`; 1 `panicked at` tests/repo_ref_ext.rs:62 t04 `left == right`; 0 compile-fail | 5/5 rc=0 |
| OPS-007 | ops_budget.rs 56d2829b…120f96763 | `admit` unconditional `Ok` (caps bypassed) + dead `if false` guard | zH-OPS-007-red.log rc=101 `FAILED 2 passed; 3 failed`; 3 `panicked at` t03 :42, t04 :113, t05 :129; 0 compile-fail | 5/5 rc=0 |
| OPS-008 | ops_repo_ref.rs ae3746e4…7ca7bc799 | `validate_branch` early `return Ok(branch)` (skip check) | zH-OPS-008-red.log rc=101 `FAILED 2 passed; 3 failed`; 3 `panicked at` t03 :55, t04 :74, t05 :114; 0 compile-fail | 5/5 rc=0 |
| OPS-009 | ops_replay.rs c775514f…dfd1885355 | `replay` early `return Ok(requests.len())` (skip matching) | zH-OPS-009-red.log rc=101 `FAILED 2 passed; 3 failed`; 3 `panicked at` t02 :35, t04 :86, t05 :118; 0 compile-fail | 5/5 rc=0 |

Restore proof: all 9 `cmp <impl> /tmp/opencode/zH-bak/<impl>.orig` IDENTICAL; `grep -rn RED-PROBE-zH` on all 9 impls empty (rc=1); sha256 now == pre-probe values above. This lane net content change zero. No test file, task card, lib.rs, or ralph.json written. Only file written: this worklog.

Serial discipline: one cargo target at a time, JOBS=1 THREADS=1, `timeout 120`, `rtk` prefix, `free -h` before runs (2.4–2.7 GiB avail throughout; no parallel builds).

## aH VERIFY-ONLY confirm 2026-09-16T04:13Z rev 248f519
Lease: OPS-001..009 VERIFY-ONLY. Zero source edits (no probes, no stubs, no lib.rs writes). NEVER touched ralph.json/frozen (git status clean, no frozen/ dir).
Serial: JOBS=1 THREADS=1 timeout 120 free -h before each run (1.6-2.7 GiB avail). Log: /tmp/opencode/aH-ops.log (352 lines).
Hashes pre==post (recorded head+tail of log): resource_ledger b72c9017..fae5ccf, repo_ref 63c45dea..af1ba00, repo_cache_store 5c017f1c..011273e, install_meta 1548fa81..afda35, config_overlay bb3579ae..af6b29e1, repo_ref_ext 84d236e3..23b64928b, ops_budget 56d2829b..120f96763, ops_repo_ref ae3746e4..7ca7bc799, ops_replay c775514f..dfd1885355.
9 suites x 5/5 GREEN rc=0, fail-markers=0: resource_ledger, repo_ref, repo_cache_store, install_meta, config_overlay, repo_ref_ext, ops_budget, ops_repo_ref, ops_replay. cargo check -p opencode-rk-foundation rc=0 (1 dead_code warning repo_cache_store.rs:141 root).
Stub grep todo!/unimplemented!/RED-PROBE on 9 impls: empty (rc=1). Test files own 5 #[test] each (45 total). Only file written: this appendix + /tmp/opencode/aH-ops.log.

## bH VERIFY-ONLY confirm-2 2026-09-16 rev 248f519
Lease: OPS-001..009 VERIFY-ONLY. Zero source edits (no probes, no stubs, NO lib.rs edits). NEVER touched ralph.json/frozen (status --short -- ralph.json frozen/ empty; no frozen/ dir).
Serial: JOBS=1 THREADS=1 timeout 120 free -h before each run (2.3-2.5 GiB avail). Log: /tmp/opencode/bH-ops.log (93 lines).
Hashes pre==post: resource_ledger b72c9017..fae5ccf, repo_ref 63c45dea..af1ba00, repo_cache_store 5c017f1c..011273e, install_meta 1548fa81..afda35, config_overlay bb3579ae..af6b29e1, repo_ref_ext 84d236e3..23b64928b, ops_budget 56d2829b..120f96763, ops_repo_ref ae3746e4..7ca7bc799, ops_replay c775514f..dfd1885355.
9 suites x 5/5 GREEN rc=0 (45 total): resource_ledger, repo_ref, repo_cache_store, install_meta, config_overlay, repo_ref_ext, ops_budget, ops_repo_ref, ops_replay. cargo check -p opencode-rk-foundation rc=0 (1 dead_code warning repo_cache_store.rs:141 root).
Stub grep todo!/unimplemented!/RED-PROBE on 9 impls: empty (rc=1). Test files own 5 #[test] each (45 total).
git diff --stat foundation paths (worktree-vs-HEAD, pre-existing dirt, no writes this lane): 13 files 111+/56- (src: install_meta 7, ops_repo_ref 12, repo_cache_store 3, repo_ref 13, repo_ref_ext 8, resource_ledger 17; tests: ops_budget 10, ops_replay 5, ops_repo_ref 13, repo_cache_store 32, repo_ref 15, repo_ref_ext 5, resource_ledger 27). Only file written: this appendix + /tmp/opencode/bH-ops.log.
