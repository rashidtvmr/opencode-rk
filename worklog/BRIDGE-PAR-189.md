# BRIDGE-PAR-189 scratchpad

Claim: BRIDGE-PAR-189, session ses_par189, owned file
`crates/opentui-bridge/src/ctx_bundle.rs`.

Source evidence:
- `crates/opentui-bridge/src/kv_ctx.rs:18` `pub struct KvCtx`, `:29`
  `set(&mut self, k: &str, v: &str) -> bool`, `:46` `get`.
- `crates/opentui-bridge/src/route_ctx.rs:15` `pub struct RouteCtx`,
  `:26` `new(initial: &str) -> Option<Self>`, `:38` `go`, `:33` `current`.
- `crates/opentui-bridge/src/runtime_ctx.rs:9` `pub struct RuntimeCtx`,
  `:51` `Default` = 80x24 not-ready, `:56` `mark_ready`, `:41` `dims`,
  `:46` `is_ready`.

Observed scenario: no existing bundle/composite ctx file; task defines new
facade. `lib.rs` NOT edited per scope (orchestrator wires `mod`).

Target boundary: ONE new file only. No edits to lib.rs, Cargo.toml,
kv_ctx.rs, route_ctx.rs, runtime_ctx.rs, helper_ctx.rs. No cargo, no commit.

Tests written (6, in-file `mod tests`):
new_seeds_cwd, set_delegates_to_kv, go_changes_route,
ready_flips_summary, summary_format_and_cap, summary_caps_long_route.

Decisions:
- `new(cwd)` seeds kv key "cwd", route "home" (expect: always valid),
  rt default 80x24 not-ready.
- `summary` = `route=<cur> <cols>x<rows> ready|not-ready`, char-capped 256.
- std-only, `forbid(unsafe_code)`. ~130 lines.

Remaining unknowns: `mod ctx_bundle` wiring in lib.rs left to integrator.
