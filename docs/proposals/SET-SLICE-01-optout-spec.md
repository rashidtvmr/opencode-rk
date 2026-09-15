# SET-SLICE-01 - Unified opt-out settings for four inbuilt tools

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-032 (no hidden cost when off).
Dependencies: BASE-006 (layered config, `Features::require` gate pattern).
Test obligations: SET-OPT-T01, SET-OPT-T02, SET-OPT-T03, SET-OPT-T04, SET-OPT-T05.

## User-observable outcome

Four inbuilt subsystems (codebase index, RTK filter, terse mode, telemetry)
are ON by default and each independently disableable via layered config
(defaults < file < env < CLI per BASE-006). A disabled subsystem is never
constructed: zero I/O, zero thread, zero retained bytes (REQ-032).

## Source evidence

- crates/foundation/src/config.rs:211-258 (`Features`, `enabled`/`set`/`require`, default-off precedent; here mirrored default-on).
- crates/foundation/src/config.rs:180-209 (`Feature::ALL` exhaustive-assert pattern; mirrored by `OptOutFeature::ALL`).
- crates/foundation/src/config.rs:275-289 (`Config::resolve` merge order); :422-438 (env `OPENCODE_RK_` + `__` separator); :382-417 (CLI dotted pairs + coercion).
- tasks/BASE-006.md: layered precedence + gate-before-init contract.
- tasks/TOOL-014.md: task-card model for this file.
- PLAN.md section 5: additive fragment, integrator assembles `lib.rs`. Section 6 + docs/TDD.md sections 2-5: RED-then-GREEN.

## Observable contract

- `InbuiltFeatures { codebase_index: bool, rtk_filter: bool, terse_mode: bool, telemetry: bool }`; `Default` is all `true`.
- `OptOutFeature` enum (`CodebaseIndex`, `RtkFilter`, `TerseMode`, `Telemetry`) with `ALL: &[OptOutFeature]` (4 entries), `name()` (`"codebase_index"`, `"rtk_filter"`, `"terse_mode"`, `"telemetry"`), `parse(name) -> Option<OptOutFeature>`.
- `enabled(f)`, `set(f, on)`, `require(f) -> Result<(), ConfigError>` mirroring `Features`; `require` on disabled returns `Err(FeatureDisabled(name))` before any init.
- Config key table `inbuilt`: file TOML/JSON (`[inbuilt] telemetry = false`), env (`OPENCODE_RK_INBUILT__TELEMETRY=false`), CLI (`inbuilt.telemetry=false`); later layers win per-key, untouched keys keep defaults, unknown keys ignored.
- Resolved via existing `Config::resolve(&[layers])`; no new precedence rules.
- Call sites gate construction: `config.inbuilt.require(OptOutFeature::Telemetry)?` before building the subsystem.

## Failure states

- Disabled gate: `require` returns `Err(FeatureDisabled(name))`; caller must construct nothing and cause no side effects (no I/O, thread, log write).
- Unknown flag name: `parse` returns `None`, never panics.
- Wrong-type layer value (e.g. `inbuilt.telemetry = 42` uncoercible): `Config::resolve` returns `Err(InvalidValue(_))`; defaults unchanged.
- Malformed TOML/JSON layer: `Err(InvalidToml/InvalidJson)`; non-object layer: `Err(LayerNotObject)`.
- Settings never widen human-only grants (ADR-006, below); a `require` success is not authority.

## Resource bounds

- Disabled subsystem cost is zero: no filesystem I/O, no spawned thread/task, no allocation beyond the 4-bool struct.
- `require` check is O(1); `ALL` iteration is 4 elements; no unbounded queue, no retained output.
- Snapshot/filter/telemetry buffers (if any) are owned by the gated subsystem, dropped when disabled; bodies never retained past the gate.

## Suggested module boundary

- Additive fragment to foundation config (e.g. `crates/foundation/src/inbuilt.rs` with `InbuiltFeatures` + `OptOutFeature`, re-exported from `config.rs` or wired by integrator).
- Worker ships the fragment only; integrator adds `pub mod` / re-export per PLAN.md section 5; worker never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.
- `Config` gains `pub inbuilt: InbuiltFeatures` with `#[serde(default)]` so old files resolve unchanged.

## Frozen test obligations (exact asserts)

- SET-OPT-T01 defaults all on: `Config::resolve(&[])` => `assert!(config.inbuilt.codebase_index && config.inbuilt.rtk_filter && config.inbuilt.terse_mode && config.inbuilt.telemetry)`.
- SET-OPT-T02 each flag flips independently: for each `f` in `OptOutFeature::ALL`: `set(f,false)` => `assert!(!enabled(f))` and the other three stay `true`; `set(f,true)` restores.
- SET-OPT-T03 env layer wins over file: file sets `inbuilt.telemetry=false`, env `OPENCODE_RK_INBUILT__TELEMETRY=true` => `assert!(config.inbuilt.telemetry)`; reverse polarity also asserted; unrelated env ignored.
- SET-OPT-T04 disabled gate returns clean error before init: disabled `f` => `assert_eq!(require(f), Err(FeatureDisabled(f.name())))`; enabled => `assert_eq!(require(f), Ok(()))`; gated constructor helper asserts no subsystem object built on `Err`.
- SET-OPT-T05 exhaustive default-on: iterate `OptOutFeature::ALL`: `assert!(InbuiltFeatures::default().enabled(*f), "default must enable {}", f.name())`; `assert_eq!(OptOutFeature::ALL.len(), 4)` so a fifth flag without test update fails; `parse` round-trips every `name()`.

## TDD steps

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, BASE-006/TOOL-014 cards; cite commit + path:line.
2. Define contract/failures/bounds (above); open discovery proposal for gaps.
3. Author RED: T01..T05 compile, fail on missing `inbuilt`/`OptOutFeature`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: CLI-over-env precedence per flag, unknown keys ignored, disabled no-I/O.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p opencode-rk-foundation` passes SET-OPT-T01..T05 on frozen hash.
- `cargo check --workspace` clean (modulo unrelated lanes, evidence by stash if needed).
- Disabled-flag test proves no subsystem construction (gate errors first).

## ADR-006 boundary

Opt-out settings configure ordinary operation only. They never grant authority,
never widen a human-only grant, never bypass mandatory system protection.
Project/file/env/CLI config is never a source of human authority.
