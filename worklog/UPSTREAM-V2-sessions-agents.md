# OpenCode V2 upstream research: sessions and agents

Source: `/home/rashid/projects/tmpcodes/opencode`
Observed commit: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` (2026-09-13).
The plan pins `95daf90670b7c039c436c85537da5fbfe2205b41`; differences remain to
be verified. This scratchpad records the observed checkout only.

## Session model and persistence

- `session/schema.ts:7-35` defines descending `ses_` session IDs and ascending
  message/part IDs.
- `session/index.ts:120-180` defines session metadata: project/workspace,
  parent, summary, share URL, title, version, timestamps, permissions, and
  revert state. `:313-362` exposes create, fork, touch, title/archive,
  permission/revert/summary, diff, messages, children, removal, and part APIs.
- `session/session.sql.ts:16-97` persists sessions, messages, parts, todos, and
  project permissions with foreign-key cascades and indexes.
- `session/index.ts:501-552` creates sessions with default titles and forks by
  cloning messages, remapping assistant parent IDs, and optionally cutting at a
  message ID. `:434-458` recursively removes children.
- `session/index.ts:601-837` supports paged messages, cursor/global listing,
  directory/workspace filtering, and newest-first ordering.
- `session/index.ts:240-305` calculates input/output/reasoning/cache usage and
  cost from provider metadata and pricing.

## Messages and parts

`session/message-v2.ts:90-353` defines snapshot, patch, text, reasoning, file,
agent, compaction, subtask, retry, step-start, step-finish, and tool parts.
Tool states at `:276-342` are pending, running, completed, and error, with
bounded metadata/attachments expected by the caller.

User and assistant messages at `:360-452` carry model/provider, agent, path,
summary, cost, tokens, structured output, finish, and typed errors. Events at
`:460-509` distinguish persisted message/part updates and removals from
ephemeral part deltas. `:585+` converts stored messages to provider model
messages, skips empty/ignored content, handles media by provider, and removes
compacted attachments. `:929-948` filters compacted context.

Projectors in `session/projectors.ts` persist creates/updates/deletes and tolerate
late foreign-key updates with warnings. This persisted-versus-ephemeral event
split is a key parity invariant.

## Prompt loop and execution

- `session/prompt.ts:70-77` exposes cancel, prompt, loop, shell, command, and
  prompt-part resolution.
- `:1708-1772` accepts session/message/model/agent/format/system/variant and
  text/file/agent/subtask parts.
- `:1273-1529` creates user messages, filters compacted context, resolves models,
  runs subtask and compaction steps, inserts reminders, creates assistant/tool
  parts, processes responses, handles structured output, and decides continue,
  compact, or stop. It uses agent step limits but defaults to `Infinity` at
  `:1398`, which must not be copied into a Rust unbounded execution path.
- `:1531-1657` separates normal loop, shell, and command execution. Commands
  expand `$1..$N` and `$ARGUMENTS`, can execute shell templates, run plugin hooks,
  and publish command events.
- `:119-151` resolves `@file` mentions to files/directories or agent mentions.
- `:153-213` generates a title only for the first real user message, using an
  agent or small model and a 100-character limit.

## Subtasks, agents, and cancellation

- `agent/agent.ts:27-52` defines agent mode, model, variant, prompt, permissions,
  options, hidden/native flags, color, and steps. Built-ins at `:107-234`
  include build, plan, general, explore, compaction, title, and summary. Config
  merging is at `:236-309`.
- `tool/task.ts:21-176` checks permission, resolves a named agent, creates a
  child session with parent ID, scopes child permissions/tools, inherits model
  where appropriate, and forwards abort signals. It returns a task ID and last
  text. Resume uses the task ID. There is no background flag: delegation is a
  synchronous Effect operation with cancellation.
- `session/prompt.ts:521-711` handles in-loop subtasks with running tool parts,
  metadata updates, abort controllers, and completed/error parts.
- `:713-887` handles shell execution with owned child processes, live metadata,
  abort handling, and a separate shell runner that can run concurrently with a
  normal session runner.
- `run-state.ts:9-108` has per-instance runner maps, BusyError for duplicate
  prompts, cancellation, idle cleanup, and finalizer cancellation on disposal.
- `status.ts:8-88` exposes idle, retry, and busy status. `retry.ts:6-122`
  honors retry-after headers/dates and uses bounded exponential backoff.

## Compaction, overflow, revert, and summary

- `compaction.ts:35-369` uses 20k prune and 40k protect thresholds, protects
  skill context, supports auto/overflow compaction, creates summary messages,
  and publishes compacted events.
- `overflow.ts` reserves output budget, supports explicit auto-compaction disable,
  and detects usable context exhaustion.
- `summary.ts:68-175` computes Git diffs between step snapshots, stores summary
  totals/diffs, and publishes session diff events.
- `revert.ts:16-177` restores snapshots, removes later messages/parts, stores
  pending revert state, and supports unrevert. Prompt/shell cleanup applies a
  pending revert before new execution.

## Events, routes, and related features

- `bus/index.ts:11-193` publishes typed/wildcard instance events and forwards
  global events. `sync/index.ts:11-243` persists versioned aggregate events,
  applies projectors, allocates sequences, and supports replay.
- `server/instance/session.ts` routes list/status/get/children/todo/create/delete,
  update/init/fork/abort/share/diff/unshare/summarize/messages, prompt,
  prompt_async, command, shell, revert/unrevert, and permission response.
- Todos replace all ordered entries transactionally and publish todo updates.
- Worktree creation in `worktree/index.ts:30-242+` uses Git worktree add,
  generated branches, owned data paths, and ready/failed events.

## Verified checklist and unresolved scope

- [x] Session CRUD, fork, children, archive, title, permissions, revert, summary
- [x] Messages/parts, tool states, usage/cost, persisted and ephemeral events
- [x] Prompt/loop/shell/command/subtask/task delegation and cancellation
- [x] Busy state, retries, compaction, overflow, prune, snapshot summary
- [x] Sync/projectors, session routes, todo, share route, worktree entry
- [ ] Full attachment lifecycle, queue/steer/stop semantics, export/share-next
- [ ] Full LLM/processor/instruction/system wiring and usage API
- [ ] Full SSE/TUI event bridge and CLI run behavior
- [ ] Upstream tests and file-level Rust parity comparison

## Rust parity candidates

Compare `crates/sessions`, `crates/agents`, and `crates/server` for: session
fork/remap, message/part persistence, task child lifecycle, BusyError and
runner ownership, retry-after handling, compaction/prune/revert, sync sequence
projectors, persisted versus ephemeral events, and bounded cancellation. Do not
claim parity until the unresolved files and tests are inspected.
