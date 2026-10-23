# NATIVE-OPENCODE-PARITY-002 — Native commands, session layout and status parity

Status: IN_PROGRESS — explicit user requirements, 2026-10-23.
Base integrated revision: `b18f1e85929c922c49d330bdd588d3234afe38b0` on `main-v2`.
Related tracker: `docs/bugs/bug_2026-10-22_native_ui_and_config_import.md`.

## Goal and authority

Deliver the six requested OpenCode-compatible behaviors in the Rust/native
application, retaining the project's explicit extensions and permission broker.
Inspect the full upstream implementation before choosing behavior. Record the
requested checkout's actual revision separately from the lock pin
`95daf90670b7c039c436c85537da5fbfe2205b41`; do not conflate them or claim
exhaustive OpenCode parity from this package.

## Task list

- [ ] **NP-01 — Native directory-change command.** Implement OpenCode's directory
  switching interaction, including directory completion/selection as applicable.
  Resolve relative and home-relative paths without shell evaluation. A successful
  switch must update the backend project/session/tool working directory and
  permission context, not just the displayed path. Invalid paths leave the
  current context intact; existing sessions and history remain preserved.
  **Acceptance:** execute the native command in a disposable two-directory
  fixture, then prove a real tool runs in the selected directory and the footer
  agrees. Verify invalid-path behavior and isolation from the previous project.

- [ ] **NP-02 — Context and token details in the right panel.** Match upstream's
  presentation of available context usage and token information. Use actual
  provider/model metadata for context capacity and authoritative input, output
  and cache counters. Distinguish turn/session totals; show unavailable values
  honestly rather than inventing counts or percentages.
  **Acceptance:** render known fixture usage in the right panel, verify updates
  during/after turns, unknown-capacity handling, sidebar toggling and compact
  layout without corrupting conversation or composer geometry.

- [ ] **NP-03 — Current-folder footer and loading indicator.** Display the active
  project directory at the bottom and move the active-work loading indicator to
  that area, following upstream layout. Preserve cell-aware truncation and keep
  footer, composer and modal regions correctly bounded.
  **Acceptance:** native frames show the actual folder and footer loader while
  work is pending; the loader settles on completion/cancel/error and does not
  remain duplicated in its old location. Directory switching updates the footer.

- [ ] **NP-04 — Immediate conversation layout on first Enter.** Submitting the
  first prompt from the start screen must immediately enter conversation mode
  with the composer at the bottom, before waiting for any provider response.
  Preserve input and show a recoverable error if submission fails.
  **Acceptance:** deliberately hold the provider response; after one Enter,
  observe conversation layout, the submitted prompt and bottom composer while
  the provider is still blocked. Verify failed/cancelled submission behavior.

- [ ] **NP-05 — Deliver the features through this PC's `oc2` command.** Build,
  integrate and install the reviewed changes. Verify the actual command resolves
  to the installed executable, loads the installed native library and uses the
  updated service. Handle a stale daemon through the supported lifecycle without
  deleting preferences/databases or interrupting active work blindly.
  **Acceptance:** record final source SHA, installed executable/library hashes,
  loader resolution, command path and service identity; run the new native
  journeys using that installed executable. Currently `oc2` resolves to
  `/Users/mymac/.local/bin/oc2`; the real-user daemon observed on 2026-10-23 was
  started on 2026-10-22, before the latest installation. This observation is not
  proof that the new features have shipped.

- [ ] **NP-06 — Execute selected command hints immediately.** Clicking or pressing
  Enter on an executable command hint such as `/exit` must invoke it directly,
  without first inserting it into the composer and requiring a second Enter.
  Commands requiring arguments must retain upstream's argument-entry behavior;
  ordinary text completion and normal prompt submission must keep their meaning.
  **Acceptance:** native keyboard and mouse journeys invoke `/exit` with one
  selection action, demonstrate another non-exit command's effect, and verify
  argument-taking commands and plain-text completion are not executed wrongly.

Unchecked means not yet accepted on the final installed revision. Delegation,
source presence, compilation or a worker's report does not complete a checkbox.

## Parallel execution and incremental integration

- Interaction owner: `tui_entry.rs`, `native_completion.rs`, CLI entry/module
  wiring and an optional directory helper; implements NP-01, NP-04 and NP-06.
- Layout owner: native view/status/chat render files; implements NP-02 and NP-03.
  Coordinate existing/public view setters with the interaction owner.
- Main owns the single build/install/runtime slot and NP-05. The existing sole
  integration writer lands each coherent, reviewed package promptly on `main-v2`.
- Appearance-hit, imported-model and cancellation repairs already in progress
  keep their separate path grants. Do not create overlapping source writers or
  wait for unrelated reviews to merge an independently ready package.
- Preserve existing semantic regressions, broker protections, user data and
  unrelated work. Follow the Phase-1 override; no Python guard/queue ceremonies.

## Verification and completion

- [ ] Record upstream file/symbol/revision evidence for the six behaviors.
- [ ] Run meaningful focused regressions and new real native interaction journeys.
- [ ] Independently review changed behavior and permission/context boundaries.
- [ ] Integrate approved packages and verify the coherent final source revision.
- [ ] Build/install and pass the new journeys via the actual installed `oc2`.
- [ ] Report exact results, remaining limitations and final push status.

Evidence belongs under `target/phase1-convergence-20261021/`; final installed
receipts must identify the exact release SHA and executable/library hashes.
