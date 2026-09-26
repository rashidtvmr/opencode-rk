# BRIDGE-GAP-07 tool_meta.rs

Claim: `BRIDGE-GAP-07`, session `ses_gap07`.

Scope: new `crates/opentui-bridge/src/tool_meta.rs`; no `lib.rs` edits.

Source evidence:
- `crates/opentui-bridge/src/tool_display.rs:33-57`: existing `ToolDisplay` owns pending/final title rendering and `MAX_TITLE_LEN`; do not duplicate.
- User target: `packages/tui/src/util/tool-display.ts:7`, `toolDisplayMetadata(state_str: &str, status: &str)`; pending status returns empty title; structured `title:`/`desc:` prefix extraction; fallback tool name.
- `BRIDGE_MIGRATION_DETAIL.md:247-252`: records `toolDisplayMetadata` as known missing bridge surface.

Contract under test:
- `pending` status yields empty title, regardless of state text.
- Lines prefixed `title:` and `desc:` produce title/description metadata.
- No structured metadata falls back to tool name.
- Empty input remains bounded, empty-safe output.

Tests: in-file only per task. No cargo available in lane; verification will use direct `rustc --test` if practical.
Unknowns: exact TS return shape/edge whitespace. Inspect local pinned source/upstream before implementation.

## Result
- File: `crates/opentui-bridge/src/tool_meta.rs` (125 lines, under 150 cap).
- API: `tool_display_metadata(state: &str, tool: &str, status: &str) -> ToolMeta { title, description }`; `ToolMeta::empty()/is_empty()`; `MAX_META_LEN=100`.
- Evidence: `tool_display.rs:33-57` owns ToolDisplay pending/final rendering, not duplicated; actual TS `tool-display.ts:7-13` takes state object, returns `{}` for pending/missing structured; port flattens structured map to `title:`/`desc:`(`description:`) prefix lines. TS line refs track a0d9b6c checkout.
- Verify: `rustfmt --check` clean; `rustc --edition 2021 --test` 6/6 green (pending_empty, titled, fallback, empty_input, truncated, description_alias). No `lib.rs` edit; no cargo.
