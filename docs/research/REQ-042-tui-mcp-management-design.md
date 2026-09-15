# REQ-042 Detailed Design: TUI Information Panel and MCP Management

**Status:** Planned, not implemented

**Research scope:** This document turns the requested TUI and MCP behavior into an
explicit product and engineering contract. It is more detailed than the short
backlog summaries in `ralph.json` and `FEATURES.md`.

## 1. User experience summary

The TUI will use a split layout when the terminal is wide enough:

```text
+-------------------------------+--------------------------+
| Conversation / MCP list       | Information panel        |
|                               |                          |
|                               | Directory                |
|                               | Workspace / session      |
|                               | Provider / model         |
|                               | Context and tokens       |
|                               | MCP health and tools     |
|                               | Queue and activity       |
|                               | Warnings                 |
+-------------------------------+--------------------------+
```

The right panel is informational. It must not become a second uncontrolled
execution path. Actions such as installing, removing, reconnecting, or changing
an MCP state are explicit commands routed through the existing permission and
confirmation systems.

When the terminal is too narrow, the panel changes to a compact single-column
view or a status-line summary. It must not force horizontal scrolling or reduce
the conversation area below its minimum usable width.

## 2. Right-side information panel: UI-019

### 2.1 Displayed information

The panel displays the following groups:

#### Location

- Current working directory.
- Workspace or project label.
- Whether the directory is inside the configured project root.
- A shortened path when the full path exceeds the available width.
- The path is display data only. It does not grant filesystem permission.

#### Session

- Session title or stable short identifier.
- Session state: idle, running, waiting for approval, paused, failed, or
  cancelled.
- Current turn number when available.
- Last update time supplied by the owning session service.

#### Provider and model

- Provider identifier.
- Model identifier.
- Auth mode, such as API key, OAuth, imported credential, or unavailable.
- Account label only when it is already a safe non-secret display label.
- Provider health or last request state.

No access token, refresh token, API key, authorization value, cookie, or raw
credential file content may enter the snapshot or renderer.

#### Context and usage

- Context window size.
- Context tokens used.
- Context tokens remaining.
- Input token count.
- Output token count.
- Compression or summarization state when active.
- Cost when the provider reports it, otherwise `unavailable`.

Remaining context uses saturating subtraction. If used exceeds the reported
window, remaining is zero and a warning is shown. It must never wrap around.

#### MCP health

- Total configured MCP servers.
- Connected servers.
- Starting servers.
- Ready servers.
- Disabled servers.
- Error servers.
- Enabled tools.
- Active tool invocations.
- Last MCP error code or safe summary.
- Last known latency when available.

The panel shows counts and safe labels, not command lines, environment values,
authorization headers, process output, or server secrets.

#### Queue and activity

- Pending user turns.
- Running agent turn.
- Running tool count.
- Pending approval count.
- Cancellation state.
- Bounded recent activity labels.

#### Warnings

Warnings are short, categorized, and bounded. Examples include:

- `Context nearly full`
- `MCP server unavailable`
- `MCP tools disabled`
- `Provider rate limit reported`
- `Some metadata unavailable`

Raw stack traces and unbounded provider or MCP errors are not rendered in the
panel. They remain available only through an appropriately redacted diagnostic
surface.

### 2.2 Snapshot data model

The renderer consumes a caller-owned immutable snapshot similar to:

```text
InfoSnapshot {
  cwd_label,
  workspace,
  session_label,
  session_state,
  provider_id,
  model,
  auth_mode,
  context_window,
  context_used,
  input_tokens,
  output_tokens,
  cost_micros,
  mcp_total,
  mcp_connected,
  mcp_starting,
  mcp_ready,
  mcp_disabled,
  mcp_errors,
  enabled_tools,
  active_tools,
  queue_depth,
  approval_count,
  status_line,
  updated_at,
  warnings
}
```

All collections have hard limits. The renderer does not read the filesystem,
environment, database, network, clock, or process table. Services collect and
sanitize data before constructing the snapshot.

### 2.3 Layout behavior

- Width 100 or greater: two-column panel with labels and values.
- Width 60 to 99: compressed right panel with abbreviated labels.
- Width 20 to 59: compact single-column status view.
- Width below 20: one bounded status line or a `terminal too narrow` marker.
- Every emitted line is at most the requested width.
- Long paths, model names, tool names, and warnings are ellipsized.
- Truncation is reported internally and represented by a `+N more` marker.
- Re-rendering the same snapshot at the same width is deterministic.

## 3. MCP catalog search: TOOL-016

### 3.1 Search placement

The MCP screen starts with a focused search field above the configured MCP list.
The search field can search the local catalog without leaving the TUI. Search
results are visually separated from installed/configured servers.

Suggested key behavior:

- `/` or the MCP search shortcut focuses the search field.
- Printable characters update the local search query.
- `Esc` clears focus or closes the result view.
- `Enter` opens the selected catalog entry.
- Arrow keys move through bounded results.
- `Tab` moves between search, results, and configured MCP list.

