# RED-VALIDITY-OPS2 — BEHAVIOR RED (compile + assertion-fail), 9 OPS impl files

## Verdict: BEHAVIOR RED valid all 9, GREEN 5/5 all 9 after byte-identical restore

Prior RED (`RED-VALIDITY-OPS.md`) was E0432/E0433 module-hidden stub =
compile-fail, weak per `docs/TDD.md` §3 ("compile failures ... are not valid
evidence"). This lane redoes RED as BEHAVIOR RED: signatures kept, one wrong
behavior forced per file, suite compiles (`test result: FAILED`, zero
`E0432/E0433`/`could not compile`), fails on assertions (`panicked at`
assertion/`unwrap` on wrong `Err`).

Method: per file serial — `sha256` pre → backup `/tmp/opencode/uB-<id>.bak` →
python one-line wrong-behavior insert (signatures unchanged) → run
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p
opencode-rk-foundation --test <id>` → capture `/tmp/opencode/uB-<id>-red.log`
(rc=101) → restore backup → `sha256` post == pre → GREEN 5/5 rerun. Each run
preceded by `rtk free -h` (~2.6–3.1 GiB avail / 6.2 GiB, serial only).
Frozen `tests/*`, `src/lib.rs`, `ralph.json`, `tasks/*` never written by this
lane (only read + `cargo test` execution). Ownership: 9 impl files only.

Caveat (same as prior lane): tree dirty on entry from another lane (src +
frozen tests both carry uncommitted hunks). RED proves test→impl wiring on
current content, not pristine-commit validity.

## Rows (impl file → forced behavior → RED → GREEN → log)

| ID | Impl file | Forced wrong behavior (sigs kept) | RED (compile, rc=101) | GREEN restored |
|----|-----------|-----------------------------------|----------------------|----------------|
| OPS-001 | resource_ledger.rs | `admit` always `Err(SlotsExhausted)` (1-line early return) | 3 pass / **2 fail** (t03 admit_release, t04 double_bound) | 5/5 ok |
| OPS-002 | repo_ref.rs | `normalize_ref` always `Err(UnsupportedForm)` | 0 pass / **5 fail** (t01–t05) | 5/5 ok |
| OPS-003 | repo_cache_store.rs | `read_state` always `Missing` | 2 pass / **3 fail** (t01 inspect, t02 sweep, t04 corrupt-guard) | 5/5 ok |
| OPS-004 | install_meta.rs | `parse_version` always `Err(Invalid)` | 0 pass / **5 fail** | 5/5 ok |
| OPS-005 | config_overlay.rs | `merge_overlay` always `Err(NoDocs)` | 0 pass / **5 fail** | 5/5 ok |
| OPS-006 | repo_ref_ext.rs | `normalize_reference` always `Err(Invalid)` | 0 pass / **5 fail** | 5/5 ok |
| OPS-007 | ops_budget.rs | `admit` always `Err(OverCap{0,0})` | 0 pass / **5 fail** | 5/5 ok |
| OPS-008 | ops_repo_ref.rs | `normalize_ref` always `Err(Malformed)` | 0 pass / **5 fail** | 5/5 ok |
| OPS-009 | ops_replay.rs | `replay` always `Err(Exhausted{0})` | 0 pass / **5 fail** | 5/5 ok |

Logs: `/tmp/opencode/uB-resource_ledger-red.log`, `/tmp/opencode/uB-repo_ref-red.log`,
`/tmp/opencode/uB-repo_cache_store-red.log`, `/tmp/opencode/uB-install_meta-red.log`,
`/tmp/opencode/uB-config_overlay-red.log`, `/tmp/opencode/uB-repo_ref_ext-red.log`,
`/tmp/opencode/uB-ops_budget-red.log`, `/tmp/opencode/uB-ops_repo_ref-red.log`,
`/tmp/opencode/uB-ops_replay-red.log`. Zero `E0432/E0433`/`could not compile`
in all 9 (verified by grep); every RED line is `test result: FAILED` +
`panicked at crates/foundation/tests/...`.

Restore check: `diff /tmp/opencode/uB-pre.sha /tmp/opencode/uB-post.sha` →
identical, `HASH_MATCH_ALL9`:
resource_ledger `b72c90179ba9`, repo_ref `63c45deaec32`,
repo_cache_store `5c017f1cc5be`, install_meta `1548fa81dc66`,
config_overlay `bb3579aeb89a`, repo_ref_ext `84d236e33160`,
ops_budget `56d2829b443f`, ops_repo_ref `ae3746e4896d`, ops_replay `c775514f8e9a`.

Partial-fail notes (expected, still valid BEHAVIOR RED): OPS-001 t01/t02
(attestation) + t05 (spill/caps) don't call `admit` → pass; OPS-003 t03
(caps/cancel: cancel checked before sweep) + t05 (transport-refused,
zero-transport-use) don't depend on `read_state` freshness → pass. Core
admission/lifecycle paths fail on assertions in both cases.

## Boundary kept

Wrote only: 9 `src` files (stub+restore, hashes match pre-state) + this
worklog. Never wrote `tests/*`, `src/lib.rs`, `ralph.json`, `tasks/*`.
Backups: `/tmp/opencode/uB-<id>.bak` + `/tmp/opencode/uB-{pre,post}.sha`.
