# RED-VALIDITY-WEB712 — WEB-007/008/009/010/011/012 temp-stub-restore re-validation

Rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. Workdir /home/rashid/projects/opencode-rk.
Scope: WEB-007/008/009/010/011/012 only. Own files: `crates/server/src/transcript_lane.rs`,
`crates/sessions/src/web_008_lane.rs`, `crates/server/src/turn_parts.rs`,
`crates/server/src/chat_composer.rs`, `crates/server/src/web_attachments.rs`,
`crates/server/src/web_tool_chooser.rs`.
READ-ONLY `crates/server/src/lib.rs`, `crates/sessions/src/lib.rs` (this lane made no edit to either;
observed working-tree diffs there pre-date this lane: stash@{0} WIP on 248f519, 204 files dirty
from sibling lanes). NEVER touched: frozen
`crates/server/tests/{transcript_lane,turn_parts,chat_composer,web_attachments,web_tool_chooser}.rs`,
`crates/sessions/tests/web_008_lane.rs`, `ralph.json`, mic.

Method per id (serial, JOBS=1 THREADS=1, `--test-threads=1`, `free -h` before each run):
backup own file to `/tmp/opencode/bak-zC-WEB-<id>.rs` -> temp behavior-stub (entry point
returns Err/false first line, signatures intact, `// TEMP-RED-STUB WEB-<id>` marker added after
return so body still parses, no write-path enablement: stubs only disable read/geometry/select,
never send/tool/select-into-turn) -> compile frozen suite standalone via
`rustc --edition 2021 --test <frozen-test> -L dependency=target/debug/deps [--extern ...]`
(workspace `cargo test` blocked by pre-existing sibling-lane breakage:
`crates/providers/src/local_credential_import.rs:184,188` references `bytes` after unreachable
`return Err(BadSchema)` + `// __STUB018__`, and `crates/sessions/src/share_merge.rs:200` has
`if not map...` non-Rust syntax; both outside this lane, untouched) -> run stub binary expecting
compile+fail -> capture RED log `/tmp/opencode/zC-WEB-<id>-red.log` -> `cp` backup restore ->
`cmp` identical + sha256 pre==post -> rebuild + GREEN rerun 5/5 log `/tmp/opencode/zC-WEB-<id>-green.log`.
`free -h` at start: 6.2G total, ~2.8G avail; ~2.5-2.8G avail held through all 6 ids (swap 23G).

Claim: all 6 suites bite on entry-point stubs and return to 5/5 GREEN after byte-identical
restore. No product delta from this lane (`cmp` exit 0 all 6; `TEMP-RED-STUB` grep 0 on all 6).

## Impl hashes (pre == post, restore verified via cmp)

- `crates/server/src/transcript_lane.rs` `97fe74595c4b778dbee35fc6e62898563f3fc6ae94a41929d3b4c076a8c38ec4`
- `crates/sessions/src/web_008_lane.rs` `fb8c7eba487e6ef3edf872f6baff8277379a05518f20401acaa51ca1ee898392`
- `crates/server/src/turn_parts.rs` `8af07d7114be1b94bc1ab455c8605bf6cd42009a36d80676aa9353128465080c`
- `crates/server/src/chat_composer.rs` `b597e6b093310630472e6c9babf90797c7eae3813d73cc8fe177b323cad8101b`
- `crates/server/src/web_attachments.rs` `45049cbd3b44630660145e960a575cbf06ceda61474f0baf868f2060825b49b9`
- `crates/server/src/web_tool_chooser.rs` `1d53a2c16e208597e461b0fa73a9ad3e25dcfbcc00cd52bebb65d1dd287007ac`

## Frozen test hashes (untouched; working-tree fmt-only diffs vs HEAD pre-date this lane)

