# RED-VALIDITY-OPS

## Verdict: RED valid, GREEN 5/5 all 9 — with caveat: tree dirty, frozen tests pre-modified by another lane

Method: serial temp-stub-restore per owned impl file only. Each `src/<id>.rs`
replaced with `pub struct __RedStub__;` stub (missing symbols), `cargo test -p
opencode-rk-foundation --test <id>` run (rc=101 compile-RED, unresolved-import /
E0432-E0433), then restored byte-identical (sha256 match). Frozen tests,
`lib.rs`, `ralph.json` never touched by this lane. Serial, `CARGO_BUILD_JOBS=2
RUST_TEST_THREADS=2`, `timeout 120`. Memory ~2.8-2.9 GiB avail / 6.2 GiB at check.

Caveat: working tree was already dirty on entry (HEAD `248f519`). Owned `src`
files AND frozen `tests` files both carry uncommitted hunks from another lane
(e.g. `resource_ledger.rs`, `repo_ref.rs`, `repo_cache_store.rs`,
`repo_ref_ext.rs`, `ops_repo_ref.rs`, `install_meta.rs` src; `resource_ledger.rs`,
`repo_ref.rs`, `repo_cache_store.rs`, `repo_ref_ext.rs`, `ops_budget.rs`,
`ops_replay.rs`, `ops_repo_ref.rs` tests). So RED proves test→impl wiring on
current content, NOT pristine-commit validity. `git diff --stat HEAD --
crates/foundation/tests/ ralph.json tasks/` shows 13 test files modified; this
lane modified none of them (verify: lane wrote only stubs+restores, hashes match
pre-stub state).

## Rows (impl file → task → RED log → GREEN)

| ID | Impl file | Task tests | RED (stub, rc=101) | GREEN (restored) | Log |
|----|-----------|------------|--------------------|------------------|-----|
| OPS-001 | resource_ledger.rs | resource_ledger 5 | E0432 unresolved imports (11 symbols) | 5/5 ok | /tmp/opencode/rE-resource_ledger-red.log |
| OPS-002 | repo_ref.rs | repo_ref 5 (ops002_t01..t05) | E0432 normalize_ref/RefError/MAX_REF_BYTES | 5/5 ok | /tmp/opencode/rE-repo_ref-red.log |
| OPS-003 | repo_cache_store.rs | repo_cache_store 5 (ops003_t01..t05) | E0433 SlotState ×N | 5/5 ok | /tmp/opencode/rE-repo_cache_store-red.log |
| OPS-004 | install_meta.rs | install_meta 5 (ops004_t01..t05) | E0432 derive_paths/parse_version/BaseDirs/Channel/VersionError | 5/5 ok | /tmp/opencode/rE-install_meta-red.log |
| OPS-005 | config_overlay.rs | config_overlay 5 (ops005_t01..t05) | E0432 merge_overlay/ConfigDoc/OverlayError/Scope | 5/5 ok | /tmp/opencode/rE-config_overlay-red.log |
| OPS-006 | repo_ref_ext.rs | repo_ref_ext 5 (ops006_t01..t05) | E0432 failure_hint/normalize_reference/safe_basename/RefError/RelPath | 5/5 ok | /tmp/opencode/rE-repo_ref_ext-red.log |
| OPS-007 | ops_budget.rs | ops_budget 5 | E0432 Admission/BudgetError/OpsBudget | 5/5 ok | /tmp/opencode/rE-ops_budget-red.log |
| OPS-008 | ops_repo_ref.rs | ops_repo_ref 5 | E0432 cache_identity/cache_path/normalize_ref/normalize_ref_with_base/RepoError | 5/5 ok | /tmp/opencode/rE-ops_repo_ref-red.log |
| OPS-009 | ops_replay.rs | ops_replay 5 | E0432 replay/Cassette/ReplayError/MAX_FRAMES/MAX_FRAME_BYTES | 5/5 ok | /tmp/opencode/rE-ops_replay-red.log |

Restore check: all 9 sha256 identical pre/post (resource_ledger `b72c90179ba9`,
repo_ref `63c45deaec32`, repo_cache_store `5c017f1cc5be`, install_meta
`1548fa81dc66`, config_overlay `bb3579aeb89a`, repo_ref_ext `84d236e33160`,
ops_budget `56d2829b443f`, ops_repo_ref `ae3746e4896d`, ops_replay `c775514f8e9a`).

GREEN cmds (serial): `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo
test -p opencode-rk-foundation --test <each>` → `running 5 tests / ok. 5 passed`
each. `lib.rs` untouched by lane (dirty from other lane, not mine).

## Boundary kept

Owned ONLY the 9 impl files. Did NOT edit `lib.rs`, `tests/*`, `ralph.json`,
`tasks/*`. Backups: `/tmp/opencode/OPS-*.bak` (pre-stub) + `OPS-<id>.fullbak`.
