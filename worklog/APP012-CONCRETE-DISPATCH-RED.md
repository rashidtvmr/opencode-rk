# APP012-CONCRETE-DISPATCH-RED

## Claim

- Task: `APP012-CONCRETE-DISPATCH-RED`
- Session: `ses_f281d2adfffeBhvqwJrqg2eLf5`
- Branch: `red/APP012-CONCRETE-DISPATCH`
- Base: `7262682e31c1f473912c9483804c9430eb4ee4c5`
- Owned path: `crates/tools/tests/app012_concrete_dispatch_red.rs`

## Source evidence

- `crates/tools/src/executor.rs:33-60`: public `ToolCall` takes `(tool_id, name, serde_json::Value)` and optional timeout.
- `crates/tools/src/executor.rs:77-122`: public `ToolExecutor` exposes `execute`, not `execute_authorized`.
- `crates/tools/src/file_ops.rs:158-200`: existing `execute_authorized` belongs to `FileTool`/`FileOperation`, not `ToolExecutor`.
- `crates/security/src/tool_authorize.rs:59-78`: direct-argv and shell intent constructors exist, but no executor authorized-call seam exists at this revision.

## Audit and blocker

The candidate test was invalid: it passed `{"command": program, "args": args}` to the JSON `ToolCall` shape while claiming direct argv, and its `sh -c` payload was a shell-string path. It also called nonexistent `ToolExecutor::execute_authorized`.

Focused compilation, twice, failed at the actual missing public method:

```text
error[E0599]: no method named `execute_authorized` found for struct `ToolExecutor`
```

No honest compiling `ToolExecutor` seam exists at base `7262682`. The file was removed. No RED hash frozen. Status: BLOCKED pending an implementation-independent public `ToolExecutor::execute_authorized` seam or an authority-approved contract change to the existing `FileTool::execute_authorized` seam.

## Verification

- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test app012_concrete_dispatch_red` -> compile failure, twice, identical missing-method error.
- No product/lib/Cargo/test changes retained. No fixture process or file side effect ran because the candidate never compiled.