- `crates/server/tests/transcript_lane.rs` `97945aa4c2c3cc151a98903721814b0dae788e01307a98b0f77890c571e0b812`
- `crates/sessions/tests/web_008_lane.rs` `7ad2eee5fe47fe16cb278ece9e1fc7e44e7a9d584ea9747c170a018151792685`
- `crates/server/tests/turn_parts.rs` `4ff2c79cbd39cbf2ab0e913285bb6990d1d64dd1b1d330db6557e1fa9bc8b0dc`
- `crates/server/tests/chat_composer.rs` `b4b33de0a48264729217e330912d7d5f229591b22cb17758a9495c11987d334e`
- `crates/server/tests/web_attachments.rs` `0c2b98229581fd8050646e31880101dba6d6ae00de37bcf35249e0f42ea1089b`
- `crates/server/tests/web_tool_chooser.rs` `f879cb77bdf61a7bbd5304d58d4ab1a104657ca9c55381c94145b2c2f26d0ae`
- Observed `git diff HEAD --stat` on the 6 frozen tests is fmt/reflow only (import order,
  chain splits); committed at/after 3db7402 by sibling lanes, not this lane. No test file
  edited during this lane (no mtime/hash change made by this lane; stubs touched src only).

## Rows (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| WEB-007 | `geometry_for` returns `full_width:false, align_right:false, actions_below:false` (`transcript_lane.rs:37`) | `/tmp/opencode/zC-WEB-007-red.log`: 4 pass / 1 fail; T01 `transcript_lane.rs:16` (`assertion failed: user.full_width`); T02/T03/T04/T05 green (capability/a11y/bounds/compat paths untouched) | `/tmp/opencode/zC-WEB-007-green.log`: 5 passed, 0 failed, EXIT=0 | `cmp` identical, sha pre==post |
| WEB-008 | `fork_from_message` returns `Err(SessionNotFound)` first (`web_008_lane.rs:140`) | `/tmp/opencode/zC-WEB-008-red.log`: 2 pass / 3 fail; T01 FAILED (no child), T04 FAILED (depth chain), T05 FAILED (no branch to snapshot); T02 (retry) + T03 (a11y/nav) green — valid negative-path survival | `/tmp/opencode/zC-WEB-008-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| WEB-009 | `TurnParts::push` returns `Err(TooManyEvents)` first (`turn_parts.rs:192`) | `/tmp/opencode/zC-WEB-009-red.log`: 0 pass / 5 fail; T01 `.expect("reasoning delta")` on `TooManyEvents`, T02 (malformed-vs-answer asserts on push-Ok shape), T03, T04, T05 all FAILED | `/tmp/opencode/zC-WEB-009-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| WEB-010 | `lower_composer_doc` returns `Err(EmptyDocument)` first (`chat_composer.rs:271`) | `/tmp/opencode/zC-WEB-010-red.log`: 1 pass / 4 fail; T01 FAILED (supported doc no longer lowers), T02 FAILED (sanitized doc lowers to EmptyDocument), T04 FAILED (bounds masked by early Empty), T05 FAILED (reload lower fails); T03 green (pure key/label paths, no lower call — valid survival) | `/tmp/opencode/zC-WEB-010-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| WEB-011 | `AttachmentStore::ingest` returns `Err(Empty)` first (`web_attachments.rs:259`) | `/tmp/opencode/zC-WEB-011-red.log`: 0 pass / 5 fail; T01 `:19` (`.expect("allowed png ingests")` on Empty), T02 `:43`, T03 `:91`, T04 `:104`, T05 `:137` | `/tmp/opencode/zC-WEB-011-green.log`: 5 passed, EXIT=0 | `cmp` identical |
| WEB-012 | `Selection::select` returns `Err(Unknown)` first (`web_tool_chooser.rs:146`) | `/tmp/opencode/zC-WEB-012-red.log`: 1 pass / 4 fail; T01 FAILED (enabled select rejected), T02 FAILED (expects Disabled/Denied variants, got Unknown), T04 FAILED (fill-to-cap), T05 FAILED (empty selection roundtrip); T03 green (search + approval-queue paths touch no select-Ok — valid survival) | `/tmp/opencode/zC-WEB-012-green.log`: 5 passed, EXIT=0 | `cmp` identical |

Notes:
- Partial-fail RED (007 1/5, 008 3/5, 010 4/5, 012 4/5) is valid suite-bite: stub hits the
  happy-path entry point; tests not touching its `Ok` path stay green.
- Write-paths stayed disabled throughout: no stub enables send/tool/select-into-turn; T02
  honesty suites (007 unavailable-action, 010 inactive-capability, 012 denied-never-substituted)
  fail toward denial, never toward fabricated success.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited by this lane.
  `git status` dirt (204 files incl. lib.rs mod lists, frozen-test fmt reflows) is pre-existing
  sibling-lane state under stash@{0} WIP on 248f519 — verified present before first stub.
- Backups: `/tmp/opencode/bak-zC-WEB-{007,008,009,010,011,012}.rs`. Work files
  `/tmp/opencode/{w8,tp,cc,wa,wc}-work.rs` removed-or-harmless scratch (src-identical + marker only).
- Evidence: task cards `tasks/WEB-00{7,8,9,10,11,12}.md`, contracts `docs/TDD.md`,
  `docs/SECURITY.md`, PLAN.md. Workspace-build blocker evidence:
  `/tmp/opencode/zC-WEB-007-green.log` first attempt (providers `bytes` E0425 x2).

## UPGRADE bQ (cargo per-target GREEN, rev 248f519) — 2026-09-16

Prior method (§4h): standalone `rustc --test` RED+GREEN (workspace cargo blocked at
that rev state by sibling breakage). This upgrade: per-target
`cargo test -p <crate> --test <target> -- --test-threads=1`, serial,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 115`, `free -h` before each run.
No product edits (this lane touched no src/test/lib.rs/ralph.json in upgrade run;
pre-existing working-tree diffs vs HEAD are sibling-lane fmt/mod-list state,
observed before run; `TEMP-RED-STUB` grep count 0 on all 6 own files).
Upgrade-mode RED stubbing NOT repeated: prior rustc RED logs stand
(`/tmp/opencode/zC-WEB-{007,008,009,010,011,012}-{red,green}.log`); re-stubbing
now would risk the dirty tree, not prove more.

