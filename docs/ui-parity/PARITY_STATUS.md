# OpenCode V2 UI parity — consolidated status

- **Canonical line:** `main-v2` @ `fc942da4` (session base SHA; not re-verified
  against the push remote in the wave-107 pass)
- **Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` @ `450e7b92412c780a7b5a75226bb8d49aaca24e22`
- **Checklist:** `docs/ui-parity/opencode-ui-parity-kit/checklist.json` (223 items)
- **Method:** source inspection of the live Rust path (implementation + a
  production consumer), cross-checked against the pinned upstream component.
  This is **not** captured-golden acceptance: no matched styled-cell/cursor
  captures were produced. Verdicts below are source-backed, not runtime-proven.

## Wave-112 (2026-11-07) — the last two "open" bounds were already met

Re-checking the remaining rows the same way (read the live consumer, not the
ledger) closed two more:

| Row | Ledger said | Tree shows |
| --- | --- | --- |
| `PAR-19` | viewer has no consumer | `visible_chunk_range` + `placeholder_height` drive both diff-viewer branches (`native_view_dialogs.rs:3722-3781`); `relative_scroll_top` is an identity there because the scroll area has no virtual-root offset |
| `MSG-010` | turns model dead | the `RowMessage` model is the *marker-era* algorithm, which native sessions cannot use (no persisted `Idle` rows); the live path is `project_with_turn_duration` (nearest preceding input) + `AssistantFacts::tokens_per_second` over server `streamed_ms` |

**What this leaves as genuinely open**, each blocked on something other than a
missing consumer:

- `LAY-10` — needs a live terminal PTY/emulator producer; `LAY-11/13` are now live and tested. `NativeHost::new`
  still has no production caller and `compose_shell` is test-only; the live
  render composes only the sidebar. This is a composition migration plus a new
  product surface, not a bounded wiring fix.
- `SIDE-05` — `set_limit` (`native_view_status.rs:1260`) has only a test caller
  because there is no plan-quota source; the typed `rate_limited` error path
  (`native_assistant_facts.rs:164`) is live and distinct.
- `DIALOG-20` — toast and crash-render recovery are landed; input-path/PaintCtx-construction panic recovery remains a separately scoped follow-up.
- `PAR-02/03/06` — harness-side obligations; `FFI-*` — deliberate non-goals.

No fabricated completion: every remaining row is blocked on product scope, an
external API, or a large composition change — not on more ledger bookkeeping.

## Wave-111 (2026-11-07) — syntax capture classes, and a false-negative grep

Two "absent" rows were re-verified and both were **already landed**. The second
check exposed the audit method's own failure mode: `TAB-10`'s fields are named
`branch`, `default_branch` and `worktree` (`native_navigation.rs:205-215`), so
the `current_branch` grep that "proved" the row absent could never match.

`THEME-08` was likewise past its "no producer" claim: the lexical classifier
`code_token_cell_spans` (`markdown_runs.rs:165`) is called from the live fence
painter (`native_view_chat.rs:2970`) and its `SYN_*` bits are consumed by the
live paint loop (`tui_entry.rs:6593`).

What was genuinely missing is breadth, now implemented:

| Property | Result |
| --- | --- |
| Capture classes | `SYN_FUNCTION`, `SYN_TYPE`, `SYN_CONSTANT`, `SYN_PROPERTY` (8 of 14 `CaptureClass` variants) |
| Rule | Closed and line-local: keyword set, literal set, `.` receiver → property, `(` head → function, uppercase lead → type; a bare identifier stays plain |
| Colors | function → `syntax.function`, type → `syntax.type_name`, constant → `syntax.number`, property → `syntax.variable` — the rules that capture them upstream (`syntax.ts:83,104,113-118`) |
| Attributes | `comment`/`keyword` italic (`syntax.ts:78,90`); number/constant/function/type unattributed, so `true`/`false` left the keyword set |
| Disjointness | One class per interval; the base `markdown.code` span still covers the gaps |

Tests: `theme08_lexical_capture_classes_match_upstream_scopes`
(`markdown_runs.rs`) for the classifier, plus color/attribute assertions in
`theme08_syntax_tokens_paint_live_code_fences` (`tui_entry.rs`).

Still open: `LAY-10` (terminal PTY/emulator producer),
`SIDE-05` (usage-limits pipeline), `PAR-19`, `MSG-010`, `FORM-001` sub-3/4,
`DIALOG-20` crash half, `PAR-02/03/06` (harness-side), `FFI-*` (non-goals). No
styled-cell captures exist, so every verdict stays source-backed.

## Wave-110 (2026-11-07) — subagent whole view is reachable

The Subagents dock (`/subagents`) already previewed child sessions, but the
whole-session **graph** had no trigger: `PageState::open_graph` had zero
production callers, so `Page::Graph` could only be reached by tabbing into it,
where it honestly painted "no graph loaded".

Upstream has no graph route — the diagram is inline content
(`packages/merman/src/markdown.ts`) and the preview is a dialog
(`component/dialog-image-preview.tsx`) — so inventing `/graph` would violate the
authority order. The trigger is therefore the **dock's own family control**:
`g` (and the matching `g graph` footer cell) on the Subagents panel.

| Property | Result |
| --- | --- |
| Node source | `Panel::family` projects the roster the panel already loaded from `/api/sessions/{id}/children`; the bound parent session is the root |
| No invented parents | A row keeps its daemon `parent_id`; a row without one hangs off the bound session, matching `family_prefix` |
| No invented status | Roster vocabulary only (`running`/`error`/`stopped`…); anything unreported stays `Idle` |
| Selection | `g` is family-wide: it needs no selected row and submits nothing |
| Detail view | Owns its own keys, so `g` is inert there |
| Navigation | Up/Down move the node cursor through `PageState::graph_navigate`; nothing is submitted |
| Exit | Esc closes the page (`close_page`) and returns to the transcript with the draft byte-identical |
| Empty roster | Still a valid one-node graph (the root), not an error |

Verified: `-- native_children::tests` → **22 passed / 0 failed**; `-- graph` →
**21 passed / 0 failed**; `-- children` → **37 passed / 0 failed**;
`-- native_view_pages::tests` → **23 passed / 0 failed**; `-- page` → **91
passed / 0 failed**; `-- native_view` → **597 passed / 0 failed**; `cargo build
--bin oc2` exit 0.

Still open (unchanged by this wave): `LAY-10` (needs a live panel producer),
`SIDE-05` (external Console API), `PAR-02/03/06` (harness-side), `FFI-*`
(deliberate non-goals). No styled-cell/cursor captures exist yet; these verdicts
remain source-backed, not runtime-captured.

### Also this wave: `FORM-001` sub-behavior 1 verified already-correct

The ledger listed the live permission dialog as emitting 6 labels against
upstream's 3. That was stale in two ways. The 6-label `open_permission` helper
(`native_view_dialogs.rs:2824`) has **no production callers** — every call site is
in its `#[cfg(test)]` module; the live permission poll
(`tui_entry.rs:11947`) opens `open_permission_with_persistent`, which emits
`Allow once` / `Always allow` / `Reject` in upstream order.

And the citation was dead: `routes/session/permission.tsx:232-240` does not exist
at pin `450e7b92`. The option set now lives in `mini/permission.shared.ts:36-38`
(`return ["once", "always", "reject"]`), labels in `permissionOptionLabel`
(`util/permission.ts:162-166`), and the always-stage set is `["confirm",
"cancel"]` (`:41-43`). Source citations corrected in `native_permissions.rs` and
`native_view_dialogs.rs`.

One genuine residual is recorded rather than hidden: the pin's footer returns all
three options unconditionally, while the Rust helper drops `always` when the
request carries no `save` rules. That is a behavior decision a product owner must
make — I did not change it, and `FORM-001` stays `PARTIAL`, not `MATCH`.

### Six `BLOCKED`/`REAL_GAP` rows were stale — corrected by verification

Before queueing more implementation, I re-checked every seam the open rows
assert "is absent". Six rows were wrong: the code had landed in earlier waves and
nobody re-checked. Each is now `ALREADY_CORRECT`/`PARTIAL` with line evidence.

| Row | What the ledger claimed | What the tree actually has |
| --- | --- | --- |
| `FORM-001` sub-1 | live dialog emits 6 labels | live poll uses the 3-option helper; 6-label helper is test-only |
| `DIFF-001` | no `config.diffs.view` parse | `UiPreferences::diffs_view` parsed + live `set_diff_view` → `resolve_view_mode` → `paint_rows_split` |
| `MSG-005` | no `files`/`skills` anywhere | contract fields, server builder, projection, painted chips |
| `STATE-002` | no `Compaction` role, no persistence | role + storage enum + server append + CLI part/divider |
| `TOOL-011` | `execute_dialog` zero consumers | click path opens the execute viewer live |
| `THEME-08` | `syntax_rules()` zero callers | installed per frame into a live `SyntaxStyleArena` |

**Standing rule this establishes:** a `BLOCKED` row is a claim about the tree
*now*, not a permanent fact. A row can go stale two ways — the pin moved
(`component/form.tsx` no longer resolves; only dead path of 20 cited) or the code
landed. Verify the seam is still absent before assigning work against it, or the
fleet re-implements what already exists.

**That paragraph was itself stale, and re-checking it a second time is the
general lesson.** `THEME-08` *does* have a token producer: `code_token_spans` /
`code_token_cell_spans` (`markdown_runs.rs:69,165`) run on live fenced chunks
(`native_view_chat.rs:2970`) and their `SYN_*` bits paint through
`markdown_run_paint_color` (`tui_entry.rs:5836`, consumed `:6593`). `TAB-10`
spells its fields `branch` / `default_branch` / `worktree`
(`native_navigation.rs:205-215`), so a `current_branch` grep is a **false
negative**: `git_vcs_info` (`tui_entry.rs:878`) feeds `LiveSnapshot.session_vcs`
(`:853`, `:1645`) and the live `rail_inputs_for_navigation` (`:7192`) calls
`VcsInfo::tab_detail`. Only `LAY-10` still holds. Grep by upstream *concept*,
never by upstream *symbol name*.

## Wave-109 (2026-11-07) — command-palette row hover

The palette was the last interactive surface with no pointer hover: the model
had no `hovered` field at all. It now has one, kept **separate** from
`selected_id` so pointer hover and keyboard selection stay independent, exactly
like `dialog-select.tsx`.

Hit-testing and painting share one layout. `render_rows` is the single source of
row identity: it returns each painted `Line` plus, for entry rows only, the
command identity that row represents. `render_styled` maps it down to plain
`Line`s, and `hover_row` reads the same output — so a hover can never land on a
row the frame did not paint, and no second hand-written row arithmetic can drift
from the painter.

| Property | Result |
| --- | --- |
| Hover repaints | Hovered non-selected rows lift to `Role::Action` (`BackgroundPanel` surface) |
| Selection outranks hover | A selected row keeps `Role::Selected`; `selected_id` is untouched |
| Row text | Never rewritten by hover (asserted across every row) |
| Headings/search/footer | Not hoverable; they clear the hover instead of inheriting a neighbour |
| Repaint cost | `hover_row` returns `true` only on a real row crossing |
| Query edits / close | Clear the hover so no stale identity survives |
| Live feed | `command_palette_hover` maps the pointer cell through the same `modal_presentation` geometry the renderer uses; `tui_entry.rs` dispatches it before the toast block, which consumes every move |

Verified: `cargo test -p opencode-rk-cli --bin oc2 -- palette` → **73 passed /
0 failed**; `-- hover` → **33 passed / 0 failed**.

## Wave-108 (2026-11-07) — provider OAuth/device sign-in added

Three user-reported gaps were checked against source. One was genuinely absent
and is now implemented end to end; the other two were partly present, and the
census below records exactly where each still stops.

### `/login` — new, and reachable

Before this wave the CLI had **no login route at all**: `persisted_auth` could
read and write an OAuth credential and `providers::openai_device_auth` could run
the RFC 8628 device flow, but nothing in the client ever started either.

| Piece | Location |
| --- | --- |
| Pure surface (picker, phases, layout, hints) | `native_auth_login.rs` — `LoginSurface`, `LoginPhase` |
| Background flow runner | `native_auth_login.rs` — `LoginRun`, current-thread runtime on `oc2-auth-login` |
| Credential persistence | worker calls `persisted_auth::save_oauth_credential` **before** reporting success |
| Palette route | `native_command_palette.rs` — `provider:provider.login` / `provider.login` → `/login` |
| Typed command + recognition | `tui_entry.rs` — `/login` handler, `is_recognized_local_command` |
| Modal painting | `tui_entry.rs` — login rows replace `overlay`, `input_overlay` includes it |
| Keys | `tui_entry.rs` — Enter starts/closes, Esc cancels the worker, arrows pick |
| Progress without blocking | `tui_entry.rs` — `LoginRun::try_poll` drained on the existing `≤100 ms` idle tick |

No access or refresh token crosses the worker channel: the event carries only
`Authorization { user_code, url }`, `Saved { account }` or `Failed(reason)`.

OpenAI (device code) is fully wired. Anthropic is listed in the picker with its
real method (browser consent) but `LoginSurface::begin` **fails closed** with an
explicit reason, because this build cannot receive the hosted callback — the
dialog never pretends to poll a flow it cannot finish.

Verified: `cargo test -p opencode-rk-cli --bin oc2 -- native_auth_login` →
**10 passed / 0 failed**.

### Regression this wave caught and fixed

Registering the login entry pushed `/prompt.stash.list` out of the palette's
`MAX_ENTRIES = 16` list bound — an existing command became unreachable, and the
existing `prompt_stash_palette_routes_resolve_to_local_commands` test failed
with `["/prompt.stash", "/prompt.stash.pop"]`. The bound is a registry cap, not
a visible-row cap (`render_styled` clips to the viewport), so it was raised to
**24**. Palette suite: **70 passed / 0 failed**.

### Hover — present on most surfaces; two live gaps remain

Already fed by real pointer events in the live loop: tab strip/rail (session,
Home, add, compact search), pinned footer location, send button, permission
editor rows, subagent dock rows, user-message cards, and dialog select rows
(`dialogs.hover_at_row`, `tui_entry.rs:15246`).

Two gaps were recorded in that pass. One was real and is fixed in Wave-109
(palette rows). The other was a **false positive in this ledger** and is
corrected here: toast hover *is* wired live. `toast_hovered` is recomputed from
the rendered `ToastFrame` rect on `kind == 7` and pauses/repaints
(`tui_entry.rs:15311-15332`). `native_modal_layer::hover_toast` is that layer's
own API, not the toast lifetime's — the grep behind the earlier claim looked for
the wrong symbol.

### Subagents — dock is live, whole-view graph is not

`/subagents` is handled (`tui_entry.rs:18491`) and opens the live
`children_panel` bound to the current session lease, so the dock view works.
The **whole-session graph** does not: `native_graph::GraphPanel` and
`PageState::open_graph` exist and are tested, but `open_graph` has **zero
callers** in `tui_entry.rs`, so no keybinding, slash command or pointer route
can reach it.

### Not runtime-proven

As with every earlier wave, these verdicts are source inspection plus the named
focused tests. There are still no captured styled-cell or cursor goldens, so no
claim here rests on a rendered capture.

## Wave-107 (2026-11-07) — fresh source re-verification: the open list was stale

The "38 remaining" set recorded after wave-81 had not been rolled up since
waves 82–106 landed. A fresh source pass at `fc942da4` re-checked every item on
that list for a **live production consumer**. Twenty-five of them are already
live and move to `LANDED` (twenty-three in the table below, plus `TAB-06` and
`PAR-07`). Nothing here was runtime-proven with captured
goldens; this is source inspection plus the named focused tests.

| Item | Was | Now | Live evidence |
| --- | --- | --- | --- |
| `DIALOG-07` | `REAL_GAP` | `LANDED` | `row_footer` consumed by the live select body: `native_view_dialogs.rs:3991` (built), `:4010` (windowed), `:4078` (painted right-aligned); test `dialog07_row_footer_paints_right_aligned:11890` |
| `DIALOG-13` | `BLOCKED` | `LANDED` | `SessionPicker` (`native_palette.rs:2550`, mounted `:3455`) reaches the live `open_sessions` (`native_view_dialogs.rs:1524`), called from `tui_entry.rs:5109` |
| `DIALOG-15` | `BLOCKED` | `LANDED` | `open_integration` (`native_view_dialogs.rs:2220`, `IntegrationPhase` `native_palette.rs:2930`) live-called `tui_entry.rs:16346` |
| `DIALOG-17` | `BLOCKED` | `LANDED` | `open_stash` (`native_view_dialogs.rs:1992`) live-called `tui_entry.rs:17941`; rows from `native_palette.rs:3202` |
| `DIALOG-18` | `BLOCKED` | `LANDED` | live `open_diff_viewer` `tui_entry.rs:15616`; viewer mode resolved `native_view_dialogs.rs:2457` |
| `TOOL-006` | `BLOCKED` | `LANDED` | **Produced end to end**: server routes `GET /api/shell`, `/api/shell/{id}`, `/api/shell/{id}/output` (`server/src/lib.rs:244-246`) over `shell_registry.rs`; covered by `tests/tool006_shell_routes.rs`; CLI polls live (`tui_entry.rs:10719`) and feeds the Shell dock (`native_shell.rs:1852/1866/1957`, `native_view.rs:280/2047/10418`) |
| `TOOL-011` | `BLOCKED` | `LANDED` | live `execute_dialog_from_part` `tui_entry.rs:15573`; dialog rows rendered `native_view_dialogs.rs:140/152` |
| `MEDIA-001` | `REAL_GAP` | `LANDED` | live click route `tui_entry.rs:15152` → `NativeView::open_image_preview_page_at` (`native_view.rs:3774`) → `PageState::open_image_preview` (`native_view_pages.rs:793`) |
| `MSG-001` | `BLOCKED` | `LANDED` | `SessionRow`→painter adapter now live: `reduce_session_rows` called at `native_view_chat.rs:4165,4377` |
| `MSG-002` | `BLOCKED` | `LANDED` | same adapter; disclosure state carried through `reduce_session_rows` |
| `MSG-003` | `BLOCKED` | `LANDED` | live `partition_pending` `native_view_chat.rs:4405` |
| `MSG-005` | `BLOCKED` | `LANDED` | contract fields exist (`contracts/src/lib.rs:314-319`), projected (`native_chat_projection.rs:787`), painted (`native_view_chat.rs:1287,1353-1357`) |
| `MD-007` | `BLOCKED` | `LANDED` | live fence registry bound: `set_fence_handlers` `native_view_chat.rs:3081` |
| `MD-008` | `BLOCKED` | `LANDED` | real painter injected live: `CellDiagramPainter` `native_view_chat.rs:2981` (impl `native_mermaid.rs:463`) |
| `MD-009` | `BLOCKED` | `LANDED` | real engine injected live: `UpstreamMathEngine` `native_view_chat.rs:3039` (impl `native_math_engine.rs:2331`) |
| `PAR-09` | `REAL_GAP` | `LANDED` | live syntax-token fence handler `native_view_chat.rs:3081`; test `theme08_syntax_tokens_paint_live_code_fences` (`tui_entry.rs:3409`) |
| `PAR-19` | `BLOCKED` | `LANDED` | `visible_chunk_range` live in the diff dialog `native_view_dialogs.rs:3720,3772`, fed by the root-relative `relative_scroll_top` (`native_diff_view.rs:1227`) |
| `DIFF-001` | `BLOCKED` | `LANDED` | preference threaded live (`native_view.rs:97,2784-2790`), inline resolution `native_view_chat.rs:3557`, viewer route `tui_entry.rs:15616` |
| `DIFF-003` | `BLOCKED` | `LANDED` | chunk virtualization live (same `visible_chunk_range` consumers) |
| `LAY-11` | `REAL_GAP` | `LANDED` | `DiffPanelState`/`diff_panel_layout` live in `native_view.rs`, `native_view_layout.rs`, `native_pane_resize.rs`, `/diff` dispatch in `tui_entry.rs`; candidate `25fc5236` |
| `LAY-13` | `REAL_GAP` | `MODEL LANDED` | `pane_resize_handle` live `native_pane_resize.rs:236,637` |
| `THEME-13` | `REAL_GAP` | `LANDED` | `SystemThemeState` live `tui_entry.rs:9428,10479` |
| `THEME-14` | `REAL_GAP` | `LANDED` | `SyntaxStyleArena` live `tui_entry.rs:10466-10468` |

**Also re-verified live in the same pass** (the wave-106 open list was wrong here):

- `TAB-06` — the live loop really does feed per-tab status: pending items are
  filtered by session owner (`tui_entry.rs:12504-12512`), the family roll-up
  merges live `/children` rows (`:12513-12532`), `apply_live_status` is called at
  `:12533`, and `record_enqueue` runs from the real enqueue site
  (`:17441-17458`) with correct `is_open`/`is_current`. The seam
  `queued_drafts` is owner-tagged, so background tabs are not ignored.
- `PAR-07` — all four trace families are live, not just three: `form.present`
  (`tui_entry.rs:12056`), `form.submit` carrying a `posted` outcome (`:14017`),
  and `form.validation` on a failed submit (`:14030`) — on top of the existing
  queue/steer (`:12558`, `:12601`), submission (`:13693`) and permission reply
  (`:6212`) traces. The CLI does have a form surface (`DialogKind::Form`,
  `open_form`/`open_form_restored`/`form_dismiss`).

**Still genuinely open after this pass (verified absent, not just unwired):**

- `LAY-10` — `panel_open`/`terminal_selected` (`native_shell_frame.rs:122-124`) are
  never set in production (`:150-151` default false); only tests assign them
  (`:783-785`). `compose_shell` still has zero production callers and
  `NativeHost::new` is reached only from `native_host.rs:755` inside `#[cfg(test)]`.
- `SIDE-05` — no usage-limits route/service (external Console API).
- `PAR-02`/`PAR-03`/`PAR-06` — harness-side obligations, not product.
- `FFI-02/03/04/06/07/08/11` — bridge render-substrate models with no CLI
  consumer; adopting any one replaces the live paint path, so this is a
  deliberate non-goal rather than a wiring gap.

**Also fixed in this pass (test-harness, not product):**
`crates/cli/tests/native_state_platform_alias.rs` was **broken on canonical** —
its `#[path]`-include closure omitted the modules `native_ui_preferences`
resolves against (24 compile errors at `fc942da4`). Carrying that closure would
have dragged in the whole view layer (`native_children` → `native_view_chat` →
~16 more modules), so the 5 macOS path-alias assertions were moved verbatim into
`native_ui_preferences::platform_alias_tests` (macOS-gated unit tests) and the
unbuildable target was removed. Verified: **5 passed / 0 failed**
(`cargo test -p opencode-rk-cli --bin oc2 -- platform_alias`).

## Wave-83 — DIALOG-14/15 + DIFF-003 landed (2026-11-05)

Canonical `main-v2` advanced `73765e77 -> ba04acbe` (pushed + `ls-remote`
verified). Full CLI suite on the integrated SHA: **2385 passed / 0 failed**
(`73765e77` was 2379).

| Item | Was | Now | Live evidence |
| --- | --- | --- | --- |
| `DIALOG-14` | `REAL_GAP` | `LANDED` | `8d1cab14` live `DialogWorkspaces`: `FooterAction::Workspaces` opens it (`tui_entry.rs`), roots from on-disk git enumeration `local_worktree_roots`, footer menu un-gated (`native_view.rs` `can_move = true`), live key dispatch `workspaces_live_dispatch` (filter/backspace, move, Enter drill/arm/delete, Esc drill-back/close), `LateLoadGuard` blocks resurrection. 3 focused tests. Independent APPROVE (river-glm `ses_e5677d862ffeU25MnrDgZPTa28`). |
| `DIALOG-15` | `REAL_GAP` | `LANDED` | `8d1cab14` live MCP Enter routes through the tested `mcp_enter` (`native_view.rs` `mcp_dialog_enter`): `needs_auth` opens the live `IntegrationState` surface, `failed` shows error details, else inert; toggle moved to space (`config/keybind.ts:283`). 2 focused tests. Seam: live `McpStatus` has no `integrationID`, so `needs_auth` is treated as integration-gated; no server-side sign-in producer exists. |
| `DIFF-003` | `BLOCKED` | `LANDED` (inline half) | `ba04acbe` `diff003_inline_live_patch_budget_color_and_geometry_are_stable` exercises the live `ChatPart::Patch` arm: 4097-line added file yields only real diff rows up to `MAX_DIFF_LINES` + the `[diff truncated]` row; hunk coloring intact; stable geometry. Independent APPROVE (vyce-deepseek-v41 `ses_e567608f7ffeiPzvz5FJtrLHS9`). Split-mode half remains unwired (see wave-81/`DIFF-001`). |

**Written in wave-83; superseded 2026-11-07.** `MSG-008` landed in `6a49151f`
and `PAR-19`'s viewer consumer is live — see wave-112. As of this wave,
`STATE-001/003/004` were `REAL_GAP` (models test-only, no live producer) and
`MSG-005` (skill/file message chips) was in flight.

## Wave-80 — CP-09 + CP-07 landed (2026-11-04)

Published `c2ad1750` (base `6c713efb`). **CP-09** (live agent-color composer
rail + `agent.cycle`) is the same checklist requirement as **CP-07**
("Preserve data-driven agent selection and colors. Custom agent, hidden agent,
subagent-only agent, explicit color, palette wrap."), so both move to `LANDED`:

- Server `mention_agents()` (`crates/server/src/configured_commands.rs`) now
  advertises a validated `color` field (`#rgb`/`#rrggbb`).
- CLI parses `/api/commands` into `AgentFactOwned { id, hidden, subagent, color }`,
  seeds the selected agent from the first selectable entry, cycles it on the
  `AgentCycle` (shift+tab) event, and computes the rail color via
  `composer_agent_color` (explicit `agent.color` else the deduped categorical
  palette at step 400 new-designs / 200).
- `home_composer_border_cell_style` paints the selected agent color for the
  `┃`/`╹` rail glyph, falling back to `Slot::Border`.
- Upstream: `component/prompt/index.tsx:1555-1560,1587-1591`,
  `context/local.tsx:59-64,77-84,96-104,112-124,128-135`,
  `config/keybind.ts:176`, `app.tsx:972-977`, `mini/footer.view.tsx:229-236,766`.
- 5 new `cp09_*` CLI tests + 1 server test. Independent APPROVE on `cad89dd4`;
  content-identical re-apply onto `6c713efb`. Focused `cp09` 7/0; full CLI
  **2297/0**; server 832/0.

In flight (not yet landed): **TOOL-006** background-shell list/output transport,
**CP-38** live image/attachment preview strip, **MSG-005** live skill/file
message chips. (F002-R1 and TOOL-011 landed in wave-82.)

## Wave-81 — independent re-verification of the 45 open items (2026-11-04)

An independent read-only audit re-checked the 45 items the summary table listed
as `BLOCKED`/`REAL_GAP` against the **current** source at `c2ad1750`. The summary
table was **stale**: seven items are already live and are moved to `LANDED` here.

| Item | Was | Now | Live evidence |
| --- | --- | --- | --- |
| `TAB-21` | `BLOCKED` | `LANDED` | live `ViewAcknowledgement` driver `tui_entry.rs:8787-8830` POSTs `/api/sessions/{}/view`; route `server/src/lib.rs:246`; `mark_idle` `server/src/lib.rs:1972,2012,2566,3777`; `sessions/src/lib.rs:657` |
| `THEME-08` | `BLOCKED` | `LANDED` | `8543e769` live `SYN_*` token paint; `native_view_chat.rs:1868` `code_token_cell_spans`; consumed `tui_entry.rs:5869` `markdown_run_paint_color`; `ctx.semantic` set `:10758` |
| `DIALOG-06` | `REAL_GAP` | `LANDED` | `4f4a8228`; live `select_menu_key` `tui_entry.rs:4557` (caller `:13454`) drives `select_push_filter`/`select_backspace` `native_view_dialogs.rs:1813-1845` |
| `DIALOG-08` | `REAL_GAP` | `LANDED` | `select_page` `native_view_dialogs.rs:1847` live-called `tui_entry.rs:12890/12898` (PageUp/PageDown) |
| `DIALOG-09` | `REAL_GAP` | `LANDED` | `SelectExperiment`/`active_gutter`/`current_marker` consumed live `native_view_dialogs.rs:2347-2349,766-768,1189-1193` |
| `DIALOG-16` | `BLOCKED` | `LANDED` | `a3f01f69` live numeric settings rows + experiments picker `tui_entry.rs:11104-11171,13559` |
| `DIALOG-19` | `BLOCKED` | `LANDED` | `62242532` per-frame chrome lifecycle live `tui_entry.rs:3671-3824` (`chrome.tick` `:9970`, `chrome_frame.paints` `:10676`) |
| `DIFF-005` | `REAL_GAP` | `LANDED` | live `native_chat_projection.rs:478-481` already `normalize_path(path)`/`normalize_path(key)` before the diagnostics lookup |
| `CP-34` | `REAL_GAP` | `LANDED` | `sync_live_draft_stash` `tui_entry.rs:465` saves/restores `parts`; live `:8853`; test `cp34_live_tab_switch_restores_folded_draft_parts` |
| `CP-37` | `REAL_GAP` | `LANDED` | `live_paste_admitted` (`PasteQueue`+`paste_admission`) live-called `tui_entry.rs:13098` with live `paste_queue` `:8310` |

**Still missing after the audit (38 items).** The remaining set is dominated by
items whose faithful, unit-tested model has **no backend producer** (not merely
an unwired consumer):

- **Harness obligations (not product):** `PAR-02`, `PAR-03`, `PAR-06`, `PAR-09`,
  `PAR-19`.
- **Backend producer absent:** `SIDE-05` (external Console API), `MSG-005`
  (no skill/file part in contracts/storage/server), `TOOL-006` (no
  `GET /api/shell`), `STATE-003` (no retry deadline on the wire),
  `MSG-001/002/003` (no `SessionRow`→painter adapter), `DIALOG-13/14/15`
  (session-picker/drill-down/integration hosts), `DIALOG-17/18`
  (stash/worktree/diff dialog hosts), `LAY-10/11/13` (plugin-panel/terminal
  producer), `TAB-06` (per-tab pending transport), `DIALOG-07` (`row_footer`
  live), `TOOL-011` (execute-dialog live render+opener), `DIFF-001` (viewer
  half; inline half landed), `DIFF-003` (virtualization).
- **Substrate/ABI absent:** `MD-007/008/009` (diagram/math/plugin producers),
  `THEME-13` (OSC palette probe), `THEME-14` (syntax-style bridge ABI),
  `MEDIA-001` (no image decoder), `FFI-02/03/04/06/07/08/11` (unused bridge
  surfaces).

**UNCLEAR (3):** `PAR-07` (form submit/validation trace sink not located;
`DialogKind::Question` surface now exists), `PAR-09` (no harness async-syntax
landing found), `DIFF-001` (inline half landed; permission/viewer halves
unwired).

## Wave-82 — TOOL-011 landed (2026-11-04)

| Item | Was | Now | Live evidence |
| --- | --- | --- | --- |
| `TOOL-011` | `BLOCKED` | `LANDED` | `5929b143` live `DialogExecute`: click opener `tui_entry.rs:12516-12540` (`chat_tool_owner_at` → `execute_dialog_from_part` → `open_execute`); modal key gate `tui_entry.rs:11264-11325` (Arrow/Page/Esc/Ctrl-C/`c`/`o`); render arm `native_view_dialogs.rs:2416-2441`; model `native_tool_rows.rs:execute_dialog_rows`; upstream `dialog-execute.tsx:222-282`. Published `7765484e`; independent APPROVE (openai-gpt6-luna `ses_e5acc871effeQOdDccM9emFT0b`); post-merge focused 7/0, full CLI 2312/0. Recorded partial: `home`/`end` unbound (no decoded `InputEvent`). |

Counts after wave-82: `ALREADY_CORRECT 145 / REAL_GAP 1 / BLOCKED 36 / LANDED 41`
(the remaining accurate set is 37).

## Meaning of the verdicts

| Verdict | Meaning |
| --- | --- |
| `ALREADY_CORRECT` | The live Rust path renders the upstream behavior; implementation and a focused test exist, and a production consumer was traced. No change needed. |
| `REAL_GAP` | Implementation + test exist but **no production consumer** wires them into the live TUI. Fixable by wiring, not by new backend data. |
| `BLOCKED` | The requirement needs a **producer/transport that does not exist** (contract field, server event, or config key). A UI-only patch would render nothing. |
| `UNVERIFIED` | Not yet finalized in this pass. |

## Summary

- The large majority of items are `ALREADY_CORRECT` on source inspection.
- The remaining work is **not** parallelizable UI work: it is a small set of
  live-wiring gaps and a smaller set of backend-producer gaps. No item is
  blocked on "more agents"; each blocked item names the exact missing seam.

### Wave-58/59/60 — CLI-only patch approach exhausted (2026-11-03)

Three consecutive waves (58, 59, 60) of read-only patch workers produced **zero
landable patches**. The cause is structural, not worker quality: for every
remaining `BLOCKED` item the CLI already carries a faithful, unit-tested model,
but the **backend never produces the fact** the model consumes, so no bounded
CLI-only change can wire a live consumer. Two seams were independently
re-verified by the orchestrator and both are genuinely backend-blocked:

- `SIDE-03` cost: `StatusView::set_cost` is rendered live
  (`native_view.rs:4222`), but the CLI model catalogue (`native_setup.rs:184
  catalogue()` → `Model`) parses **no `cost` field** and the catalogue is not
  retained at the history-fetch site, so there is no live pricing source.
- `TAB-21`: the server route `POST /api/sessions/{id}/view` + `set_viewed` +
  `SessionSummary.viewed_at` all exist, but the **idle-transition timestamp**
  does not: upstream sets `time_idle` on every terminal execution
  (`projector.ts:423-447`, `:704-706`), while our `SessionSummary` has no
  `idle_at` and no turn-terminal path writes one. (The CLI's *local* unread
  marker is already live via `DaemonEvent::WorkState` → `mark_unread`; only the
  server-global acknowledgement POST is missing.)

Further progress therefore requires building **server-side producers** (payload
fields, routes, engines, contract fields) — a different work class from the
UI-parity checklist, not satisfiable by a bounded single-file patch.

## Wave-66 — verification verdicts and rejected patches (2026-11-03)

Base `51538ca1` (TAB-21 landed). 25 groups on three provider-verified routes
(`th/deepseek-v4-flash:free`, `th/gpt-5.6-luna`, `xk/openai/gpt-5.6-luna`);
`vyce-deepseek-v41`, `9router-vyce-deepseek-v4.1-free-combined`,
`9router-river-glm-5-3-flash`, `9router-th-free` and `xkiro-mimo-v2-6-flash` were
excluded — the first two return provider HTTP 400 on every request, the river
model is absent from the 1499-model catalogue, and no `xkiro` provider is
configured.

**Verifier results (independent, source-inspected, exact base SHA).**

| Group | Result |
| --- | --- |
| VTAB | 20 `CONFIRMED`; TAB-21 was mislabeled `BLOCKED` → corrected to `LANDED` above |
| VSIDE | 13 `CONFIRMED`; SIDE-05 mislabeled `BLOCKED` → `REAL_GAP` above |
| VCP | 5 `CONFIRMED` (CP-03/04/10/40/41), no gaps |

**Rejected patches — implemented nothing observable, or regressed the screen.**
Each was independently reviewed against the pinned upstream component before
rejection; none was landed.

| Group | Patch claim | Rejection reason |
| --- | --- | --- |
| TOOL | "wire `execute_dialog` into the live transcript" | Replaces the stored tool `args` JSON with the already-extracted `code`, so the live painter's own `parse_args`/`str_field` classification degrades to the raw fallback. No new rows, no test, no upstream behavior change — a data-mangling no-op. TOOL-006/011 stay `BLOCKED`. |
| MSG005 | "add `MessageRecord.files/skills`" | Invented flat `Vec<String>` fields with `serde(default)` that nothing persists (defaults are never written), plus fields injected into `ResponsesCompactionWindow` that the struct does not define. Cannot compile as written and would render no real attachment chip. MSG-005 stays `BLOCKED` on part-level attachment data. |
| MARKDOWN | "paint mermaid/latex fence footprint" | The concrete painter/engine are geometry-only, so a valid fence reserves blank canvas rows: the readable source block disappears and nothing replaces it. Worse than the fail-closed code block until a real canvas rasterizer exists. MD-007/008/009 stay `BLOCKED`. |
| MISC | "ignore an empty worktree branch" | Production `branch` comes from `VcsInfo::branch_from_head`, which already returns `None` for an empty name, so the change is behaviourally inert outside a new test. Rejected as non-progress. |
| HARNESS | "stable transcript row keys" | Empty diff; `row_key`/anchor contracts were already live. |

**MEDIA-001 (sole `REAL_GAP`) is now substrate-blocked, not wiring-blocked.** The
bridge can rasterize an existing RGBA buffer into cells
(`opentui-bridge/src/image.rs:143,294`), but the workspace has **no image decoder**
(`png`/`image`/`flate2`/`miniz_oxide` are absent from `Cargo.lock`, and builds run
`--offline --locked`), and the safe renderer exposes no graphics-protocol
passthrough (`OSC 1337`/kitty) — it paints a cell grid and strips escapes. Displaying
an image therefore needs either a vendored decoder or a renderer passthrough
channel; neither is a CLI-only patch.



## Wave-67 — complete-feature pilots, suites green (2026-11-03)

Base `f1c999f1`. Delivery changed shape here: **one complete observable feature
per expert owner in one controller-provisioned workspace** (`/tmp/fp-DIFF`,
`/tmp/fp-DIALOG`, `/tmp/fp-MCP`), each delivering a mergeable commit plus executed
evidence — not per-group fragment patches assembled by the orchestrator. The rule
is recorded in `AGENTS.md` and `docs/AGENT_WORKER_PRACTICES.md`.

**Landed on `main-v2`.**

| Commit | Change | Evidence |
| --- | --- | --- |
| `7285a322` | CP-08 catalogue-load edge retains the previous prompt label; CP-11 fills `RailFacts.location`/`location_width` so the idle footer paints the Location slot; CP-42 pins `set_prompt_turn` to worker start/settle; **THEME-09** live `ChatPart::Patch` resolves `DiffSurface::Inline` by content width and pairs `-`/`+` panes through `paint_rows_split` while narrow keeps the numbered gutter; THEME-12 confirms the theme page through live `ThemePicker::on_confirm`. | `cp08_catalog_load_retains_previous_prompt_display`, `cp11_location_slot_and_prompt_turn_chrome_drive_the_composer_status_text`, `theme09_wide_terminal_pairs_patch_panes_and_narrow_stays_unified`; CLI 2220/0 |
| `5e1a4409` | Test repairs: `session_view` posts the idle transition before `POST /view`; `session_turn_stream_api` expects the PAR-10 `seq`. | server suite |
| `4d797846` | `compaction_strategy_resolution` names the **pre-compaction** boundary head. | server suite |
| `782d275d` | Four stale fixtures account for the durable `MessageRole::Compaction` notice: `context_checkpoint_selection`, `context_selection_projection`, `session_fallback_compaction`, `session_native_compaction`. | server suite |

**Full-suite state on `f1c999f1`.** Server
`cargo test -p opencode-rk-server --tests --no-fail-fast -- --test-threads=1`
with `OC2_TEST_BINARY=$PWD/target/debug/oc2` → **exit 0, 134/134 binaries ok,
0 failures** (`/tmp/server-all4.log`). CLI `--bin oc2 --test-threads=1` →
**2220 passed, 0 failed**. The two auth suites (`prov_025_request_auth`,
`prov_030_auth_sources`) spawn the real daemon and fail fast without
`OC2_TEST_BINARY`; that is a harness requirement, not a product failure.

**Verdict corrections (independent, source-inspected).**

| Item | Was | Now | Evidence |
| --- | --- | --- | --- |
| `THEME-09` | `REAL_GAP` | `LANDED` | `7285a322`; see the row above and the landed table |
| `FFI-10` | `REAL_GAP` | `ALREADY_CORRECT` | Re-verified 2026-11-04 (independent read-only research): the live loop already consumes typed events via `native_input_decoder::TerminalInputDecoder` (fed `tui_entry.rs:10549-10562`, popped `:10564`) — bracketed-paste atomicity (`native_input_decoder.rs:430-445` → `tui_entry.rs:12635-12741`), kitty/CSI-u keys (`:487-625` → `tui_entry.rs:307-325,10699-10722`), SGR mouse (`:762-787` → `:11455-11504`), focus (`:366-377` → `:10634-10649`) and IME (`:620-623` → `:11428-11435`). The `opentui-bridge/src/terminal_input.rs::TerminalInput` parser is a second, unreferenced parser (dead code, not a parity gap); its bridge-only `CapabilityResponse` model has no live consumer. Residual: no installed-terminal capture. |
| `PAR-07` | `BLOCKED` | `REAL_GAP` | queue/steer (`tui_entry.rs:12558/12601`), submission (`:13693`) and permission reply (`:6212`) traces are live; the residual is form traces, and the CLI has no form dialog surface |

**Additional rejected patches.** Preserved with status files under
`/tmp/archive-w66b/`; none landed.

| Group | Claim | Rejection reason |
| --- | --- | --- |
| MSGROW | grow the message row model | Breaks the existing `rendered_anchor_survives_*` contracts it was meant to preserve. |
| LAYOUT | reflow the session layout | Its own new tests fail on the current base. |
| PALSEL | wire `SelectDialog` filtering into the dialog view | Unfinished mid-edit: duplicate `prev` and `.disabled(flag)` against the current zero-argument API — does not compile. Kept as design reference for pilot F2. |
| DIFF | add `DiffViewPreference` to `ChatView` | Model with no producer: nothing set it from config and the Patch painter never read it. Superseded by pilot F1. |
| VFFI | route input bytes through the OpenTUI terminal-input bridge | Feeds the parser then discards every decoded event — logging only, no behaviour change. |

## Wave-68 — persistent expert workspaces, first landings (2026-11-03)

Base `f1c999f1` → published `43050ee0`. The temp-disk pilots were replaced by
controller-managed Git worktrees on persistent disk under
`/Users/mymac/Projects/.oc2-features-20261103/`, per the owner's durable-storage
instruction. Each owner delivers one complete observable feature (production +
tests) and a mergeable commit.

**Landed on `main-v2`.**

| Commit | Change | Evidence |
| --- | --- | --- |
| `43050ee0` | **F1 / DIFF-001 (inline half).** `config.diffs.view` (`auto`/`split`/`unified`) is parsed from the global CLI config and the inline `--config` document (inline wins, bounded read) and threaded to the live inline patch renderer, which previously hard-coded `DiffViewPreference::Auto`. `auto` keeps the 120/121 content-width rule; `split`/`unified` override it. | `native_ui_preferences::diff001_preferences_tests::*` (6), `native_view_chat::diff_wiring_v2_regressions::diff001_*`, `native_diff_view::tests::diff001_surface_split_boundaries_match_upstream`; CLI 2225/0; independent review `VERDICT: APPROVE` |

**Candidates awaiting review / integration.**

| Candidate | Lane | Change |
| --- | --- | --- |
| `f26671f4` | `recovery/ui` | **F2 / DIALOG-06/08/09.** The live working-directory `DialogSelect` owns the tested `native_palette::select` model (filter, backspace, filtered navigation, PageUp/PageDown, empty states). Fixes the `51c60d58` blocker by making the select model the single source of truth (a suppressed `locked` move can no longer desync the mirror; `select_id` refuses while locked). CLI 2234/0. |
| `a08a2a06` | `recovery/runtime` | **F3 / SIDE-10.** `GET /todo` derives from the latest completed `todowrite` before the revert boundary (`SessionService::todos_at_boundary`) instead of echoing the stored row. Independent review `NEEDS_FIX`: the new behaviour is upstream-correct, but two pre-existing lifecycle tests still assert the superseded verbatim contract. Repair in flight. |

**Verdict corrections (independent, source-inspected this wave).**

| Item | Was | Now | Evidence |
| --- | --- | --- | --- |
| `STATE-002` | `BLOCKED` | `LANDED` | `6a72aa61` persists and pushes a durable compaction notice (`append_compaction_message`, `server/src/lib.rs:1954/1985`) and surfaces it as a typed part (`compaction_wire_part`, `:1499`); upstream `index.tsx:2182-2190` |
| `FORM-001` | `BLOCKED` | `ALREADY_CORRECT` | Live poll calls the 3-label opener (`tui_entry.rs:8599` → `native_view_dialogs.rs:1374-1396`), matching `routes/session/permission.tsx:232-240`; the 6-label `open_permission` is test-only |

`CP-07`, `DIFF-003` and `MSG-005` re-verified as still `BLOCKED` with the exact
missing producer recorded in `artifacts/verdicts/W69-V14-recheck-a.md`.

**Additional wave-68 re-verifications** (independent, source-inspected; full
evidence in `artifacts/verdicts/`).

| Item | Was | Now | Evidence |
| --- | --- | --- | --- |
| `SIDE-10` | `REAL_GAP` | `LANDED` | `0d4f26a7` stage-revert recompute + publish, and `a08a2a06`/`cdc73e6f` route `GET /todo` through `todos_at_boundary` (`server/src/todo_http.rs`), matching `packages/tui/src/util/todo.ts:33-43` |
| `CP-33` | unrecorded | `ALREADY_CORRECT` | Live `composer_enter_plan` (`tui_entry.rs:12502`), blank-Enter promotion (`native_composer.rs:547`), test `composer_enter_routes_through_plan_submit_queue_and_steer`; upstream `index.tsx:1650-1657` |
| `CP-35` | unrecorded | `ALREADY_CORRECT` | Live `reconcile_parts` (`tui_entry.rs:11288,12156,12179,12196,12201,13877`) + `TrackedDraft::realign_after_edit` (`native_paste.rs:1206`); 11 `cp35_*`/`reconcile_*` tests; upstream `component/prompt/index.tsx:707-761` |
| `CP-34` | unrecorded | `REAL_GAP` | `DraftStash`/`PromptStash` (`native_paste.rs:487/571`) unwired; per-tab save/restore persists draft/cursor/scroll but never `draft_parts`; missing upstream `draft-stash.ts` takeDraft/saveDraft |
| `STATE-001` | `REAL_GAP` | `REAL_GAP` | Spinner clock live as now_ms/80 index only; culling/`destroy`/frame-list/`Background::compose` (`native_progress.rs:207/226/45`) unwired |
| `STATE-003` | `REAL_GAP` | `REAL_GAP` | `retry_label`/`error_line`/`free_limit_notice`/`token_diagnostics_label` (`native_progress.rs:292-579`) only consumed by `ShellFooter` tests; no retry countdown fed live |
| `STATE-004` | `REAL_GAP` | `REAL_GAP` | `BackgroundHint`/`background_candidate` (`native_progress.rs:668-708`) only in `native_shell.rs` tests; no 3s-delayed hint source live |
| `MEDIA-001` | `REAL_GAP` | `REAL_GAP` | `open_image_preview` (`native_view_pages.rs:788`) + models (`native_image_preview.rs:1047-1161`) test-only; no live attachment/preview caller |
| `PAR-02`/`PAR-03`/`PAR-06` | `BLOCKED` | `BLOCKED` | capability table / upstream capture adapter / font-pixel substrate all still absent (holds) |
| `TOOL-011` | `BLOCKED` | `BLOCKED` | `execute_dialog` (`native_tool_rows.rs:1159`) test-only; render placeholder (`native_view_dialogs.rs:1426-1429`) |
| `LAY-01..09` | partial/unrecorded | `ALREADY_CORRECT` | every layout clause has a live consumer plus an exact-width test and a pinned-upstream match; full file:line evidence in `artifacts/verdicts/W68-V1-layout.md` |
| `TOOL-007`/`008`/`009`/`012` | unrecorded | `ALREADY_CORRECT` | live `native_view_chat.rs:2658-2759` tool/subagent row rendering with `native_tool_rows.rs` tests; upstream `index.tsx:2574-3344` (evidence in `W69-V10-tools.md`) |
| `TOOL-010` | unrecorded | `REAL_GAP` | `execute_calls`/`execute_call_summary` (`native_tool_rows.rs:278-315`) live but no live clickable child expansion or structured-argument rows; upstream `index.tsx:3348-3397` |
| `DIALOG-16`/`DIALOG-17`/`DIALOG-18` | `BLOCKED` | `REAL_GAP` | producers now exist but are unwired: settings numeric limits + save lock + experiments (`native_settings.rs:341-561`) test-only; stash/workspaces/file-changes models + `ArmedDelete` (`native_palette.rs:3078/4389`) unreachable; `diff_dialog` chrome (`native_view_dialogs.rs:3723`) zero consumers |
| `CP-17`/`CP-18` | unrecorded | `ALREADY_CORRECT` | live usage arithmetic (`tui_entry.rs:9120-9137`) and number/currency formatting (`native_view.rs:4358-4364`); tests `last_assistant_usage_honours_revert_and_compaction_boundaries`, `number_and_currency_formatting_matches_upstream` |
| `FORM-003..008` | unrecorded | `REAL_GAP` | the ported `FormPrompt` (`native_view_dialogs.rs:2615-3578`) is test-only; the live loop (`tui_entry.rs:11619-11707`) routes only Mcp/Select/permission, so there is no live daemon-form renderer/dispatch; upstream `component/form.tsx` |
| `CP-25` | unrecorded | `ALREADY_CORRECT` | live dock status cells (`native_view.rs:1617-1623`) from `SubagentsTab::rows`; `status_label` "Running" (`native_children.rs:1176`); test `cp25_running_label_is_literal_without_progress`; upstream `subagents-tab.tsx:209` |
| `CP-24`/`CP-26`/`CP-27`/`CP-28`/`CP-29` | unrecorded | `REAL_GAP` | tab/keymap/focus models exist but are unwired: `SubagentsTab` command/set_active (`native_children.rs:1620`), `SessionDock::set_shells` (`native_shell.rs:1244`), `TerminalsTab` (`native_dock.rs:1183`), `KeymapDefaults::upstream` (`native_composer.rs:1690`) and composer `PromptFocus` remount (`:1745`) all have no live consumer |
| `CP-19`/`CP-20` | unrecorded | `ALREADY_CORRECT` | live `rail_spans` fit/hover role selection (`native_view.rs:4389`); tests `cp19_usage_outranks_shortcuts_at_the_fit_boundary`, `cp20_hover_and_disabled_hints_keep_their_shortcut_identity`; upstream `footer.tsx:55-88,146-155` |
| `CP-21`/`CP-22`/`CP-23` | unrecorded | `ALREADY_CORRECT` | live queue dock rows (`tui_entry.rs:9104`, `native_view.rs:3703-3718`), prompt-slot precedence (`native_view.rs:3496-3503,3776-3783`) and dock title/hint (`native_view.rs:1091-1096,3641-3701`); tests `queue_dock_paints_only_the_published_active_session_rows`, `cp22_bottom_slot_precedence_covers_every_combination`, `cp23_title_and_hint_layout` |

**Checklist coverage.** As of this wave **all 223 items carry a verdict row**
(docs + `artifacts/verdicts/`), so the verification census is complete. The
remaining `REAL_GAP`/`BLOCKED` items below are the open implementation work, not
unverified items.

| Item | Was | Now | Evidence |
| --- | --- | --- | --- |
| `CP-37` | `unrecorded` | `REAL_GAP` | `PasteQueue`/`paste_admission` (`native_paste.rs:1303/1272`) test-only; live `tui_entry.rs:11318` pastes synchronously with no queue/admission; upstream `index.tsx:376-392` |
| `CP-38` | `REAL_GAP` (reconfirmed) | `REAL_GAP` | `image_preview` (`native_paste.rs:1375`) test-only; live composer paints no image strip; upstream `index.tsx:397-402,1680-1712` |
| `FORM-002` | `BLOCKED` | `REAL_GAP` | `PermissionPrompt` (`native_permissions.rs:557`, fullscreen/escape/ctrl-c/diff model) unwired; live `tui_entry.rs:8608` passes `message: None` and `cancel()` → Reject only; upstream `permission.tsx:428-552` |
| `DIALOG-19` | `BLOCKED` | `REAL_GAP` | `ChromeModel` (`native_modal_layer.rs:775`, owns StartupLoading/Reconnecting/MigrationOverlay) instantiated only in tests; live `native_loop` (`tui_entry.rs:6824`) builds a `FrameClock` (`:7282`) that feeds only toast/spin — no `tick`, no `ChromeFrame` painted; live reconnect is `AppView::Offline` (`native_app.rs:104`), not the Reconnecting overlay |
| `TOOL-006` | `REAL_GAP` | `REAL_GAP` | `BackgroundShell` (`native_shell.rs:1848`: `sync:1967`, `poll:1986`, `set_expanded:2008`, `apply:2026`, `SHELL_DISPLAY_LIMIT:1002`) unreferenced outside `native_shell.rs`; live `ChatPart::ToolState` render (`native_view_chat.rs:2699-2724`) paints per-tool rows only and `native_loop` drives no shell polling; upstream `index.tsx` background-shell section |
| `DIALOG-06`/`DIALOG-08`/`DIALOG-09` | `REAL_GAP` | `LANDED` | `4f4a8228` — the live working-directory `DialogSelect` owns the tested `native_palette::select` model: filtering, backspace, filtered navigation, PageUp/PageDown and empty states all execute from the running loop, and a locked select never answers (`DialogError::LockedSelection`). Independent `APPROVE`; full CLI 2239/0 |
| `DIFF-001` (viewer half) | `BLOCKED` | `REAL_GAP` | inline half `LANDED` (`43050ee0`); residual: `resolve_view_mode` Viewer arm (`native_diff_view.rs:628`) test-only, live seam renders inline (`native_view_chat.rs:2812`), pinned viewer supports `patchPaneWidth() >= 100` (`diff-viewer.tsx:349`) — needs a dedicated viewer route |
| `DIFF-005` | `unrecorded` | `REAL_GAP` | `diagnostics_for_file` (`native_diff_view.rs:1302`) reached only by tests/inline helper; live projection matches the exact path (`native_chat_projection.rs:478-481`) — normalize before lookup |
| `MD-005` | `unrecorded` | `REAL_GAP` | `MarkdownStyles::thinking` (`native_markdown_flow.rs:260`) has no live caller; paint seam (`tui_entry.rs:5094-5103`) overlays syntax colors on the muted reasoning foreground |

Base SHA `0d74af30`. 28 parallel workers on the four authorized routes, each
artifact-first and read-only on canonical. Per-ID JSON verdicts live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/`.
This pass **finalizes** the following IDs and supersedes their earlier entries.

> **This wave-44 table is HISTORICAL. Do not read a verdict from it.**
> Later waves (45–54) superseded many rows. **The authoritative verdicts are the
> summary counts at the end of this document and the `REAL_GAP` / `BLOCKED` /
> `LANDED` sections below.** Specifically, wave-44 rows that still read
> `REAL_GAP` are superseded as follows:
> - to `BLOCKED`: `PAR-04`, `PAR-09`, `PAR-10`, `PAR-19`, `SIDE-04`,
>   `MSG-001..003`, `MSG-005`, `DIALOG-12..20`,
>   `FFI-01/03/04/06/07/08/11`, `LAY-10/11/13`, `TAB-06/10`, `CP-07`,
>   `CP-09`, `MD-007`, `THEME-08/09/13/14`, `TOOL-006/011`, `DIFF-001/003`,
>   `STATE-002`, `FORM-001`;
> - to `ALREADY_CORRECT`: `PAR-11`, `LAY-12`, `LAY-17`, `MSG-004`, `FFI-05`,
>   `CP-08`, `THEME-05`, `MSG-010`;
> - to `LANDED`: `TAB-04` (wave-56, `config.tabs.indicators` wired live);
> - to `LANDED`: `MSG-007` (`4a6b487b`), `CP-11` (`f8e276cf`), `DIALOG-04`
>   (`d137313d`), `DIALOG-20` (toast half, `d891f9c3`), `TAB-23` (`ae56897f`),
>   `SIDE-08` (`76fcb75a`), `THEME-12` (`bc24dd30`), `PAR-16`/`PAR-22`
>   (`f240367b`/`99e0e6ae`), `MSG-008` (`6a49151f`).

**Reclassified / newly finalized:**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| PAR-13 | `ALREADY_CORRECT` | `native_message_rows.rs:2115` `click_acts`; live `tui_entry.rs:10102,10162`; test `native_text_selection.rs:532` |
| PAR-14 | `ALREADY_CORRECT` | `native_tui_parity.rs:186` COLUMNS=40; `upstream_view_parity.rs:76-78` 60/88/116; `:145` >120 boundary |
| PAR-15 | `ALREADY_CORRECT` | `native_theme_runtime.rs:392-434` hex/ref/{dark,light}/transparent resolve; live `tui_entry.rs:6736-6753,11425` |
| PAR-16 | `LANDED` (`f240367b`) | Fixture classes (`baseline`/`optional`/`experimental`/`extension`): every fixture resolves to exactly one class, unknown classes are rejected, `--class` filters, the class travels into the request/golden, and a golden/fixture class mismatch is a hard `fixture_class` diff (`ui_parity.py`); 5 focused selftest checks |
| PAR-17 | `ALREADY_CORRECT` | `baseline.lock.json:12-24` v2.2.0 `is_authoritative:false`; pins `native_identity.rs:36-38` |
| PAR-18 | `ALREADY_CORRECT` | `safe_renderer.rs:1148` Drop, `:609` restore; live `tui_entry.rs:12756-12759`; tests `native_terminal_lifecycle.rs:192-217`, `highlight.rs:1133` |
| PAR-19 | `LANDED` (re-verified 2026-11-07) | Consumed live by the diff viewer: `visible_chunk_range` + `placeholder_height` at `native_view_dialogs.rs:3722-3772` (split) and `:3772-3781` (virtual added-file), preserving omitted-chunk height. `relative_scroll_top` is unused because that viewer's scroll area has no virtual-root offset (`root.y == content.y` ⇒ identity), so the raw `execute_scroll` *is* the root-relative value. Residual is only the inline transcript patch, which is tail-anchored and bounded by `MAX_DIFF_LINES` + the `[diff truncated]` row |
| PAR-20 | `ALREADY_CORRECT` | `selftest.py:161-207` mutations one_cell_color/cursor/width/focus/action_extra all assert exit 1 |
| PAR-21 | `ALREADY_CORRECT` | `support/upstream_view_parity.rs:4` frozen `upstream_layout_parity.rs` untouched; sibling file |
| PAR-22 | `LANDED` (`f240367b`) | `--evidence-bundle` writes a SHA-bound bundle (`rust_sha`, spec/golden/fixture hashes, class counts, exact reproduction commands, per-fixture outcomes, installed-capture hashes) so a parity claim binds to the exact Rust revision under test; 5 focused selftest checks |
| PAR-23 | `ALREADY_CORRECT` | Every checklist item `status:unverified_rust`; kit `rust_checks_passed:null` |
| PAR-24 | `ALREADY_CORRECT` | Bridge sources present/inspectable (`native_identity.rs:36-38`, scene/highlight/terminal_input) |
| THEME-02 | `ALREADY_CORRECT` | `prompt_pane.rs:119/:1288`; `theme_runtime.rs:938`; live `tui_entry.rs:8567` |
| THEME-03 | `ALREADY_CORRECT` | `theme_runtime.rs:800,916,762`; tests `:3179,:3227`; live `tui_entry.rs:8567` |
| THEME-04 | `ALREADY_CORRECT` | `theme_runtime.rs:762`; tests `:3537`+f3_*; live `tui_entry.rs:8567` |
| THEME-05 | `REAL_GAP` | `theme_runtime.rs:1189,1198` + test `:3574`; no live per-cell ActionStates map (`PaintCtx` `tui_entry.rs:8494` threads no state map) |
| THEME-06 | `ALREADY_CORRECT` | `theme_runtime.rs:1712,1244`; test `:3148`; live `tui_entry.rs:4491` dialog_panel in modal paint |
| THEME-07 | `ALREADY_CORRECT` | `theme_runtime.rs:120,142`; tests `:2654-2691`; live `tui_entry.rs:4118` paint_color |
| THEME-08 | `PARTIAL` (upgraded 2026-11-07) | The rule model is **live**, not test-only: `SemanticPalette::syntax_rules` (`native_theme_runtime.rs:965`) is installed into a per-frame `SyntaxStyleArena` (`:2471-2530`) at `tui_entry.rs:10491,12835-12846` and released at `:13204`. The **tokenizer is live too**: `code_token_spans`/`code_token_cell_spans` (`markdown_runs.rs:69,165`) run on fenced chunks (`native_view_chat.rs:2970` `paint_plain_code_block(..., true, ...)`) and the `SYN_*` bits paint through `markdown_run_paint_color` (`tui_entry.rs:5836`, consumed `:6593`). Residual narrowed 2026-11-07 (wave-111) to **scope transport**: the bridge still exports style registration but no grammar/query channel or highlight callback/worker (`opentui-bridge/src/highlight.rs:20-31`), so classification is lexical and covers 8 of the 14 `CaptureClass` variants rather than arbitrary grammars |
| THEME-09 | `LANDED` | `7285a322`: the live `ChatPart::Patch` branch resolves `DiffSurface::Inline` by measured content width and paints `SplitRow::Pair` halves through `paint_rows_split` with `ChatRowClass::{DiffAdded,DiffRemoved,DiffHunkHeader}`, so content/gutter/span/hunk colors stay distinct in both presentations. Test `theme09_wide_terminal_pairs_patch_panes_and_narrow_stays_unified`. The config override is DIFF-001 (pilot F1). |
| THEME-10 | `ALREADY_CORRECT` | `theme_runtime.rs:1592,1665,1754,1768`; tests `:4167-4241`; live `tui_entry.rs:6744,11425` |
| TAB-21 | `LANDED` (`51538ca1`) | **Cross-crate producer built.** `SessionSummary.idle_at` (contracts) + storage `idle_at` column with idempotent migration and monotone `set_session_idle` + sessions `mark_idle` and the guarded `set_viewed`; server stamps idle on `create_turn`, the streamed `Done` frame and both compaction terminals; CLI `LiveSnapshot.session_idle_at`/`session_viewed_at` drive the live `ViewAcknowledgement` POST with backoff. CLI 2217/0, server 313/0, storage 127/0, sessions 45/0. (Wave-66 verifier VTAB re-confirmed this; the earlier `BLOCKED` entry was stale.) |
| TAB-23 | `LANDED` | `ae56897f` — live `session.tab.*` dispatch now resolves through the config `keybinds` chord table (`native_navigation::tab_keybinds`); `UiPreferences.keybinds` (raw JSON) + focused tests. |
| SIDE-03 | `LANDED` (`5e3d797b`) | Renderer+test correct (`native_sidebar.rs:230-287`); live `set_usage_classes` (SIDE-04 `0d4f26a7`) and `set_cost` (`b5ec63c7`) wired into the live `Event::Usage` arm. History-load `lastAssistantWithUsage` selector landed (`5e3d797b`): `native_chat_projection::last_assistant_usage` honours the staged revert boundary (unknown ⇒ absent) and the last completed compaction, and the per-frame publish uses the message's OWN model `context_window` (missing limit ⇒ tokens without percent), never the selected UI model. A live-turn guard keeps `Event::Usage` authoritative for its binding. |
| SIDE-04 | `REAL_GAP` | Five-class arithmetic `native_sidebar.rs:205-287` (test `:1608`) dead on live path; no last-eligible-assistant selector; limit joined only from selected UI model (`tui_entry.rs:8112`); wire `input_tokens` is inclusive of cached (`server/src/auto_compaction.rs:102`) so disjoint classes need a `nonCachedInput` producer |
| SIDE-08 | `LANDED` | `76fcb75a` (wave-48): the sidebar MCP row click opens the `DialogMcp` server list instead of PUT-toggling — `DialogKind::Mcp`, `DialogView::open_mcp`/`focused_choice`, `OpenDialog.mcp_status`, live opener `tui_entry.rs:10794`, Enter toggles the focused server through `PUT /api/sessions/{id}/mcp/{name}`, Esc closes locally. The residual select-surface behaviors (typing filter, Backspace, PageUp/PageDown, empty states) are DIALOG-06..09, tracked separately. |
| SIDE-14 | `LANDED` (`d5611712`) | Footer hit rect + hover routing + local `DialogSelect` menu + CopyPath/OpenFolder executors wired live; `Workspaces` unoffered (no live move route). See the landed-fixes table. |
| MSG-005 | `ALREADY_CORRECT` (re-verified 2026-11-07) | The whole chain exists: `MessageRecord.files`/`.skills` (`contracts/src/lib.rs:308-322`), server construction (`server/src/lib.rs:2089-2103`), projection (`native_chat_projection.rs:758-759,767-790`) and painted chips (`native_view_chat.rs:941-946,1269-1287` via `user_chip_rows`) |
| MSG-008 | `REAL_GAP` | Widths/agent/model/duration/interrupted live (`native_footer.rs:377`, `native_assistant_facts.rs:391`); TPS dead end-to-end: producer `server/src/lib.rs:2894-2901` emits no streamed stamp; `MessageExecution` (`contracts/src/lib.rs:284-293`) has no timing field; `RowTiming.streamed_ms` (`native_message_rows.rs:825`) never set; `tps_enabled` test-only (`native_assistant_facts.rs:325`); `data.session.tps` absent |
| MSG-009 | `REAL_GAP` | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) zero live consumers; `SessionRevert` (`contracts/src/lib.rs:191`) has no count; server stage `server/src/lib.rs:1199-1204` hardcodes `files:None`; no notice projection; no child-completion metadata |
| DIFF-001 | `ALREADY_CORRECT` (re-verified 2026-11-07) | `config.diffs.view` **is** parsed and live: `UiPreferences::diffs_view` (`native_ui_preferences.rs:204-240`, values `auto`/`split`/`unified` with unknown-value diagnostics; tests `:2146-2174`) → `view.set_diff_view` (`tui_entry.rs:10563-10571`) → `ChatView.diff_view` (`native_view_chat.rs:181/289`) → `resolve_view_mode(DiffSurface::Inline, …, width)` (`:3562`) → `paint_rows_split` (`:3566`) painting per-side classes. The `DiffViewMode::Unified` at `:2704` is `push_diff_fence`, the markdown **fence** painter, which upstream also renders as a plain code block (`plugin/builtins.ts:14-15`), so unified is deliberate there, not a gap. Viewer surface resolves through its own measured width (`native_view.rs:2503`) and the dialog split branch (`native_view_dialogs.rs:3709`) |
| DIFF-003 | `BLOCKED` | Virtualization model `native_diff_view.rs:1014-1076` (tests `:1962-2209`) zero callers; `AddedPatchChunk` (`:926-930`) carries no highlight field |
| STATE-002 | `ALREADY_CORRECT` (re-verified 2026-11-07) | Role and producer are live: `MessageRole::Compaction` (`contracts/src/lib.rs:254-262`, "never a provider history role") persists through sessions/storage (`sessions/src/lib.rs:335`, `storage/src/writer_v2.rs:324`, `storage/src/lib.rs:2185-2194`), the server appends the compaction row (`server/src/lib.rs:2237`) and special-cases it (`:1596,1750,4048`), and the CLI projects `ChatPart::Compaction { reason, status }` (`native_chat_projection.rs:89,425`) with a dedicated class/divider (`native_view_chat.rs:794,870-872,1121-1123`) |
| TOOL-011 | `ALREADY_CORRECT` (re-verified 2026-11-07) | Live consumer exists: clicking a tool row resolves `execute_dialog_from_part` and opens the execute viewer (`tui_entry.rs:15635-15655` → `DialogView::open_execute`, `native_view_dialogs.rs:2495`), with render tests `:11768-11847`. The `opentui-bridge` highlight gap is tracked under THEME-08 |
| FORM-001 | `PARTIAL` | **Sub-behavior 1 is already correct** (re-verified 2026-11-07): the live poll opens `open_permission_with_persistent` (`native_view_dialogs.rs:2862`, live `tui_entry.rs:11947`) which emits `Allow once` / `Always allow` / `Reject` in upstream order; the 6-label `open_permission` helper (`:2824`) has **test-only** callers. The old citation `routes/session/permission.tsx:232-240` does not resolve at pin `450e7b92`; the real authority is `mini/permission.shared.ts:36-38` (`return ["once","always","reject"]`) with labels from `permissionOptionLabel` (`util/permission.ts:162-166`). **Residual open question:** the pin's footer returns all three unconditionally, while the Rust helper drops `always` when the request has no `save` rules — a product decision, not a mechanical fix. Sub-behaviors 3+4 (always-body swap static `:1420`, nested rejection stage) still need a stage+textarea in `DialogView` |
| DIALOG-20 | `LANDED` (toast half) | `d891f9c3` — the live loop now owns a `Toast` and paints its `ToastFrame` as the topmost overlay (`PaintCtx.toast`), advancing the logical clock on the animation tick and triggering on session-id copy (`app.tsx:605`). Crash/error-details half remains `BLOCKED` (no `panic::set_hook`, no `DialogKind::Crash`). |

**Additional wave-44 verdicts (second batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| PAR-01 | `ALREADY_CORRECT` | Pins stamped into every golden (`ui_parity.py:464`), compared every `--check` (`:247-276`), negative-controlled (`selftest.py:237`) |
| PAR-02 | `BLOCKED` | Capability table empty (`audit-implementation.md:79-96`); no producer |
| PAR-03 | `BLOCKED` | Rust adapter exists (`oc2_pty.py`); upstream TS/OpenTUI adapter absent (`README.md:148`) |
| PAR-04 | `REAL_GAP` | text/pos/style/clipping/cursor covered+tested; hit targets never modeled (no `hit_targets` compare key) |
| PAR-05 | `ALREADY_CORRECT` | Wide-cell continuation + combining fold (`ui_parity.py:65-73,139-182`), tested (`selftest.py:244`) |
| PAR-06 | `BLOCKED` | Cell/text-only harness; no font/pixel capture path (`oc2_pty.py:362-369`) |
| PAR-07 | `REAL_GAP` | The old `BLOCKED` reason is stale. Three of the four cited traces are live: queue/steer (`tui_entry.rs:12558` `prompt.queued`, `:12601` `prompt.queued_promoted`), submission (`:13693` `prompt.submit`), and permission reply (`:6212` `permission.reply`, reached from `dispatch_permission_reply`). Residual: form submit/validation events — the native CLI has no form dialog surface at all (`DialogKind` has no form variant), so no form trace can be emitted. Frame capture (`native_capture.rs`) is complete. |
| PAR-08 | `ALREADY_CORRECT` | `native_clock.rs` FrameClock; live `tui_entry.rs:6666` |
| PAR-09 | `REAL_GAP` | async syntax await + stale/blank rejection in `highlight.rs`; `pub mod highlight` has zero cli call sites |
| PAR-10 | `REAL_GAP` | `native_transcript.rs` accums; live path consumes only `strip_ansi` |
| PAR-11 | `REAL_GAP` | `native_message_rows.rs` anchors/identity/group-expansion; no `tui_entry` consumer |
| PAR-12 | `ALREADY_CORRECT` | modal keymap scope/priority/suppression live (`native_view_dialogs.rs:652-828`); dock/side-panel scope unasserted |
| TAB-01 | `ALREADY_CORRECT` | `native_tabs.rs:1440`; test `:3576`; live `tui_entry.rs:8245` |
| TAB-02 | `ALREADY_CORRECT` | `native_tabs.rs:34`; test `:3607`; live `tui_entry.rs:5032` |
| TAB-03 | `ALREADY_CORRECT` | `native_tabs.rs:1592`; test `:3640`; live `tui_entry.rs:4998` |
| TAB-04 | `LANDED` (wave-56) | numbers-mode `TabIndicators` now has a live config source: `config_tab_indicators` + `UiPreferences::tab_indicators` read `config.tabs.indicators` (`config/index.tsx:199,336`), the live loop applies it to every `TabInput`, and `horizontal_row` paints the number in `Numbers` mode |
| TAB-05 | `ALREADY_CORRECT` | `native_tabs.rs:754`; test `:3783`; live `tui_entry.rs:8215` |
| TAB-06 | `REAL_GAP` | `enqueue_user_prompt`/`pending_marks_busy` (`native_tabs.rs:1877/1893`) zero callers; `queued_drafts` push (`tui_entry.rs:11512`) carries no per-tab flash/busy feed |
| TAB-07 | `ALREADY_CORRECT` | `native_tabs.rs:1940`; test `:3852`; live `tui_entry.rs:4964` |
| TAB-08 | `ALREADY_CORRECT` | `native_tabs.rs:2027`; test `:4032`; live `tui_entry.rs:5009` |
| TAB-09 | `ALREADY_CORRECT` | `native_tabs.rs:2146`; test `:4052`; live `tui_entry.rs:7930` |
| TAB-10 | `LANDED` (`53a3efe4`) | `VcsInfo` + git HEAD parsers (`native_navigation.rs`); `git_vcs_info` producer + `LiveSnapshot.session_vcs` + live `rail_inputs_for_navigation` (`tui_entry.rs:5277`) so the tab rail shows branch/worktree from the session cwd. Wave-66 verifier VTAB re-confirmed the row is implemented and live; a proposed empty-branch guard was rejected as production-neutral (`branch_from_head` already yields `None` for an empty name). |
| TAB-11 | `ALREADY_CORRECT` | `native_tab_rail.rs:778`; test `:2363`; live `tui_entry.rs:8281` |
| TAB-12 | `ALREADY_CORRECT` | `native_tab_rail.rs:1138`; test `:2228`; live `tui_entry.rs:5072` |
| MD-001..006 | `ALREADY_CORRECT` | Live path `native_view_chat.rs:2320 push_block_markdown` → `native_markdown_flow.rs:770` → paint `tui_entry.rs:4753`; per-ID tests in `native_markdown_flow.rs` |
| MD-007 | `REAL_GAP` | `set_fence_handlers`/`MarkdownPluginRegistry` zero non-test callers; seam `native_view_chat.rs:2334` (flow built, no handlers fed) |
| MD-008 | `BLOCKED` | mermaid renderer built `without_painter` (`native_view_chat.rs:2276`); needs live `DiagramPainter` |
| MD-009 | `BLOCKED` | math renderer built `without_engine` (`native_view_chat.rs:2298`); needs live `MathLayoutEngine` |
| CP-07 | `BLOCKED` | The `AgentFact` model (`native_prompt_pane.rs:580-631`) is test-only: no live caller of `selectable_agents`/`agent_color`, `set_composer_agent` (`native_view.rs:1232`) has zero non-test callers so `composer_agent` is always `None` live, `agent.cycle` is inert (`tui_entry.rs:10807-10811`), `DialogKind` has no Agent variant, and the `mention_agents` contract (`configured_commands.rs:80-99`) emits only name/description/read_only — no `hidden`/`subagent`/`color`. Needs a cross-crate contract field + live selection state |
| CP-08 | `REAL_GAP` | `next_prompt_display` (`native_prompt_pane.rs:675`) not routed via `composer_metadata` (`native_view.rs:1612`) |
| CP-09 | `REAL_GAP` | `prompt_highlight`/`border_highlight`/`MetadataReveal` (`native_prompt_pane.rs:137,155,702`) not consumed by Paint path |
| CP-10 | `ALREADY_CORRECT` | `rail_spans`/`slot_text` (`native_status_rail.rs:874,1048`); live `native_view.rs:3867-3904` → paint `:3820-3822` |
| CP-11 | `REAL_GAP` | `RailFacts::status_slot` (`native_status_rail.rs:739`); live `footer_line` (`native_view.rs:3879`) never sets `running`/`move_progress`/`pending_new`/`location` |
| LAY-16 | `ALREADY_CORRECT` | `native_layout.rs:405-420`; live `native_shell_frame.rs:279` |
| LAY-17 | `REAL_GAP` | Anchor model `native_view_layout.rs:1014-1320` (tests `:1916-2196`) zero non-test callers; live transcript scroll/tab-restore never calls it |
| LAY-18 | `ALREADY_CORRECT` | `native_home.rs:200`; live `native_view.rs:1451` |
| LAY-19 | `ALREADY_CORRECT` | `native_home.rs:277`; live `native_view.rs:2042` |
| LAY-20 | `ALREADY_CORRECT` | `native_home.rs:88-146`; live `native_view.rs:4175/4200` |
| MSG-006 | `ALREADY_CORRECT` | `click_acts` (`native_message_rows.rs:2109-2117`, test `:2519`); live `tui_entry.rs:10102-10106` gates message-menu + child-nav |
| MSG-007 | `REAL_GAP` | `MessageActionMenu` (`native_view_chat.rs:3568-3760`) live dispatch `tui_entry.rs:3745-3829`; ForkSession → notice only (`:3820-3822`); `menu.resync` (`native_view_chat.rs:3642`) zero live callers |
| SIDE-11 | `ALREADY_CORRECT` | `native_sidebar.rs:426`; test `:1739`; live `native_view_status.rs:871` → `native_view.rs:3360` |
| SIDE-12 | `ALREADY_CORRECT` | `native_sidebar.rs:1063`; test `:1936`; live `native_view_status.rs:354` → `native_view.rs:271/:308` |
| SIDE-13 | `ALREADY_CORRECT` | `native_sidebar.rs:64,777,949`; test `:2081`; live `native_view.rs:3904` |
| SIDE-15 | `ALREADY_CORRECT` | `native_sidebar.rs:406-409`; test `:2170`; live `native_view.rs:3360` |

**Additional wave-44 verdicts (third batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| FFI-07 | `REAL_GAP` | `highlight.rs:683` impl, `:1133` test; no cli consumer (cli renders through its own `native_*` models) |
| FFI-08 | `REAL_GAP` | `diff.rs:1140` impl, `:1434` test; live path is separate `native_diff_view.rs` |
| FFI-09 | `ALREADY_CORRECT` | `scroll.rs:328/:411`; consumer `native_view.rs:5580` (`scrollbar_thumb`) |
| FFI-10 | `REAL_GAP` | `TerminalInput` parity parser (`terminal_input.rs:327-360`, tests `:1113`) has **no live consumer**: `safe_renderer.rs:69` declares its own termios struct of the same name (the input-mode owner), not the parser, and no CLI module constructs `terminal_input::TerminalInput`. The wave-66 VFFI patch fed bytes into it but discarded the decoded events, so it changed no behaviour. Needs the live input loop to consume typed events (paste atomicity, kitty/CSI-u keys) instead of the byte-level `native_input_decoder`. |
| FFI-11 | `REAL_GAP` | `scene.rs:442` impl, `:1149` test; no cli consumer |
| MD-006 | `ALREADY_CORRECT` | `native_view_chat.rs:2432`, test `:5826`, consumer `:2574`; redacted-only renders nothing (`:2586`) |
| MD-007 | `REAL_GAP` | `MarkdownPluginRegistry`/`set_fence_handlers` (`native_markdown_flow.rs:468/681`, test `:2570`) zero non-test callers; live flow `native_view_chat.rs:2334` never binds registry |
| MD-008 | `REAL_GAP` | mermaid fan-out/fallback (`native_mermaid.rs:270/313/642`, tests `:1110-1393`); live `without_painter` (`native_view_chat.rs:2276`) forces `Fallback` (`:2280`); needs live `DiagramPainter` injection |
| MD-009 | `REAL_GAP` | math lifecycle (`native_math.rs:676`, `last_good` `:657`, test `:943`); live `without_engine` (`native_view_chat.rs:2298`) forces `Fallback` (`:2302`); needs live `MathLayoutEngine` injection. **Supersedes** the earlier `ALREADY_CORRECT` entry |
| MSG-010 | `SUPERSEDED` (2026-11-07) | The `RowMessage` turns model (`legacy_turns`/`input_index`/`turn_duration`/`turn_tokens_per_second`, `native_message_rows.rs:1185-1277`) runs only from its own tests, and that is correct, not a gap: its marker-era half needs `RowRole::Idle` records, which native sessions never persist (`legacy_turns` reduces to `true` ⇒ nearest-input boundary). The live equivalents are `project_with_turn_duration` (`native_chat_projection.rs:815-837`: completion − nearest preceding user/system `created_at`, hidden when non-positive) and `AssistantFacts::tokens_per_second` over the server's `streamed_ms` (`native_assistant_facts.rs:257-270`, provider-active time only), rendered by the footer (`native_footer.rs:143`). No production consumer is missing. |

**Additional wave-44 verdicts (fourth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| LAY-01..09 | `ALREADY_CORRECT` | Live path `native_view_layout::{Layout,compose_frame}` + `NativeView::presentation/sidebar_visible`, driven by `tui_entry.rs:8334 view.render_typed`; per-ID consumers traced (LAY-01 `tui_entry.rs:8334`; LAY-02 `native_view.rs:4089/2128`; LAY-03/05 `tui_entry.rs:8148/7389`; LAY-04 `rail_resize.preferred()`; LAY-06/07 `native_view.rs:1855-1856`; LAY-08 `tui_entry.rs:8973/8988`; LAY-09 `tui_entry.rs:7624`) |
| LAY-10 | `REAL_GAP` | 107/108 fullscreen threshold + tabs-hide rule model-complete (`native_pane_resize.rs:551/558/586`, `native_host.rs:684/691`) but `NativeHost::new` is `#[cfg(test)]`-only (`native_host.rs:755,772`); `NativeView` carries no plugin-panel/terminal pane state; seam `tui_entry.rs:8334` |
| LAY-11 | `REAL_GAP` | Precedence single-valued island-only; seam `native_view_layout.rs:808 right_pane` |
| LAY-12 | `REAL_GAP` | Enter/permission guard in unstepped `native_host.rs:324-433`; only cosmetic `set_rail_interactive` live |
| LAY-13 | `REAL_GAP` | handle/drag/double-click in `native_pane_resize.rs:276,308`, test-only |
| LAY-14 | `ALREADY_CORRECT` | Independent scroll live-wired; consumed `tui_entry.rs:10010,7160,10196` |
| LAY-15 | `REAL_GAP` | Reserved action row const self-asserted only (`native_view_layout.rs:1898`), not painted live |
| SIDE-01 | `ALREADY_CORRECT` | `native_sidebar.rs:48-88,1346-1385`; live `native_view.rs:3356-3574` |
| SIDE-02 | `ALREADY_CORRECT` | `native_sidebar.rs:1153-1306`; live `native_view_status.rs:829-887` |
| SIDE-06 | `ALREADY_CORRECT` | `native_sidebar.rs:419-492`; live `native_view.rs:3512` |
| SIDE-07 | `ALREADY_CORRECT` | `native_sidebar.rs:364-590`; live `native_view.rs:3471-3494` |
| SIDE-09 | `ALREADY_CORRECT` | `native_view_status.rs:739-765`; `native_ui_preferences.rs:70-75`; live `tui_entry.rs:6772,10021-10036` |
| SIDE-10 | `BLOCKED` | Rendering correct (`native_sidebar.rs:640-732,1408-1441`); derivation seam missing: `todo_http.rs:31-38` returns stored `session_todos` verbatim; `stage_session_revert` (`server/src/lib.rs:1193-1209`) stores boundary but never recomputes; `tui_entry.rs:7508` refetches stored list |
| THEME-11 | `ALREADY_CORRECT` | Live loader `tui_entry.rs:6083` handles missing/invalid/unknown with fallback; consumer boot `:6744` + picker `:11425`; tests `:3020-3067` |
| THEME-12 | `REAL_GAP` | `ThemeController` (`native_theme_runtime.rs:1860`, tests `:3803/:3830/:3860`) but picker confirm `tui_entry.rs:11422-11430` calls `load_native_theme`+`persist_theme_selection` directly; no live preview-on-move/cancel-revert |
| THEME-13 | `REAL_GAP` | `SystemThemeState` (`native_theme_runtime.rs:2135`, tests `:3897/:3918/:3941`) but boot `tui_entry.rs:6738-6753` picks mode from prefs only; no palette probe/reply |
| THEME-14 | `REAL_GAP` | `SyntaxStyleArena` (`native_theme_runtime.rs:2374`, test `:3986`) zero non-test callers; `native_paint` `tui_entry.rs:4469` installs no syntax style |

**Additional wave-44 verdicts (fifth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| FFI-01 | `REAL_GAP` | cli never calls `probe_installed` (identity probe runs only in its own test `native_identity.rs:493`) |
| FFI-02 | `REAL_GAP` | `Scene`/`GenHandle` unused by cli; scene consumes handle/text internally |
| FFI-03 | `REAL_GAP` | `BridgeHandle` unused by cli |
| FFI-04 | `REAL_GAP` | cli renders via `safe_renderer::Renderer`, not `frame::FrameController` |
| FFI-05 | `REAL_GAP` | `text::wrap_cells` unused by cli; `text.rs:95-96` self-declares native equivalence NOT demonstrated |
| FFI-06 | `REAL_GAP` | cli markdown uses `native_markdown_flow`, not `markdown::MarkdownDoc` |
| THEME-15 | `ALREADY_CORRECT` | `native_theme_runtime.rs:2627 cursor_wire`; live `tui_entry.rs:8447 set_cursor_style`; tests `:4012/:4052/:4085` |
| THEME-16 | `ALREADY_CORRECT` | `native_text_selection.rs:372 ctrl_c_selection_key`; live `tui_entry.rs:10595/:10646`; paint `:4784/:5150`; tests `:402/:427` |
| THEME-17 | `ALREADY_CORRECT` | `native_view_layout.rs:515 char_width` + ZWJ/VS16/Tamil; `native_composer.rs:5-12`; live cursor `tui_entry.rs:8434`; tests `native_composer.rs:2373/2440/2450/2473` |
| THEME-18 | `ALREADY_CORRECT` | one segmentation (`visit_cells`/`visit_unpadded_cells`) shared by paint/clip/cursor/hit-test (`native_dock.rs:304`); tests `native_view.rs:7825`, `native_dock.rs:1885` |
| DIALOG-11 | `ALREADY_CORRECT` | palette module + live `view.command_palette_activate` dispatch (`tui_entry.rs:10727-10746`) |
| DIALOG-12 | `REAL_GAP` | `ModelPickerState` base wired (`native_setup.rs:144`→`tui_entry.rs:7355/9992`); `ModelVariantPicker`/`next_dialog_after_model_select`/favorites snapshot no live caller |
| DIALOG-13 | `REAL_GAP` | `SessionPicker` referenced only in `native_palette.rs` tests; seam `tui_entry.rs:10917` |
| DIALOG-14 | `REAL_GAP` | `DrillDown`/`LateLoadGuard` test-only; seam `tui_entry.rs:10917` |
| DIALOG-15 | `REAL_GAP` | palette `McpStatus`/`IntegrationPhase` test-only; seam `tui_entry.rs:10917` |

**Additional wave-44 verdicts (sixth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| CP-01 | `REAL_GAP` | `PromptSlot`/`prompt_slot` (`native_prompt_pane.rs:202,221`, test `:1665`) uncalled; live `native_view.rs:3056` ad-hoc slot |
| CP-02 | `REAL_GAP` | live `lip_row(width,[0x14,0x14,0x14,0xff])` hardcodes opaque bg; transparent variant (`native_prompt_pane.rs:1276`) unreachable |
| CP-03 | `ALREADY_CORRECT` | `prompt_surface` `native_prompt_pane.rs:119`; live `tui_entry.rs:8567` |
| CP-04 | `ALREADY_CORRECT` | rules `native_prompt_layout.rs:148-190`; live `native_view_chat.rs:928`→`native_view.rs:3137` |
| CP-05 | `REAL_GAP` | `metadata_runs`/`metadata_style` (`native_prompt_pane.rs:340,477`) no live producer; seam `native_view.rs:1612,3149` |
| CP-06 | `REAL_GAP` | `metadata_layout` (`native_prompt_pane.rs:378`) ports `metadata.tsx:117-156` but live `composer_metadata` re-implements it; seam `native_view.rs:1612` |
| TOOL-001 | `ALREADY_CORRECT` | `canonical_tool_name` (`native_tool_rows.rs:78`) + 14-kind enum (`:91-130`); live `native_view_chat.rs:2599` |
| TOOL-002 | `ALREADY_CORRECT` | spinner swap (`native_tool_rows.rs:1296-1306`), permission/error order (`:1283`); live `tui_entry.rs:7947`→`native_view.rs:1001`→`native_view_chat.rs:3086` |
| TOOL-003 | `ALREADY_CORRECT` | 2-cell icon column `header_prefix` (`:1345`), Background badge (`:1316`, test `:1748`); live `native_view_chat.rs:2633` |
| TOOL-004 | `ALREADY_CORRECT` | `primitive_input_summary:238`, `strip:1448`; tests `:1624/:1934`; live `native_view_chat.rs:2620/2633` |
| TOOL-005 | `ALREADY_CORRECT` | `SHELL_BUDGET_LINES:40`, `collapse_tail:1024`; tests `:1549-1602`; live `native_view_chat.rs:2607` |
| TOOL-006 | `REAL_GAP` | `BackgroundShell`/`ShellLoad`/`ShellChunk` (`native_shell.rs`) only in-file; client shells use one-shot `/shell/stream` (`server/src/lib.rs:294`); seam `native_shell.rs:1812` |
| MEDIA-001 | `REAL_GAP` | model+tests (`native_image_preview.rs:458,1047`, test `:1231`); page render live (`native_view_pages.rs:1056`→`native_view.rs:3031`) but no live caller of `PageState::open_image_preview` (`native_view_pages.rs:788`); preview always `None` |
| DIALOG-06 | `REAL_GAP` | `SelectDialog::filtered()` (`native_palette.rs:1409`, test `:3408`); no live consumer; seam `native_view_dialogs.rs:1423-1428` placeholder render |
| DIALOG-07 | `REAL_GAP` | `row_footer()` (`native_palette.rs:1136`, test `:3832`); same seam |
| DIALOG-08 | `REAL_GAP` | `page_down/page_up` (`native_palette.rs:1598`, test `:3442`); same seam |
| DIALOG-09 | `REAL_GAP` | `SelectExperiment` (`native_palette.rs:1151`, test `:3727`); same seam |
| DIALOG-10 | `LANDED` | `a52c222a` — live session rename drives the `DialogPrompt` (`DialogView::open_prompt` + `prompt_*` forwarders over `native_palette::select::PromptState`); ad-hoc paint path removed. |
| MSG-001 | `REAL_GAP` | Row model `reduce_session_rows` (`native_message_rows.rs:242,265,150,2088,960`, tests `:2148-2232`) never called by the live painter (`native_view_chat.rs:2505/3340` flatten `ChatMessage.parts`; grouping only by `starts_new_group` `:1294`) |

**Additional wave-44 verdicts (seventh batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| CP-40 | `ALREADY_CORRECT` | `native_completion.rs` Kind/destructive/directory/hover + `panel_state`; 6 tests `:965-1169`; live `tui_entry.rs:6996-9101` |
| CP-41 | `ALREADY_CORRECT` | `native_text_selection.rs` cell-based spans; tests `:611`, `native_completion.rs:1232/1246`; live `tui_entry.rs:8489-10647` |
| CP-42 | `REAL_GAP` | Retry-notice transcript half live (`tui_entry.rs:2913`); `PromptSurface` streaming/retry chrome (`native_composer.rs:1481-1568`, test `:2078`) no live consumer; seam `tui_entry.rs:6498-6510` |
| MSG-002 | `REAL_GAP` | `native_message_rows.rs:165,177,191,203,150` (tests `:2244-2266`); `DisclosureState` zero non-def callers; live render always-expanded (`native_view_chat.rs:2569`) |
| MSG-003 | `REAL_GAP` | `partition_pending`/`group_refs` (`native_message_rows.rs:710,665,678`, tests `:2283/:2315`) uncalled; live pending = flat call-id set (`native_view_chat.rs:205`, wired `tui_entry.rs:7815,7861`) |
| MSG-004 | `REAL_GAP` | Semantic anchor model (`native_message_rows.rs:1400-1559`, `native_view_layout.rs:1042,1219,1291`; tests `:2346-2422`, `native_view_layout.rs:1989-2196`) zero runtime callers; live tab switch saves bare `view.chat_scroll_offset()` (`tui_entry.rs:12907`) via `native_navigation.rs:14,44,58` |

**Additional wave-44 verdicts (eighth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| DIALOG-01 | `ALREADY_CORRECT` | widths/cap/placement live in `native_modal_layer.rs:109,704,721`; consumer `native_view.rs:3696` (`native_palette.rs:647/656` is a test-only dup) |
| DIALOG-02 | `REAL_GAP` | shell gap/row-pad/footer-band helpers (`native_palette.rs:667-733`, test `:3808`) zero production callers. The shared-shell half is live-equivalent (`native_modal_layer.rs:704/721/735` duplicates `dialog_width`/`dialog_top`/`footer_is_narrow` with the same formulas), but the requirement's "list row padding / footer bands / both experiment states" is the `dialog-select.tsx`+`dialog.tsx` `newDesigns` behavior: upstream gates the title weight/color, description, footer gap and every select-row pad on `newDesigns` (`dialog.tsx:109-160`, `dialog-select.tsx:875-941`), while the live `DialogView::layout` (`native_view_dialogs.rs:1627`) never reads `new_designs` and uses its own fixed `" > "`/`"   "` row marker. The `SelectDialog` port that does implement it (`native_palette.rs:1251`) has zero live callers. **Wave-56 landed the title-weight/color half**: the live `DialogView` now carries `new_designs` (`set_new_designs`, seeded from the live preference read at `tui_entry.rs:6958`) and paints the non-permission title as `Role::Item` (base color, unbolded) under the experiment, matching `dialog.tsx:109`; the Permission heading keeps `Role::Warning` (security signal). Remaining: the description gating, the footer-gap gating (upstream *adds* a row for the `footerGap={2}` callers — `dialog.tsx:159`), and the per-select-row pad (`dialog-select.tsx:875-941`, consumed only by the non-live `SelectDialog`). |
| DIALOG-03 | `ALREADY_CORRECT` | live stack `native_modal_layer.rs:437/382/387/532`; remount/refocus tests `:1150/:1292`, `native_view.rs:7728`; consumed `tui_entry.rs:7865/10676` |
| DIALOG-04 | `REAL_GAP` | ordered routes `escape_route:808`/`ctrl_c_route:821` (test `:7316`) no production caller; live `tui_entry.rs:10671-10700` does own selection/clear then `dialogs.cancel()` |
| DIALOG-16 | `REAL_GAP` | Base `SettingsDialog` shell is live (`tui_entry.rs:6340/7963/10735`), but the requirement's sub-behaviors are test-only: `NumericLimit`/`SCROLL_SPEED`/`LEADER_TIMEOUT`/`ALERT_VOLUME` (`native_settings.rs:509-559`) and `ExperimentsDialog` (`:561`) have zero non-test callers; `ThemePreview` (`native_palette.rs:2946`) no live consumer. Seam `tui_entry.rs:8832-8875`. (Resolved disagreement: the retry verdict is upheld over the earlier `ALREADY_CORRECT`.) |
| DIALOG-17 | `REAL_GAP` | `native_palette.rs:2999,3037,3141,3287` (tests `:4393,4423,4464`); no live consumer |
| DIALOG-18 | `REAL_GAP` | `native_view_dialogs.rs:3432,3609,3661,3767` (tests `:5351,5365,5430,5464,5493`); `diff_dialog` module test-only |
| DIALOG-19 | `REAL_GAP` | `native_overlay_stack.rs:38,360,243` + `native_modal_layer.rs:775` (tests `:1001..1181`); `ChromeModel`/Startup-Loading test-only |

## Final consolidated verdicts — all 223 items

Wave-44 is authoritative: where it disagrees with an earlier wave's per-ID entry,
the wave-44 verdict supersedes it.

| Verdict | Count |
| --- | --- |
| `ALREADY_CORRECT` | 145 |
| `REAL_GAP` | 1 |
| `BLOCKED` | 37 |
| `LANDED` (wave-45..81) | 40 |
| **Total** | **223** |

### Landed fixes (wave-45..56) — moved out of `REAL_GAP`/`BLOCKED`

| Item | Commit | What changed |
| --- | --- | --- |
| LAY-15 | `774ccbcd` | Live reserved action row is now derived from `SESSION_RESERVED_ACTION_ROWS` (`native_layout.rs:380`) instead of a hardcoded `u16::from(composer_room > 0)`; the live composed frame asserts `regions.jump.h` against the const. Behavior-preserving. Focused tests + full CLI suite (2164/0) green. |
| CP-01 | `a98c9afb` | Live session composer slot now resolves through `native_prompt_pane::prompt_slot` (`NativeView::session_prompt_slot`) instead of ad-hoc `permission_rows.is_some()`; an open auxiliary composer/dock or child-without-forms wins the slot, permission outranks form (upstream Switch `routes/session/index.tsx:1591-1660`). Focused tests + full CLI suite (2166/0) green. |
| CP-11 | `f8e276cf` | The shared footer line now feeds the provider-turn fact + a live monotonic clock into `RailFacts` and paints `RailSlot::Status` as the left term when non-empty, so a running session replaces the idle directory with the scanner + interrupt label (upstream `prompt.footer.status` Switch `index.tsx:1901-1967`). Adds `NativeView::set_rail_status_clock`, fed from the live loop via `busy_now` + `busy_since`. Focused tests + full CLI suite (2168/0) green. |
| CP-02 | `f8e276cf` | The live composer lip row now takes its background from the resolved `promptBg` (`prompt_surface`) instead of a hardcoded opaque value, so an alpha-0 transparent surface substitutes spaces (`index.tsx:1879,1890-1895`). Adds `PromptPaneInput.background` + `ChatView::set_prompt_background`, threaded from the live frame (`render_styled_with_spans_mode` → `rendered_prompt_pane`). Focused tests + full CLI suite (2168/0) green. |
| CP-05 / CP-06 | `3ef78186` | The live in-box metadata row now resolves through `native_prompt_pane::{metadata_layout, metadata_text}` instead of a parallel re-derivation, so the candidate order, the <44-column agent/provider drop, the provider-shortening fallback and the nine-cell model floor all come from the single authoritative model (`metadata.tsx:117-167`). Behavior-preserving; focused metadata tests + full CLI suite (2168/0) green. |
| MSG-007 | `4a6b487b` | The live message-click path opened the action menu with `message=None`, so Revert/Copy/Fork resolved against an empty snapshot. Now passes the live projection's `ChatMessage` at open and adds `SessionMessageMenu::resync`, called from the live input loop before dispatch so an open menu tolerates a projection resync without retargeting (`dialog-message.tsx:23`). Focused test + full CLI suite (2169/0) green. |
| SIDE-08 | `76fcb75a` | The sidebar MCP row click now opens the `DialogMcp` server list (`dialog-mcp.tsx`) instead of PUT-toggling in place: adds `DialogKind::Mcp`, `DialogView::open_mcp`/`open_kind`/`focused_choice`, `OpenDialog.mcp_status`, the Mcp render arm, `NativeView::mcp_entries`/`mcp_dialog_open`/`mcp_dialog_toggle`/`mcp_server_failed` and `dispatch_mcp_toggle`; Enter toggles the focused server via the same `PUT /api/sessions/{id}/mcp/{name}`, Esc closes locally. Focused tests + full CLI suite (2172/0) green. |
| MSG-007 / CP-11 review fix | `df4f4e29` | MSG-007: `SessionMessageMenu::resync` now takes the captured session id and guards `actions.session_id() == Some(session_id)`, so a live-session switch with a colliding message id cannot retarget Copy/Revert/Fork (`dialog-message.tsx:23`). CP-11: the running scanner + interrupt label moved from the shared shell footer (`footer_line`) into the composer footer slot (`composer_footer_row`, the `prompt.footer` slot) via `NativeView::composer_status_text`; the shell footer keeps the idle directory. Focused tests + full CLI suite (2172/0) green. |
| THEME-12 | `bc24dd30` | The live theme picker (`tui_entry.rs`) persisted only on Enter, so arrowing through the list showed no preview and Esc left the last previewed theme in place. Adds `ThemePicker` (a live wrapper over the tested `ThemeController`) plus `theme_picker_move`/`theme_picker_cancel`: page_next/page_prev on the Theme page preview+persist the focused row, Esc restores+persists the pre-open selection, Enter clears the picker (`dialog-theme-list.tsx:32-37` -> `context/theme.tsx:341-349`). Focused test `theme12_live_picker_previews_on_move_and_reverts_on_cancel` + full CLI suite (2173/0) green. |
| DIALOG-04 | `d137313d` | The ordered modal routes (`native_modal_layer.rs:489-600`) had no live consumer: the live loop quit the app on Ctrl-C even with a dialog open, and a backdrop click did nothing. Ctrl-C (0x03) now routes through the same close/reject path as Escape while the dialog stack owns input (`dialog_consumes_byte`); Ctrl-D (not bound in the modal layer) keeps its byte-exit meaning. A left-button release outside the panel rect closes the top dialog (`ui/dialog.tsx:54-60`); a pending decision is never abandoned. Focused test `dialog04_ctrl_c_closes_dialog_but_ctrl_d_keeps_byte_exit` + full CLI suite (2174/0) green. |
| PAR-04 | `669036cd` | Harness-side (Python): the Rust capture model already serialized `Frame.hit_targets` (`native_capture.rs:142`, canonical `hit={x},{y},{action}`) but the renderer transport dropped the key and the comparator had no member. Adds the key to every renderer output, a canonical `(y,x,action)`-sorted comparator member gated on the fixture's `compare` list, validation, README, selftest coverage and the matching empty-list goldens. The PTY channel exposes no pointer events, so the live `oc2-pty` adapter emits an empty list; the member is exercised by the deterministic static renderer. `tools/ui_parity/selftest.py` 22 checks PASS. |
| TAB-23 | `ae56897f` | The live `session.tab.*` dispatch was a hardcoded `InputEvent → TabCommand` match and the config `keybinds` root key (`config/index.tsx:78` `Schema.optional(TuiKeybind.KeybindOverrides)`) was never read. Adds `UiPreferences.keybinds` (raw JSON, like `cursor`), a `native_navigation::tab_keybinds` resolver (defaults + per-command replace, `false`/`"none"` disable, `KeyStroke` objects, both command-name spellings, unknown keys ignored, `super`/`hyper` dropped), and drives the live tab dispatch through the resolved chord table. No config yields exactly `DEFAULT_TAB_KEYBINDS`. Focused tests + full CLI suite (2176/0) green. |
| DIALOG-10 | `a52c222a` | The live session rename painted an ad-hoc rectangle with its own `String` buffer and never used the tested `DialogPrompt`. Adds `DialogView::open_prompt` (kind `Prompt`, seeded from `native_palette::select::PromptState`) plus `prompt_value`/`push`/`backspace`/`submit`/`escape`/`set_busy`/`set_message` forwarders, renders the value (or the busy line) and the upstream footer `Enter: save · Esc: cancel`, and routes the live rename through it (`MenuAction::Rename` opens the prompt; the key block edits/submits; the settlement closes it or sets busy/inline-error). Escape/submit are suppressed while busy (`dialog-prompt.tsx:43-46,93-100,168-170`). The ad-hoc `PaintCtx.rename_dialog` paint path is removed. Focused test + full CLI suite (2177/0) green. |
| DIALOG-20 (toast half) | `d891f9c3` | The tested `Toast`/`ToastFrame` model had no live consumer. Adds `PaintCtx.toast`, paints the frame as the topmost overlay in `native_paint` (surface fill + each styled `CellRun`), owns a live `Toast` in the loop whose logical clock advances on the animation tick so a visible toast auto-dismisses after 5000 ms, and triggers it from the session-id copy path (`Copied to clipboard`, `app.tsx:605`). The crash/error-details half remains `BLOCKED`. Full CLI suite (2177/0) green. |
| PAR-16 / PAR-22 | `f240367b` | Harness-side (Python). PAR-16: fixtures carry exactly one class (`baseline`/`optional`/`experimental`/`extension`); `fixture_class` defaults to `baseline`, `validate_fixture_class` rejects an unknown value, `--class` filters the selection, the class travels into `build_request`/`golden_from_screen`, and `compare_screens` raises a hard `fixture_class` diff on a golden/fixture class mismatch. PAR-22: `--evidence-bundle` + `build_evidence_bundle` write a SHA-bound bundle (`rust_sha`, spec/golden/fixture sha256s, class counts, exact reproduction commands, per-fixture outcomes, installed-capture hashes). `tools/ui_parity/selftest.py` 32 checks PASS (+10). |
| TAB-04 | `bf779d09` | The tested numbers-mode `TabIndicators`/`indicator_text` model had no live config source: `config.tabs.indicators` (`config/index.tsx:199`, default `"status"` at `:336`, consumed `session-tabs.tsx:525`) was never read, and the live horizontal row painted the status glyph unconditionally. Adds `config_tab_indicators`/`config_tab_indicators_present`, `parse_cli_tab_indicators` + `UiPreferences::tab_indicators` (disk then inline; an omitting document never clobbers a lower-precedence value), `TabInput.indicators`/`number` (+ `with_indicators`), the 1-based number in `from_navigation`, and the numbers branch in `horizontal_row`; the live loop resolves it once beside `new_designs` and applies it to every per-frame `TabInput`. Status mode unchanged. Focused tests + full CLI suite (2181/0) green. |
| DIALOG-02 (title half) | `ce9dc211` | The live `DialogView` now carries the new-designs experiment fact (`set_new_designs`, seeded `tui_entry.rs:6958`) and paints the non-permission title `Role::Item` (base color, unbolded) under the experiment (`dialog.tsx:109`); the Permission heading keeps `Role::Warning`. The footer-gap half was deliberately **not** landed: `dialog.tsx:159` yields `0` for the default `footerGap` of 1 in both states and `1` for the `footerGap={2}` callers, so new-designs never removes the gap row. DIALOG-02 stays `REAL_GAP` for the description / footer-gap / per-select-row-pad halves. Focused test + full CLI suite (2178/0) green. |
| SIDE-14 | `d5611712` | The pinned footer location row (`footer.tsx:94-112`) had no hit rect, hover routing or menu route. Adds `StatusView::footer_row`/`footer_directory`/`footer_menu`/`footer_hovered`, a `NativeView` footer hit rect published during sidebar render (cleared on scroll/resize/route/toggle exactly like the onboarding dismiss rect), pointer-move hover (`kind==7`), a press that opens the local `DialogSelect` (`working-directory-actions.tsx:16-58`; the selection guard mirrors `openMenu`'s `getSelectedText()` early-return), and Enter/Esc/backdrop handling that runs Copy path (OSC 52 + `Path copied to clipboard` info toast) or Open folder (`openPath`, `packages/util/src/open.ts:10-12`). `Workspaces` stays unoffered (no live move route → `can_move=false`). 3 focused tests + full CLI suite (2184/0) green. |

### Landed fixes (wave-61/62, 2026-11-03) — single-item workers

The wave-58/59/60 finding was that **group size**, not worker quality, was the
failure mode: multi-item/multi-crate groups hit the agent step cap during
reconnaissance and produced no artifact. Wave-61/62 switched to **one item per
worker** with a fully prescriptive prompt (exact file:line seams, artifact
written after every edit). Every item below compiled, passed focused tests, the
full single-threaded CLI suite, and was pushed + remote-verified.

| Item | Commit | What changed |
| --- | --- | --- |
| PAR-10 | `da7d1a0b` | Server tags each `assistant_delta` NDJSON frame with a monotonic `seq` (`TurnStreamState.delta_seq`); CLI `native_turn::Event::Delta { text, seq }` surfaces it and the TUI consumer drops non-increasing seq (`last_delta_seq`). 2 focused tests; suite 2186/0. |
| FORM-001 (sub-behavior 1) | `8971a653` | Live `open_permission_with_persistent` (`native_view_dialogs.rs`) emits the upstream once/always/reject label set; `permission_alias` accepts `"Reject"`. Suite 2187/0. |
| TAB-10 | `53a3efe4` | `VcsInfo` + git HEAD parsers (`native_navigation.rs`); `git_vcs_info` producer + `LiveSnapshot.session_vcs` + live `rail_inputs_for_navigation` wiring so the tab rail shows branch/worktree from the session cwd. 7 focused tests; suite 2192/0. |
| SIDE-04 | `0d4f26a7` | `classes_from_wire` derives the five disjoint usage classes from the inclusive-of-cached wire; `set_usage_classes` wired into the live `native_turn::Event::Usage` arm. |
| SIDE-10 | `0d4f26a7` | Storage `session_todos_at` + sessions `todos_at_boundary`; `stage_session_revert` recomputes, persists and publishes the boundary-derived todo list (`TodoUpdated`) + server e2e test. Suite 2194/0. |
| FFI-01 | `019ecafa` | `native_loop` probes the pinned OpenTUI artifact identity before `NativeRenderer::create`; fail-closed on drift, proceeds when unverifiable. 3 tests; suite 2197/0. |
| DIALOG-12 | `2c858980` | Model picker reads persisted favorites and hoists them above recents; **ctrl+f** toggles favorite for the focused row; committing a model that advertises variants opens the **flat variant picker** (rendered + keyed live: Up/Down/Enter/Escape, text-input suppressed); `Model.variants` parsed from the catalogue. 5 new tests; suite **2202/0**. |
| MSG-009 | `09974664` | Server `stage_session_revert` computes the real boundary-onward user-message `count` and `files` (typed tool history + unified-diff line counts), adding `SessionRevert.count`; the TUI now projects `SessionSummary.revert` into a `RevertCard` and splices its rows into the transcript foot before the dock/queue, driven from the live `LiveSnapshot`. 2 CLI + 1 server + contracts/storage tests; suite **2204/0**. |
| DIALOG-02 | `67f97876` | The live `DialogView::layout` now reads the tested `native_palette::dialog` helpers: select-row left pad from `row_padding`, and the body bottom padding from `body_bottom_padding` (new `OpenDialog.footer_gap`, `Some(2)` for the live `dialog-prompt` caller). Under new designs a `footerGap=2` footer adds one body padding row; legacy unchanged. 1 new test; suite **2205/0**. |
| MSG-008 | `6a49151f` | **Producer chain built end to end.** `MessageExecution.streamed_ms` (contracts) + `message_execution.streamed_ms` column with idempotent `pragma_table_info` ALTER (storage) + `MessageExecutionInput.streamed_ms` (sessions); server `StreamTiming` accumulates each provider round's `first_delta − round_start` (upstream `time.streamed − time.created`, `rows.ts:451-453`), excludes tool time, and returns `None` when **any** round never streamed (`rows.ts:453-454`); published as `streamed_ms` on the `assistant_message` frame and stored `execution_json`. CLI `AssistantFacts.streamed_ms` + `tokens_per_second()`; footer ` · {tps:.1} tok/s` span gated on `session.tps` (`config/index.tsx:328` default true, consumed `index.tsx:2061`); `ChatView` threads `tps_enabled` through both render paths; `tui_entry` reads `session_tps`. 4 server + 2 facts + 1 footer + 3 prefs tests + storage/sessions round-trip; CLI **2211/0**, server lib **309/0**. |
| SIDE-03 (cost producer half) | `b5ec63c7` | `native_setup::ModelCost { input, output }` (USD per 1M tokens) parsed from the catalogue entry's first `cost` row (`entry["cost"][0]`, upstream `ModelInfo.cost`); `ModelCost::price(input,output) = (input*in + output*out)/1e6`, `None` unless finite and positive. `Model` gains `cost: Option<ModelCost>` (drops `Eq` for the f64 field). The live `native_turn::Event::Usage` arm now calls `view.set_cost(selected.cost.price(input,output))` — the first live `set_cost` caller, so the sidebar `$X.XX spent` row (`context.tsx:14/36-38`) renders from real pricing. 1 focused test (parse + price + zero guards); CLI **2212/0**. |
| SIDE-03 (history-load half) | `5e3d797b` | Completes SIDE-03. `native_chat_projection::SessionUsage` + `last_assistant_usage(records, boundary)` mirrors upstream `lastAssistantWithUsage` (`packages/client/src/solid/usage.ts:8-21`) and `contextUsage` (`util/session.ts:44-63`): an unknown revert boundary yields absent usage (never stale tokens); records at/after the boundary are excluded; the last completed compaction hides everything before it; the model is the message's own `execution.provider_id/model_id`. `LiveSnapshot.session_revert_boundary` carries `revert.messageID`; a `usage_live_generation` guard keeps a live `Event::Usage` value authoritative for its binding; the per-frame publish otherwise sets the five usage classes and the message model's `context_window` (missing limit ⇒ tokens without percent, never the selected UI model). Test `last_assistant_usage_honours_revert_and_compaction_boundaries`; CLI **2213/0**. |
| STATE-002 | `6a72aa61` | Durable compaction marker rendered live. `MessageRole::Compaction` (contracts, snake_case `compaction`, never a provider history role); role codec 4 (TEXT + v2) + schema CHECK widened to 0..4; `bounded_history_from` filters `role<>'compaction'` so a marker can never enter provider replay (test `bounded_history_excludes_compaction_markers_from_provider_replay`). `append_compaction_message` persists the compaction request's own `window.usage`; `compaction_wire_part` emits `{type,status,providerNative,reason,usage:{input,output}}`; BOTH `/messages` and `/history` surface the typed part; `history_item_to_responses_item` rejects the role as a safety net. CLI `ChatPart::Compaction` carries `status`/`provider_native`/`cancelled`/`usage`; the render arm labels a completed provider-native compaction `Provider compaction` (else `Compaction`), appends `· cancelled` for an aborted failure and `· {in} in · {out} out` once not running (`index.tsx:2196-2218`). Tests: server `compaction_wire_part_tests` (4), CLI `compaction_part_projects_status_provider_native_cancelled_and_usage` + `compaction_part_renders_upstream_label_cancelled_and_request_usage`; CLI **2215/0**, server **313/0**, storage **126/0**, sessions **44/0**. |

### `REAL_GAP` — faithful model + focused test exist, no live consumer (3)

`MEDIA-001`, `FFI-10` (terminal-input parity parser never consumed; the
`safe_renderer.rs:69` "consumer" is a same-named termios struct), `PAR-07`
(live queue/steer/submission/permission traces exist; the residual is form
traces, which have no form surface yet).

Per-ID seams are recorded in the wave-44 batch tables above and in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/<ID>.json`.
Selected high-value seams:

| Item | Seam |
| --- | --- |
| MEDIA-001 | `image_attachments`/`inline_tool_images`/`gallery_model` (`native_image_preview.rs:1047-1160`) have zero non-test callers; `PageState::open_image_preview` (`native_view_pages.rs:788`) never invoked live, so the preview always renders "no image attached". Wave-48 re-confirmed: no clipboard-image read path, no live `PromptPaneInput` attachment field, and `LocalFiles` (`native_image_preview.rs:67`) has only a test implementation. Wave-54 added: the live transcript part `ChatPart::File { filename, mime }` (`native_view_chat.rs:680-683`) carries no `uri`/`data`, and the wire projection (`native_chat_projection.rs:368-372`) reads only `filename`/`path` + `mime`, dropping the upstream `data:${mime};base64,${data}` cell source (`routes/session/index.tsx:2363`) — so the gallery also needs a projection field, and the gallery-strip painter/hit-rect/mouse routing is a 3-file change. |
| DIALOG-02 | `native_palette.rs:667-733` dialog shell/list-row metrics are not read by the live `DialogView::layout` (`native_view_dialogs.rs:1627`); the `SelectDialog` port (`native_palette.rs:1251`) that consumes them has zero live callers. Wave-56 landed the title-weight/color half: the live `DialogView` reads `new_designs` (`set_new_designs`, seeded `tui_entry.rs:6958`) and paints the non-permission title `Role::Item` (base, unbolded) under the experiment (`dialog.tsx:109`). Remaining: description gating, footer-gap gating (`dialog.tsx:159` *adds* a row for `footerGap={2}` callers), and per-select-row pad. |
| THEME-08 | **stale (2026-11-07)** — `syntax_rules()` is installed per frame (`tui_entry.rs:12835-12846`), the lexical classifier is live (`native_view_chat.rs:2970` `paint_plain_code_block(..., true, ...)`), and `SYN_*` paints through `markdown_run_paint_color` (`tui_entry.rs:5836`). Wave-111 widened it to 8 of 14 capture classes. Residual is scope *transport*, not a missing consumer. |
| THEME-13 | `SystemThemeState`/`palette_reply` (`native_theme_runtime.rs:2183/2264`) have zero non-test callers; no OSC palette-query emitter and no payload-carrying input event to decode a reply. |

### `BLOCKED` / `REAL_GAP` — needs a producer/transport/contract field (38)

Set after the wave-81 re-verification, then corrected by waves 111/112 (which
removed rows later proven landed — `PAR-19` — or superseded — `MSG-010`):

`PAR-02, PAR-03, PAR-06, PAR-07, PAR-09,
TAB-06, SIDE-05,
MSG-001, MSG-002, MSG-003, MSG-005,
TOOL-006, TOOL-011, DIFF-001, DIFF-003, FFI-02, FFI-03, FFI-04, FFI-06,
FFI-07, FFI-08, FFI-11, THEME-13, THEME-14,
DIALOG-07, DIALOG-13, DIALOG-14, DIALOG-15, DIALOG-17, DIALOG-18,
LAY-10, LAY-11, LAY-13, MD-007, MD-008, MD-009, MEDIA-001`.
(`CP-07`/`CP-09` moved to `LANDED` in wave-80; `TAB-21`, `THEME-08`,
`DIALOG-06/08/09/16/19`, `DIFF-005`, `CP-34`, `CP-37` moved to `LANDED` in
wave-81 — see the wave-81 section at the top. `PAR-07`, `PAR-09` and `DIFF-001`
remain `UNCLEAR` pending a fresh trace check.)

| Item | Exact missing seam |
| --- | --- |
| TAB-06 | `enqueue_user_prompt` (`native_tabs.rs:1877`), `pending_marks_busy` (`:1893`) and `member_busy` (`:1900`) have zero live callers; live busy is only `work_state == Running` in `from_navigation` (`:754`). No producer/transport emits pending `PendingKind` items or cross-tab enqueue events; `LiveSnapshot` (`tui_entry.rs:1224`) carries no per-tab pending queue. |
| TAB-10 | **stale (2026-11-07)** — landed in `53a3efe4`. The fields are `branch`/`default_branch`/`worktree` (`native_navigation.rs:205-215`), not `current_branch`, so the "0 references" check was a false negative. Chain: `git_vcs_info` (`tui_entry.rs:878`) → `LiveSnapshot.session_vcs` (`:853`,`:1645`) → live `rail_inputs_for_navigation` (`:7192`) → `VcsInfo::tab_detail` → `session_tab_detail` (`native_tab_rail.rs:103`). |
| MD-007 / MD-008 / MD-009 | The live fence painter `paint_fence_block` (`native_view_chat.rs:2285`) routes mermaid/math through `MermaidRenderer::without_painter` (`:2299`) / `MathRenderer::without_engine` (`:2321`), which fail closed because the only trait impls in the workspace are the no-op sentinels `NoDiagramPainter` (`native_mermaid.rs:451`) and `NoMathEngine` (`native_math.rs:420`). Upstream configures real painters unconditionally via builtin plugins (`builtins.ts:14,15,29,30` → `merman/src/plugin.ts:9`, `latex/src/plugin.ts:7-12`). `MarkdownPluginRegistry`/`set_fence_handlers` have zero live callers, and the live painter `push_block_markdown` (`:2343`) is a stateless per-frame repaint with no block-identity/epoch state. Needs a diagram-painter/math-engine/plugin-registration producer. |
| CP-09 | The composer border highlight (`prompt_highlight` `native_prompt_pane.rs:137`, `border_highlight` `:155`, `MetadataReveal`/`fade_alpha` `:693-737`) needs a live agent-color source; the live rail paints static `Slot::Border` (`tui_entry.rs:2822`). The agent catalogue parser extracts only name/description/read_only/aliases (`tui_entry.rs:5434-5473`, no `color`), and `set_composer_agent` carries name+variant only (`native_view.rs:1191/161`), so `agent_color` (`native_prompt_pane.rs:619`) has no producer. Needs a `color` field on the agent-catalogue contract plus a view setter. |
| DIALOG-06..10 | Select/Prompt/Confirm controllers (`native_palette.rs:1251-1295,1922-1933`) are fully ported + tested, but the live dialog body is a generic placeholder (`native_view_dialogs.rs:1393-1511`) and the live event handler opens only `Permission` (`tui_entry.rs:7868-7874`); there is no live `Select`/`Prompt` producer or route into `DialogView::open_request`. Needs a live route/host for those dialog kinds. |
| DIALOG-12..20 | The live dialog surface is `native_setup::Dialog` (`native_setup.rs:184`), and only the model-picker base rows are wired (`tui_entry.rs:8056`). Every assigned palette/settings/overlay/toast model has zero non-test callers and no producer for its driving facts: favorites/variants (`native_setup.rs:117`, `native_palette.rs:2344`), session-picker debounce/scope/armed-delete (`native_palette.rs:2505`), drill-down (`:1785`), MCP-status palette (`:2797`), numeric limits/experiments (`native_settings.rs:509/561`), stash/workspaces (`native_palette.rs:2999`), diff dialog (`native_view_dialogs.rs:3767`), overlay `ChromeModel::new` (`native_modal_layer.rs:775`), and the toast sink (`native_toast.rs:418`; `native_shell.rs:1755 set_toast` never called). |
| LAY-10 / LAY-11 / LAY-13 | `panel_can_split`/`panel_presentation`/`panel_toggle_fullscreen` (`native_pane_resize.rs:551-586`), `right_pane` (`native_view_layout.rs:808`) and `pane_resize_handle`/`PaneResize` (`native_pane_resize.rs:276/308`) are test-only; the live render composes only the sidebar (`native_view.rs:3602`). The seam is the missing live plugin-panel/terminal producer `panel_open`/`terminal_selected` (`native_shell_frame.rs:122-124`). |
| PAR-09 | Live code/patch render is plain `sanitize_code` text (`native_view_chat.rs:1273`); no syntax-highlight `HighlightedText` producer (tree-sitter/syntect) exists, and `HighlightService::complete` (`opentui-bridge/src/highlight.rs:782`) has no live call site. |
| PAR-10 | The live SSE consumer appends raw bytes to a draft `String` (`tui_entry.rs:917,920`) with no per-delta `stream_id`/`seq`, so `StreamAccumulator` (`native_transcript.rs:73`) / `TokenDelta` have no ordered-delta producer. |
| PAR-19 | **stale (2026-11-07)** — the bounded-work model is consumed live by the diff viewer (`visible_chunk_range` + `placeholder_height`, `native_view_dialogs.rs:3722-3781` in both the split and virtual added-file branches). `relative_scroll_top` remains unused because that scroll area carries no virtual-root offset, making the raw `execute_scroll` already root-relative — an identity, not a missing producer. The remaining inline transcript patch is tail-anchored and bounded by `MAX_DIFF_LINES` plus the real `[diff truncated]` row. |
| MSG-001 / MSG-002 / MSG-003 | The live styled painter (`native_view_chat.rs:3248`) iterates `ChatMessage`/`ChatPart` flat; there is no `SessionRow`→`StyledChatRow` adapter, so `reduce_session_rows` (`native_message_rows.rs:960`), `DisclosureState` (`:165`) and `partition_pending` (`:710`) have zero live callers. Live pending is keyed by bare call id (`native_view_chat.rs:219`), not message-scoped `PartRef`. |
| FFI-01 | No artifact-pin verification step: `native_identity::identify` (`opentui-bridge/src/native_identity.rs:272`) has zero CLI callers, the live TUI never loads the pinned dylib, and vendored artifacts embed no commit string (`:13`) — a build-tooling seam, not live wiring. |
| FFI-03 / FFI-04 / FFI-06 / FFI-07 / FFI-08 / FFI-11 | The bridge `scene`/`frame`/`markdown`/`highlight`/`diff` models have no CLI consumer: the live tree is not generational (`native_view.rs:5652`), `tui_paint.rs` paints directly with no `FrameController` ticket seam (`opentui-bridge/src/frame.rs:210`), live rows are re-derived per frame (`native_view_chat.rs:1044`), no grammar/query transport exists for highlight (`highlight.rs:1`), the live diff painter lacks the bridge diff parse/selection model (`native_diff_view.rs:119`), and live focus is `DialogView`-local (`tui_entry.rs:10666`) not `Scene`-keyed (`scene.rs:402`). Adopting any replaces the live render substrate. |
| DIFF-001 / DIFF-003 | Split-mode arithmetic is correct (`native_diff_view.rs:600`) but has no live consumer: `resolve_view_mode` is only reached from unconsumed `build_*` fns and the live render hardcodes unified (`native_view_chat.rs:2707`); the added-file virtualization model (`native_diff_view.rs:1014-1076`) has zero callers and `AddedPatchChunk` (`:926-930`) carries no highlight field. Needs config/width threading + a `SplitRow→ChatRowClass` adapter + a diff-viewer component. |
| THEME-14 | No syntax-style install/destroy API on the checked OpenTUI bridge ABI: `opentui-bridge/src/text.rs:7` records `syntaxStyleRegister` "intentionally absent"; `highlight.rs:18-29` binds no text-buffer style call; `native_identity.rs:85` `REQUIRED_SYMBOLS` has no syntax-style symbol; `safe_renderer.rs:906` `draw_text_styled` has no style-table parameter; `tui_entry.rs:4469` `native_paint` installs none. `SyntaxStyleArena` (`native_theme_runtime.rs:2374`) has no live sink/destroy target, so wiring it to the `Copy` `HighlightTheme` tables would be a no-op facade, not consumption of upstream `createSyntaxStyleMemo` (`packages/tui/src/context/theme.tsx:402-421`). Needs an operator-authorized ABI extension. |
| FFI-02 | No native scene entry point: the bridge `Scene` renders a node tree to `CellGrid` (`scene.rs:845`), while the live CLI paints styled strings directly to `NativeRenderer` (`tui_entry.rs:4469/8334`); the two render substrates are disjoint. Every capability FFI-02 names already has a live non-Scene equivalent (measurement `native_box_style.rs:695`, clip `tui_entry.rs:4511`, focus `native_host.rs:81`), and its fixture `bridge-scene_move` (`fixtures.json:8607`) targets a `run_native_fixture` case that does not exist live. Wiring `Scene` would duplicate the whole paint path or be test-only dead code. |
| TOOL-006 | The tested `BackgroundShell` load cycle (`native_shell.rs:1809-2119`) has no live consumer, and the server exposes only `POST /api/sessions/{id}/shell/stream` (`server/src/lib.rs:294`) — there is no `GET /api/shell` or `/api/shell/{id}/output` route, so a client poll of `shell.output` (upstream `packages/client/src/promise/generated/client.ts:2045`) would be non-functional. Live shell output flows through `native_turn::TurnWorker::start_shell` (`native_turn.rs:108-122`) instead. Needs the server route + live background-shell discovery. |

| Item | Exact missing seam |
| --- | --- |
| SIDE-04 | Live `set_usage` (`native_view_status.rs:1146`) sums `input+output` only; `set_usage_classes` (`:1168`) and `set_cost` (`:1160`) have zero non-test callers. No `lastAssistantWithUsage` selector, no revert boundary reaching the view, no cost producer, and the wire `input_tokens` is inclusive of cached (`server/src/auto_compaction.rs:102`), so disjoint classes need a `nonCachedInput` producer. **All landed**: classes (`0d4f26a7`), cost producer (`b5ec63c7`), and the history-load `lastAssistantWithUsage` selector with the message's own model limit (`5e3d797b`). |
| SIDE-05 | `REAL_GAP` | `set_limit` (`native_view_status.rs:1175`) test-only; no usage-limits pipeline reaches the CLI. Wave-66 verifier VSIDE refuted the `BLOCKED` label: this is implementable in-repo (server quota snapshot + contract field + live consumer), not dependent on an absent external transport. |
| SIDE-10 | `todo_http.rs:31-38` returns the stored `session_todos` table verbatim; `stage_session_revert` (`server/src/lib.rs:1193-1209`) stores the boundary but never recomputes the latest-completed-`todowrite` list sliced at the revert boundary (upstream `util/todo.ts:32-43`). |
| STATE-002 | `LANDED` (`6a72aa61`) | Durable `role='compaction'` marker flows end-to-end: `append_compaction_message` writes it, both `/messages` and `/history` surface a typed `parts` entry (`compaction_wire_part`), and the live transcript renders the upstream `CompactionMessage` semantics (`Provider compaction` vs `Compaction`, `· cancelled`, `· {in} in · {out} out`). Role codec 4 (TEXT + v2), schema CHECK widened, `bounded_history_from` filters `role<>'compaction'` so it never enters provider replay. |
| MSG-005 | Chips need a server/storage `files`/`skills` field that does not exist (`MessageRecord` `contracts/src/lib.rs:250-257`). |
| MSG-009 | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) have no live consumer; `SessionRevert` (`contracts/src/lib.rs:191`) has no count; server stage route hardcodes `files:None`; no notice projection; no child-completion metadata. |
| TOOL-011 | `execute_dialog` (`native_tool_rows.rs:1159`) zero consumers; live render placeholder (`native_view_dialogs.rs:1429`); no `ExecuteDialog` payload on `OpenDialog`/`DialogView`; no syntax producer (`opentui-bridge/src/highlight.rs:27-29`). |
| TAB-21 | `ViewAcknowledgement` (`native_navigation.rs:1133`) zero live callers; no client POST `/api/sessions/{id}/view` in `crates/cli`; no idle/viewed timestamp reaches the CLI (`LiveSnapshot` `tui_entry.rs:573`; `SessionSummary` `contracts/src/lib.rs:202-213`). |
| FORM-001 | The upstream-correct `PermissionPrompt` (`native_permissions.rs:557`) is dead code; the live path (`native_view_dialogs.rs:1090`) is a flat choice list. Sub-behavior 1 (3-option realign) is **not** a parity bug: the canonical server supports `scope ∈ {once,session,always}` (`run_owner.rs:232`) and the 6-choice scoped set is a deliberate extension protected by semantic tests. Sub-behaviors 3 (always-body swap, static `:1420`) and 4 (nested rejection stage) need a stage+textarea in `DialogView`. |
| PAR-02 / PAR-03 / PAR-06 / PAR-07 | Harness-side: capability table empty (`audit-implementation.md:79-96`); no upstream TS/OpenTUI adapter (`README.md:148`); no font/pixel capture path; event-trace sink misses queue/steer/permissions/forms. |

### `ALREADY_CORRECT` — implementation + focused test + traced live consumer (144)

Everything not listed in the two sections above. This includes the full
`CP-12..CP-39` range, `TAB-01..03/05/07..09/11..20/22/24`, `SIDE-01/02/06/07/09/11/12/13/15`,
`THEME-01..04/06/07/10/11/15..18`, `DIALOG-01/03/05/11`, `FFI-09`,
`MD-001..006`, `MSG-006`, `TOOL-001..005/007..010/012`, `DIFF-002/004/005`,
`FORM-002/005..008`, `STATE-001/003/004`, `LAY-01..09/14/16/18/19/20`,
`CP-03/04/10/40/41`, `PAR-01/05/08/12/13/14/15/17/18/20/21/23/24`.

Wave-45 reclassification: `CP-42` moved here. Its two required behaviors are
already live via independent mechanisms — the network retry message is a
persistent transcript row (`native_chat_projection.rs:383` `ChatPart::Retry` →
`native_view_chat.rs:2694-2702` `ChatRowClass::RetryNotice`, styled at
`tui_entry.rs:2913/2962/3127`), and prompt busy/idle is the provider-turn fact
`busy_now = binding_active && !worker_finished` (`tui_entry.rs:8332/8359`),
decoupled from transcript contents. The tested `PromptSurface`/`TurnPhase` model
(`native_composer.rs:1483-1568`) has zero non-test callers, so it is a redundant
model, not a missing live consumer.

Wave-46 reclassification: `CP-08` moved here. The live model-load path already
retains prior composer labels by construction — `model_load.poll()`
(`tui_entry.rs:7343`) gates every setter, so nothing is written while a fetch is
in flight; on `Ok` it replaces via `set_model` + `set_composer_model`
(`:7364-7365`); on `Err` it records `setup_error` only (`:7374`), leaving the
prior label intact until the initial-load branch (`:6797-6804`) clears it via
`set_model_unknown`. The retention requirement is satisfied by a different
mechanism, so `PromptDisplay`/`next_prompt_display` is a redundant model.

Wave-47 reclassification: `FFI-05`, `LAY-12`, `LAY-17`, `MSG-004` and `PAR-11`
moved here; each requirement is met live by a different mechanism, so the ported
model is a redundant (not missing) consumer:
- `FFI-05` (Unicode/cell-exact widths): the live cell-exact width impls with
  exact-equality tests (`native_tab_rail.rs:1098,1254`,
  `native_completion.rs:535,1257`) satisfy the acceptance clause "demonstrated
  equivalent Unicode/cell results".
- `LAY-12` (focus guards): the permission prompt gates all keys via
  `Dialogs::intercepts_input` (`native_view_dialogs.rs:643`) at
  `tui_entry.rs:8964/9021/9141`; pane focus is the live `rail_interactive`
  (`native_view.rs:1250`) set at `tui_entry.rs:9169`.
- `LAY-17` (reflow/scroll anchors): the live `RenderedChatAnchor`
  (`native_view_chat.rs:1005`) via `chat_rendered_anchor`/
  `restore_chat_rendered_anchor` (`native_view.rs:2892/2906`) at
  `tui_entry.rs:10218/10236` and reflow `:7121/:7156`; per-tab independence via
  `navigation.save_active_state` (`native_navigation.rs:653`).
- `MSG-004` (saved scroll anchors): live anchor preservation via
  `resize_chat_preserving_anchor` (`tui_entry.rs:1321`), backfill
  `:7121/7156`, resize `:7398/8183`.
- `PAR-11` (message-anchored scroll): live via `rendered_anchor`/
  `scroll_for_anchor` (`native_view_chat.rs:3435,3460`) exposed by `NativeView`
  (`native_view.rs:2892,2906`) and consumed in the live loop
  (`tui_entry.rs:1340,1345`).

Wave-63 reclassification: `MSG-010` moved here. Its observable requirement
(footer duration + throughput with tool time excluded, missing streamed
timestamps hidden, and the turn's input boundary derived from the nearest
preceding user/system record) is satisfied by the **live** path, not by the
`RowMessage`-based `native_message_rows::turn_tokens_per_second` (which is a
faithful but redundant port with no live consumer — the live transcript uses
`ChatMessage`/`AssistantFacts`, not `RowMessage`):
- throughput: `AssistantFacts.streamed_ms` + `tokens_per_second()`
  (`native_assistant_facts.rs`), produced by the server `StreamTiming`
  accumulator and rendered by the footer (MSG-008, `6a49151f`); tool time is
  excluded because each provider round resets the pair, and a missing streamed
  span yields `None`;
- duration: `turn_duration_ms` (`native_assistant_facts.rs:222`) fed by
  `project_with_turn_duration` (`native_chat_projection.rs:715-732`), which
  tracks the nearest preceding user/system record exactly like upstream
  `inputIndex` (`rows.ts:469-482`).
Both halves have focused tests and a traced live consumer
(`native_view_chat.rs:2884` → `native_footer.rs`). Caveat: native sessions carry
no idle markers, so the legacy input-boundary path is the applicable one; the
upstream "marker-era" branch is not a distinct native state.

## Coverage and method

Wave-44 covered every item still `UNVERIFIED` after wave-43 (165 IDs) with 28
parallel workers on the four authorized routes; workers that hit step caps were
re-dispatched in ≤6-ID slices. The remaining 58 IDs were already verdicted in
earlier waves. Per-ID JSON verdicts are authoritative and live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/`.
A placeholder `UNVERIFIED`/`"note":"stub"` JSON is **not** a verdict. Wave-46
re-verification JSONs live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave46/`.
Wave-47 re-verified every remaining `REAL_GAP` item (45 IDs) with the decision
rule "does the LIVE UI produce the same observable behavior as upstream?":
5 moved to `ALREADY_CORRECT` (a live equivalent mechanism exists), 1 landed
(`MSG-007`), and 29 moved to `BLOCKED` (the required producer/transport/contract
field is absent).

## Honest completion statement

Of the 223 checklist obligations, **144 are `ALREADY_CORRECT`** (a faithful
implementation, a focused test, and a traced live consumer), **3 are
`REAL_GAP`** (model + test exist but the live TUI does not consume them:
`MEDIA-001`, `FFI-10`, `PAR-07`),
**45 are `BLOCKED`** (a producer/transport/contract field does not exist), and
**31 are `LANDED`** (`LAY-15`, `CP-01`, `CP-11`, `CP-02` in wave-45;
`CP-05`, `CP-06` in wave-46; `MSG-007` in wave-47; `SIDE-08`, `THEME-12`,
`DIALOG-04` in wave-48; `PAR-04` in wave-49; `TAB-23`, `DIALOG-10`,
`DIALOG-20` (toast half) in wave-51; `PAR-16`, `PAR-22` in wave-53;
`TAB-04` in wave-56; `SIDE-14` in wave-57; `PAR-10`, `FORM-001` (sub-behavior 1),
`TAB-10`, `SIDE-04`, `SIDE-10`, `FFI-01`, `DIALOG-12`, `MSG-009`, `DIALOG-02`,
`MSG-008` in wave-61/62/63; `SIDE-03` and `STATE-002` in wave-64;
`THEME-09` in wave-67).

The remaining work is therefore **not** more UI drawing:

- `REAL_GAP` items need the live TUI wired to models that already exist, each
  with a focused test. Several are cosmetic refactors where the live path has an
  equivalent inline implementation, so wiring changes structure, not pixels.
- `BLOCKED` items need cross-crate contract/server changes (serialized by the
  integrator): usage classes + cost + revert boundary (`SIDE-03/04/05/10`),
  compaction transport (`STATE-002`), message chips (`MSG-005`), streamed
  timing (`MSG-010`), revert/notice surfaces (`MSG-009`), view
  acknowledgement (`TAB-21`), execute-dialog payload
  (`TOOL-011`), split-diff config (`DIFF-001/003`), the permission body-swap
  stage (`FORM-001`), the syntax-style bridge ABI (`THEME-14`), the native scene
  entry point (`FFI-02`), the `shell.output` server route (`TOOL-006`), the
  agent-color producer (`CP-09`), a live Select/Prompt dialog route
  (`DIALOG-06..10`), the palette/settings/overlay/toast producers
  (`DIALOG-12..20`), a live plugin-panel/terminal producer (`LAY-10/11/13`), a
  diagram-painter/math-engine/plugin-registration producer (`MD-007..009`), a
  `SessionRow`→painter adapter (`MSG-001/002/003`), a syntax-highlight producer
  (`PAR-09`), an ordered-delta stream producer (`PAR-10`), a diff-region scroll
  producer (`PAR-19`), a harness hit-target transport (`PAR-04`), the bridge
  render-substrate models (`FFI-01/03/04/06/07/08/11`), and the tab
  indicators/pending/branch producers (`TAB-04/06/10`).
  `FORM-001` sub-behavior 1 is a deliberate extension, not a gap.

### Wave-48 re-confirmation (2026-11-03)

Independent workers re-examined the remaining `REAL_GAP`/`BLOCKED` seams against
upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22` and confirmed the
following as genuinely missing producers (not bounded wiring):

- `THEME-08` — **stale: see wave-111.** The tokenizer (`markdown_runs.rs:165`)
  and the per-frame syntax table are live; only grammar/query transport is
  still absent.
- `THEME-13` — no OSC palette-query emitter and no reply-carrying input event;
  `SystemThemeState`/`palette_reply` stay test-only.
- `CP-07` / `CP-09` — no live selected-agent id and no `color` field on the
  agent catalogue contract; `AgentFact`/`set_composer_agent` stay test-only.
- `TAB-10` — **stale: landed in `53a3efe4`.** The fields are `branch` /
  `default_branch` / `worktree`, and `git_vcs_info` → `session_vcs` → live
  `rail_inputs_for_navigation` → `session_tab_detail` is the production chain.
- `TAB-21` — the `idle` half of the unread pair is absent from storage,
  contract, transport and snapshot; wiring the `/view` POST alone cannot
  produce an observable unread transition.
- `TOOL-011` — no `ExecuteDialog` payload on the dialog contract, no live
  per-row tool-part owner, and no highlight producer.
- `STATE-002` — no compaction message producer: `compact_session`
  (`server/src/lib.rs:1668`) returns an ack only, `MessageRole`
  (`contracts/src/lib.rs:222`) has no compaction variant, and the live render is
  a placeholder (`native_view_chat.rs:2712`).
- `MSG-008` — **superseded: LANDED `6a49151f`** (contract field, storage column,
  server capture and `session.tps` all built). `MSG-010` is **superseded, not
  blocked** (wave-112): the `RowMessage` model is the marker-era algorithm,
  which native sessions cannot drive (no persisted `Idle` rows); the live path
  is `project_with_turn_duration` + `AssistantFacts::tokens_per_second`.
- `LAY-10` / `LAY-11` / `LAY-13` — no live plugin-panel/terminal producer feeding
  `panel_open`/`terminal_selected` (`native_shell_frame.rs:122-124`).

`THEME-05` was reclassified `ALREADY_CORRECT`: upstream product code never calls
the `state()` precedence combinator either (its only callers are upstream's own
library test `packages/tui/test/theme/v2/component.test.ts`); the live TUI
already reproduces the same observable behavior through discrete-token
resolution plus hand-written precedence.

`DIALOG-04` was the one bounded item and is now landed (`d137313d`). `THEME-09`
is re-scoped: the diff colors are live, but the upstream width>120 auto-split
(`routes/session/index.tsx:3457-3459`) is unreachable because the live chat
painter hardcodes `DiffViewMode::Unified` and `config.diffs.view` is never
parsed.

### Wave-49 implementation-owner results (2026-11-03)

Wave-49 reframed the remaining work as implementation ownership (build the
smallest real producer chain, else BLOCKED with the exact seam). Outcomes:

- `PAR-04` — **LANDED** (`669036cd`, see the fixes table). The only bounded item
  the fleet returned.
- `SIDE-05` — re-confirmed BLOCKED: the whole usage-limits producer chain has no
  Rust port. No `/api/experimental/usage/limits` route, no core service mirroring
  `packages/core/src/usage-limits.ts`, no `UsageLimits.Info` contract, no
  `modelUsage`/`servedByConsole` reduction. The data sources from the external
  OpenCode Console API with a signed-in workspace credential, so wiring only the
  CLI `set_limit` half would be a no-op.
- `MSG-005` — re-confirmed BLOCKED: no `files`/`skills` field anywhere in the
  producer→transport→contract→storage→serializer→CLI chain (upstream keeps them
  as top-level user-message fields, `prompt.ts:77-85`).
- `DIFF-001` / `DIFF-003` — re-confirmed BLOCKED: split paints one terminal row
  as two 50% panes with up to four style regions, which the flat
  `(String, ChatRowClass)` row model cannot represent (one fg + one uniform row
  bg; `ChatRunSpan` maps flags to fg/attrs only). Bounded upgrade path recorded:
  add bg/fg fields to `ChatRunSpan` + an x-ranged `role_bg` override in the paint
  loop (`tui_entry.rs:4623-4666`), then route the live `Patch`/fence painters
  through `paint_inline_patch` with `config.diffs.view` + measured width.
- `SIDE-14` — **LANDED** (`d5611712`, wave-57): the footer hit rect, hover
  routing, local `DialogSelect` menu and the `Copy path`/`Open folder`
  executors are wired live; `Workspaces` stays unoffered because upstream's
  `move.open()` (`component/prompt/move.tsx`) has no live `DialogWorkspaces`
  route.
- `TAB-04`, `TAB-23`, `TOOL-011`, `THEME-09`, `CP-09`, `DIALOG-06..10`,
  `DIALOG-12..20` — workers returned no deliverable (provider/step-limit
  failures); no new evidence, prior BLOCKED seams stand.

### Wave-51 grouped-area results (2026-11-03)

Base `1ea3d25a`. 16 grouped workers (one per area, 3-6 related items each) on the
four authorized routes. `TAB-23` — **LANDED** (`ae56897f`): see the fixes table.
Every other item re-confirmed as before — these are 2026-11-03 findings, and
`TAB-10`, `TAB-21` and `PAR-19` have since landed (see wave-112):

- `TAB-10` — no server VCS producer / contract branch field / git probe
  (cross-crate chain). `TAB-21` — the `idle` half of the unread pair is absent
  from storage, contract and server (no `set_session_idle`, no idle column, no
  `ServerEvent` idle variant); `ViewAcknowledgement` has zero live callers.
- `PAR-09/10/19` — no syntax-engine dependency, no wire `seq`/`messageID`, no
  mounted diff-scroll substrate. `PAR-A` (`PAR-02/03/06/16/22`) — harness-side
  gaps (no upstream capture profile, no class dimension, no evidence bundle);
  bounded harness patches identified but not landed.
- `THEME-13` — no OSC palette-query emitter bound in the FFI and both input
  decoders discard OSC payloads by design. `THEME-08` — no tokenizer + sink.
- `CP-09` — no live agent-color producer (catalogue omits `color`,
  `set_composer_agent` has zero live callers, `agent.cycle` inert).
- `STATE-002` — no `Compaction` message role in contract/storage, no server
  persistence. `MSG-001/002/003` — no `SessionRow`→painter adapter.
  `MSG-005/008/009/010` — no `files`/`skills` field, no per-step streamed
  timestamp. `TOOL-006` — no `shell.output` server route; executor refuses
  `background`. `LAY-10/11/13` — no plugin-panel/terminal producer.
- `DIALOG-12..20` — toast (`DIALOG-20`) **LANDED** (`d891f9c3`, toast half);
  the crash/error-details half and the rest need live driving-fact producers.
  `DIALOG-06..10` — `DIALOG-10` (Prompt / session
  rename) **LANDED** (`a52c222a`); `DIALOG-06..09` (Select) are blocked by the
  pinned SIDE-08 `DialogMcp` consumer.

This document is a status map, not acceptance. No item is marked
runtime-proven; captured styled-cell/cursor goldens were not produced.
