# ChatGPT-class web client parity

Status: user-directed product feature. Research and decomposition completed before
implementation. This document is product scope, not release or acceptance evidence.

## User outcome

OpenCode RK's browser client should feel like a complete modern AI coding client,
not a static chat shell. It must use the same per-user/data-directory native daemon
as every other application client, preserve real coding-agent semantics, and expose
rich chat controls without fabricating capabilities that the native runtime does not
yet implement.

The target is **ChatGPT-class chat-surface parity for a local coding agent**. It is
not a mandate to clone unrelated vertical products such as shopping, health or
financial services.

## Research basis (2026-09-15)

Current OpenAI product documentation was reviewed before this feature was sliced:

- ChatGPT Capabilities Overview: https://help.openai.com/en/articles/9260256
  documents Search, deep research, image input, file uploads, data analysis, voice,
  interactive editing, memory, projects, scheduled tasks and specialized assistants.
- ChatGPT release notes: https://help.openai.com/en/articles/6825453-chatgpt-release-notes
  document composer model/reasoning controls, plugins, Library and connected files,
  progressive long-chat loading, project memory, dictation, pinned chats, branching,
  advanced voice, temporary-chat controls, and source-linked file previews.
- The September 4, 2025 ChatGPT release note documents the concrete web branching
  interaction: hover a message, open More actions, then choose **Branch in new chat**
  to start a separate conversation from that point while keeping the original thread.
- Projects in ChatGPT documents that branched chats remain alongside the original
  project conversation. OpenCode RK should therefore preserve project/workspace scope
  when WEB-015 supplies project ownership instead of silently moving a branch global.
- Working with writing blocks and code blocks:
  https://help.openai.com/en/articles/20001246 documents editable/copyable text and
  code surfaces, preview/run affordances where supported, and side-by-side editing.
- Apps in ChatGPT: https://help.openai.com/en/articles/11487775-connectors-in-chatgpt
  documents discoverable/installable app/plugin capabilities, connected data,
  interactive UI, permissioned actions and deep-research use.
- Deep research in ChatGPT: https://help.openai.com/en/articles/10500283-research-faq
  documents a reviewable research plan, source selection, progress, steering and a
  cited final result.
- Chat history search: https://help.openai.com/en/articles/10056348-how-do-i-search-my-chat-history-in-the-chatgpt
  documents keyboard-accessible chat search across retained conversation content.
- The OpenAI Responses API documentation was reviewed for actual transport support:
  reasoning summaries, function/tool events, web/file/MCP tools, files/images and
  streaming events are provider capabilities. OpenCode RK must still implement and
  persist its own native adapters rather than merely drawing matching controls.

## Safety and truthfulness rules

1. **No raw hidden chain-of-thought UI.** A collapsed "Reasoning" section may show
   only a provider-supplied reasoning summary, explicit model-visible progress, or
   OpenCode RK's own auditable execution/activity records. Hidden model chain of
   thought is neither requested from providers nor reconstructed from internal state.
2. **No fake tools, plugins, references or attachments.** A control is enabled only
   when the daemon exposes the corresponding real capability. Unsupported features
   render an honest disabled/unavailable state until their native adapter lands.
3. Provider credentials stay in the native daemon. Browser code never receives
   secrets such as `OPENAI_API_KEY`.
4. Destructive tool actions remain subject to native permission and mandatory-human
   approval policy. The browser is a client, not an authority bypass.
5. Every stream, upload, search and tool surface must be bounded and cancellable.
6. WCAG 2.2 AA is the quality target. Automated axe coverage is necessary but not
   sufficient for a formal conformance claim; keyboard, zoom and screen-reader
   validation remain required.

## Architecture invariant: one daemon, many clients

`PLAN.md` ADR-001/ADR-002 is authoritative: one Tokio runtime per daemon and one
singleton daemon per OS user + data directory. CLI, native TUI, web and desktop are
clients of that runtime.

At research time, the source did **not** satisfy that invariant:

- `crates/server/src/daemon.rs` has a PID lock and Unix listener, but no production
  caller and no application protocol; accepted connections currently only discard
  bytes.
- `crates/cli/src/main.rs::serve` independently opens sessions/catalog and binds an
  HTTP server on `127.0.0.1:4096`.
- the repository currently has TUI state machines but no interactive TUI binary.
- the web README required a separately started `opencode-rk serve`.