The exact key binding remains subject to the existing TUI keymap, but every
action must also be reachable without a mouse.

### 3.2 Catalog contents

The catalog is a versioned, source-attributed data file. Each entry contains:

- Stable catalog identifier.
- Display name.
- Short description.
- Tags.
- Provider or ecosystem label.
- Source URL or source identifier.
- Maintainer or publisher label when available.
- Supported transport or capability metadata.
- Compatibility notes.
- Installation metadata that is declarative, not executable.

The catalog does not contain credentials, environment values, shell command
strings, arbitrary scripts, or opaque executable payloads.

### 3.3 Search semantics

- Case-insensitive matching.
- Match fields: name, description, tags, provider, and publisher.
- Optional tag and provider filters.
- Deterministic ranking: exact name match, prefix match, token match, then
  description or tag match.
- Alphabetical tie-breaker.
- Maximum 512 catalog entries.
- Maximum 256 query characters.
- Maximum 50 displayed results, with an honest truncation indicator.
- Empty query with no active filter is rejected rather than scanning endlessly.
- Invalid or over-cap catalog files fail closed.

### 3.4 Opening and installing a result

Selecting a result opens a detail view showing source, publisher, capabilities,
transport, required permissions, and compatibility notes. Installation is a
separate explicit action.

The install flow must:

1. Show the source and requested permissions.
2. Show whether installation changes configuration, downloads files, starts a
   process, or requires network access.
3. Require explicit confirmation.
4. Route filesystem, process, network, and secret access through the permission
   broker.
5. Validate the resolved manifest before writing configuration.
6. Use bounded downloads and bounded retained output.
7. Report success, partial completion, or failure without claiming installation
   when only metadata was saved.

Catalog search itself never starts a process or performs network installation.

## 4. MCP selection and bulk actions: TOOL-017

### 4.1 Selection model

Each configured MCP row has a selection indicator separate from its power state.
Selection means the row is included in the next bulk action. It does not itself
enable a server.

Supported actions:

- Select one row.
- Clear one row.
- Select all visible rows.
- Clear all.
- Invert selection.
- Select by search result.
- Select only enabled rows.
- Select only error rows.

Selection is bounded to a hard maximum, currently proposed as 64 servers. If the
limit is reached, the TUI shows a clear error and does not silently replace an
existing selection.

### 4.2 Bulk actions

Bulk actions include:

- Enable selected servers.
- Disable selected servers.
- Reconnect selected servers.
- Refresh tool discovery for selected servers.
- Remove selected configuration.
- Install selected catalog entries.

Enable, disable, reconnect, and refresh produce per-item results. Removal and
installation require confirmation and show the complete bounded impact summary.

Partial failure is represented explicitly. For example, three servers may be
enabled while one fails due to a permission denial. The result must identify
success, failure code, and unchanged state for each item.

No action may claim success merely because a request was queued. The UI must
distinguish accepted, running, completed, and failed states.

## 5. Per-MCP power toggle and lifecycle: TOOL-018

### 5.1 Row controls

Each configured MCP row contains:

- A checkbox for bulk selection.
- A power control for enable/disable.
- Server name and safe description.
- Lifecycle state.
- Tool count.
- Last error code or safe summary.

The checkbox and power control are separate controls. Selecting a row must not
toggle it, and toggling it must not alter selection.

The power control must have a textual accessible label in addition to its visual
icon. The icon is not the only indication of state.

### 5.2 Lifecycle states

```text
Disabled
Starting
Ready
Error { safe_code }
Stopping
```

The persisted state is explicit and bounded. A disabled server is not started at
application launch and is not reconnected by a background task. Re-enable is an
explicit user action or an explicitly configured, permission-approved workflow.

### 5.3 State transitions

```text
Disabled --enable--> Starting
Starting --initialized--> Ready
Starting --failure--> Error
Ready --disable--> Disabled
Ready --connection loss--> Error
Error --retry/reconnect--> Starting
Error --disable--> Disabled
```

State transitions are serialized per MCP server. Duplicate enable or disable
requests are idempotent and do not create duplicate processes or registrations.

## 6. Disabled MCP payload filtering: TOOL-019

This is a mandatory security and correctness boundary.

Before an agent message payload is built, the system creates a snapshot of MCP
servers whose lifecycle state is eligible for use. Only `Ready` servers and their
currently registered tools may contribute tool definitions.

Disabled, starting, stopped, and error servers are excluded from:

- Tool definitions sent to the provider.
- Tool name lookup.
- Tool invocation dispatch.
- Tool result routing.
- Active tool counts.
- Advertised capabilities for the current turn.

Filtering occurs again at invocation time. This second check prevents a stale
message payload or race from invoking a server that was disabled after payload
construction.

An invocation against a disabled or stale tool returns a deterministic
`ToolUnavailable` or `McpDisabled` result and does not start or wake the server.

