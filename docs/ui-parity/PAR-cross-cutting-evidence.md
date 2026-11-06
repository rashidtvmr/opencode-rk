# Cross-cutting parity evidence (PAR-01, PAR-04, PAR-05, PAR-08, PAR-12–18, PAR-20–23)

**Evidence scope.** This is a source audit of lane26 at base `b66bba92607233cd175b4107b5c080b3162d48d1`, against the selected OpenCode checkout `/Users/mymac/Projects/tmp/opencode` at `450e7b92412c780a7b5a75226bb8d49aaca24e22`. Line references below are intentionally concrete and were obtained from the checked-out files; they are not claims that a screenshot or installed end-to-end capture has been run.

The requirements are copied verbatim from `docs/ui-parity/opencode-ui-parity-kit/audit-implementation.md:144-166` (the checklist source report recorded in `checklist.json`). “Already-live” means a bounded production/test path exists in Rust. “Blocked” means this obligation needs an artifact or execution outside this source-only lane; it is not silently marked complete.

## PAR-01 — already-live (capture/evidence path)

**Requirement:** “Record selected product, renderer, runtime and theme pins for every golden; reject mixed-version captures.”

* Rust: `crates/cli/src/native_capture.rs:20-62` defines the selected product commit/version, OpenTUI renderer tag/commit, Bun/Solid runtime, theme and mode; `CapturePins::matches_selected` is the exact selected-set predicate at `:61-62`. `validate_capture_set` rejects unequal pin envelopes at `:395-403`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/app.tsx:241` selects production `targetFps`; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/config/index.tsx:63` persists the theme name (mode is runtime); the selected product/renderer/runtime values are pinned in `baseline.lock.json:4-22`.
* Limitation: no claim is made that every historical golden has been regenerated; the Rust envelope rejects a mixed set when the capture runner uses it.

## PAR-04 — already-live (structured comparison)

**Requirement:** “Compare text, positions, styles, clipping, cursor and hit targets; text-only equality is insufficient.”

* Rust: `native_capture.rs:300-371` returns structured `Mismatch` values, including clipping, text, position, style, cursor and hit-target differences; the comparison checks each category rather than comparing only row text.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:53-57` requires explicit terminal sizes and render-before-layout assertions; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/ui/dialog.tsx:54-117` demonstrates concrete pointer hit targets.
* Discriminator: `native_capture.rs:543-582` tests style-only, cursor and hit-target differences; `:585-597` tests clipping.

## PAR-05 — already-live (canonical frame serializer)

**Requirement:** “Preserve wide-cell continuation and grapheme semantics in canonical frame serialization.”

* Rust: `native_capture.rs:110-114` models a wide base cell plus continuation; `:159-184` paints grapheme clusters and explicit continuation cells; `:192-224` serializes `<cont>` distinctly; `:613-649` tests wide glyph, continuation and grapheme behavior.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:55-57` makes terminal cell geometry part of the test contract; the native layout scanner used by Rust is the cell-boundary authority (`native_view_layout::visit_cells`, called at `native_capture.rs:181`).
* No implementation gap was found in this obligation.

## PAR-08 — already-live (injectable clock)

**Requirement:** “Freeze time and use explicit checkpoints for every motion effect and deadline.”

* Rust: `crates/cli/src/native_clock.rs:14-79` provides one injectable `FrameClock`, explicit `freeze`, `advance`, `deadline`, and `release`; `:82-86` wires it into the modal clock contract. Boundary tests are at `:92-115`. Toast deadlines use the same explicit logical-time pattern at `native_toast.rs:350-365`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:31-37` requires an injected `ManualClock` for time-driven renderables and explicit `renderOnce`; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/component/session-tabs.tsx:154-214` identifies the upstream motion durations.
* The source audit does not assert every production visual effect has a golden checkpoint; it establishes the shared Rust clock path and its tests.

## PAR-12 — already-live (modal/keymap scope path; execution evidence outstanding)

**Requirement:** “Match upstream keymap scope, priority and reachability in popups, docks, modals and focused side panels; suppress background actions only where upstream suppresses them, while retaining intentionally global commands.”

* Rust: `native_modal_layer.rs:430-535` activates/deactivates modal layers and applies modal-local routing; `:766-806` composes the chrome stack; `:1328-1347` tests dialog ownership of keys. `native_command_palette.rs:629-680` tests modal-local selection and activation.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/context/keymap.tsx:43-66,152` creates the OpenTUI keymap provider; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/ui/dialog.tsx:318-370` creates the dialog keymap layer and clears selection before modal pointer activation.
* Remaining acceptance work is an integrated event-trace run across every popup/dock/panel; no bounded missing Rust path was identified.

## PAR-13 — already-live (owned release/activation paths)

**Requirement:** “Verify there are no duplicate sends or unintended tool actions from pointer release/drag selection.”

* Rust: `native_tab_resize.rs:76-86` commits only on the owned release and exposes `cancel`; its discriminating test is `:118-128`. Toast activation dismisses before running an action at `native_toast.rs:337-346`, tested at `:877-910`. Hit targets are compared structurally by `native_capture.rs:360-371`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/ui/dialog.tsx:54-117` uses `onMouseUp` for dialog actions; `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:39-51` documents mouse input, `clearSelection`, and click/selection timing.
* A real pointer trace against the installed binary remains evidence work, not an unimplemented source path.

