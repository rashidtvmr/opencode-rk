# OpenCode V2 upstream research: tools, MCP, plugins, skills, permissions

Source: `/home/rashid/projects/tmpcodes/opencode`
Observed commit: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` (2026-09-13).

## Tool registry and built-ins

`packages/opencode/src/tool/tool.ts:9-141` defines tool context, execute results,
definitions, initialization, permission asking, abort signals, metadata, and
output formatting. `tool/registry.ts:51-346` registers built-ins including
invalid, question, bash, read, glob, grep, edit, write, task, fetch, todo,
search, code, skill, patch, LSP, and plan tools. It conditionally exposes
question, LSP, plan mode, web/codesearch, and apply_patch/edit combinations.
Custom tools load from configured `tool`/`tools` directories and plugins.

Tool permission mapping is explicit: read, edit/write, bash, external_directory,
glob, grep, list, webfetch, websearch, codesearch, skill, todo, LSP, task, and
question each ask the relevant permission through `ctx.ask()`. Tool execution
receives an `AbortSignal`; task and bash propagate cancellation.

`tool/truncate.ts:12-60` caps output at 2,000 lines and 50 KB, spills full output
to a seven-day truncation directory, and returns a bounded preview/delegation
hint. Bash parses shell syntax, discovers external paths, streams bounded
metadata, times out by default after two minutes, and force-kills after three
seconds. Read/write/edit preserve file safety, stale-file detection, line
limits, diff metadata, formatting, and LSP/file watcher events.

## MCP lifecycle and transports

`mcp/index.ts:1-892` supports local stdio and remote Streamable HTTP with SSE
fallback. It has a 30-second default timeout, per-server timeout configuration,
owned connect/disconnect/finalizer behavior, descendant cleanup, tool-list
change watching, prompts, resources, and cached tool definitions.

MCP statuses are discriminated as connected, disabled, failed, needs_auth, and
needs_client_registration. Tool names are sanitized. Only connected servers
with cached definitions are exposed by `tools()`. Tool calls reset timeout on
progress and are routed through provider tool transforms and permission asks in
`session/prompt.ts:95,440,974`.

MCP OAuth uses `mcp/auth.ts`, `oauth-provider.ts`, and `oauth-callback.ts`: tokens,
client info, verifier, CSRF state, and server URL are stored under the global
data path; dynamic registration follows RFC 7591; callback defaults to a local
127.0.0.1 endpoint; browser opening failures produce a fallback event.

Routes in `server/instance/mcp.ts:1-245` provide status, add, connect,
disconnect, remove, OAuth start/callback/authenticate, and auth removal. CLI
`cli/cmd/mcp.ts` provides list, add, auth, logout, and debug. TUI
`cli/cmd/tui/component/dialog-mcp.tsx` provides sorted selection and space
toggle between connect/disconnect; sidebar MCP shows status dots and counts.

## Important upstream absence

The observed upstream tree has **no** MCP catalog search, popular-library index,
catalog install flow, bulk selection, separate checkbox and power controls,
lifecycle persistence, or invocation-time disabled-payload recheck. Disabled
servers are excluded by connection/state layers, but local REQ-042 remains a
newer and stronger requested contract: filter before payload construction and
again before invocation.

## Plugins, skills, commands, and permissions

`plugin/index.ts`, `loader.ts`, `install.ts`, and `meta.ts` load internal and npm/
file plugins, patch project/global plugin config, track metadata with locks, and
run sequential hooks. Plugin hooks include config, auth, provider, chat,
permission, command, shell environment, tool before/after, and tool-definition
hooks. Plugins have broad Bun/Node access, so local Rust must preserve explicit
capabilities rather than equating plugin text with a sandbox.

`skill/index.ts:1-264` discovers skills from project ancestors, config paths,
external Claude/agents directories, and configured URLs. It requires SKILL.md,
uses bounded download concurrency, caches files, warns on duplicate names, and
filters by permissions. Content is injected as untrusted prompt material; there
is no signature/catalog pin in the observed implementation.

`command/index.ts:1-191` merges built-in/config/MCP/skill commands with template
arguments and emits execution events. `permission/index.ts:1-325` evaluates
last-match rules, defers asks to human replies, supports always/reject/cascade,
and persists project approvals. `permission/arity.ts` applies longest-prefix
bash patterns. LSP and formatter modules discover servers, launch bounded
processes, provide nine LSP operations, and run configured formatters.

## Discovery-to-invocation data flow

Configured directories and plugins discover tools -> ToolRegistry resolves
definitions -> MCP clients contribute connected cached definitions -> provider
transforms and plugin definition hooks modify schemas -> session prompt builds
the provider payload -> permission broker asks before execution -> tool executes
with cancellation -> output is truncated/spilled -> bus events update UI.

## Resource and security observations

The observed upstream has strong tool output and process timeout controls, but
MCP startup/tool collection can be concurrent without the bounded guarantees
required by this Rust repository. MCP output must not be treated as bounded by
prompt truncation alone. Prompt text cannot be a sandbox. Catalog data must be
untrusted, install must be consent-gated, and disabled state must be enforced at
both payload and invocation boundaries.

## Checklist

- [x] Built-in tool registry and conditional tools
- [x] Tool permissions, abort, metadata, and output truncation
- [x] Bash parsing, external-directory checks, timeout and kill
- [x] MCP stdio/HTTP/SSE, status, tools, prompts, resources, OAuth, routes
- [x] MCP CLI/TUI connect/disconnect/status surfaces
- [x] Plugins, loading, npm/file install, hooks, metadata
- [x] Skills discovery, cache, URL sources, permission filtering
- [x] Commands, permission broker, bash arity, LSP, formatter
- [x] MCP catalog/search/install/bulk/power persistence checked: absent upstream
- [x] Disabled MCP payload/invocation recheck checked: absent upstream