WEB-006 closes that gap instead of creating a second web-only daemon.
The first client may ensure/start the singleton runtime, but all later clients attach
to the same authority and data state. Production web assets and `/api/*` are served
by that daemon; Vite remains development tooling only.

## Capability synthesis

| Product capability | Existing native/web support | Target contract |
| --- | --- | --- |
| Shared runtime | Singleton primitive exists; `serve` is separate | One daemon owns sessions, catalog, providers, tools and HTTP; web/TUI/CLI attach to it |
| Conversation layout | Basic transcript bubbles | Full-width message rows; user content aligned right, assistant left; responsive max readable content width |
| Message actions | Session rename/archive only | Role-appropriate copy, edit, retry/regenerate, **Fork**, feedback and metadata actions below each message; Fork opens an accessible popover with **Branch in new chat** |
| Branch/edit history | V2 fork storage exists | Branch from any persisted message copies history through that selected message into a new session, records parent + boundary provenance and leaves the original unchanged |
| Reasoning | Provider-visible summary streaming/persistence is implemented for the OpenAI web turn path; raw CoT is never requested | Collapsed provider reasoning-summary/activity section, never raw hidden CoT |
| Tool calls | V2 tool-call persistence exists; Responses adapter rejects tool transcript | Collapsed tool-call cards with input/state/output/error and approval UI, driven by real execution records |
| References/citations | Plugin reference registry exists, not answer citations | Dedicated References section bound to per-turn source records/citations; links/files reopen their source |
| Streaming | Real OpenAI text + provider reasoning-summary deltas stream; unsupported structured events fail closed | Text, reasoning-summary, tool and source events stream progressively without live-region token spam |
| Composer | Plain textarea + model/effort | Accessible WYSIWYG structured editor with text/code, slash commands, mentions, model/effort, attachment chips and tool/plugin chooser |
| Attachments | Blob storage/multipart schema exists; turn adapter rejects blobs | Real file/image/screenshot paste/drop/upload pipeline with bounded metadata and provider adapter |
| Library/context files | No browser library | Searchable local/project Library backed by native content records, attach-by-reference, source preview |
| Plugins/apps/tools | Tool registry + partial plugin contracts | Capability picker exposes installed/enabled real tools/plugins; permissions and unavailable states are explicit |
| Search/research | Generic grep tool only; no web-research turn mode | Search/deep-research mode with source selection, plan/progress/steering and cited result once native tools exist |
| Chat navigation | Sidebar + text filter | Pin, unified search, archive, share, temporary chat, progressive history paging and keyboard shortcuts |
| Projects/workspaces/memory | Workspace storage primitives; TUI context/memory state | Project-scoped chats/files/context, memory/context inspector and disclosure of context sources used |
| Voice/dictation | No adapter | Browser capture + native audio/realtime adapter; honest unavailable state until real transport exists |
| Editable artifacts/code | Plain assistant text | Separate editable writing/code artifacts with copy/edit/preview/run only through safe native execution boundaries |
| Long conversation UX | Legacy oldest-page context limitation | Recent-window provider context plus progressively paged UI history, both bounded |
| Accessibility | Semantic shell + axe smoke | Keyboard-complete menus/editor/dialogs, focus restoration, 200%/400% reflow, reduced motion and non-spammy live regions |

## Message presentation contract

- Each transcript item owns a full-width row.
- User/request content is aligned to the right; assistant/response content is aligned
  to the left. Alignment must not reverse reading order or DOM order.
- The action bar sits **below** its message content and remains keyboard reachable.
- Every persisted user/assistant response row exposes a **Fork** action. Activating it
  opens a popover/menu owned by that message, with **Branch in new chat** as the first
  supported method. Escape/click-away closes it and focus returns to the Fork trigger.
- Choosing **Branch in new chat** creates a new session whose inherited history ends
  at the selected message **inclusive**. The source session and all later source
  messages remain untouched. The request is keyed by the selected message ID; the
  server resolves that ID to an immutable sequence and the durable child stores parent
  session ID plus `fork_message_seq`. The API may echo/derive the boundary message ID
  for clients, but durable storage does not invent a separate `fork_message_id`. The UI
  then navigates to the child session.
- Forking is valid from either a user or assistant message. A user-message fork ends
  with that request awaiting the user's next action; an assistant-message fork includes
  that answer and lets the user continue after it. System/tool-only internal rows are
  not exposed as user-clickable branch boundaries until their presentation is defined.
