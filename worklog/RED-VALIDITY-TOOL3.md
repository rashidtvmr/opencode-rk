# RED-VALIDITY-TOOL3 — TOOL-016..020 + SYNC-001/002 + UI-019

Base rev: `248f519`. Method: per file, temp behavior-stub (signatures intact,
no test/lib.rs/ralph.json touch), frozen suite expecting fail, restore
byte-identical (`cmp` vs `/tmp/opencode/xJ-bak-*` backup), re-run GREEN.
Bounds: serial, one heavy cmd at a time, `CARGO_BUILD_JOBS=1`,
`--test-threads=1`, `timeout 120` per run. `free -h` at start: 6.2G total,
~2.6G avail. NOTE: prior TOOL2 lane did not pin `CARGO_BUILD_JOBS`; this lane
pins JOBS=1 per task order.

Claim: all 8 suites bite on behavior-stubbed impls and return to 5/5 GREEN
after byte-identical restore. No product delta remains from this lane.

## Impl hashes (pre == post, `cmp` restored OK)

- `crates/tools/src/mcp_catalog_search.rs` `8c2142c3349307cb8be12face57713a081215497e6450b7f7d11cb4556d960fd`
- `crates/tools/src/mcp_bulk_actions.rs` `53dcae475c08a3e515db6312a05fefc66ed8ef0493cdaf06fffeef5d0c782419`
- `crates/tools/src/mcp_lifecycle.rs` `5e63c909997155a7afb4cd9288127b3eb179a8e4b8b8e18a20f4bd9f3938ece4`
- `crates/tools/src/mcp_payload_filter.rs` `565d28f011c3cb7b62465d1d8044d72994fe17a88a97cd60ad96f60e779d0aa8`
- `crates/sessions/src/mcp_status_panel.rs` `61e86d8d85fb1caaf63ad00aa2c85ea0cbe25f9d79b1dc7df116f163bdac295c`
- `crates/sessions/src/tui_info_panel.rs` `606de856f564456fb9a804fd9a1819177167717f978f76ed55a0def31fedb45e`
- `crates/sessions/src/part_events.rs` `8ab97dea64e32e4fdf50c88704fe9eae3fbd0f9cb71d22d8c3edd4ee1cf633a6`
- `crates/server/src/sync_log.rs` `021a9936bf436ea46b8754cdcfedf550c78a54b8268f026ba0c5316c66bb93bd`

## Frozen test hashes (read-only, never edited)

- `crates/tools/tests/mcp_catalog_search.rs` `4de7cdde25cf4a6091cc92c7c5b44671234000831c5ea9107cc3a8719c103bab`
- `crates/tools/tests/mcp_bulk_actions.rs` `5a3c0250cb3a34b52c4be1d7de515da0cfa05e8dc93c966786391d90a278b0c5`
- `crates/tools/tests/mcp_lifecycle.rs` `2fbaca626ae6a38419ec101573d4a8ff9da9f1d210f0c0f1692ae95b0c71b257`
- `crates/tools/tests/mcp_payload_filter.rs` `f9f02e976234260147d7a3f49700909152dd1bf21b63151874cd36ff9756c2fd`
- `crates/sessions/tests/mcp_status_panel.rs` `0f38b20ac9719ef50480b600c0b2e38dde13582b30c92b2d99f80cd423ca43e8`
- `crates/sessions/tests/tui_info_panel.rs` `d202bb25c675889a1bb2b56827db6234aaf5675237ce7aef9f6f431f1f868eda`
- `crates/sessions/tests/part_events.rs` `1a585f56280c251f24fef2df8af6576fe4fc8a17b00ba46d3b6f8767d481e99c`
- `crates/server/tests/sync_log.rs` `a38c2e7256660b1819f22c972255dee0cf1a0a5e18c4d8871e2388f129f72a37`

