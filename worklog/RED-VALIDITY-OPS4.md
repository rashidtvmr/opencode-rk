# RED-VALIDITY OPS-001..009 (rev 248f519)

Lease: OPS-001..009 only. Owned impls: `crates/foundation/src/{resource_ledger,repo_ref,repo_cache_store,install_meta,config_overlay,repo_ref_ext,ops_budget,ops_repo_ref,ops_replay}.rs`. `lib.rs` untouched by this lane (pre-existing other-lane reorder diff 5+/2- left alone; no E0432 seen in this lane, so no additive edit). Frozen tests + `ralph.json` untouched (`git status --short -- ralph.json` empty).

Method per ID: signatures kept, one compiling wrong-value probe injected (marked `RED-PROBE-xF-<ID>`), single failing test run serial (`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `free -h` before runs), rc=101 + assertion-fail confirmed (not compile error), probe removed, file restored byte-identical vs `/tmp/opencode/xF-bak/*.orig` (`cmp` identical), full 5/5 GREEN rc=0 re-run.

Backups: `/tmp/opencode/xF-bak/<name>.rs.orig` (9 files). RED logs: `/tmp/opencode/xF-OPS-<001..009>-red.log` (all rc=101).

| id | impl (sha256[:8] restored) | RED probe | RED log / rc / fail | GREEN 5/5 |
|---|---|---|---|---|
| OPS-001 | resource_ledger.rs b72c9017 | `admit` early `return Err(SlotsExhausted)` | xF-OPS-001-red.log rc=101 `resource_ledger_t03_admit_release_accounting` panicked tests/resource_ledger.rs:71 `unwrap()` on `Err(SlotsExhausted{SessionInput})` | 5/5 rc=0 |
| OPS-002 | repo_ref.rs 63c45dea | skip `validate_branch`, explicit branch forced to `main` | xF-OPS-002-red.log rc=101 `ops002_t03_unsafe_rejection` panicked tests/repo_ref.rs:89 `unwrap_err()` on `Ok(NormalizedRef{branch:main,...})` | 5/5 rc=0 |
| OPS-003 | repo_cache_store.rs 5c017f1c | `read_state` early `return Fresh` | xF-OPS-003-red.log rc=101 `ops003_t01_inspect_mark_happy_path` panicked tests/repo_cache_store.rs:49 `left:Fresh right:Stale{idle}` | 5/5 rc=0 |
| OPS-004 | install_meta.rs 1548fa81 | unknown channel falls back `Ok(Stable)` not `UnknownChannel` | xF-OPS-004-red.log rc=101 `ops004_t04_failure_states` panicked tests/install_meta.rs:72 `left:Ok(9.9.9-beta/Stable) right:Err(UnknownChannel)` | 5/5 rc=0 |
| OPS-005 | config_overlay.rs bb3579ae | empty docs `Ok(empty)` not `NoDocs` | xF-OPS-005-red.log rc=101 `ops005_t04_failure_states` panicked tests/config_overlay.rs:87 `unwrap_err()` on `Ok(EffectiveConfig{keys:[],winner:[]})` | 5/5 rc=0 |
| OPS-006 | repo_ref_ext.rs 84d236e3 | empty-raw accept-anything stub (`canonical:stub`, 64x`0` id) | xF-OPS-006-red.log rc=101 `ops006_t04_failure_states` panicked tests/repo_ref_ext.rs:62 `left:Ok(stub...) right:Err(Invalid)` | 5/5 rc=0 |
| OPS-007 | ops_budget.rs 56d2829b4 | live-cap check `if false && ...` (over-cap admits Ok) | xF-OPS-007-red.log rc=101 `ops_budget_t03_caps_refuse` panicked tests/ops_budget.rs:42 `left:Ok(Admission{4,0,0}) right:Err(OverCap{5,4})` | 5/5 rc=0 |
| OPS-008 | ops_repo_ref.rs ae3746e4 | `validate_branch` early `return Ok(branch)` (skip check) | xF-OPS-008-red.log rc=101 `ops_repo_ref_t04_failure_states` panicked tests/ops_repo_ref.rs:74 branch `"../x"` admitted | 5/5 rc=0 |
| OPS-009 | ops_replay.rs c775514f | `replay` early `return Ok(requests.len())` (skip matching) | xF-OPS-009-red.log rc=101 `ops009_t02_determinism_and_order` panicked tests/ops_replay.rs:35 `matches! Mismatch{index:1}` failed | 5/5 rc=0 |

Restore proof: all 9 `cmp <impl> /tmp/opencode/xF-bak/<impl>.orig` identical; `grep -rn RED-PROBE` on all 9 impls empty; sha256 match pre-probe values above. Working-tree `M` on owned impls/tests predates this lane (other-lane content); this lane net content change zero. No test file, task card, or ralph.json written.