- Assistant content is split into independently renderable parts:
  1. optional collapsed Reasoning summary/activity;
  2. zero or more collapsed Tool call/activity cards;
  3. final answer text/artifacts;
  4. dedicated References/Sources section when present;
  5. message action bar and turn metadata.
- Screen readers announce meaningful phase/completion state, not every streamed token.

## Composer contract

The composer is a structured WYSIWYG editor rather than a styled plain textarea.
The stored/send representation must remain deterministic and safe; arbitrary pasted
HTML is sanitized/lowered to the supported document model.

Required affordances:

- rich multiline text + code formatting and undo/redo;
- Enter/shortcut behavior that never traps multiline editing;
- model and reasoning-effort selection;
- file/image/screenshot attachment chips, paste and drag/drop;
- slash-command discovery;
- `@` mentions for real project/library/plugin/tool sources;
- plugin/tool chooser with enabled/disabled and permission state;
- optional Search/Research mode selector when those native adapters exist;
- dictation/voice control when the audio adapter exists;
- send, stop/cancel, queue/steer state and recoverable draft persistence.

## Ralph slices

This user request introduces `REQ-040` and twelve new product slices. They are
deliberately separate from the older unresolved WEB placeholders; numeric proximity
or shared UI surface is not ownership evidence for those older stories.

1. **WEB-006 — shared singleton daemon + embedded web host.** Close the current
   separate-`serve` architecture. One process owns runtime state; first client can
   ensure it, later clients attach; production assets and API share the daemon.
2. **WEB-007 — transcript geometry + per-message action bars.** Full-width aligned
   rows, role-specific action bar placement, timestamps/status, copy and accessible
   action menus. Persisted user/assistant rows include the visible Fork trigger; its
   branch method becomes enabled when WEB-008's native fork endpoint is available.
3. **WEB-008 — edit/retry/regenerate/branch semantics.** Wire real branch/fork and
   retry boundaries. The Fork popover's **Branch in new chat** creates a new session
   through the selected message inclusive, records ancestry, preserves original
   history, navigates to the child and exposes branch navigation.
4. **WEB-009 — structured assistant activity.** Persist/project reasoning summaries,
   tool calls and answer references separately from final answer; stream collapsed
   activity without exposing hidden CoT.
5. **WEB-010 — WYSIWYG composer.** Structured editor, model/effort, slash commands,
   mentions, draft/cancel/queue/steer behavior and keyboard/a11y contract.
6. **WEB-011 — attachments + local Library.** Real bounded file/image/screenshot
   ingest, paste/drop/upload, attach-by-reference, preview/download and native
   Responses attachment adapter.
7. **WEB-012 — plugins/tools/apps + approvals.** Real capability discovery/selection,
   installed/enabled state, permission prompts and tool-call execution projection.
8. **WEB-013 — Search + deep-research mode.** Native search/research adapters,
   source selection, reviewable plan, progress/steering and cited final result.
9. **WEB-014 — chat navigation parity.** Pin, unified search, archive/share,
   temporary chat, progressive history paging and keyboard navigation.
10. **WEB-015 — projects/workspaces/context/memory.** Project-scoped chats/files,
    reusable instructions/context, memory/context inspector and source disclosure.
11. **WEB-016 — voice + dictation.** Audio capture lifecycle, permission/error
    states, native realtime/audio adapter and accessible transcript fallback.
12. **WEB-017 — editable artifacts + code blocks.** Writing/code artifact model,
    copy/edit/preview and explicitly authorized safe run/apply actions.

Each task has five task-specific obligations: real happy path, explicit failure or
unsupported path, accessibility/keyboard semantics, cancellation/resource bounds,
and persistence/reload/compatibility behavior. A task is not accepted from a visual
mock alone.

## Planned implementation order

The sequence is contract-driven rather than visual-only:

1. WEB-006 singleton runtime authority.
2. WEB-007 transcript/action presentation using existing message records.
3. WEB-008 real edit/retry/branch actions.
4. WEB-009 richer per-turn event/message projection.
5. WEB-010 structured composer.
6. WEB-011 attachments/library.
7. WEB-012 plugins/tools/approvals.
8. WEB-013 research/search.
9. WEB-014 navigation/history scaling.
10. WEB-015 project/context/memory.
11. WEB-016 audio.
12. WEB-017 artifact editing/execution.

Slices can only move ahead when their required native contract exists. UI affordances
for later slices may be visually present only as explicit disabled/unavailable states,
never as fake successful behavior.
