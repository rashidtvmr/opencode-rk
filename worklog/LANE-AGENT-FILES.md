# LANE-AGENT-FILES — Scratchpad

## Claim
- **Task**: LANE-AGENT-FILES (RAW_FEATURE 1.3 gap "Custom agents from config/markdown")
- **Session**: ses_worker_agent_files (reclaimed from ses_orch_wave4)
- **Owned files**: `crates/agents/src/agent_files.rs`, `crates/agents/tests/agent_files.rs`

## Source Evidence
- `crates/server/src/rules_loader.rs` — house pattern for bounded file loading (MAX_FILES=64, MAX_FILE_BYTES=64KB, TOTAL_BUDGET=512KB, symlink rejection, deterministic ordering)
- `crates/agents/tests/delegation_lane.rs` — `#[path = "../src/X.rs"] mod X;` standalone test pattern
- `RAW_FEATURE.md:65` — "Custom agents from config/markdown | 🔴 GAP"

## Implementation
- `agent_files.rs`: 310 lines. std + serde + serde_json only. `#![forbid(unsafe_code)]` at crate level.
- Discovers `.md` and `.json` files in a directory root.
- Markdown: YAML frontmatter with `name`, `model`, `temperature`, `mode`, `tools` (multi-line list supported). Body = prompt content.
- JSON: `name` (required), `prompt`/`body` (accepts both), `model`, `temperature`, `mode`, `tools` (array).
- Bounds: MAX_FILES=64, MAX_FILE_BYTES=64KB, TOTAL_BUDGET=512KB.
- Symlink escape rejection via canonicalize + starts_with check.
- Deterministic ordering: alphabetical by path.
- Duplicate name detection: error returned.
- Unknown extensions ignored; oversized files skipped (counted toward file limit, not budget).

## RED Hash
`f040ddaada1967be5427a36784fb3b4adf50c0e6f16f6817b59804292f7e4ae2`

## GREEN Results
- `cargo test -p opencode-rk-agents --test agent_files`: 14 passed (0.01s)
- `cargo test -p opencode-rk-agents` (full crate gate): 68 passed (0.03s)

## Decisions
- Used `parse_list_value` for frontmatter tools (handles `[a, b]` and `a, b` formats).
- Multi-line YAML lists (`key:\n  - item`) parsed via line-by-line state machine in `parse_frontmatter`.
- JSON accepts both `"prompt"` and `"body"` field names for backward compat.
- Dev-dependency `serde_json` added to `crates/agents/Cargo.toml` (required for test `serde_json::json!` macro).

## Remaining Unknowns
- None. All 14 tests GREEN, full crate gate GREEN.
