# COMPOSER-PARITY scratchpad (task-1, lane `recovery/ui`)

- **Role:** expert implementation owner, composer subsystem.
- **Base:** `57cb5e46fccc0857c130cb435dc19e48590d4c63` (main-v2 sync point).
- **Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` @
  `450e7b92412c780a7b5a75226bb8d49aaca24e22` (HEAD verified).
- **Write scope honored:** `crates/cli/src/native_prompt_pane.rs` only.
  `crates/cli/src/native_composer.rs` is pure input state (EditBuffer, keymap,
  paste gate, busy queue) and owns no geometry or footer facts — no change was
  needed there; every parity fact below is a view/presentation concern that
  already lives in `native_prompt_pane.rs`.

## 1. Runtime evidence, decoded

`p1-ui3-initial.txt` (home 140x40) info row `Default fixture openai` and
`p1-ui3-session.txt` row 36 `Non-default fixture openai` are **model+provider
only**: the fixture catalogue names `gpt-5.6` → "Default fixture" and
`gpt-5.6-mini` → "Non-default fixture" (`tests/e2e/native_provider_setup.py:483`),
so the row is `metadata_text` with `agent=None`:
`"{model} {provider}"` — no agent cell, no `·` separator, no variant, because
**the live loop never calls `NativeView::set_composer_agent` /
`set_composer_effort`** (their only call sites are tests,
`native_view.rs:7468-7538`).

`p1-ui-initial.raw` (80 cols pre-connect) row 16 `unset` is the status summary
(`ModelSlot::ConfirmedUnset`, `native_view_status.rs:214`) leaking through the
`composer_metadata` fallback (`native_view.rs:2599-2605`) instead of upstream's
honest empty state.

Composer block geometry in both captures is `prompt_rows(1)` = 5 rows
(padding / 1 editor row / metadata padding / metadata / lip) — that IS
upstream's empty-draft minimum (`minHeight={1}`), so "too small" is the missing
cells, not the height.

## 2. Upstream citations (pin `450e7b92`)

| Behavior | Upstream | oc2 model |
| --- | --- | --- |
| Textarea `minHeight={1}`, `maxHeight={max(6, floor(h/3))}` | `component/prompt/index.tsx:1674,1773-1774` | `native_prompt_layout::editor_max_rows` / `editor_rows` (unchanged, already exact) |
| Surface `paddingTop=1`, inset `<44 ? 1 : 2` | `index.tsx:1690-1698` | `prompt_inset`, `surface_rows` |
| Metadata block `paddingTop={1}` + row | `index.tsx:1855-1870` | `PromptPane::compose` MetadataPadding/Metadata rows |
| Lip `╹`/`▀`, transparent → spaces | `index.tsx:1873-1898` | `lip_row`, `prompt_border` |
| Cell order agent · auto · model provider · ·variant · limit | `component/prompt/metadata.tsx:39-101` | `metadata_runs` + `metadata_style` |
| Width budget `terminalWidth - (<44 ? 9 : 13)`, candidate fitting, 9-cell model floor | `metadata.tsx:28,117-156` | `metadata_layout` |
| Shell mode: agent-or-"Shell", empty model | `metadata.tsx:26` | `metadata_layout` Shell branch |
| Agent label = `Locale.titlecase(agent.id)` | `index.tsx:1559` | caller-supplied (`set_composer_agent`) |
| **Empty state**: `provider: "Connect a provider"`, `model: "No provider selected"` | `context/local.tsx:398-417` | **NEW** `PROMPT_EMPTY_PROVIDER_LABEL` / `PROMPT_EMPTY_MODEL_LABEL` + `PromptMetadataInput::unconnected` |
| Mini-mode statusline (separate surface): `idle|running · agent X · model[ variant] · …` | `mini/footer.view.tsx:239-248` | informational only; the full-screen composer row is `PromptMetadataRow` |

## 3. What I changed (`native_prompt_pane.rs`, +274 lines)

1. Module header: added the `context/local.tsx:398-417` citation.
2. `PROMPT_EMPTY_PROVIDER_LABEL` / `PROMPT_EMPTY_MODEL_LABEL` consts and
   `PromptMetadataInput::unconnected(agent)` — the upstream no-selection state
   (`agent` stays the caller's resolved label; `None` omits the cell exactly
   like upstream's `agent: undefined`).
3. Six focused tests (`composer_parity_*`) pinning the acceptance criteria:
   - empty draft → 1 editor row, 5-row block, placeholder painted — at 80x24
     and 140x40;
   - growth: one row per wrapped line to the terminal cap (13 at 40 rows,
     8 at 24 rows), plus the structural body-budget clamp;
   - full cells at 140 (`Build · Non-default fixture openai · balanced`) with
     per-role styles (Agent=highlight, separators/model-context=muted,
     Model=base, Variant=warning+bold);
   - `auto` cell between agent and model, fitting the 80-column budget;
   - honest empty state at 80 and 140 (`Build · No provider selected Connect a
     provider`), never `unset`; agent-less and shell variants.

## 4. Test evidence (exact commands, lane worktree)

```
DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin \
  cargo test --locked -p opencode-rk-cli --bin oc2 composer_parity
