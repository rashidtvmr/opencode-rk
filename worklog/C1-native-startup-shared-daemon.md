# C1 native startup/shared daemon source candidate

- Base: `15db75ec0fe88790398f2ff19e9cd8760a5d9a5c`
- Scope: native default startup/provider setup and shared per-data-directory daemon reuse.
- Owned source paths: `crates/cli/src/main.rs`, `chat.rs`, `tui_entry.rs`, `daemon_client.rs`.
- Provider presence now uses the existing bounded `persisted_auth::api_key` discovery API and only propagates a boolean; model metadata is not treated as credentials.
- The typed default-launch `StartupView::Setup` is propagated to the native first frame, opening the existing `Connect a provider` dialog. Explicit `tui` remains unchanged.
- Auto-started daemon children are service-owned: client lease drop only hands the exact spawned `Child` to a reaper and never kills a descriptor-discovered/foreign process. Startup failure still kills only the child actually spawned. Readiness remains bounded to 200 probes at 10ms.
- Upstream evidence inspected at pinned reference: `packages/opencode/src/cli/cmd/tui.ts` default TUI command and V2 `packages/opencode/src/cli/cmd/run/runtime*.ts` lifecycle/runtime surfaces; legacy worker lifetime was not used as authority.
- Status: source-ready candidate only. No Cargo/runtime/PTY/server verification was run under this lease; not GREEN, PREVERIFIED, or ACCEPTED. Remaining uncertainty is exact integrated-runtime behavior of service ownership and native frame/auth setup.
