# RED-VALIDITY-TOOL2 — TOOL-016..020 + SYNC-001/002 + UI-019

Base rev: `248f519`. Method: per file, temp behavior-stub (signatures intact,
no test/lib.rs/ralph.json touch), frozen suite expecting fail, restore
byte-identical (`cmp` vs `/tmp/opencode/bak2-*` backup), re-run GREEN.
Bounds: serial, one heavy cmd at a time, `--test-threads=1`, `timeout 115`
per run. `free -h` at start: 6.2G total, ~3.0G avail. NOTE: `CARGO_BUILD_JOBS`
not pinned (default parallelism); test threads pinned to 1, runs serial.

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

## Rows (stub -> RED fails -> restore -> GREEN 5/5)

| id | stub (behavior-only, compiles) | RED log: pass/fail + witness | GREEN log | restore |
|----|-------------------------------|------------------------------|-----------|---------|
| TOOL-016 | `search` early-returns empty hits, `truncated:false` | `/tmp/opencode/wI-tool016-red.log`: 2 pass / 3 fail; T01 `mcp_catalog_search.rs:77`, T02 `:96`, T04 `:158`; T03/T05 pass (empty-query + load_index paths need no search) | `/tmp/opencode/wI-tool016-green.log`: 5 passed | cmp identical, sha pre==post |
| TOOL-017 | `apply` adds `ids.clear()` after sort | `/tmp/opencode/wI-tool017-red.log`: 1 pass / 4 fail; T01 `:18`, T03 `:63`, T04 `:74`, T05 `:108`; T02 passes (selection-only, no apply) | `/tmp/opencode/wI-tool017-green.log`: 5 passed | cmp identical |
| TOOL-018 | `mark_ready` early-returns `Err(NotEnabled)` | `/tmp/opencode/wI-tool018-red.log`: 4 pass / 1 fail; T01 `:20` (ready gate); T02-T05 pass (error/reconnect, guards, persist, determinism need no ready) | `/tmp/opencode/wI-tool018-green.log`: 5 passed | cmp identical |
| TOOL-019 | `filter_payload` adds `enabled_servers.clear()` after dedup | `/tmp/opencode/wI-tool019-red.log`: 1 pass / 4 fail; T01 `:44`, T02 `:56`, T03 `:85`, T05 `:136`; T04 passes (validation-only) | `/tmp/opencode/wI-tool019-green.log`: 5 passed | cmp identical |
| TOOL-020 | `summarize`+`event` force empty tools / `None` error | `/tmp/opencode/wI-tool020-red.log`: 3 pass / 2 fail; T01 `:29`, T04 `:63`; T02/T03/T05 pass (narrow/stale, bus, determinism paths) | `/tmp/opencode/wI-tool020-green.log`: 5 passed | cmp identical |
| UI-019 | `render` early-returns `Ok(vec!["stub"])` after width check | `/tmp/opencode/wI-ui019-red.log`: 1 pass / 4 fail; T01 `:49`, T02 `:62`, T03 `:76`, T05 `:101`; T04 passes (EmptyField/TooNarrow gates fire before stub) | `/tmp/opencode/wI-ui019-green.log`: 5 passed | cmp identical |
| SYNC-002 | `apply_update` early-returns `Ok(())` without validation/store | `/tmp/opencode/wI-sync002-red.log`: 3 pass / 2 fail; T01 `:27`, T04 `:121`; T02/T03/T05 pass (classify/filter/Debug paths need no store) | `/tmp/opencode/wI-sync002-green.log`: 5 passed | cmp identical |
| SYNC-001 | `append` early-returns `Err(Full)` | `/tmp/opencode/wI-sync001-red.log`: 0 pass / 5 fail; T01 `:25`, T02 `:41`, T03 `:62`, T04 `:82`, T05 `:105` — full-fail | `/tmp/opencode/wI-sync001-green.log`: 5 passed | cmp identical |

Notes:
- Partial-fail REDs are valid suite-bite receipts: stub hits the happy-path
  entry point; tests not touching it (negative-path, validation-only,
  encode/focus gates) stay green.
- Stub-compile fixups (no behavior change): TOOL-018/SYNC-002/SYNC-001
  early-returns needed dead-code deletion to satisfy `;` before the orphaned
  tail; final stubs compile and fail on behavior, not on build errors
  (RED logs show `FAILED` per-test lines, not `could not compile`).
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited by
  this lane (`git diff --stat -- ralph.json` empty; owned test files are
  untracked pre-existing lane state, never written here).
- Backups: `/tmp/opencode/bak2-<path-with-slashes-as-underscores>`.
