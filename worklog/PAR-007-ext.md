# PAR-007-ext worklog

Claim: owned file `crates/tools/src/app_extensions.rs` implements PAR-007
extension types standalone (std only, `#![forbid(unsafe_code)]`, no I/O).

Source evidence (HEAD 5af7884):
- `crates/tools/src/skill_defs.rs:23` `qualify_skills` — name/desc shape + 128 cap; new file reuses bounded-count pattern, adds scope/precedence layer it lacks.
- `crates/tools/src/plugin_lifecycle.rs:85` `PluginRegistry` — name/contract/cap validation, SUPPORTED_CONTRACT_VERSION=1, MAX 64/16/128/64; new `NativePlugin::validate` mirrors label/cap rules, single supported version.
- `crates/tools/src/plugin_ui_boundary.rs` — inert declarations, `request_render` always `Deferred`; new `UiContribution::advertise_native` refuses `SolidTs` (never native), `UiCatalog::list_native` excludes it.
- `crates/tools/src/ext_compat.rs`, `ext_manifest_lane.rs`, `ext_commands.rs`, `skill_commands.rs`, `skill_gate.rs`, `ext_secure.rs` — compat verdicts, manifest bytes, `/cmd` shape, skill registry, secure grant; new file composes: command `/`-shape check, marker printable-ASCII bound, authority `request` all-or-nothing.

Observed scenario: file did not exist (`MISSING` pre-check). Created only owned path; `lib.rs` untouched (integrator wires module).

Target boundary: pure in-memory registries (`ExtensionRegistry`, `UiCatalog`, `CompatHost`, `Authority`), bounded (`MAX_CONTRIBUTORS 128`, `MAX_CAPABILITIES 16`, `MAX_UI 64`, `MAX_MARKER_LEN 256`, `MAX_COMPAT_BUDGET 8MiB`). `CompatHost::start(NativeOnly)` always `CompatRefusedNativeOnly`; `account` checked-add vs budget, over-budget accounts nothing. `Authority::request` validates all via `grant_input_ok` (label charset, no `/ \ * ..`, no `human./system./authority./grant.`), rejects whole batch unchanged.

Tests (in-file `#[cfg(test)]`, 8 tests):
- custom_command_affects_turn_marker (T01: command affects turn, user wins)
- precedence_and_scope_project_beats_plugin (T01: scope rank + skill path)
- native_capability_versioning (T02)
- unsupported_ui_not_advertised_native (T03)
- compat_never_starts_native_only + compat_account_requires_start_and_bounded_budget (T04)
- malicious_grant_rejected_authority_unchanged (T05: batch atomic, granted unchanged)
- registry_bounds_and_duplicates (bounds/unknown)

Decisions: `Scope` rank Builtin<Plugin<Project<User; same kind+name across scopes allowed, resolve picks max. `grant_input_ok` uses `valid_cap` + explicit `/ \ * ..` + reserved prefixes.

Remaining unknowns: none in lane; e2e `tests/e2e/extension_parity` owned by integrator/parent.
