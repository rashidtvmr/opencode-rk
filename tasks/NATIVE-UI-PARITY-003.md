# NATIVE-UI-PARITY-003 — Layout, padding and section surfaces

Status: IN_PROGRESS — user requirement recorded 2026-10-23.
Baseline release: `bbabe6f2b8a33ed2835d6dece05b768082e3cb93`.

## Requested behavior

- [x] Research OpenCode TUI layout against the pinned upstream revision, recording any checkout mismatch. Source research complete; this is not runtime acceptance.
- [ ] Match root, home and conversation spacing, sizing and padding in the Rust/native stack.
- [ ] Keep every section background inside its outer border.
- [ ] Apply the selected corner style consistently to outline and visible section fill.
- [ ] Audit composer, sidebar, menus, dialogs, permissions and child panels for the same defects.
- [ ] Verify narrow/wide resizing, Unicode content, themes and all supported corner settings with meaningful rendered-cell evidence.
- [ ] Independently review candidate changes and serialize integration through the existing sole writer.
- [ ] Build/install the final exact committed source and verify installed behavior before delivery.
- [ ] Continue HTTP 400 route repair and outstanding lifecycle/security/child-control acceptance alongside this work.

## Execution constraints

Use only `9router-oc-deepseek-v4-1-flash-free`,
`9router-oc-glm-5-3-flash-free`, `xkiro-gpt-6-luna`,
`vyce-deepseek-v41`, and `9router-luna-free`.
At most 30 concurrent subagents. Research, implementation proposals, independent
review, audit and integration preparation run in parallel where useful. Existing
sole integration writer remains `ses_e9b8e0d4effenNbsq3BmSns5Nz`.
Preserve user work, preferences, history, databases, permission semantics and
earlier failed evidence. No completion claim from research or source presence.

## Evidence and scratchpad

`docs/worklogs/NATIVE_UI_PARITY_003_SCRATCHPAD.md` is the running coordination
record. Topic evidence belongs under `docs/worklogs/ui-parity-003/`.
The existing release manifest remains PARTIAL until outstanding product and
installed acceptance gaps are resolved on the final exact artifact.

## Candidate work underway

- **Surface containment:** shared clipped geometry and corner-background handling
  across actual framed surfaces; preserve intentional default Home rail and
  unframed sections.
- **Dialog/palette spacing:** pinned title/filter/list/footer insets with native
  configurable border extension preserved.
- **Display-cell width:** Unicode-aware chat truncation/wrapping to keep text
  within pane bounds without silently dropping wrapped tails.
- **Verification:** independent unit-regression proposals plus real-PTY RGB
  corner/interior/outside sampling on exact installed artifacts.

These are active candidate scopes, not completed or accepted features.

## Integrated source progress

- `69cc60dda266312dada3a3ff7e694a690259b587`: retain conversation on provider error.
- `b99af670af6e2794b3fce9f5a1376d12dddc8b81`: discover background-child permissions.
- `e7b45664389cd470c1dcf8acf28ede90c2320121`: palette content spacing, meaningful
  compiling RED then focused4/full673 GREEN, independent source review.

Commit identities independently confirmed with Git history. These are source
integration milestones, not installed acceptance. Installed artifact is still
bbabe6f2 with overall PARTIAL acceptance. Shared rounded-surface implementation
is under independent review after coordinator took over product edits. The
existing sole integration writer continues to own commits.

## Current verification — 2026-10-24

- `b85626412da11da305c0d6ae8f4866a09e60137e`: independently reviewed completed-frame
  readiness repair, only `installed_default_entrypoint.rs` committed.
- Repaired installed entrypoint gate on old bbabe6f2:5PASS2FAIL. Daemon reuse,
  native terminal restoration and completed-frame determinism pass. Two automatic
  provider-dialog expectations fail; independent authority diagnosis is pending.
- Coordinator rounded-surface candidate: meaningful compiling RED then6focused
  GREEN; Unicode candidate: compiling RED then5focused GREEN. Combined native
  CLI suite685PASS. Both source candidates await independent actual-diff review.
- Installed before-fix modal probe independently confirms PRODUCT_RED for rounded
  corner BG at both viewports; sharp cases pass and all owned daemons are gone.
  This probe covers modal only; Home/composer acceptance is explicitly excluded.
- Controls attempt3 stops before semantic checks. Cycle4 static review found typed
  parent-message correlation and PTY-draining defects; narrow repair is underway.
- Configurable Menu padding remains open; actual call-site research assigned.
- No new installation or push. Final exact-artifact GREEN remains required.

## Subsequent source integration

- `4ec1daa1a25ebdc194476c9cd796b9b15552ef9f`: independently reviewed bounded
  Unicode display-cell wrapping/truncation, exactly3sourcepaths committed.
- Rounded candidate v2 fixes the independently found Home top-right overlap;
  compilingRED then7focused/full686GREEN. ReviewerA approves; test/contract
  evaluator verdict still pending. No renderer commit yet.
- Provider startup supersession is saved independently: approved nativeHome then
  explicit `/connect`, not pinned-upstream automaticoverlay. Actual narrow test
  patch compiled; two independent applied-diff reviews requested. Existing5PASS/
  2FAIL runtime evidence retained; no new installedPASS claimed.

## Integrated source as of42ed6130

- `113d5158f1c976390d12bb8cb4dbaf72d71e140a`: rounded corner fill, shared actual
  underlying-glyph resolution and clipped modal geometry; two independent
  approvals, recorded RED/GREEN and688native tests.
- `01ac87a4c01f3ace1112e7cca376150747dada89`: portable shared parser declaration
  and matching native-only test guard; no-native build check,14portable transcript
  and3native catalogue tests pass after independently reviewed repairs.
- `85121410f781d2152ccfaaa7bba4ca3a250db306`: explicit Home-to-connect setup test
  contract. Installed attempt exposes a delta-frame observer gap; repair pending,
  not accepted runtime completion.
- `42ed61300335f86c8e135626afb0b5e2ddca53b9`: configurable Menu/Dialog padding
  shared across render/hit geometry, footer bounds and permission-editor content
  mapping. Independent test/security approvals; meaningful RED then690native
  tests pass. Installed configurable-padding evidence is still required.

Controls attempt6 passed native permission approval and held-child startup but
stopped at panel command entry before Stop/Retry. Separate command edit/submit
repair is underway. Installed entrypoint observer is being changed to decode
completed terminal screen state rather than treating deltas as full snapshots.
No final-artifact installation, restart, acceptance or push is claimed here.