The filtered payload is bounded by the existing tool and context budgets. The
filter does not mutate the authoritative MCP registry and does not silently
re-enable anything.

## 7. MCP status event integration: TOOL-020

MCP state changes publish bounded status events through the existing event bus.
Events carry safe metadata only:

- Server identifier.
- New lifecycle state.
- Tool count.
- Safe error code.
- Latency when available.
- Event timestamp supplied by the publisher.

Events must not contain command lines, environment variables, credentials,
authorization values, raw process output, or unbounded tool schemas.

The TUI consumes events without blocking the active agent turn. If the event bus
is full, the UI may coalesce repeated state updates and retain the newest status,
but it must show stale metadata or a dropped-update warning honestly.

The panel refreshes after:

- MCP enable or disable.
- Server start, ready, stop, or error.
- Tool discovery refresh.
- Installation completion.
- Removal completion.
- Payload filtering snapshot change.

## 8. Persistence and restart behavior

Persist only declarative safe state:

- MCP identifier.
- Enabled or disabled preference.
- Display metadata.
- Catalog version and source reference.
- User-selected transport/configuration references where permitted.

Do not persist runtime process handles, raw output, access tokens, arbitrary
environment values, or secret-bearing command arguments.

On restart:

- Disabled MCPs remain disabled.
- Enabled MCP preferences may be restored, but startup still passes through the
  permission and lifecycle broker.
- A persisted `Ready` state is restored as `Starting` or `Disabled`, never as a
  false live connection.
- Stale catalog entries are shown as requiring review, not silently upgraded.

Writes are bounded and atomic. A failed write leaves the previous valid state
intact.

## 9. Security and resource requirements

The implementation must preserve the repository security policy:

- MCP commands and environments remain capability-controlled.
- Catalog content is untrusted data and cannot change policy.
- Search does not execute catalog content.
- Install requires explicit user consent.
- Disabled state cannot be bypassed by a model-generated tool call.
- Permission `*` does not bypass mandatory controls.
- No unbounded queue, result list, event history, or retained process output.
- No detached MCP process without an owned lifecycle and cancellation path.
- No secret values in TUI snapshots, events, logs, errors, or transcripts.

Relevant current evidence includes `crates/tools/src/mcp.rs:13-24` for MCP
process configuration and timeouts, `crates/tools/src/mcp.rs:73-98` for bounded
error categories, `crates/tools/src/mcp.rs:145-179` for discovered tool data,
`crates/sessions/src/ui_006.rs:13-41` for the existing bounded status-panel
precedent, and `docs/SECURITY.md:14-32` for brokered permissions and secret
handling.

## 10. Test plan by feature

### UI-019

1. Full-width panel renders every required metadata group.
2. Narrow layout keeps every line within the terminal width.
3. Tool and warning lists truncate with honest counts.
4. Context counters saturate instead of overflowing.
5. Rendering is deterministic and contains no credential-like material.

### TOOL-016

1. Name, tag, provider, and combined searches return deterministic results.
2. Empty, malformed, and over-cap indexes fail closed.
3. Results are capped and report truncation.
4. Catalog entries contain source attribution.
5. Search performs no network, process, or install operation.

### TOOL-017

1. Individual and keyboard-driven selection works.
2. Select-all, clear, and invert obey the selection cap.
3. Bulk actions report each item independently.
4. Destructive actions require confirmation.
5. Partial failures preserve and report unchanged item state.

### TOOL-018

1. Power toggle transitions through the lifecycle state machine.
2. Duplicate transitions are idempotent.
3. Disabled servers are not started or reconnected.
4. Persisted state round-trips without secrets.
5. Failure and reconnect behavior remains bounded and cancellable.

### TOOL-019

1. Ready MCP tools are included in a payload snapshot.
2. Disabled MCP tools are absent from payloads.
3. Disabled MCP tools are rejected during invocation lookup.
4. A disable race after payload construction cannot invoke the server.
5. Filtering is deterministic and does not mutate authoritative configuration.

### TOOL-020

1. Lifecycle changes update the panel through bounded events.
2. Counts and active-tool metadata remain accurate after toggles.
3. Event-bus saturation coalesces or reports stale state honestly.
4. Events never block an active agent turn.
5. Events and rendered status contain no secret or raw process data.

## 11. Implementation order

1. Implement and freeze the pure TUI snapshot renderer.
2. Implement catalog schema loading and deterministic search.
3. Implement MCP lifecycle and persistence state.
4. Implement selection and bulk-action planning.
5. Implement payload filtering and the invocation-time safety check.
6. Wire lifecycle events into the panel.
7. Add interactive TUI bindings and confirmation dialogs.
8. Run focused RED/GREEN tests, then the relevant crate checks.

This order keeps pure, testable contracts ahead of process orchestration and UI
wiring. It also ensures the disabled-MCP payload boundary exists before adding
convenience controls that could otherwise create stale state.
