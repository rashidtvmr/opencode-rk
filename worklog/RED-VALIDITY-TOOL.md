# RED-VALIDITY-TOOL — TOOL-016/017/018/019/020 + UI-019 + SYNC-001/002 retroactive RED receipts

Base revision: `248f519` (impl predates lane; impl files present in workdir with
sibling-lane modifications elsewhere in tree).
Method: per file, temp behavior-stub impl (signatures intact, no test/lib.rs/ralph.json
touch), run frozen suite expecting compile+fail, restore byte-identical (`cmp`
+ sha256 vs pre-run backup), re-run GREEN 5/5.
Bounds: serial, one heavy cmd at a time, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`,
`timeout 115` per run, `--test-threads=1`. `free -h` at start: 6.2G total, ~2.8-3.0G avail.

Claim: all 8 suites bite on behavior-stubbed impls (RED proven below) and return to
5/5 GREEN after byte-identical restore. No product delta remains from this lane.

## Impl hashes (pre == post, restore verified via cmp against /tmp/opencode/bak-*.rs)

- `crates/tools/src/mcp_catalog_search.rs` `8c2142c3349307cb8be12face57713a081215497e6450b7f7d11cb4556d960fd`
- `crates/tools/src/mcp_bulk_actions.rs` `53dcae475c08a3e515db6312a05fefc66ed8ef0493cdaf06fffeef5d0c782419`
- `crates/tools/src/mcp_lifecycle.rs` `5e63c909997155a7afb4cd9288127b3eb179a8e4b8b8e18a20f4bd9f3938ece4`
- `crates/tools/src/mcp_payload_filter.rs` `565d28f011c3cb7b62465d1d8044d72994fe17a88a97cd60ad96f60e779d0aa8`
- `crates/sessions/src/mcp_status_panel.rs` `61e86d8d85fb1caaf63ad00aa2c85ea0cbe25f9d79b1dc7df116f163bdac295c`
- `crates/sessions/src/tui_info_panel.rs` `606de856f564456fb9a804fd9a1819177167717f978f76ed55a0def31fedb45e`
- `crates/sessions/src/part_events.rs` `8ab97dea64e32e4fdf50c88704fe9eae3fbd0f9cb71d22d8c3edd4ee1cf633a6`
- `crates/server/src/sync_log.rs` `021a9936bf436ea46b8754cdcfedf550c78a54b8268f026ba0c5316c66bb93bd`

## Frozen test hashes (untouched by this lane; files untracked, never edited)

- `crates/tools/tests/mcp_catalog_search.rs` `4de7cdde25cf4a6091cc92c7c5b44671234000831c5ea9107cc3a8719c103bab`
- `crates/tools/tests/mcp_bulk_actions.rs` `5a3c0250cb3a34b52c4be1d7de515da0cfa05e8dc93c966786391d90a278b0c5`
- `crates/tools/tests/mcp_lifecycle.rs` `2fbaca626ae6a38419ec101573d4a8ff9da9f1d210f0c0f1692ae95b0c71b257`
- `crates/tools/tests/mcp_payload_filter.rs` `f9f02e976234260147d7a3f49700909152dd1bf21b63151874cd36ff9756c2fd`
- `crates/sessions/tests/mcp_status_panel.rs` `0f38b20ac9719ef50480b600c0b2e38dde13582b30c92b2d99f80cd423ca43e8`
- `crates/sessions/tests/tui_info_panel.rs` `d202bb25c675889a1bb2b56827db6234aaf5675237ce7aef9f6f431f1f868eda`
- `crates/sessions/tests/part_events.rs` `1a585f56280c251f24fef2df8af6576fe4fc8a17b00ba46d3b6f8767d481e99c`
- `crates/server/tests/sync_log.rs` `a38c2e7256660b1819f22c972255dee0cf1a0a5e18c4d8871e2388f129f72a37`

## Per-task receipts (stub -> RED fails -> restore -> GREEN)

| id | stub (behavior-only, compiles) | RED log: fail count + witness | GREEN log: count | restore |
|----|-------------------------------|-------------------------------|------------------|---------|
| TOOL-016 | `search` shadows index with `&[]`, early-returns empty hits (`mcp_catalog_search.rs:114`) | `/tmp/opencode/rG-tool016-red.log`: 1 pass / 4 fail; T01 panics `mcp_catalog_search.rs:77`, T02 `:96`, T03 `:126`, T04 `:158`; T05 passes (load_index/determinism path, no search) — suite bites | `/tmp/opencode/rG-tool016-green.log`: 5 passed | cmp identical, sha pre==post |
| TOOL-017 | `apply` adds `ids.clear()` after sort (`mcp_bulk_actions.rs:241`) | `/tmp/opencode/rG-tool017-red.log`: 1 pass / 4 fail; T01 panics `mcp_bulk_actions.rs:18`, T03 `:63`; T02 passes (selection-only, no apply) — suite bites | `/tmp/opencode/rG-tool017-green.log`: 5 passed | cmp identical |
| TOOL-018 | `mark_ready` early-returns `Err(NotEnabled)` (`mcp_lifecycle.rs:248`) | `/tmp/opencode/rG-tool018-red.log`: 4 pass / 1 fail; T01 panics `mcp_lifecycle.rs:20` (ready gate); T02-T05 pass (error/reconnect, guards, persist, determinism need no ready) — suite bites | `/tmp/opencode/rG-tool018-green.log`: 5 passed | cmp identical |
| TOOL-019 | `filter_payload` adds `enabled_servers.clear()` after dedup (`mcp_payload_filter.rs`) | `/tmp/opencode/rG-tool019-red.log`: 1 pass / 4 fail; T01 panics `mcp_payload_filter.rs:44`, T02 `:56`; T04 passes (validation-only, no happy path) — suite bites | `/tmp/opencode/rG-tool019-green.log`: 5 passed | cmp identical |
| TOOL-020 | `summarize`+`event` force empty tools / `None` error (`mcp_status_panel.rs`) | `/tmp/opencode/rG-tool020-red.log`: 3 pass / 2 fail; T01 panics `mcp_status_panel.rs:29`, T04 `:63`; T02/T03/T05 pass (narrow/stale, bus, determinism paths) — suite bites | `/tmp/opencode/rG-tool020-green.log`: 5 passed | cmp identical |
| UI-019 | `render` early-returns `Ok(vec!["stub"])` after width check (`tui_info_panel.rs`) | `/tmp/opencode/rG-ui019-red.log`: 1 pass / 4 fail; T01 panics `tui_info_panel.rs:49`, T02 `:62`; T04 passes (EmptyField/TooNarrow gates fire before stub) — suite bites | `/tmp/opencode/rG-ui019-green.log`: 5 passed | cmp identical |
| SYNC-002 | `apply_update` early-returns `Ok(())` without validation/store (`part_events.rs`) | `/tmp/opencode/rG-sync002-red.log`: 3 pass / 2 fail; T01 panics `part_events.rs:27`, T04 `:121`; T02/T03/T05 pass (classify/filter/Debug paths need no store) — suite bites | `/tmp/opencode/rG-sync002-green.log`: 5 passed | cmp identical |
| SYNC-001 | `append` early-returns `Err(Full)` (`sync_log.rs`) | `/tmp/opencode/rG-sync001-red.log`: 0 pass / 5 fail; T01 panics `sync_log.rs:25`, T02 `:41` — full-fail | `/tmp/opencode/rG-sync001-green.log`: 5 passed | cmp identical |

Notes:
- GREEN "5 passed" lines are per-suite `cargo test -p <crate> --test <name> -- --test-threads=1`; RED logs carry full per-test panic output and `test result: FAILED. N passed; M failed` lines.
- Partial-fail REDs are valid suite-bite receipts: stub exercises the happy-path entry point, so tests not touching it (negative-path, validation-only, encode/focus gates) stay green.
- No frozen test, `lib.rs`, `Cargo.toml`, schema, or `ralph.json` edited by this lane. Working-tree `M` on `crates/*/src/lib.rs` and other files is pre-existing sibling-lane state, not this lane. `ralph.json` untouched (no diff).
- Backup copies: `/tmp/opencode/bak-tool-016.rs`, `bak-tool-017.rs`, `bak-tool-018.rs`, `bak-tool-019.rs`, `bak-tool-020.rs`, `bak-ui-019.rs`, `bak-sync-002.rs`, `bak-sync-001.rs`.
- Pre-existing tree hazard noted: `crates/tools/src/plugin_transform.rs` in workdir has `_scope/_key/_value` stub-signature breakage from a sibling lane; it did not block these 8 suites (their crates compiled), but workspace-wide `cargo check`/`cargo test --workspace` is red independent of this lane.
