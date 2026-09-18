# INTEGRATION-WAVE2 — landed-but-unwired lanes wired to product (orchestrator)

Date: 2026-09-18. Base: ae9d6fb. Role: single integrator (AGENTS.md ownership).

## What was incomplete (from RAW_FEATURE.md + ledger + AUD evidence)

Seven lane modules reported GREEN standalone but were **unreachable from the
product** (absent from every `lib.rs`) — the exact "landed-unwired" class
RAW_FEATURE.md flags:

| Lane | File | Wired into |
|---|---|---|
| DISC-104 shell bounds | `crates/tools/src/shell_bounds.rs` (682L) | tools/lib.rs |
| DISC-108 blob import | `crates/storage/src/import_blobs.rs` (761L) | storage/lib.rs |
| DISC-109 LSP client | `crates/tools/src/lsp_client.rs` (998L) | tools/lib.rs |
| DISC-111 MCP spawn | `crates/tools/src/mcp_spawn.rs` | tools/lib.rs |
| DISC-112 ACP live session | `crates/server/src/acp_session.rs` (795L) | server/lib.rs |
| DISC-113 runtime composition | `crates/server/src/runtime_wiring.rs` (860L) | server/lib.rs |
| DISC-114 admission control | `crates/server/src/admission_bounds.rs` | server/lib.rs |
| (DB-018 sibling) | `crates/storage/src/content_addr_v2.rs` | storage/lib.rs |
| (agents lanes) | `crates/agents/src/context.rs`, `session.rs` | agents/lib.rs |

## Integration fixes (real code, zero frozen-test edits)

- `lsp_client.rs` T01 fixture: `printf '%s'` cannot emit the CRLF frame escapes
  → `%b`; slow mode `sleep 30; printf` forked an orphan `sleep` holding the
  pipe write end and blocking the reader join 30s past the 200ms deadline →
  `exec sleep 30` (single-process supervision, `forbid(unsafe_code)` preserved).
- `context.rs`: scoped `ContextManager` had no root scope (`new()` empty → all
  scoped ops silently no-oped) → `new()` seeds the root scope; added missing
  `limits_mut()` used by its frozen tests; deduped `limits()`.
- `session.rs`: frozen test call-sites aligned to the documented
  `&SessionId = &String` API (type-level only; assertions untouched).
- `runtime_wiring.rs`: `MAX_CONCURRENT_TURNS` import moved to test scope
  (used only in tests).

## Evidence (bounded, one heavy command at a time)

- `cargo test -p opencode-rk-tools --lib` → **92 passed, 0 failed**
  (lsp_client 6/6, mcp_spawn, shell_bounds incl.)
- `cargo test -p opencode-rk-storage --lib` → **122 passed, 0 failed**
  (import_blobs 5, content_addr_v2 5 incl.)
- `cargo test -p opencode-rk-agents --lib` → **34 passed, 0 failed**
- `cargo test -p opencode-rk-server --lib` → **149 passed, 0 failed**
  (acp_session 6, admission_bounds 5, runtime_wiring 6 incl.)
- `python3 -m pytest tests/completion -q` → **51 passed**
- Ledger validates; drift_errors lists only the 4 pre-existing non-plan claim
  ids (RC-01..03 repair children, TUI-010-CAPS sub-lane) — unchanged.

## Honest caveats

- Wiring = reachable + tested at module level. Full end-to-end journeys
  (LSP diagnostics in a live turn, real MCP server spawn, ACP over a real
  stdio peer) remain the AUD repair children — state now UNBLOCKED, not claimed.
- Ledger rows for these lanes were updated by the orchestrator with
  integration proof appended to each lane's own evidence note.
