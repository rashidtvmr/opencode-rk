# OpenCode V2 upstream research: core architecture

Source repository: `/home/rashid/projects/tmpcodes/opencode`
Observed commit: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` (2026-09-13 checkpoint).
Local comparison repository: `/home/rashid/projects/opencode-rk`.

## Scope and version qualification

The upstream `v2/` surface is currently a schema/service bridge, not a complete
replacement runtime. `packages/opencode/src/v2/session.ts:9-71` defines the
V2 namespace and `SessionV2.Service`, but `create` and `prompt` are marked not
implemented; `fromID` bridges to the existing session service. The operational
behavior below is therefore the current Effect-based runtime backing the V2
application, not proof that every feature is a V2-native implementation.

## Runtime and lifecycle

- `effect/app-runtime.ts:51-100` composes the application layer from roughly 47
  services. `effect/bootstrap-runtime.ts:15-27` composes the smaller bootstrap
  layer.
- `effect/instance-state.ts:39-84` provides a directory-keyed scoped cache with
  disposer and invalidation. `project/instance.ts:26-192` boots, tracks, reloads,
  disposes, and restores per-directory instances.
- `project/bootstrap.ts:16-34` initializes plugin, LSP, file, watcher, VCS,
  snapshot, format, and share services with owned fork/detach behavior.
- `project/project.ts:18-521` discovers Git roots using `rev-parse`, finds the
  repository root using `rev-list`, upserts project state, and prunes missing
  sandboxes. `project/state.ts:12-70` owns per-directory disposal and warns when
  disposal exceeds its bound.
- `effect/runner.ts:11-207` models Idle, Running, Shell, and ShellThenRun;
  `session/run-state.ts:9-108` rejects concurrent work with `BusyError`, tracks
  cancellation, and removes runners on idle.

## Sessions, messages, and V2 entry model

- `session/index.ts:34-852` exposes session create, fork, list, get, title,
  archive, permission, revert, messages, children, remove, and part operations.
- `session/message-v2.ts:27-947` models user/assistant messages and Text,
  Reasoning, File, Agent, Compaction, Subtask, Retry, StepStart, StepFinish,
  Tool, Snapshot, and Patch parts, with update/remove/part-delta events.
- `v2/session-entry.ts:6-186` defines sortable `ent_` entries: User, Synthetic,
  Request, Text, Reasoning, Tool states, Complete, Retry, and Compaction. The
  V2 `SessionEntryTable` is commented out in `session/session.sql.ts:100-118`,
  so entry persistence is not complete.
- `session/prompt.ts:66-1834` handles text/file/agent/subtask input, shell and
  command modes, abort signals, and loop execution. Default `maxSteps` is
  `Infinity` at `session/prompt.ts:1398`, requiring a deliberate bounded Rust
  equivalent rather than copying the unbounded default.
- `session/processor.ts:24-598` processes tool calls and completion and uses
  `DOOM_LOOP_THRESHOLD=3`. `session/compaction.ts:26-59` protects the newest
  40k tokens, prunes at 20k, and protects skill tool context.

## Storage, synchronization, and events

- `storage/db.ts:30-174` uses Bun SQLite with WAL, NORMAL synchronous mode,
  foreign keys, a 5-second busy timeout, and a 64 MB cache. Schema includes
  session, message, part, todo, permission, project, workspace, event, account,
  and share data.
- `storage/storage.ts:60-330` provides JSON key/value storage with transactional
  re-entrant locks, bounded file operations, migrations, and NotFound handling.
- `sync/index.ts:11-265` defines versioned sync types, sequence allocation,
  replay, projectors, and idempotence. Old versions, sequence gaps, missing
  projectors, and post-freeze definitions fail.
- `bus/index.ts:11-193` provides typed and wildcard event publication. The global
  bus includes directory, project, workspace, and payload metadata.
- `server/instance/event.ts:11-83` streams SSE with a 10-second heartbeat and
  closes on instance disposal. `session/projectors.ts:65-154` projects sync
  events into session/message/part state.

## Server and API

- `server/server.ts:18-109` creates middleware, control-plane, instance, UI,
  OpenAPI, listen, mDNS, and idempotent stop behavior.
- `server/instance/index.ts:29-283` mounts project, PTY, config, session,
  permission, question, provider, file, event, MCP, TUI, command, agent, skill,
  LSP, formatter, workspace, and experimental routes.
- `server/instance/session.ts` exposes session CRUD, status, children, todos,
  init/fork/abort/share/diff/unshare/summarize/messages, prompt, async prompt,
  command, shell, revert, permissions, and part updates.
- `server/instance/middleware.ts:15-126` resolves directory from query/header/CWD,
  maps workspace requests, and proxies remote workspace calls.
- `server/middleware.ts:16-92` handles named errors, basic auth, CORS, logging,
  compression exclusions, and OPTIONS. Unset server password leaves the server
  open with a warning, a behavior requiring an explicit safer Rust policy.

## Configuration, permissions, and safety

- `config/config.ts:865-1062` has strict configuration covering commands, skills,
  plugins, sharing, providers, models, MCP, formatters, LSP, permissions,
  tools, enterprise, compaction, and experimental options.
- `config/paths.ts:11-167` resolves global and project config, walks `.opencode`
  directories, supports JSONC, and substitutes environment/file references.
- `permission/index.ts:19-325` implements deny/allow/ask evaluation, deferred
  human replies, always/reject behavior, same-session rejection cascades, and
  persisted project approval tables. Last matching wildcard rule wins.
- `question/index.ts:10-108` provides deferred question requests and ordered
  answers; dismissal becomes `RejectedError`.
- `project/instance.ts:26-192` enforces `containsPath` for workspace paths.
- `snapshot/index.ts:35-58` tracks, patches, restores, reverts, and diffs Git
  snapshots, pruning after seven days and at 2 MB with per-key semaphore.

## CLI and feature inventory

`index.ts:65-241` registers ACP, MCP, TUI, run, generate, debug, account,
providers, agent, upgrade, uninstall, serve, web, models, stats, export, import,
GitHub, PR, session, plugin, database, and related commands. Other observed
surfaces include PTY (`pty/index.ts:15-362`), worktrees
(`worktree/index.ts:25-594`), agents (`agent/agent.ts:26-401`), providers,
MCP, skills, commands, LSP, formatters, sharing, and workspace adaptors.

## Bounds and failure states

- Bash timeout: 2 minutes, with forced kill after 3 seconds.
- Tool output: 2,000 lines or 50 KB, with spill-to-disk delegation.
- Snapshot retention: 7 days and 2 MB.
- Retry: exponential 2 seconds, capped at 30 seconds and integer overflow safe.
- Compaction: 20k prune threshold, 40k protected tail.
- Errors include Busy, NotFound, ModelNotFound, permission denied/rejected,
  question rejected, runner cancelled, sync version/sequence/projector errors,
  storage migration failure, SSE abort, config validation, and workspace retry.

## Parity candidates for synthesis

Likely gaps requiring explicit comparison: scoped directory services, sync
projectors and replay, typed/global buses and SSE heartbeat, workspace proxy and
sync, deferred permission/question flow, runner busy/shell states, snapshots,
compaction, retry headers, strict JSONC configuration, authentication middleware,
PTY/worktree, and the V2 entry persistence/create/prompt surfaces.

## Checklist

- [x] Runtime layers and directory lifecycle
- [x] Git project discovery and workspaces
- [x] Sessions, messages, parts, prompts, agents
- [x] V2 entry schema and incomplete service bridge
- [x] SQLite/JSON storage and migrations
- [x] Sync, projectors, typed/global buses, SSE
- [x] Server routes, proxy, middleware, auth, CORS
- [x] Config, flags, permissions, questions
- [x] Tools, truncation, bash, snapshots, compaction, retry
- [x] PTY, worktrees, providers, MCP, skills, commands, LSP, formatter
- [x] CLI inventory and bounded failure states