Prior blocker resolved: `crates/sessions/src/share_merge.rs` now valid Rust
(`if !valid_record(record)`), and `crates/providers/src/local_credential_import.rs`
`bytes` param is a legit `&[u8]` function arg (no `__STUB018__` marker present) —
per-target `cargo test` now builds all 6 suites.

| id | cargo target | GREEN result | log anchor |
|----|--------------|--------------|------------|
| WEB-007 | `-p opencode-rk-server --test transcript_lane` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:195,197` |
| WEB-008 | `-p opencode-rk-sessions --test web_008_lane` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:245,247` |
| WEB-009 | `-p opencode-rk-server --test turn_parts` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:341,343` |
| WEB-010 | `-p opencode-rk-server --test chat_composer` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:449,451` |
| WEB-011 | `-p opencode-rk-server --test web_attachments` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:545,547` |
| WEB-012 | `-p opencode-rk-server --test web_tool_chooser` | 5 passed, 0 failed, EXIT=0 | `/tmp/opencode/bQ-web712.log:656,658` |

Counts: cargo GREEN 30/30 (6 suites x 5). Method: cargo YES (per-target).
Write-paths disabled: confirmed entries intact — `fork_from_message` atomic-checks-first
(web_008_lane.rs:136-140, real Err variants SessionNotFound/MessageNotFound/InvalidBoundary
at :117-156), `TurnParts::push` bound-guarded (:192-194 TooManyEvents),
`lower_composer_doc` node-limit-first (:271-277) + EmptyDocument (:291,505),
`AttachmentStore::ingest` Empty-validate (:147) + TooMany caps (:268,376,456),
`Selection::select` Denied/Disabled reject, no substitution (:146-155). No mic/media
paths in own files (grep hit was shell-noise only; `navigator.media|getUserMedia|
MediaRecorder` absent from all 6). Frozen/lib.rs/ralph.json untouched by this lane.
