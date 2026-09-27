# Page and feature acceptance matrix

Baseline `767a86a84376a69b2814ea959bb2c94831783768`. All rows are **draft/unverified**. Existing modules or prior tests are reuse candidates, not acceptance.

This matrix is a proposed observable checklist. W1 freezes the exact pinned/observed-dev dynamic source inventory and conditional-feature decisions. Full UI-plugin compatibility and provider-specific dialogs require explicit scope disposition.

| ID | Client | Surface | Owning packages | Observable proof |
|---|---|---|---|---|
| N01 | native | Fresh start, setup and home | P1-W1-07, P1-W2-02, P1-W3-05 | Fresh disposable HOME opens setup/home; successful setup creates/opens actual workspace session. |
| N02 | native | Session transcript and composer | P1-W2-03, P1-W3-03 | Multiline send streams a real turn with durable history and preserved drafts on failure. |
| N03 | native | Command palette and slash commands | P1-W3-04, P1-W4-05 | Search, select and execute registered actions; no menu label without callback. |
| N04 | native | Provider connection and credential entry | P1-W1-09, P1-W3-05 | Authorized configuration persists securely; invalid/cancelled setup leaves no leaked or half-committed credential. |
| N05 | native | Model, variant and effort selection | P1-W3-05, P1-W4-02 | Next actual provider request uses exact selected model/effort; unsupported variants are honest. |
| N06 | native | Agent/plan/build selector | P1-W3-06, P1-W4-01 | Mode changes effective execution policy; selecting plan cannot write files. |
| N07 | native | Session list/search/switch/new | P1-W3-01 | Keyboard selection changes actual session; per-tab drafts/model/scroll remain isolated. |
| N08 | native | Rename/archive/delete/error confirmation | P1-W3-01, P1-W4-06 | Mutations persist and synchronize; cancellation or denial changes nothing; failure is actionable. |
| N09 | native | Workspace list/create/move/unavailable | P1-W3-01, P1-W4-06 | Actions affect scoped workspace authority and session membership; unavailable workspace does not silently redirect. |
| N10 | native | Message actions and timeline jump | P1-W3-02, P1-W3-03 | Jump uses stable message identity under streaming/pagination, not stale indices. |
| N11 | native | Fork at message/timeline | P1-W3-02 | Child history ends at selected immutable boundary; parent remains unchanged. |
| N12 | native | Undo/redo/rewind/compact/retry | P1-W3-02 | Durable semantics distinguish pointer move, destructive rollback, context compaction and uncertain retries. |
| N13 | native | Child inspector, parent/sibling navigation | P1-W3-06, P1-W4-01 | Inspect real running child, steer/cancel correct child and return without privilege escalation. |
| N14 | native | Tool permission and human-only confirmation | P1-W2-06, P1-W5-04 | Exact operation/scope shown; denial prevents effects; stale/replayed grant is refused. |
| N15 | native | Question/elicitation/multi-choice/custom answer | P1-W2-06, P1-W4-03 | Actual waiting tool/agent receives one scoped answer; cancellation and expiry settle safely. |
| N16 | native | Retry action, connection/error recovery | P1-W2-06, P1-W5-06 | No automatic repeat of ambiguous write; drafts/history remain recoverable. |
| N17 | native | Themes, mode and contrast settings | P1-W3-07 | Selection changes native rendering, persists and has tested no-color/contrast behavior. |
| N18 | native | Keyboard help and configurable bindings | P1-W2-01, P1-W3-07 | Displayed shortcuts match actual handlers; paste cannot activate shortcuts. |
| N19 | native | Status, debug and operational side panel | P1-W3-07, P1-W4-02 | Shows actual provider/tool/queue/usage health; unknown values are not zeros or fake live states. |
| N20 | native | Context/memory inspection and unload | P1-W3-07, P1-W4-05 | Displayed sources and unload/reload influence next real prompt under visibility policy. |
| N21 | native | MCP catalog/search/selection/toggles | P1-W4-03 | Enable/disable affects process lifecycle, tool advertisement and dispatch atomically. |
| N22 | native | Skill picker, command completion and file mentions | P1-W3-04, P1-W4-05 | Selected content reaches real turn with permissions and correct scoped precedence. |
| N23 | native | Prompt history/stash/tags | P1-W2-03, P1-W3-01, P1-W3-04 | Drafts/history/stash round-trip without submitting or leaking into another workspace; tag behavior source-classified. |
| N24 | native | Code blocks, diff, selection/copy | P1-W3-03, P1-W5-05 | Correct Unicode/cell widths, stable scroll, safe rendered controls and authorized clipboard action. |
| N25 | native | Local files and constrained terminal | P1-W3-08, P1-W4-04 | Real scoped file/diff and approved process output; child/PTY teardown and denial verified. |
| N26 | native | Session export/import/local share | P1-W3-09 | Redaction, size limits and revocation operate on actual durable session; original DB untouched. |
| N27 | native | Provider-specific organization dialog | P1-W3-05 | Conditional account/organization selection exists only for verified authorized provider support. |
| N28 | native | Loading, missing plugin route, empty and offline states | P1-W2-02, P1-W3-04, P1-W5-05 | Every state has truthful recovery/focus behavior; unsupported plugin route is not advertised as implemented. |
| N29 | native | Raw input, resize, suspend/resume and terminal restoration | P1-W1-04, P1-W2-01, P1-W6-03 | Real PTY/console records split Unicode/paste/resize and exact restoration after normal/error/cancel exit. |
| N30 | native | Plugin-contributed pages and bindings | P1-W4-05, P1-W1-01 | Supported native contributions register real actions; arbitrary Solid/TS compatibility separately approved or deferred. |
| W01 | web | Embedded launch and authenticated session | P1-W2-07 | Packaged daemon serves UI without Vite; authorization works without provider-secret disclosure. |
| W02 | web | Sidebar, pinned history and search | P1-W4-06 | Pins/search/archive/page cursors persist and retrieve correct scoped sessions. |
| W03 | web | Projects/workspaces and context files | P1-W4-06, P1-W4-05 | Grouping and disclosure match actual workspace session authority and next prompt context. |
| W04 | web | Full-width role-aligned conversation/actions | P1-W4-07 | Action bars belong to correct durable message; keyboard/screen-reader behavior is tested. |
| W05 | web | Edit/retry/regenerate/fork/branch navigation | P1-W4-07, P1-W3-02 | Original history preserved, branch boundary correct, retries do not duplicate effects. |
| W06 | web | Reasoning summary/tool activity/references | P1-W4-07 | Provider-safe summaries only; actual tool events and retrieved sources separate from final answer. |
| W07 | web | Structured composer, mentions, slash actions, stop/queue | P1-W2-08, P1-W4-05 | Structured input maps to actual request; drafts survive interruption and busy handling. |
| W08 | web | Providers/models/variants/effort/accounts | P1-W3-05, P1-W4-02 | Selection reaches shared execution service; UI never receives credentials. |
| W09 | web | Files/images/screenshots, paste/drop and Library | P1-W4-08 | Selected durable attachment reaches an authorized capable provider, not just upload storage. |
| W10 | web | Plugin/app/tool chooser and MCP settings | P1-W4-03, P1-W4-05 | Selection changes actual advertised tools and respects permission/lifecycle rules. |
| W11 | web | Approval/question/retry dialogs | P1-W2-06, P1-W4-03 | Same pending operation and decision state seen by native client; stale grant rejected. |
| W12 | web | Agent inspector, background jobs and settings | P1-W4-01 | Real parent/child execution and cancel/steer/results are shared with TUI. |
| W13 | web | Writing/code artifacts and versions | P1-W4-09 | Edit/undo/version state persists; preview isolated; run/apply invoke actual safe brokered operation. |
| W14 | web | File/diff/local terminal views | P1-W3-08, P1-W4-04 | No unrestricted host shell; output/source display sanitized, terminal lifetime bounded. |
| W15 | web | Search and deep research | P1-W5-01 | Real source retrieval, plan/review/progress/steer/cancel and cited output; local grep not mislabelled web search. |
| W16 | web | Voice/dictation and microphone state | P1-W5-02 | Opt-in actual adapter, visible transcript, stop/revoke/disconnect cleanup; no fake transcription. |
| W17 | web | Execution graph/canvas | P1-W5-03 | Nodes/actions use real persisted agents/turns, not synthetic sample graph or generic workflow expansion. |
| W18 | web | Settings/themes/status/diagnostics | P1-W3-07, P1-W5-05 | Persisted effective settings match backend and redact diagnostics. |
| W19 | web | Temporary chat, local share/export/import | P1-W4-06, P1-W3-09 | Temporary retention is explicit, archive/share consent correct, no unexpected durable history loss. |
| W20 | web | Responsive, accessibility and reconnect behavior | P1-W4-10, P1-W6-04 | Real browser keyboard/screen reader geometry and cursor resync; slow clients bounded. |
