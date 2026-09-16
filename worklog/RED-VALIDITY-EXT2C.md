# RED-VALIDITY-EXT2C — EXT-009/010/011/012 temp-stub-restore re-validation

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. Workdir /home/rashid/projects/opencode-rk.
Scope: EXT-009/010/011/012 only. Own files: `crates/tools/src/plugin_namespace.rs`,
`plugin_discover.rs`, `plugin_transform.rs`, `plugin_ui_boundary.rs`.
READ-ONLY `crates/tools/src/lib.rs` (untouched; pre-existing 1-line mod-order fmt diff left as-is).
Twins (`ext_*_lane`) untouched by this lane. NEVER touched: frozen
`crates/tools/tests/plugin_{namespace,discover,transform,ui_boundary}.rs`, `ralph.json`.

Method per task (serial, JOBS=1 THREADS=1, `timeout 120`, `--test-threads=1`,
`free -h` before each stub run): backup own file to `/tmp/opencode/bak-xD-<id>.rs` ->
temp behavior-stub (entry-point returns `Err(Overflow)` first line, signatures intact,
no test/lib.rs/ralph.json touch) -> run frozen suite expecting compile+fail ->
capture RED log `/tmp/opencode/xD-<id>-red.log` -> restore via `cp` backup ->
`cmp` identical + sha256 pre==post -> GREEN rerun 5/5 log `/tmp/opencode/xD-<id>-green.log`.
`free -h` at start: 6.2G total, ~2.6-2.7G avail (swap 23G).

Claim: all 4 suites bite on entry-point stubs and return to 5/5 GREEN after
byte-identical restore. No product delta from this lane (`cmp` exit 0 all 4;
`TEMP-RED-STUB` grep clean).

## Impl hashes (pre == post, restore verified via cmp)

- `crates/tools/src/plugin_namespace.rs` `5eaa922a867dacf82eda4b7d5599d8e34c7acd29e5d967a6bf150b1587cb9258`
- `crates/tools/src/plugin_discover.rs` `d7d7c5c0cf60590b232fc46376ddf3ee46bc212af83704cf68cca416f42825fd`
- `crates/tools/src/plugin_transform.rs` `b4c517e36a3d5141bb9561ef0bf526aabd54372e2c43a239fab6caaa1d004d43` (differs from prior lane `0885c157...` only by pre-existing sibling hardening; identical before/after this lane)
- `crates/tools/src/plugin_ui_boundary.rs` `90883e97473bb51bdb8dc71966b5cb991fef77f80119d33362cd6604d60631b9`

## Frozen test hashes (untouched, match prior lanes)

- `crates/tools/tests/plugin_namespace.rs` `1768220718be90f39986309464e9389d4db6cb5a91b50e3fbcff3b254bd75627`
- `crates/tools/tests/plugin_discover.rs` `06c872b301e3b75f7e0f4d65f85815d44301678c7dc511121b0a8dce5dd51c66`
- `crates/tools/tests/plugin_transform.rs` `55fc1d44c99381f14da2998b7add36aa6eb3d02dc5fea9e7dfda0989d9dbf86e`
- `crates/tools/tests/plugin_ui_boundary.rs` `db18b1e37c5f9fe13d58bbd6d22b83e2a404aa0f472cd286f6ee01a40e57d7c1`

## Rows (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| EXT-009 | `Namespace::register` returns `Err(Overflow)` first (`plugin_namespace.rs:75`) | `/tmp/opencode/xD-EXT-009-red.log`: 0 pass / 5 fail; T01 `:49` (`Err(Overflow)` vs `Ok(true)`), T02 `:70`, T03 `:97`, T04 `:141` (`unwrap` on `Overflow`), T05 `:178` same | `/tmp/opencode/xD-EXT-009-green.log`: 5 passed, 0 failed, EXIT=0 | `cmp` identical, sha pre==post |
| EXT-010 | `DiscoveryBoundary::declare` returns `Err(Overflow)` first (`plugin_discover.rs:120`) | `/tmp/opencode/xD-EXT-010-red.log`: 1 pass / 4 fail; T01 `:119` (`Err(Overflow)` vs `Ok(1)`), T02 `:154`, T03 `:176`, T05 `:341` (debug missing ref); T04 ok (no-loading proof touches no declare-Ok path; valid negative-path survival) | `/tmp/opencode/xD-EXT-010-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| EXT-011 | `TransformLog::add` returns `Err(Overflow)` first (`plugin_transform.rs:97`) | `/tmp/opencode/xD-EXT-011-red.log`: 0 pass / 5 fail; T01 `:48` (`Err(Overflow)` vs `Ok(1)`), T02 `:67` (`unwrap`), T03 `:103`, T04 `:219`, T05 `:262` | `/tmp/opencode/xD-EXT-011-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| EXT-012 | `UiBoundary::declare` returns `Err(Overflow)` first (`plugin_ui_boundary.rs:138`) | `/tmp/opencode/xD-EXT-012-red.log`: 0 pass / 5 fail; T01 `:25` (`.expect("decl 1")` on `Overflow`), T02 `:57`, T03 `:83`, T04 `:163`, T05 `:224` | `/tmp/opencode/xD-EXT-012-green.log`: 5 passed, EXIT=0 | `cmp` identical |

Notes:
- Partial-fail RED (EXT-010 4/5, T04 green) is valid suite-bite: stub hits
  happy-path entry `declare`; test not touching its `Ok` path stays green.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited.
  `git status` shows pre-existing fmt-only diffs on owned paths (e.g. namespace
  `bytes[1..]` reflow 3/3, transform guard/comment hardening) and on `lib.rs`
  (1-line mod-order) plus sibling-lane `ext_*_lane` test diffs — all present
  before this lane, none introduced here. Diffs outside the 4 owned src files
  are sibling-lane state, not this lane.
- Backups: `/tmp/opencode/bak-xD-EXT009.rs`, `bak-xD-EXT010.rs`,
  `bak-xD-EXT011.rs`, `bak-xD-EXT012.rs`.
- Evidence: task cards `tasks/EXT-009.md`/`EXT-010.md`/`EXT-011.md`, contracts
  `docs/TDD.md`, `docs/SECURITY.md`, PLAN.md 5-6.
