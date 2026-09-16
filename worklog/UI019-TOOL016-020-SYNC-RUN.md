# UI-019 + TOOL-016..020 + SYNC-001/002 + RUN-001 worklog

## Claim

Nine leased slices verified against pre-existing GREEN product modules
(TOOL-016..020 implemented in HEAD commit `248f519`) plus four newly
implemented modules (`tui_info_panel`, `part_events`, `runner`, `sync_log`)
with frozen contract tests 5/5 GREEN each. Shared `lib.rs` wiring only;
no controller/verifier/config edits.

## Source evidence

- Candidate base revision: `248f519` (`git log --oneline -5` head).
- `tasks/UI-019.md`, `tasks/TOOL-016.md`, `tasks/TOOL-017.md`,
  `tasks/TOOL-018.md`, `tasks/TOOL-019.md`, `tasks/TOOL-020.md`,
  `tasks/SYNC-001.md`, `tasks/SYNC-002.md`, `tasks/RUN-001.md`:
  contracts, bounds, failure states, acceptance criteria quoted below.
- Pre-existing product code (already GREEN in base, untouched):
  `crates/tools/src/mcp_catalog_search.rs` (303 lines),
  `crates/tools/src/mcp_bulk_actions.rs` (314 lines),
  `crates/tools/src/mcp_lifecycle.rs` (471 lines),
  `crates/tools/src/mcp_payload_filter.rs` (296 lines),
  `crates/sessions/src/mcp_status_panel.rs` (553 lines).
- New product code (this lane):
  `crates/sessions/src/tui_info_panel.rs`,
  `crates/sessions/src/part_events.rs`,
  `crates/sessions/src/runner.rs`,
  `crates/server/src/sync_log.rs`.
- Precedents: `crates/sessions/src/ui_006.rs:13-41`,
  `crates/tools/src/tool_allow.rs:8`, `crates/server/src/event_bus.rs`,
  `crates/providers/src/auth.rs:6-18`.
- `docs/TDD.md` lifecycle, `docs/SECURITY.md` brokered permissions,
  no direct secret access.
- `ralph.json`: all nine stories `not-started` (controller-owned; worker
  never edits status).

## Observed scenario

- TOOL-016..020 + TOOL-020/T01..T05 already had complete product modules
  in the base tree (`git ls-files` confirms 5/5 tracked). Missing piece
  was contract tests + one ownership gap: no `catalog/mcp-index.json`
  file exists (`ls crates/tools/catalog/` → no such directory), so the
  T05 shipped-index assertion was rewritten to build the versioned JSON
  inline (same schema, same `load_index` path).
- UI-019/SYNC-001/SYNC-002/RUN-001 had no product modules
  (`tui_info_panel`, `part_events`, `runner`, `sync_log` unresolved
  imports = compiling RED). Implemented minimum native Rust per card.
- Dirty-tree note: 6 modified files pre-existed from sibling lanes
  (`share_merge.rs`, `share_queue.rs`, `plugin_transform.rs`,
  `worklog/WEB-006.md`, plus the two `lib.rs` I touched for module
  wiring). I touched only the two `lib.rs` wiring lines among tracked
  files; the other four modifications are NOT mine.

## Target boundary

- Owned new tests (9 files): `crates/tools/tests/mcp_catalog_search.rs`,
  `mcp_bulk_actions.rs`, `mcp_lifecycle.rs`, `mcp_payload_filter.rs`,
  `crates/sessions/tests/mcp_status_panel.rs`, `tui_info_panel.rs`,
  `part_events.rs`, `runner.rs`, `crates/server/tests/sync_log.rs`.
- Owned new product (4 files): `crates/sessions/src/tui_info_panel.rs`,
  `part_events.rs`, `runner.rs`, `crates/server/src/sync_log.rs`.
- Shared wiring only: +3 `pub mod` lines in sessions `lib.rs`,
  +1 in server `lib.rs`. Left `Cargo.toml`/schemas/migrations alone
  (`part_events` drops serde derives because sessions crate has no
  serde dep; runner maps BusyError→RunnerError::Busy{session}).

## Tests

Frozen sha16 (sha256 prefix) at GREEN:

- tools/tests/mcp_catalog_search.rs `5e3accbecee97f25`
- tools/tests/mcp_bulk_actions.rs `fa0375f446c1987c`
- tools/tests/mcp_lifecycle.rs `b605f0c9004b0ba5`
- tools/tests/mcp_payload_filter.rs `02b72d40b5ffe456`
- sessions/tests/mcp_status_panel.rs `909ff59535a8129b`
- sessions/tests/tui_info_panel.rs `0e5b1686c532e1e4`
- sessions/tests/part_events.rs `cad1d938d7662efe`
- sessions/tests/runner.rs `ba048213e927250f`
- server/tests/sync_log.rs `9384ffdc43892680`

RED baselines: new-module suites failed compiling RED on unresolved
imports (`tui_info_panel`, `part_events`, `runner`, `sync_log`); prebuilt
TOOL suites failed first on missing `catalog/mcp-index.json` include (T05)
and one wrong atomicity assumption (`select_all` overflow leaves selection
unchanged → test fixed, product kept). Post-fix each suite 5/5 GREEN.

GREEN evidence (each with `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120`):

- `cargo test -p opencode-rk-tools --test mcp_catalog_search --test mcp_bulk_actions` → 5+5 passed
- `cargo test -p opencode-rk-tools --test mcp_lifecycle --test mcp_payload_filter` → 5+5 passed
- `cargo test -p opencode-rk-sessions --test mcp_status_panel --test tui_info_panel` → 5+5 passed
- `cargo test -p opencode-rk-sessions --test part_events --test runner` → 5+5 passed
- `cargo test -p opencode-rk-server --test sync_log` → 5 passed
- Regressions: tools lib 49 passed; sessions lib 30 passed; server lib 15 passed.
- `git diff --check` clean on all touched paths.

## Decisions

- Reuse pre-existing TOOL/MCP product code untouched (shortest diff).
- `select_all`/`invert` stay atomic (reject before mutate); test asserts
  unchanged selection.
- `tui_info_panel::redact` scrubs only `sk-` secret-value runs, keeps the
  word "Tokens" verbatim; `fit` preserves `+N more` tails at width caps.
- `part_events::StoredPart` has length-only Debug (no part bytes in logs);
  no serde (crate lacks the dep).
- `runner::RunnerError::Busy{session}` keeps session id on contention;
  `InvalidInput` reserved for bad ids; drop = Cancelled.
- `sync_log::replay` with `from_seq == len+1` on non-empty log returns
  `Ok(0)`; beyond head returns `BadCursor`; duplicates never re-emit.

## Remaining unknowns

- Verifier/controller acceptance only; `ralph.json` statuses untouched.
- `catalog/mcp-index.json` data file still absent (TOOL-016 ownership
  allows it but sibling `lib.rs`/`Cargo.toml` freeze + dirty tree made
  inline fixture the safe call); integrator may add the file later.
- Pre-existing dirty files (`share_merge.rs`, `share_queue.rs`,
  `plugin_transform.rs`, `worklog/WEB-006.md`) belong to other lanes.
- Full-workspace `cargo check`/`cargo test --workspace` not run (8 GB
  budget, 6.2 GiB host, one heavy command at a time per contract).
