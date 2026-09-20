# LANE-SHELL-AUTHZ scratchpad

Claim: LANE-SHELL-AUTHZ, session ses_f423cd0dfffenzXnM7HZSns8eh, status in-progress.
Owned file: crates/tools/src/shell_tool.rs (+ this scratchpad + ledger row only).

Source evidence:
- crates/tools/src/shell_tool.rs:148 `tokio::process::Command::new(&self.command)` — zero authorize refs in file.
- crates/tools/src/mcp_spawn.rs:379 broker.authorize(Process{program,args,cwd}) -> Allow()/Deny/RequireHuman-as-Denied; deny spawns nothing.
- Broker API: crates/security/src/lib.rs:41 OperationIntent::Process{program,args,cwd}, :62 Decision, :154 PermissionBroker (Clone), :205 authorize().
- Frozen constraint: shell_tool.rs tests call `tool.execute(cfg)` where cfg builds ShellConfig literally; `sh -c` fixtures expect success -> broker gate must be opt-in (injected broker), else signature/field changes break frozen tests. Allowlist stays first gate.

Target boundary: authorize-before-spawn inside `execute`, after allowlist check, before Command::new. Deny/RequireHuman -> ShellError::Denied, no process.

Tests: existing shell_tool suite (no edits) + 2 new additive tests (deny-no-spawn, allow-through-broker) in owned file.

Decisions:
- `broker: Option<PermissionBroker>` field + `.broker()` builder; None = legacy allowlist-only (ponytail: new callers must inject broker).
- RequireHuman maps to Denied (fail-closed), mirroring mcp_spawn.rs:386-388.
- cwd for intent: self.cwd or /tmp.

Unknowns: none. Verify cmd: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 110 cargo test -p opencode-rk-tools --lib shell_tool.
