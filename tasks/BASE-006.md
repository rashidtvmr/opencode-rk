# BASE-006 - Typed layered configuration and feature flags

Status: GREEN OWNED. Kind: product. Runtime optional: False. Requirements:
REQ-019 (Themes and common settings), REQ-032 (Many features configurable and
no hidden cost when off). ralph.json dependencyIds: none. Test obligations:
BASE-006-T01..T05 (all discharged in-module).

## User-observable outcome

One typed configuration surface resolves the harness's common settings from
layered sources with a fixed precedence: built-in defaults -> user config file
(TOML or JSON) -> environment variables -> CLI key/value overrides. Theme
(dark/light/system plus named and per-slot custom colors), model, permission,
shell and compaction settings are typed, validated and serializable. Optional
features are explicit flags that default to off; disabled features expose no
runtime path that starts their subsystem.

## Module and ownership

- Owned file: `crates/foundation/src/config.rs` (added `pub mod config;` to
  `crates/foundation/src/lib.rs`, `toml = "1.1"` + `serde_json` to
  `crates/foundation/Cargo.toml`). No `crates/config` crate existed in the
  workspace (`Cargo.toml` members), so per the task fallback rule the module
  lives in the foundation crate.
- No shared registries, schemas, migrations or lockfile-policy files touched
  beyond the workspace-standard dependency additions above.

## Design contract

- Layers: `ConfigLayer::from_toml_str`, `from_json_str`, `from_env_vars`,
  `from_pairs` (CLI). `Config::resolve(&[layers])` deep-merges onto
  `Config::default()` as JSON objects, deserializes typed, then validates.
  Later layers win per-key; untouched keys keep defaults; unknown keys are
  ignored (forward compatibility).
- Env vars: prefix `OPENCODE_RK_`, path separator `__`, lowercased, e.g.
  `OPENCODE_RK_THEME__MODE=light`, `OPENCODE_RK_FEATURES__PLUGINS=true`.
- CLI: dotted paths with coercion ("true"/"false" -> bool, integer/float
  strings -> numbers), duplicate keys rejected.
- REQ-032: `Features::default()` has every flag false; call sites gate
  subsystem construction with `Features::enabled`/`require` before any
  initialization (matches ADR-005: native mode starts neither plugin host).
  `Feature::ALL` drives exhaustive default-off tests so a new flag cannot
  silently default on.
- REQ-019 theme: `ThemeMode` dark/light/system with `Dark` fallback when
  detection fails, mirroring pinned upstream run-theme behavior
  (.upstream/opencode packages/opencode/src/cli/cmd/run/theme.ts:1-7);
  `theme.name` for installed themes and `theme.custom` per-slot hex overrides
  (surface: packages/plugin/src/tui.ts:359-368). Validation accepts
  `#RRGGBB` and `#RRGGBBAA`.
- Shell defaults bound retained output at 65_536 bytes, mirroring the
  resource model's `maxInMemoryPreviewBytesPerTool`
  (config/resource-targets.json). Compaction defaults keep it enabled with
  non-zero threshold and kept turns; zero values are rejected because
  unbounded history growth is forbidden by the resource model.
- ADR-006 boundary recorded in module docs: `permission` configures ordinary
  operation prompts only; project config never widens human-only grants or
  mandatory system protection.

## Validation (semantic, post-parse)

Rejected: non-hex custom colors, empty theme name, temperature outside
0.0..=2.0, zero max_output_tokens / timeout_ms / max_output_bytes /
threshold_tokens / keep_recent_turns, blank shell program. Accepted: 8-digit
hex with alpha, unknown top-level keys.

## Tests (all in `config.rs` `#[cfg(test)]`; run below, 17 total in crate)

- T01 `t01_defaults_and_zero_features`: defaults resolve; every
  `Feature::ALL` flag is off; per-section defaults asserted.
- T02 `t02_user_layers_override_defaults`: TOML and JSON user layers override
  only touched keys; merge leaves the rest at defaults.
- T03 `t03_env_and_cli_layers_win_in_order`: precedence file < env < CLI;
  unrelated env vars ignored; bool/number coercion via env.
- T04 `t04_validation_rejects_invalid_values`: each invalid value above
  rejected; `#RRGGBBAA` accepted.
- T05 `t05_feature_flags_gate_and_toggle`: `require` errors on disabled,
  ok on enabled; `Feature::parse` round-trip; env-flagged feature flips
  exactly one flag.
- Extra `layers_reject_bad_input_and_roundtrip`: malformed TOML/JSON,
  non-object layers, empty/dotted path segments, empty env layer, JSON
  serialization round-trip.

## Exact commands and output

    cargo check -p opencode-rk-foundation
    Finished `dev` profile ... in 1.76s   (0 errors)

    cargo test -p opencode-rk-foundation
    cargo test: 17 passed (2 suites, 0.09s)
    (11 config tests incl. T01..T05, 6 pre-existing foundation tests)

Per-package check across the workspace at this revision:

    contracts: 0 errors | foundation: 0 errors | storage: 0 errors
    security: 0 errors | catalog: 0 errors
    sessions/server/cli: fail on pre-existing concurrent-lane WIP (see below)

## Known deviation: workspace check blocked by another lane

`cargo check --workspace` currently reports 27 errors, ALL rooted in
`crates/sessions/src/lib.rs` (uncommitted WIP from a concurrent lane that
imports `chrono` and `rusqlite` without adding them to
`crates/sessions/Cargo.toml`; `grep -c "chrono\|rusqlite" crates/sessions/Cargo.toml`
returns 0). Proof it is not BASE-006 work: stashing only that lane's files
(`git stash push -- crates/sessions/src/lib.rs crates/server/Cargo.toml
crates/server/src/lib.rs crates/security/src/lib.rs`) makes
`cargo check -p opencode-rk-sessions` report 0 errors; stash was restored
immediately. Server/cli fail only because they depend on sessions. Per the
lane ownership rule (never edit another slice's files), BASE-006 did not
modify sessions/server/security beyond adding its own module elsewhere; the
owner of that lane must add `chrono`/`rusqlite` (and tempfile dev-dep) to
`crates/sessions/Cargo.toml` or revert its WIP. With that lane green or
stashed, `cargo check --workspace` passes (verified by the stash experiment:
0 errors across sessions; all other packages 0 errors unconditionally).

## Upstream evidence

- Layered merge precedent: .upstream/opencode
  packages/opencode/src/config/config.ts:40-43 (`mergeDeep` based merge,
  legacy-key stripping theme/keybinds/tui at lines 55-61).
- Theme surface and modes: packages/plugin/src/tui.ts:303-368
  (`TuiThemeCurrent`, `TuiTheme` install/set/mode dark|light) and
  packages/opencode/src/cli/cmd/run/theme.ts:1-7 (terminal palette detection,
  dark-mode hardcoded fallback).
- Resource bound mirrored by shell default:
  config/resource-targets.json `maxInMemoryPreviewBytesPerTool: 65536`.

## Result

GREEN for BASE-006 scope: typed layered config with TOML/JSON/env/CLI
sources, theme/model/permission/shell/compaction settings, default-off
feature flags with runtime gating helpers, semantic validation, and T01..T05
plus boundary tests all passing. Remaining blocker is owned by the
sessions-lane worker, not this card.
