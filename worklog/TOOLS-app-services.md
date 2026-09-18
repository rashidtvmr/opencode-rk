# Worklog TOOLS-app-services (PAR-005 slice)

## Claim
Types-only tool-service composition boundary: `ToolCall` (id/tool/args/scope),
`ToolResult`, `DispatchError` (Denied/Timeout/Cancelled/Bounds), size/timeout
caps, approval-digest binding helper. Pure validation only (arg-size cap,
`..` traversal reject, absolute-path gate, cancel gate). No execution bypass:
no `Command`, no I/O, no clock, no threads; `forbid(unsafe_code)`.

## Source evidence (HEAD 5af7884)
- `crates/tools/src/registry.rs:60-133` — `ToolRegistry` lookup only, no
  dispatch path (`register` at :133, `get`/`find_by_tag` only).
- `crates/tools/src/executor.rs:79-130` — `ToolExecutor::new` (:84),
  `Command::new("bash").arg("-c")` at :130 with no broker assertion.
- `crates/tools/src/permission.rs:1` — stub (`//! Tool permission module stub.`).
- `sources/completion/audits/AUD-005.json:56,64-72` — registry->executor->
  policy->state->output chain `unwired`; shell/file auth/cancel/bounds `unwired`.
- `sources/completion/audits/AUD-009.json:51,67-72` — subprocess behind broker
  `bypass`; LSP/Git lanes `missing` (out of scope for this types-only slice).
- `crates/storage/schema/v2/workspace.sql:186-220` — `tool_calls.intent_hash`
  BLOB(32), `tool_binding_immutable` trigger; `approvals.intent_hash` at :228.
- `crates/storage/src/approvals_v2.rs:25-128` — pending single-winner resolve,
  mandatory-human identity gate (binding consumer, not duplicated here).
- Shape conventions mirrored: caps-first check order (`ext_secure.rs:1-2`),
  relative-only file rule (`plugin_discover.rs:70-89` `file_ok`), absolute/
  non-normal reject (`skill_gate.rs:84-97`), `MAX_ARGS_BYTES=4096`
  (`plugin_scoped_exec.rs:10`), timeout defaults (`executor.rs:24-31`
  30s default / 300s max). `blake3` deliberately NOT used: not a `crates/tools`
  dep (`Cargo.toml`); std-only FNV-1a 4-lane fold, 32-byte shape matches
  `intent_hash`. Marked `ponytail:` in file header.

## Observed scenario
Owned file `crates/tools/src/app_services.rs` did not exist (read → File not
found). That is the RED: no composition types, no gates. Implemented in one
owned file; no other edits (`lib.rs` mod wiring left to orchestrator
pre-wire — `grep -c app_services lib.rs` → 0, noted below).

## Target boundary
- In: `ToolCall` construction (infallible), `validate` (caps→path→timeout),
  `effective_timeout`, `check_cancel`, `approval_digest`/`check_binding`,
  `validate_path`.
- Out: execution (`Command`), registry mutation, storage I/O, broker impl.
- Failure states: `Bounds` (shape/size/timeout), `Denied` (traversal/absolute/
  digest-mismatch), `Cancelled`, `Timeout` (carrier only — no timer here).
- Deny paths take `&ToolCall` — provably no mutation (asserted in tests).

## Tests (frozen in-file `#[cfg(test)]`, 11 tests)
benign_validates_with_default_timeout, rejects_traversal_in_scope_without_
mutation, rejects_traversal_in_args, rejects_absolute_path_in_args,
rejects_absolute_scope_and_windows_drive, rejects_oversize_args,
rejects_empty_and_bad_labels, timeout_caps_hold, cancel_gate_trips,
approval_digest_stable_and_bound, deny_paths_report_without_side_effects.

## Decisions
- `timeout > MAX` → `Bounds` error, never silent clamp (fail-closed).
- Transport `id` and `timeout_ms` excluded from digest; only tool/args/scope
  bound (mirrors immutable binding columns).
- `..` matched per path segment on `/` and `\`; absolute = leading `/` or
  `X:/`/`X:\` token; `://` tokens skipped as URLs.
- NUL in args/paths → `Bounds` (shape), not `Denied`.

## Verification
- `rustc --edition 2021 --test crates/tools/src/app_services.rs` + run:
  11 passed, 0 failed.
- `cargo check -p opencode-rk-tools` → Finished ok (pre-existing warnings
  only: `shell_tool.rs:168 unused mut`, `rtk_core.rs:194 unused assign`,
  `hook_bus_v2.rs:5 unused Duration` — none from this file).
- `cargo check -p opencode-rk-tools --tests` → Finished ok.
- `cargo test -p opencode-rk-tools --lib app_services` → 0 tests (expected:
  mod not yet wired into `lib.rs`; orchestrator pre-wires shared files).
- `grep Command|process:: app_services.rs` → only doc mentions, no bypass.

## Remaining unknowns / gaps
- `lib.rs` `pub mod app_services;` wiring pending (orchestrator-owned).
- Digest algorithm swap to blake3 pending dependency approval (ponytail).
- Dispatch executor actually calling `validate`→`check_cancel`→`check_binding`
  is a separate wiring lane, not this file.
- RED run was the pre-existing MISSING file state, not a separate frozen
  RED suite hash — controller/verifier decides sufficiency.