## Rows (stub -> RED fails -> restore -> GREEN 5/5)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| TOOL-016 | `search` early-returns empty hits, `truncated:false` (orphan tail left, unreachable warning only) | `/tmp/opencode/xJ-TOOL-016-red.log`: 1 pass / 4 fail; T01 `mcp_catalog_search.rs:77`, T02 `:96`, T03 `:126`, T04 `:158`; T05 passes (load_index/determinism-over-empty path) | `/tmp/opencode/xJ-TOOL-016-green.log`: 5 passed | cmp identical, sha pre==post |
| TOOL-017 | `apply` adds `ids.clear()` after sort | `/tmp/opencode/xJ-TOOL-017-red.log`: 1 pass / 4 fail; T01 `:18`, T03 `:63`, T04 `:74`, T05 `:108`; T02 passes (selection-only, no apply) | `/tmp/opencode/xJ-TOOL-017-green.log`: 5 passed | cmp identical |
| TOOL-018 | `mark_ready` early-returns `Err(NotEnabled)` after lookup | `/tmp/opencode/xJ-TOOL-018-red.log`: 4 pass / 1 fail; T01 `:20` (ready gate) | `/tmp/opencode/xJ-TOOL-018-green.log`: 5 passed | cmp identical |
| TOOL-019 | `filter_payload` adds `enabled_servers.clear()` after dedup | `/tmp/opencode/xJ-TOOL-019-red.log`: 1 pass / 4 fail; T01 `:44`, T02 `:56`, T03 `:85`, T05 `:136`; T04 passes (validation-only) | `/tmp/opencode/xJ-TOOL-019-green.log`: 5 passed | cmp identical |
| TOOL-020 | `summarize` forced zeros + `event` forced empty tools/`None` error | `/tmp/opencode/xJ-TOOL-020-red.log`: 3 pass / 2 fail; T01 `:29`, T04 `:63`; T02/T03/T05 pass (narrow/stale, bus, determinism paths) | `/tmp/opencode/xJ-TOOL-020-green.log`: 5 passed | cmp identical |
| UI-019 | `render` early-returns `Ok(vec!["stub"])` after width check | `/tmp/opencode/xJ-UI-019-red.log`: 1 pass / 4 fail; T01 `:49`, T02 `:62`, T03 `:76`, T05 `:101`; T04 passes (EmptyField/TooNarrow gates fire before stub) | `/tmp/opencode/xJ-UI-019-green.log`: 5 passed | cmp identical |
| SYNC-002 | `apply_update` early-returns `Ok(())` without validation/store (orphan tail deleted, restored) | `/tmp/opencode/xJ-SYNC-002-red.log`: 3 pass / 2 fail; T01 `:27`, T04 `:121`; T02/T03/T05 pass (classify/filter/Debug paths) | `/tmp/opencode/xJ-SYNC-002-green.log`: 5 passed | cmp identical |
| SYNC-001 | `append` early-returns `Err(Full)` (orphan tail deleted, restored) | `/tmp/opencode/xJ-SYNC-001-red.log`: 0 pass / 5 fail; T01 `:25`, T02 `:41`, T03 `:62`, T04 `:82`, T05 `:105` — full-fail | `/tmp/opencode/xJ-SYNC-001-green.log`: 5 passed | cmp identical |

Notes:
- Partial-fail REDs are valid suite-bite receipts: stub hits the happy-path
  entry point; tests not touching it (negative-path, validation-only,
  encode/focus gates) stay green.
- Full-fail SYNC-001 confirms `append` is the single load-bearing entry point.
- Single-fail TOOL-018 confirms `mark_ready` gate covered by exactly T01.
- RED deltas vs prior TOOL2 lane (same files): TOOL-016 now 1/4 (was 2/3 —
  stub moved above EmptyQuery gate so T03 also fails); TOOL-020 now 3/2
  (was 3/2 same shape: T01+T04); all others identical. Stronger stubs here,
  same conclusion.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited by
  this lane (`git status --short -- ralph.json` empty; `lib.rs` diffs are
  pre-existing lane state, untouched — this lane only temp-edited its 8 owned
  `src` files and restored each before the next stub).
- Backups: `/tmp/opencode/xJ-bak-<path-with-slashes-as-underscores>`.
- GREEN logs: `/tmp/opencode/xJ-<TOOL-016|TOOL-017|TOOL-018|TOOL-019|TOOL-020|UI-019|SYNC-002|SYNC-001>-green.log`, each `5 passed`.
