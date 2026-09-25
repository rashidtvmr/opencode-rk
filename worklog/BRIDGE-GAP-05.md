# BRIDGE-GAP-05 error_format.rs

## Claim
- Task: `BRIDGE-GAP-05`
- Session: `ses_f28c0df33ffd42j4xvSA23Rjr6`
- Owned file: `crates/opentui-bridge/src/error_format.rs` (new; no `lib.rs` edit)
- Scratchpad: `worklog/BRIDGE-GAP-05.md`
- Candidate revision: `d460eb965b200e1454941db5be200b548746dea9`

## Source evidence
- TS checkout `a0d9b6c7014dab2d6e94819d4ad1d3d2a2e4f737`.
- `packages/tui/src/util/error.ts:19-75`: ProviderModelNotFound, ProviderInit, ConfigJson, ConfigDirectoryTypo, ConfigFrontmatter, ConfigInvalid, UICancelled, MCP message mapping; `CliError` exit code is a process side effect at `:11-13`.
- `packages/tui/src/util/error.ts:97-123`: native error format, record pretty format, non-record `String` fallback.
- `packages/tui/src/util/error.ts:125-145`: native/record/data-message extraction and unknown fallback.
- `packages/tui/src/util/error.ts:147-181`: error data extraction, scalar passthrough, nested value debug-string conversion.
- `packages/tui/src/util/record.ts:1-3`: record excludes arrays/null.
- `crates/opentui-bridge/src/cli_error.rs:11-18,31-59,79-130`: existing Cli/Account tags, typed cause chain, caller-owned exit code. New formatter must extend, not duplicate side effects.

## Contract
- Value model covers native errors, tagged/records, arrays, scalar unknowns, null.
- Cause formatting is iterative and terminates; message joins each cause using `cause: <message>`.
- Cli/Account and every Config/Provider/MCP branch return known message; Cli exit code returned separately.
- `error_data` returns native fields, flattened record data, and formatted value without panics.
- Complex record data uses Rust debug string; primitive values remain typed where observable.
- No IO, dependencies, unsafe, process exit, recursion, or global state.

## Tests
- Compiling RED: `rtk timeout 120 rustc --edition=2021 --test crates/opentui-bridge/src/error_format.rs -o /home/rashid/.cache/bun-tmp/opencode/bridge-gap05-red && rtk timeout 120 /home/rashid/.cache/bun-tmp/opencode/bridge-gap05-red --test-threads=1` exited 101: 11 compiled, 11 behavior failures, 0 passed.
- Frozen in-file test suffix SHA-256: `373f36ca1368d96b4856c14beeb142707ad06aedcbe47aab95fa9d494c6880d0`.
- Coverage: nested cause, tagged shape/data/exit code, unknown int, none fallback, MCP, Provider/Config, Account/cancel, record data, 2,000-deep cause chain.
- GREEN command: direct `rustc --test` on owned file; cargo prohibited by lane.

## Decisions
- Use one small public `Value` tree plus direct result structs. `lib.rs` wiring remains integrator-owned.
- Cause traversal is iterative to avoid stack overflow on deep finite chains.

## Remaining unknowns
- Hidden acceptance may require exact Rust symbol names; expose TS-shaped snake_case functions and obvious public model types.

## Verification
- `rtk timeout 120 rustc --edition=2021 --test crates/opentui-bridge/src/error_format.rs -o /home/rashid/.cache/bun-tmp/opencode/bridge-gap05 && rtk timeout 120 /home/rashid/.cache/bun-tmp/opencode/bridge-gap05 --test-threads=1`: 11 passed, 0 failed.
- `rtk rustfmt --check --edition 2021 crates/opentui-bridge/src/error_format.rs`: clean.
- `error_format` line 376, `error_message` line 398, `error_data` line 451; eleven in-file tests cover the three APIs and mapped branches.
- No cargo; no lib.rs change.