## PAR-14 — already-live (bounded geometry/predicate tests)

**Requirement:** “Test narrow/short terminals and every exact predicate boundary, not only one 160-column screenshot.”

* Rust: narrow viewport tests include `native_command_palette.rs:710-711`, `native_modal_layer.rs:1138-1139`, and `native_children.rs:397-405`; exact deadline boundaries are tested by `native_toast.rs:1058-1068`; clock boundaries are `native_clock.rs:92-115`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:53-57` requires explicit width/height and narrow/wide coverage; `/Users/mymac/Projects/tmp/opencode/packages/tui/test/ui/pane-resize.test.ts:42-54` exercises resize predicates.
* Integrated terminal matrix/capture remains to be executed; no source gap found.

## PAR-15 — already-live (theme resolver)

**Requirement:** “Resolve full light/dark/transparent/stateful theme values, preserving hue provenance.”

* Rust: `native_theme_runtime.rs:93-105` implements the upstream terminal luminance mode boundary; `:109-120` and `:120+` implement source-over/transparent color resolution; palette roles and provenance are exposed by the runtime resolver and its tests (including overlay tests at `:2752-2760`).
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/theme/index.ts:95-96` resolves a theme by mode; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/theme/system.ts:4` defines terminal mode; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/mini/theme.ts:285-301` chooses detected mode and resolves the theme.
* No new implementation was justified; capture evidence must still include the chosen theme/mode, which PAR-01 records.

## PAR-16 — blocked (fixture taxonomy artifact missing)

**Requirement:** “Separate baseline, optional configuration, experimental design and approved extension fixtures.”

* Rust: `native_capture::Capture` (`native_capture.rs:374-383`) has one fixture identity and pins, but no fixture-class field or validator distinguishing baseline/optional/experimental/approved-extension. The current live path therefore cannot prove this separation.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/config/index.tsx:63` exposes optional persisted configuration (theme name); `/Users/mymac/Projects/tmp/opencode/packages/tui/src/app.tsx:182` exposes feature/config command wiring. These are not a fixture taxonomy.
* Exact blocker: the parity kit provides no class schema/fixture inventory for these four classes, and adding an invented enum would guess the product contract. Requires an approved fixture manifest/schema (and then a bounded `Capture` class field plus test).

## PAR-17 — already-live (selected-pin separation)

**Requirement:** “Keep selected-pin bugs and deliberate fixes explicit; do not silently merge v2.2.0 behavior.”

* Rust: `native_capture.rs:12-17` explicitly says v2.2.0 is comparison-only; `:20-26` hard-codes the selected v2.1.4/OpenTUI 0.6.8 set. The authoritative distinction is also recorded in `baseline.lock.json:24-28` (`is_authoritative: false`).
* Upstream measurement: selected checkout is the pinned `450e...` revision; comparison-only upstream is explicitly recorded in `baseline.lock.json:24-28`, rather than being used by Rust.
* No silent v2.2.0 merge was found.

## PAR-18 — already-live (RAII teardown paths)

**Requirement:** “Verify teardown cancels timers, rejects late workers, releases resources once and restores terminal state.”

* Rust: terminal restoration is RAII in `app_start.rs:148-180`; pending descriptor cleanup is RAII at `:187-242`; tests are `:450-506`. Late/cancelled approval state is audited by `native_approvals.rs:367-440` and tests at `:691-827`. Daemon lease cleanup is `chat.rs:91-95`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/packages/tui/src/app.tsx:269-326` destroys the renderer on shutdown/signals; `/Users/mymac/Projects/tmp/opencode/packages/tui/src/util/renderer.ts` is exercised by `/Users/mymac/Projects/tmp/opencode/packages/tui/test/util/renderer.test.ts:4-35`; `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:59-65` requires timers first, renderer/app teardown, then fixture removal.
* Installed late-worker/resource trace remains to be run; no bounded source gap found.

