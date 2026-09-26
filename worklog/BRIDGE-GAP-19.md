# BRIDGE-GAP-19

- Claim: `BRIDGE-GAP-19`; session `ses_gap19`; status `in-progress`.
- Scope: `crates/opentui-bridge/src/theme_paint.rs` only (plus this scratchpad).
- Evidence:
  - `crates/opentui-bridge/src/theme.rs:31-88`: `Theme` color fields include `background`, `background_panel`, `background_element`, `text`, `selected_list_item_text`.
  - `crates/opentui-bridge/src/theme_resolve.rs:76-110`: `resolve_color`; selected text fallback resolves theme `background`.
  - `crates/opentui-bridge/src/terminal_mode.rs:25-36`: `terminal_mode` and `is_dark` semantics.
  - TypeScript source location requested by task: `packages/tui/src/theme/index.ts:241-300`; path in this checkout still to locate.
- Target: std-only region color mapping, dark/light fallback behavior, unknown-theme fallback, selected fallback.
- Tests: add at least four unit tests in owned file.
- Decisions: inspect neighboring bridge APIs before choosing signatures; no lib.rs/test edits.
- Unknowns: exact theme-name catalog and expected color value representation.

- Decisions: `selected_bg` follows resolver compatibility fallback to transcript/background. `opencode` is the default palette; common named themes use stable dark presets. `is_dark` remains explicit because terminal mode is optional upstream.
- Remaining verification: rustfmt check, then ledger completion.
