# RED-VALIDITY OPS-001..009 (rev 248f519)

Lease: OPS-001..009 only. Impl files owned: `crates/foundation/src/{resource_ledger,repo_ref,repo_cache_store,install_meta,config_overlay,repo_ref_ext,ops_budget,ops_repo_ref,ops_replay}.rs`. Frozen tests + `ralph.json` untouched. `lib.rs` pre-wired by another lane (no E0432), left alone.

Method per ID: keep signatures, inject one wrong-value probe (admit always SlotsExhausted / skip UnsafeBranch check / always Fresh / fallback Stable / Ok-empty instead of NoDocs / accept-anything stub / over-cap admit Ok / skip branch check / prefix-Ok replay). Run single failing test serial (`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, timeout 100-115, `free -h` before each), confirm rc=101 + assertion-fail (not compile error — one OPS-004 probe variant broke compile, discarded, replaced with compiling wrong-value probe). Restore byte-identical from `/tmp/opencode/redval-bak/*.orig` (sha256 match), run full 5/5 GREEN rc=0.

| id | impl | RED probe | RED log | RED rc / fail | GREEN log | GREEN |
|---|---|---|---|---|---|---|
| OPS-001 | resource_ledger.rs b72c9017 | admit always SlotsExhausted | /tmp/opencode/wE-ops001-red.log | 101, t04 `unwrap()` on Err(SlotsExhausted) line 118 | /tmp/opencode/wE-ops001-green.log | 5/5 rc=0 |
| OPS-002 | repo_ref.rs 63c45dea | skip branch validation, always main | /tmp/opencode/wE-ops002-red.log | 101, t03 UnsafeBranch assert tests/repo_ref.rs:89 | /tmp/opencode/wE-ops002-green.log | 5/5 rc=0 |
| OPS-003 | repo_cache_store.rs 5c017f1c | read_state always Fresh | /tmp/opencode/wE-ops003-red.log | 101, t01 left Fresh right Stale tests/repo_cache_store.rs:49 | /tmp/opencode/wE-ops003-green.log | 5/5 rc=0 |
| OPS-004 | install_meta.rs 1548fa81 | UnknownChannel falls back Stable | /tmp/opencode/wE-ops004-red.log | 101, t04 left Ok(Stable) right Err(UnknownChannel) tests/install_meta.rs:72 | /tmp/opencode/wE-ops004-green.log | 5/5 rc=0 |
| OPS-005 | config_overlay.rs bb3579ae | empty docs Ok-empty not NoDocs | /tmp/opencode/wE-ops005-red.log | 101, t04 NoDocs assert tests/config_overlay.rs:87 | /tmp/opencode/wE-ops005-green.log | 5/5 rc=0 |
| OPS-006 | repo_ref_ext.rs 84d236e3 | accept-anything stub | /tmp/opencode/wE-ops006-red.log | 101, t04 left Ok-stub right Err(Invalid) tests/repo_ref_ext.rs:62 | /tmp/opencode/wE-ops006-green.log | 5/5 rc=0 |
| OPS-007 | ops_budget.rs 56d2829b4 | over-cap admit Ok | /tmp/opencode/wE-ops007-red.log | 101, t03 left Ok(Admission) right Err(OverCap) tests/ops_budget.rs:42 | /tmp/opencode/wE-ops007-green.log | 5/5 rc=0 |
| OPS-008 | ops_repo_ref.rs ae3746e4 | skip branch check | /tmp/opencode/wE-ops008-red.log | 101, t04 UnsafeBranch assert tests/ops_repo_ref.rs:74 | /tmp/opencode/wE-ops008-green.log | 5/5 rc=0 |
| OPS-009 | ops_replay.rs c775514f | prefix-Ok, skip matching | /tmp/opencode/wE-ops009-red.log | 101, t02 Mismatch assert tests/ops_replay.rs:35 | /tmp/opencode/wE-ops009-green.log | 5/5 rc=0 |

Restore proof: all 9 `cmp` identical vs `.orig`, `grep RED-PROBE` on all 9 impls empty. Working-tree `M` on owned impl files predates this lane (other-lane content diffs, e.g. lib.rs reorder); this lane made zero net content change. No test file, task card, or ralph.json touched.