## PAR-20 — already-live (negative controls in unit tests)

**Requirement:** “Run negative controls that prove one-cell, color, threshold and duplicate-action regressions fail.”

* Rust: one-cell/wide and style negatives are in `native_capture.rs:543-649`; luminance threshold tests are in `native_theme_runtime.rs` (terminal mode tests near the runtime test module); duplicate-action/owned activation tests are in `native_toast.rs:877-910` and resize release tests in `native_tab_resize.rs:118-128`.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:76-81` explicitly requires an impossible-predicate negative control and frame dump; `/Users/mymac/Projects/tmp/opencode/packages/tui/test/util/renderer.test.ts:21-35` checks repeated destruction is harmless.
* The source-level negative controls exist; a final installed capture run is separate evidence.

## PAR-21 — already-live (semantic tests preserved)

**Requirement:** “Preserve existing semantic tests; investigate source disagreements before changing expected output.”

* Rust: the relevant modules retain their `#[cfg(test)]` suites (for example `native_capture.rs:479-675`, `native_clock.rs:88-115`, and `native_toast.rs:762-1079`); this lane made no semantic-test edits and no test was weakened/deleted.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:68-74` says to triage the first real failure and not hide timing failures; `/Users/mymac/Projects/tmp/opencode/packages/tui/test/util/renderer.test.ts:4-35` is a concrete preserved semantic lifecycle test.
* This is a process/evidence obligation and is satisfied for this candidate by preserving tests; full suite execution is reported separately.

## PAR-22 — blocked (final installed evidence not available in this lane)

**Requirement:** “Attach source/fixture hashes, exact test commands, failures and installed captures to the final Rust SHA.”

* Rust: `native_capture.rs:415-475` defines `ProvenanceManifest`, including Rust SHA, source commits, fixture digests, exact commands, failures and installed capture identifiers; `:438-444` and `:446-475` make it auditable/deterministic.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:76-80` requires exact per-file commands and reruns; `/Users/mymac/Projects/tmp/opencode/packages/tui/test/util/renderer.test.ts:4-35` demonstrates the installed renderer lifecycle test shape.
* Exact blocker: this source-only feature lane cannot truthfully attach installed captures or a final integrated `main-v2` SHA. The manifest API is live, but the required final artifact must be generated by the integration/release runner after merge.

## PAR-23 — blocked by definition (not yet integrated)

**Requirement:** “Keep all Rust parity statuses unverified until executed against the real integrated Rust build.”

* Rust: `native_capture.rs:415-434` requires a final Rust SHA in provenance; `checklist.json` intentionally still records these obligations as `unverified_rust`. There is no source change that can legitimately turn this status into verified from a feature worktree.
* Upstream measurement: `/Users/mymac/Projects/tmp/opencode/.opencode/skills/tui-tests/SKILL.md:76-80` requires commands run against the actual package; the selected upstream revision remains the oracle, not a substitute for the integrated Rust build.
* Exact blocker: lane26 is based at `b66bba92`, not the final integrated `main-v2` release SHA; therefore status must remain `unverified_rust` until the real integrated build and captures execute.

## Verification boundary

No implementation gap with a sufficiently specified, bounded live path was found among these IDs. Consequently this evidence-only candidate intentionally changes no Rust implementation and adds no speculative fixture taxonomy. The remaining blocked items (PAR-16, PAR-22, PAR-23) require respectively an approved fixture schema and integration/release execution evidence.
