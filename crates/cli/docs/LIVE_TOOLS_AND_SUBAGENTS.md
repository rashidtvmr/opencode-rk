# Native tools and child sessions

The native client uses the daemon's streaming turn engine for local tools,
public-web tools and child sessions. Every effect crosses the permission broker.
Use **Up/Down** to choose a permission action, **Enter** to submit it, and
**Escape** to deny it. Merely opening or navigating a permission dialog never
approves an operation.

## Tools

Default turn tools are `bash`, `shell`, `read`, `write`, `edit`, `grep`, `glob`,
`websearch`, `webfetch`, and `task`. `OPENCODE_RK_TURN_TOOLS` can narrow this list;
it does not grant permissions.

- `websearch` uses the pinned upstream public Exa MCP transport. It accepts
  `query`, `numResults` (1–10), `type` (`auto`, `fast`, `deep`), `livecrawl`
  (`fallback`, `preferred`), and `contextMaxCharacters` (1–16384).
- `webfetch` accepts a public HTTP(S) `url` and `format` (`text` or `html`).
  Text mode extracts readable HTML text; HTML mode retains the original body.
  Markdown conversion and image attachments are not supported by this native
  adapter. Requests have a 30-second overall limit, a 512-KiB response limit,
  and a visibly truncated 16-KiB output limit. Credentials in URLs, private
  addresses, proxy inheritance and redirects are refused. DNS answers are
  checked and pinned for the request.
- Network access and task admission require an explicit permission decision.
  Child tool approvals remain bound to the child's run and exact operation.

## Subagents

Ask the model to delegate with `task`, providing `description`, `prompt`, and
`subagent_type`. Built-in child types include `general`, `explore`, `build`,
and `plan`; configured agents can also be used. `explore` and `plan` advertise
only read-only file and public-web tools. Children cannot delegate recursively.
The child inherits the parent model unless its agent configuration selects one.
Agent-specific permission overrides are explicitly rejected.

`background: true` returns the child session identity immediately. Foreground
tasks wait for completion and are cancelled with their parent. `task_id` sends a
new prompt to an existing child belonging to that same parent.

Open **View subagents** from Ctrl+P, or enter `/subagents`:

| Key | Action |
| --- | --- |
| Up / Down | Select a child; scroll its history in detail view |
| Enter | Open the selected child's typed history and state |
| s | Stop the selected child and wait for execution cleanup |
| r | Retry a failed/cancelled child only if no tool was admitted |
| c | Compose an explicit follow-up in detail view |
| Enter while composing | Send the new follow-up to that child |
| Escape | Cancel follow-up, return to roster, or close panel |

The roster refreshes while open. Pending child approvals appear in the parent
client, including for background children while the panel is open. Retry never
silently replays a tool-bearing or uncertain effect; inspect history and send an
explicit follow-up instead.

The daemon retains at most 128 child records, refusing new records at capacity
without deleting history. The existing global two-turn execution limit also
applies to children. Capacity-only refusals wait up to 60 seconds before a child
turn begins; no dispatched provider request is automatically replayed.

Child transcripts are durable regular sessions. The parent/child roster and
control records currently belong to the running daemon lifetime; they are not
reconstructed after a daemon restart. Existing transcripts remain available in
the session picker and through explicit `--session ID` resume.
