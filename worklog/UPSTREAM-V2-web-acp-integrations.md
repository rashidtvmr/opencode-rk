# OpenCode V2 upstream research: web, ACP, SDK, sync, and integrations

Source: `/home/rashid/projects/tmpcodes/opencode`
Observed checkout: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb`.
Pinned plan revision differs: `95daf90670b7c039c436c85537da5fbfe2205b41`.
All observed-head claims require pinned-blob reconciliation before parity claims.

## Server, control plane, and remote workspace

`server/server.ts:1-96` creates Hono/Bun/Node adapters, control/instance/UI
routes, OpenAPI, mDNS, and idempotent shutdown. Middleware provides error
mapping, optional Basic auth and auth-token handling, CORS, compression exclusions,
logging, and OPTIONS bypass. `server/control/index.ts` exposes provider auth,
OpenAPI docs, and logging; `server/instance/global.ts` exposes health, events,
sync events, config, dispose, and upgrade.

`server/instance/index.ts` mounts project, VCS, file, PTY, session, provider,
MCP, permission, question, LSP, formatter, TUI, command, agent, skill, and
workspace routes. `server/proxy.ts` forwards HTTP/WebSocket workspaces, strips
hop-by-hop and routing headers, queues until the connection opens, and bridges
`/__workspace_ws`.

`control-plane/workspace.ts` maintains local/remote workspace state and an SSE
sync loop with connected/connecting/disconnected/error statuses. Middleware
resolves directory from query/header/CWD and routes local session/status calls
versus remote workspace calls.

## SSE, sync, and persistence

Instance and global event endpoints use bounded connection queues, a 10-second
heartbeat, and disposal closure. `sync/index.ts` defines versioned event types,
projectors, sequence allocation, replay, idempotence, and event-log gating behind
the experimental workspace flag. ShareNext persists session/message/part/model
data remotely with create/sync/remove APIs and a local session-share table.

## ACP and headless execution

`acp/agent.ts`, `session.ts`, `types.ts`, and `cli/cmd/acp.ts` implement an Agent
Client Protocol v1 JSON-lines server over stdin/stdout. It supports initialize,
session new/load/prompt, modes/models/variants, text-file read/write, and an
embedded SDK/server. The README records limitations: no complete session/update
stream, tool-call reporting, mode switch, auth, real terminal support, or full
history restoration.

`cli/cmd/run.ts` provides headless execution with inline/block renderers and
attachment conversion. `cli/cmd/export.ts` exports session JSON with an
interactive picker. `web.ts` and `serve.ts` expose network options, browser open,
password warning, mDNS, and local/network address reporting.

## SDK and client surfaces

JavaScript SDK files under `packages/sdk/js/src` provide generated API clients,
server/process/TUI spawners, directory header/query rewriting, and abort/timeout
behavior. The plugin API exposes client, project, worktree, server URL, shell,
hooks, tools, providers, and workspace adaptor registration.

## IDE, desktop, GitHub, and telemetry

IDE integration supports Windsurf, Code, Cursor, Codium, and VS Code extension
commands/keybindings with an ephemeral port handshake and `OPENCODE_CALLER`.
Desktop shells include Tauri/Electron and the web app includes session/home,
directory, prompt, provider, terminal, context, i18n, and layout components.
GitHub integration uses Octokit, webhooks, workflows, session sharing, and an
action. OTLP telemetry is enabled only when configured, with service/client
metadata and bounded user-agent/header fields.

## Security and bounds

Auth middleware is open with a warning when no server password is configured;
local Rust must decide whether to preserve or strengthen this behavior. CORS
allows localhost/loopback, Tauri, opencode.ai, and configured origins. Embedded
web UI uses CSP hashes. SSE queues close on instance disposal; proxy headers are
stripped; prompt streaming is uncompressed. Port zero supports ephemeral local
binding, and shutdown is idempotent.

## Local parity gaps and uncertainty

Local Rust has server/event/share/web scaffolds, but each must be verified for
real behavior rather than compilation. Likely gaps include ACP JSON-RPC stdio,
mDNS, HTTP/WebSocket workspace proxy, remote sync, ShareNext HTTP sync, CSP-safe
embedded UI, SDK client/server/TUI spawners, IDE/VS Code, desktop shells, GitHub
automation, OTLP, export picker, and headless pretty rendering.

Several older local task cards cite paths not present in the observed upstream
tree (`packages/server`, `packages/core`, `packages/protocol`, or
`packages/client`). Those references may target the pinned revision or stale
layout and must be reconciled before implementation.

## Complete checklist

- [x] Hono server/adapters/OpenAPI/middleware/auth/CORS/CSP/compression
- [x] Control and instance route inventory
- [x] Workspace router, remote HTTP/WebSocket proxy, and sync loop
- [x] Instance/global SSE and heartbeat/disposal lifecycle
- [x] ACP JSON-lines protocol and known limitations
- [x] Headless run, web/serve, export
- [x] ShareNext and session sharing
- [x] SDK clients/server/process/TUI spawners
- [x] Plugin API and workspace adaptors
- [x] IDE/VS Code, desktop, GitHub, OTLP surfaces
- [x] Persistence and feature flags
- [x] Pinned-versus-observed commit drift recorded
- [ ] Pinned revision blob-by-blob reconciliation
