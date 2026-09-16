# EXT-TWINS-DISPOSITION2: plugin_* vs ext_*_lane re-audit (2026-09-16)

Prior: worklog/EXT-TWINS-DISPOSITION.md. Scope: 8 EXT twin pairs + manifest note.
Lane: READ-ONLY except this file. No src/lib.rs/test edits, no deletions.
No cargo run in this lane (budget rule); evidence cited from frozen logs.

## Wiring (crates/tools/src/lib.rs:35-44)

WIRED side = `plugin_*` (10 mods): plugin_builtins, plugin_deferred,
plugin_discover, plugin_hook_boundary, plugin_lifecycle, plugin_manifest,
plugin_namespace, plugin_scoped_exec, plugin_transform, plugin_ui_boundary.
UNWIRED side = all 8 `ext_*_lane` (zero `mod ext_*_lane` in lib.rs).
lib.rs:14-21 `ext_*` mods (ext_commands/compat/enable/hooks/lifecycle/perms/
rate/secure) are NOT lane twins; untouched by this disposition.

## Test import style

15/16 twin suites self-contained via `#[path = "../src/<own>.rs"]` + private
`mod <own>;` (5 `#[test]` each = 80 total). No suite depends on lib.rs.
EXCEPTION: `tests/plugin_manifest.rs` imports via crate
(`use opencode_rk_tools::plugin_manifest::{...}`); `tests/ext_manifest_lane.rs`
uses `#[path]`. Deleting a twin src breaks only its own `#[path]` test file;
migrate/delete src+test in same commit.
External users outside `crates/tools/src|tests`: 0 hits for all 8 lane names.

## Pair table (`/usr/bin/diff -q` + `/usr/bin/wc -l`, worktree)

| # | plugin_* (KEEP-canonical, wired) | ext_*_lane (twin, unwired) | lines | diff -q | class |
|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | 166 / 170 | differ | FMT-ONLY + 1 doc phrase (guard PORTED, see below) |
| 2 | plugin_namespace | ext_namespacing_lane | 124 / 124 | identical (exit 0) | BYTE-IDENTICAL (sha256 5eaa922a both) |
| 3 | plugin_discover | ext_discovery_lane | 160 / 128 | differ | COMMENT-ONLY (code same; delta = doc lines stripped only) |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | 191 / 191 | differ | COMMENT-ONLY (1 doc word: "boundary" vs "boundary lane", line 1) |
| 5 | plugin_builtins | ext_builtins_lane | 267 / 268 | differ | COMMENT-ONLY (mod-name word + `#![forbid(unsafe_code)]` twin-only line 11) |
| 6 | plugin_deferred | ext_deferred_lane | 192 / 193 | differ | COMMENT-ONLY (`#![forbid(unsafe_code)]` twin-only) |
| 7 | plugin_lifecycle | ext_lifecycle_lane | 183 / 188 | differ | COMMENT-ONLY (5-line lane doc block + `#![forbid]` twin-only) |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | 286 / 286 | identical (exit 0) | BYTE-IDENTICAL (sha256 4ddc1b58 both) |

Pair-1 detail (prior doc said DIVERGENT; now converged): `/usr/bin/diff`
plugin_transform vs ext_replay_lane = 2 hunks only: (a) `register` if-let
brace reflow (fmt-only, same predicate), (b) `project` doc "seq order; later
seq wins" vs "insertion order; later entry wins" (same iteration, same output).
`set_scope_disabled` guard IDENTICAL both sides
(plugin_transform.rs:128-139 = ext_replay_lane.rs:132-143):
`if !self.disabled.contains(&scope) && self.entries.iter().any(|t| t.scope == scope)`
so `disabled` bounded by EXT11_MAX_TRANSFORMS on both. Invariant docs present
both sides. No behavior delta remains.
Pair-3 proof of comment-only: diff hunks all `-` on plugin side / doc lines,
zero code-line changes (validated hunks: header, MAX-len docs, arity docs,
scoped-npm comment, relative-path comment, boundary docs, method docs).

## Disposition (KEEP-canonical = `plugin_*` in every pair; integrator executes)

| pair | recommendation | rationale | test impact on drop |
|---|---|---|---|
| 1 transform/replay | KEEP `plugin_transform`; DROP twin src+test NOW | Guard port confirmed landed; only fmt + 1 doc phrase remain, zero behavior delta | -5 dup, canonical 5 retained |
| 2 namespace/namespacing | KEEP `plugin_namespace`; DROP twin src+test | Byte-identical | -5 dup, 0 behavior loss |
| 3 discover/discovery | KEEP `plugin_discover`; DROP twin src+test | Code-identical; canonical richer docs | -5 dup |
| 4 ui_boundary | KEEP `plugin_ui_boundary`; DROP twin src+test | 1-word doc delta | -5 dup |
| 5 builtins | KEEP `plugin_builtins`; DROP twin src+test | Comment-only; port twin `#![forbid(unsafe_code)]` (1 line; lib.rs:2 already forbids, so optional) at drop | -5 dup |
| 6 deferred | KEEP `plugin_deferred`; DROP twin src+test | Comment-only; same forbid note | -5 dup |
| 7 lifecycle | KEEP `plugin_lifecycle`; DROP twin src+test | Comment-only; lane doc block stays dropped | -5 dup |
| 8 scoped_exec | KEEP `plugin_scoped_exec`; DROP twin src+test | Byte-identical | -5 dup |

Totals: 8/8 twins droppable = -40 dup tests; retained 40 canonical (8x5) + 0 twins.
Do NOT delete in this lane; integrator owns drops + test-file migration.

## Guard-port pair-1 droppable: YES

Evidence: (1) worktree `set_scope_disabled` bodies byte-equal logic both files
(see hunks above, lines cited); (2) post-port GREEN `plugin_transform` 5/5 in
/tmp/opencode/uM-ext.log:98-107 (`Running tests/plugin_transform.rs ...
test result: ok. 5 passed`, EXIT=0, 7-suite 35/35 with ext_replay_lane 5/5 at
lines 43-52); (3) full-tools serial 344 passed / 60 suites per
worklog/EXT-VERIFY-3.md:43 (log /tmp/opencode/w3-toolsfull.log).
No re-port needed; drop twin directly.

## Twin-note: ext_manifest_lane vs plugin_manifest (NOT duplicates, both KEEP)

`/usr/bin/diff -q` differ; `wc -l` 55 vs 142. Divergent contracts:
`plugin_manifest` = name/version/permissions shape (`PluginManifest`,
`MAX_PERMISSIONS=32`, wired, crate-import test); `ext_manifest_lane` = EXT-005
name/contract_version/capabilities over raw JSON bytes (`Manifest`,
`SUPPORTED_CONTRACT_VERSION=1`, `MAX_MANIFEST_BYTES=16384`, `#[path]` test).
Keep both; no disposition change. (Same for ext_hooks vs plugin_hook_boundary
per EXT-DEDUP.md:47-49.)

## Budget

No cargo executed this lane (read-only). Commands prefixed `rtk`, bounded;
no 120s timeout hit. Host mem reference /tmp/opencode/aF-guard.log
(1.7Gi avail at guard check); prior GREEN logs cited above, not re-run.
Worktree `git status` shows concurrent-lane dirty files under crates/tools
(fmt-only hunks); left untouched.
