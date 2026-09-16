# RED-VALIDITY-EXT2B — EXT-009/010/011/012 independent re-validation (lane wC)

Base rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (dirty workdir; sibling-lane mods pre-existing).
Scope: EXT-009/010/011/012 only. Own files: `crates/tools/src/plugin_namespace.rs`,
`plugin_discover.rs`, `plugin_transform.rs`, `plugin_ui_boundary.rs`.
READ-ONLY on `crates/tools/src/lib.rs` (untouched; pre-existing 1-line fmt diff left as-is).
Twins (`ext_*_lane`) untouched. NEVER touched: frozen `crates/tools/tests/plugin_*`, `ralph.json`.

Protocol per task (serial, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`,
`--test-threads=1`): backup own file to `/tmp/opencode/bak-wC-<id>.rs` ->
temp behavior-stub (entry-point returns `Err(Overflow)` first line, signatures intact,
no test/lib.rs/ralph.json touch) -> run frozen suite expecting compile+fail ->
capture RED log `/tmp/opencode/wC-<id>-red.log` -> restore via `cp` backup ->
`cmp` identical + sha256 pre==post -> GREEN rerun 5/5 log `/tmp/opencode/wC-<id>-green*.log`.
`free -h` at start: 6.2G total, ~3.0G avail (swap 23G, heavy use; serial low-job runs only).

Claim: all 4 suites bite on entry-point stubs and return to 5/5 GREEN after
byte-identical restore. No product delta from this lane (all `cmp` exit 0).

## Impl hashes (pre == post, restore verified via cmp)

- `crates/tools/src/plugin_namespace.rs` `5eaa922a867dacf82eda4b7d5599d8e34c7acd29e5d967a6bf150b1587cb9258`
- `crates/tools/src/plugin_discover.rs` `d7d7c5c0cf60590b232fc46376ddf3ee46bc212af83704cf68cca416f42825fd`
- `crates/tools/src/plugin_transform.rs` (`b4c517e3...` current workdir; differs from prior lane's `0885c157...` only by pre-existing comment+guard hardening, identical before/after this lane)
- `crates/tools/src/plugin_ui_boundary.rs` `90883e97473bb51bdb8dc71966b5cb991fef77f80119d33362cd6604d60631b9`

## Frozen test hashes (untouched, match prior lane)

- `crates/tools/tests/plugin_namespace.rs` `1768220718be90f39986309464e9389d4db6cb5a91b50e3fbcff3b254bd75627`
- `crates/tools/tests/plugin_discover.rs` `06c872b301e3b75f7e0f4d65f85815d44301678c7dc511121b0a8dce5dd51c66`
- `crates/tools/tests/plugin_transform.rs` `55fc1d44c99381f14da2998b7add36aa6eb3d02dc5fea9e7dfda0989d9dbf86e`
- `crates/tools/tests/plugin_ui_boundary.rs` `db18b1e37c5f9fe13d58bbd6d22b83e2a404aa0f472cd286f6ee01a40e57d7c1`

## Per-task receipts (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| EXT-009 | `Namespace::register` returns `Err(Overflow)` first | `/tmp/opencode/wC-EXT-009-red.log`: 0 pass / 5 fail; T01 `:49` (`Err(Overflow)` vs `Ok(true)`), T02 `:70`, T03 `:97`, T04 `:141`, T05 `:178` | `/tmp/opencode/wC-EXT-009-green2.log`: 5 passed, 0 failed | cmp exit 0, sha pre==post |
| EXT-010 | `DiscoveryBoundary::declare` returns `Err(Overflow)` first | `/tmp/opencode/wC-EXT-010-red.log`: 1 pass / 4 fail; T01 `:119`, T02 `:154`, T03 `:176`, T05 `:341`; T04 ok (no-loading proof touches no declare-Ok path; valid negative-path survival) | `/tmp/opencode/wC-EXT-010-green.log`: 5 passed | cmp exit 0 |
| EXT-011 | `TransformLog::add` returns `Err(Overflow)` first | `/tmp/opencode/wC-EXT-011-red.log`: 0 pass / 5 fail; T01 `:48`, T02 `:67`, T03 `:103`, T04 `:219`, T05 `:262` | `/tmp/opencode/wC-EXT-011-green.log`: 5 passed | cmp exit 0 |
| EXT-012 | `UiBoundary::declare` returns `Err(Overflow)` first | `/tmp/opencode/wC-EXT-012-red.log`: 0 pass / 5 fail; T01 `:25`, T02 `:57`, T03 `:83`, T04 `:163`, T05 `:224` | `/tmp/opencode/wC-EXT-012-green.log`: 5 passed | cmp exit 0 |

Notes:
- Partial-fail RED (EXT-010 4/5, T04 green) is valid suite-bite: stub hits
  happy-path entry `declare`; test not touching its `Ok` path stays green.
- Stubs removed after each RED (`grep TEMP-RED-STUB` clean: `STUBS-GONE` confirmed
  after final restore; per-step `cmp` exit 0 recorded).
- First GREEN attempt for EXT-009 (`wC-EXT-009-green.log`, EXIT=101) failed to
  compile on sibling-lane `crates/tools/src/mcp_lifecycle.rs` (`entry` scope
  error, pre-existing dirty-workdir breakage outside owned files, lib.rs
  READ-ONLY so not fixed here). Retried per-target serially; all 4 owned
  suites then GREEN 5/5 with `--test-threads=1`. No lib.rs/Cargo.toml/schema/ralph.json edit.
- Backups: `/tmp/opencode/bak-wC-EXT009.rs`, `bak-wC-EXT010.rs`, `bak-wC-EXT011.rs`, `bak-wC-EXT012.rs`.
- Evidence: task cards `tasks/EXT-009.md`/`EXT-010.md`/`EXT-011.md`/`EXT-012.md`,
  contracts `docs/TDD.md`, `docs/SECURITY.md`, PLAN.md 5-6.