→ 6 passed; 0 failed (2505 filtered)
… --bin oc2 native_prompt_pane → 36 passed; 0 failed (30 pre-existing + 6 new)
… --bin oc2 native_composer    → 41 passed; 0 failed (file untouched)
```

Full-bin suite: `2510 passed; 1 failed` at the time of run — the failure is
`native_view_pages::tests::image_preview_page_renders_surface_and_honest_empty_state`
(MEDIA-001, lane HEAD `57cb5e46`), unrelated to this change; a concurrent
worker was mid-edit in `crates/server/src/lib.rs` (in-flight agent-stream
endpoint, transient E0063) in the same shared worktree while I ran. My file is
purely additive and its tests passed both before and after that edit began.

## 5. Integrator seam (exact call-sites, NOT edited by me)

The model API is complete and consumed by the live row already; two wiring gaps
keep the cells invisible. All sites are outside my write scope:

1. **Feed the agent/variant/effort facts** — `crates/cli/src/tui_entry.rs`:
   - startup / snapshot application: next to `view.set_composer_model(...)`
     (`:10788`, also `:11552`, `:18175`, `:18685`), add
     `view.set_composer_agent(Some(crate::native_assistant_facts::titlecase(agent_id).into()), variant)`
     using `live_agent_facts` / `live_agent_selected` (`:10652-10662`) —
     upstream `Locale.titlecase(agent.id)` (`index.tsx:1559`); variant from the
     model-selection state (`local.model.variant.current()`).
   - agent cycle: in the `cycle_agent` block after the switch is accepted
     (`:13827-13845`, where `view.set_live_notices(...)` runs), re-call
     `set_composer_agent` with the new selection.
   - effort: `view.set_composer_effort(...)` where the request's reasoning
     effort is resolved.
2. **Honest empty state** — `crates/cli/src/native_view.rs::composer_metadata`
   (`:2590-2605`): when `composer_model` is `None`, build
   `PromptMetadataInput::unconnected(self.composer_agent.as_deref())` instead of
   falling back to `self.status.model_label()` (`unset`/`Unavailable`). Keep the
   CP-08 loading-retention branch as is.
3. **Optional styled upgrade** — the rail overlay `native_view.rs:4742-4752`
   paints plain text; to color the cells, replace the text overlay with the
   pane's runs: feed the real `PromptMetadataInput` through
   `rendered_prompt_pane` (`native_view_chat.rs:378-393`, currently
   `PromptMetadataInput::default()`) and paint `pane.metadata_runs()` with
   `metadata_style(role, muted, limit_danger)`; the pane already exposes
   `metadata_row()` / `metadata_text()` / `metadata_runs()`.
4. **Fresh pyte capture** (acceptance 1's capture-vs-upstream step) belongs to
   the integrator's post-merge check on the installed build; the geometry is
   pinned at model level by the new tests.

## 6. Residual notes

- `auto` stays `false` in the live input until a permission-mode fact exists
  (`native_view.rs:2613-2615` documents the same reserve).
- `limit` stays `None` (upstream Console usage limits; no oc2 producer).
- The `[Send]` affordance is an approved oc2 extension painted by
  `native_view.rs:5374` over the metadata row; upstream's row has no send cell.
  Left untouched (outside scope; not part of the parity defect).
- `help_regions`-style accumulation concerns do not apply here; the pane is
  recomposed per render.
