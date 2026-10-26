# Orchestrator scratchpad

Updated: 2026-10-26. Session: `ses_ef3b17240ffeYKnIlsBn552l4m`.

This is a durable reasoning/handoff note, not an acceptance ledger. Revalidate
source hashes, active runtime state and raw logs on resume. Keep this file current
at meaningful transitions and before compaction. Apply the exclusive allowlist
and operating rules at the top of `AGENTS.md` before dispatching more work.

## Product objective and authority

- Complete the working native Rust OpenCode application, faithful to OpenCode V2
  plus approved extensions; Phase 1 is the immediate release priority.
- Canonical checkout: `/Users/mymac/Projects/opencode-rk-recovery-20261005`.
- Last observed canonical HEAD before these documentation edits:
  `52a89c156a6e6e30268c49d2587b131836c11233` (`main-v2`). The operating-rule
  documentation subsequently landed as `b7a3178307832945538cc9fa815044b2fe8d3574`.
- User-selected upstream: `/Users/mymac/Projects/tmp/opencode`, verified HEAD
  `450e7b92412c780a7b5a75226bb8d49aaca24e22`. The older lock pin `95daf906...`
  is not the selected reference for current parity work.
- Sole integration writer: main orchestrator. Preserve all legacy refs, user work,
  databases and preferences. Preserve user daemon PID 73389 on port 4096.

## Route allowance snapshot

The complete allowed routes and stop rules are in `AGENTS.md`. Aggregate consumed
tokens and remaining balances are **UNKNOWN**: no reliable cumulative telemetry
has been recorded. Do not infer remaining quota from a successful call or reset
budgets on compaction. `vyce-deepseek-v41` has an eight-concurrent-assignment cap.
Resolve the user's `xkiro-gpt-6-luna` spelling against the actual harness inventory
before dispatch. No unlisted fallback is authorized. Maintain a session/route
usage and error log here as new telemetry becomes available.

## Verified release and installed journeys

Release source SHA: `b24eb60918c41ae215dedce996ef27b8a96968f3` (older than HEAD).
Evidence root:
`/Users/mymac/Projects/opencode-rk/.git/recovery-evidence/b24eb60918c41ae215dedce996ef27b8a96968f3/native-release/`.

- Earlier exact-release focused validation: 890 tests passed; release build,
  package and disposable installer checks passed. This is not final-SHA acceptance.
- Correct canonical installed scripts were rerun serially; results under
  `installed-spine-short-runtime/`: provider setup PASS, stream/tool PASS,
  second-client FAIL (`PTY EOF before marker b'stream fixture openai'`).
- Earlier runner used wrong-tree evidence scripts; retain those logs but do not
  treat them as canonical-script acceptance. A separate first canonical attempt
  failed before launch because TMPDIR made the socket path exceed 100 bytes.
- Short-runtime run used the macOS `.../T` parent, not an extra nested directory.
- Do not install to the real user prefix or push as a verified final release yet.

## Context repair: proven status bug and candidate

User requirement: context must be accurate; clicking it and `/context` open the
same session-bound detailed inspector. Requirements are recorded in
`docs/upcoming-features/context-inspection-parity.md`.

Confirmed root cause: `StatusView::set_usage` used `window.map(...)`, discarding
measured input/output when the selected model's capacity was absent. Repair keeps
the measured total with an optional limit, showing capacity as unreported. Cache
and reasoning are inclusive transport subsets in this path; do not blindly add
upstream's decomposed fields again without tracing normalization.

Private verification tree:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/context-red-green-d7y4lapv/`.

- Independent coordinator regression compiled and failed for the exact bug:
  `red-bin.stdout`; original `--lib` invocation was invalid (CLI is a binary).
- Status repair passed that same regression, all 34 status tests, and all 838
  CLI tests (`status-only-cli.stdout`). Disposable HOME, offline locked Cargo,
  Rust 1.98.1, one build job/test thread; real native library linked.
- Exact status-only package:
  `.../T/opencode/context-status-reviewed-package-155do1nh/candidate.patch`.
  Patch SHA256 `b4e30adaa07fac5ba289b39d2a1754985db28d75b1adbd394447a1ba023af14f`.
  Resulting status file SHA256
  `43e309db7c63b6a8708bf9e45081ffd8a4170919da1a4dd2c02b5f0262c0bbd3`.
- Reviewer `ses_e88e5d841ffes0OS7vHhKj793o` APPROVED this exact status-only
  package and existing-assertion changes. Integrated as
  `14a4fb44bb2fcf8660b28ffef6e77abac5489b30` after the operating-rule commit.
  Candidate preverification passed 838 CLI tests. Exact integrated release build
  and installed acceptance still required; receipt is next to the package.
- Integration preflight after the documentation commit found an unexpected
  canonical status-file edit, SHA256
  `103e49eda891ad95a122df4c1b1a5ee70a70e685f1cc48018be7a75f592f0017`.
  It adds cache/reasoning to inclusive totals and changes the 150 assertion to
  200; it is NOT the reviewed candidate. No approved patch was applied, no
  status code committed, and the unexpected working file was left untouched.
  Exact bytes/diff were preserved at
  `.../T/opencode/unexpected-status-edit-preserved-kll4yq2i/`.
  Provenance/stop-write requests went to research sessions
  `ses_e89031db5ffehnSPru0cN5qDUZ` and `ses_e88f61dccffeqIbc4Ma0YUzlLv`.
  Session `ses_e88f61dccffeqIbc4Ma0YUzlLv` subsequently completed and identified
  its exact canonical candidate SHA (despite contradictory prose claiming no
  edits); the other session denied writing. The completed worker's identified
  delta was preserved, hash-checked and reversed, then the independently approved
  package applied and committed. No unrelated work was touched. Future assignments
  must reiterate read-only grants and absolute private artifact paths.

## Detailed inspector candidate and independent diagnosis

Candidate adds truthful optional byte/estimated counts, provenance, withheld-content
placeholders and page-inventory category rows. It is not a measured whole-request
token split or proven exact Claude Code visual clone. No local Claude Code source
was found. Upstream sidebar usage is distinct from the requested `/context` extension.

- v1 fabricated category zero totals for unknown items; rejected.
- v2 corrected unknown/mixed counts but five rendering assertions failed because
  one long breakdown row hid category data. No integration.
- v3 split categories into short rows. A test helper initially failed to compile
  because scrolling needed `&mut ContextView`; corrected only in private tree.
- The resulting run passed 22/23 tests. Remaining failure was generic
  `!seen.contains("3 items")`, colliding with truthful page header
  `Context · 3 items · projection available`.
- Independent reviewer `ses_e88ea7c96ffeXWoM1LdIeaU8aB` confirmed this as a
  mechanical assertion-scoping defect, approved product code provisionally, and
  required a category-scoped negative before acceptance.
- Coordinator corrected it to reject `3 items` only on a `conversation:` row.
  Final rerun and exact aggregate re-review are still required.
- Exact assembled aggregate candidate (three files, includes status regression):
  `.../T/opencode/context-inspector-final-review-1ykpznlf/candidate.patch`.
  SHA256 `af4f1d5c2a51f48c137b1651f5b64bbeabca7c94fae502ea01e69860a2cdae7a`.
  Product `native_context.rs` SHA256
  `f5adeda00904178c42b586bc1b7324c5248c137d1babcd950a1d8bdeac1b2aed`;
  test SHA256 `40d84932925c3ecac764bb8e920a24e9e930bddc507b40dc86bdec7447768b7b`.
  Do not mistake status-only approval for approval of this larger patch.

## Rendered parity evidence and current dependency blocker

Pixel-perfect labels, coordinates and colors remain UNPROVEN. Multiple source
audits used the wrong pin or inferred color/position claims from selected text
excerpts; require verified sources and actual rendered frames before acting.

- Actual full-app probe v2:
  `.../T/opencode/rk-visual-probe-20261026-v2/home-chat-capture.test.tsx`.
  SHA256 `56d51ea1a63a336438a71e2b48b5054db0d66a2899ef41b67e0760444e1d7d12`.
  Mounts real upstream app using `createAppFixture`; waits for bounded `ready`,
  Home/chat markers and visual idle; captures grids/spans at 120x40, 106x40, 105x40.
- Disposable source and logs:
  `.../T/opencode/upstream-render-capture-roo47m1l/`.
- First run failed before mount: source requires OpenTUI 0.6.8, pre-existing
  upstream node_modules resolved 0.4.5, missing `NativeError`. No frames produced.
- Known disposable dependency symlinks were removed without touching upstream;
  isolated `bun install --frozen-lockfile --ignore-scripts` completed with exit 0
  (shell `sh_177214fe3001eo7zjH9R1QG01e`). Inspect the install receipt,
  then retry probe with disposable HOME. Do not poll or start concurrent heavy
  validation. Do not silently alter dependencies or swallow readiness failures.

## Other pending packages and review cautions

- Second-client EOF diagnosis: `ses_e88ea7c8cffekjUIJZtZkr6uPJ`.
  Discovery audit claims about macOS PID liveness/config roots are unverified;
  correlate actual source and launch stderr before proposing a fix.
- Horizontal fixture repair: `ses_e89031dbaffe6BM26XShd7ipi9`; reviewer
  `ses_e88eb2ed1ffe6ExuTgL0v6GS6U` rejected prompt-label registry because tabs
  render daemon titles (`Chat`), not prompts, and no real UUID binding existed.
- Vertical fixture already committed in `52a89...`, self-check PASS, installed
  journey UNEXECUTED. `ses_e88fed35fffeX6HLf9r1akKOtL` owns private repair for
  catalog display-name readiness and the same false prompt-as-title assumption.
- Home audit corrections: `ses_e88f83ce1ffeTeqkUDWUvQJTEM`; capture author:
  `ses_e88f83cd9ffe1o6ItW6EsgujZn`. Require exact component/paint-path evidence.
- Compaction retained-tail regression: `ses_e89012d76ffe21U8NIArlnrfXX`;
  independent strategy review `ses_e88e8f6ecffeu84DgOMbalOC4l` rejected earlier
  standalone tests (private helper visibility, dropped env guard, waiting for a
  background checkpoint that is committed only on foreground reuse, overspecified
  boundary assertion). Need real preserved-history regression before product fix.
- PTY persistent ownership, keyboard tabs, browser/Copilot OAuth, full catalog,
  subagent inspection and whole-product coverage remain unfinished/unproven.
  Scoped device-auth coverage was 91.74% lines, not whole-product or 100%.

## Preservation and dispatch follow-up

- Untracked `.private-tmp/`, decoder `.red_snapshot.rs`, `tests/e2e/T/` include
  worker mistakes; preserve bytes before any cleanup and verify writers stopped.
  Preserve unrelated `integration/` and `manifest_output.txt` user work.
- Existing assignments predate the new scratchpad requirement. On their next
  substantive dispatch require explicit role/goal/success criteria first, then
  `docs/AGENT_WORKER_PRACTICES.md`, and a unique scratchpad path/hash.
- Session IDs above identify prior assignments, not a verified live-worker count.
  Rebuild the active assignment roster from harness state/notifications; maintain
  20+ useful allowed assignments with the per-route concurrency cap and one heavy
  slot. Do not describe completed agents as active or poll pending agents.

## Immediate next actions

1. Finish and preserve the requested agent instructions/scratchpad documentation.
2. Receive isolated dependency-install result; retry real upstream capture.
3. Integrate the approved status-only package promptly, or the combined inspector
   only after its corrected tests and exact independent review pass; do not stall
   the status repair on unrelated blockers.
4. Rerun corrected context suite and affected CLI regressions; preserve receipts.
5. Repair second-client failure and tab fixture identities; run exact-release
   installed journeys after final integration. Rebuild for the actual final SHA.
6. Complete verified installation and fast-forward push only after release checks.

## New role-first dispatch roster — 2026-10-26

Twenty new background assignments were launched on the allowed unlimited
`9router-vyce-deepseek-v4.1-free-combined` route after the rules were committed.
Each prompt assigns role, goal and success criteria first, then worker practices,
explicit read-only/private grants, source pins, resource limits and a unique
research scratchpad path/hash requirement. These are launch receipts, not a claim
that every assignment remains active after this snapshot; update on completion.
No route usage telemetry was returned; consumed/remaining tokens remain UNKNOWN.

| Session | Assignment |
| --- | --- |
| `ses_e88d69ce8ffeZCusa8cOLB0F2I` | Final context aggregate review |
| `ses_e88d69ce2ffeQjDPvtUDUFQuh6` | Wire-to-UI token normalization evidence |
| `ses_e88d69cd6ffeJHwwPLF1hy3irJ` | Private context installed-journey author |
| `ses_e88d69cd4ffeXQ7i0yUzX5nXew` | Operating-instructions audit |
| `ses_e88d69c4affelIqL0fWyzjnAYe` | Model output limit propagation |
| `ses_e88d69c49ffelbqhgSyI6SudfU` | Credential/catalog availability contract |
| `ses_e88d69c44ffevqSB0bay7HDBvD` | Browser OAuth parity package |
| `ses_e88d69c3effefvtxyLUf4G1uMv` | Copilot entitlement/transport contract |
| `ses_e88d69c3dffeP2Guqpqw6bOIE1` | Real PTY cancellation regression review |
| `ses_e88d69c37ffejbFbCJF1HONrRe` | Persistent PTY ownership contract |
| `ses_e88d56019ffeYYipe9Aw99jevG` | Private keyboard tab decoder candidate |
| `ses_e88d5600fffepyWKGmG4cZKga7` | Private leader tab dispatch candidate |
| `ses_e88d56006ffewtocB1Wmf4VBr1` | Child inspection behavior review |
| `ses_e88d56000ffeqoLH4ZPxXG05qe` | Executable honest coverage manifest |
| `ses_e88d55f84ffe61W2pion4IU1Qz` | Release/package provenance prep |
| `ses_e88d55f7dffefeGDtU1f8fLfQu` | Worker artifact preservation inventory |
| `ses_e88d55f7dffdly4XpexGhsHhLE` | Global CLI settings parity |
| `ses_e88d55f6affe1nSMUg6fSHhOTM` | Last-tab Home direct-send safety |
| `ses_e88d55f68ffeS42RTNZ4kig7v2` | Third compaction regression strategy review |
| `ses_e88d55f68ffdrB0eOsXmq6vHmD` | Current device-auth edge fixture review |

Heavy slot: upstream capture retry with correct locked dependencies launched as
`sh_1772acd18001sScpqMRKhltHgg`; logs/receipt `locked.*` under the disposable
upstream capture root above. It completed: **4 PASS / 2 FAIL**, exit 1. Home at
120x40 and chat at all three sizes captured actual frames; Home 106/105 timed out
because the fixture demanded one specific rotating example while the rendered
placeholder showed another valid example. Preserve failures; repair readiness
against the actual placeholder contract before retrying only the failing cases.
The captured upstream footer visibly includes `ctrl+p commands`, refuting the
earlier source-only audit that called it a fabricated Rust hint. No Rust visual
comparison or pixel parity is established. Heavy slot is released. Corrected
inspector tests and exact integrated-SHA CLI checks are next in that serial slot.

Operating-instructions audit completed APPROVE with a minor authority-list wording
clarification, now applied. Durable rationale and original scratchpad SHA256 are
in `docs/worklogs/research/2026-10-26-agent-operating-rules-review.md`. The worker
wrote inside canonical `.private-tmp/` despite its read-only grant; preserve the
original and require a genuinely external absolute scratchpad on reassignment.

## Subsequent verified findings and availability changes

- Corrected inspector tests now PASS: 23 context tests and all 847 CLI tests in
  `context-red-green-d7y4lapv/final-{context,cli}.stdout`; final receipt records
  exact source hashes. Original evaluator `ses_e88ea7c96ffeXWoM1LdIeaU8aB`
  received the final aggregate and APPROVED exact SHAaf4f1d5c... . The two detail
  files were integrated as `27722646f55ba59276d1308e7b2e55eaff031b04` (status was
  already integrated). A fresh exact-27722646 source archive with a fresh Cargo
  target passed **847/847 CLI tests**; raw logs and receipt:
  `.../T/opencode/context-exact-integrated-jjhruynv/`, plus the shared Git evidence
  receipt at `recovery-evidence/27722646f55ba59276d1308e7b2e55eaff031b04/context-integrated/`.
  Installed click and `/context` journeys still PENDING; do not repeat a reviewer's
  claim that preintegration candidate logs were exact integrated verification.
- Full normalization trace confirms inclusive Rust input/output must NOT have
  cache/reasoning added again. Upstream normalizes before session summation:
  `packages/ai/src/protocols/open-responses.ts::mapUsage` and
  `packages/core/src/session/usage.ts::tokens`. For input=100, cache=40,
  output=20, reasoning=5, upstream decomposes 60+40+15+5=120; Rust uses
  100+20=120. The unreviewed 200-total worker proposal was incorrect.
  Research note: `.../T/opencode/research-token-normalization/scratchpad.md`,
  SHA256 `4c5903f633dd66bffda5342986e92db7040cf342a6d1cc60ccd6aaa20e029cda`.
  Missing Rust cache-write detail is a separate gap, not a reason to inflate totals.
- Upstream Home retry accepted the actual rotating-placeholder contract rather
  than one particular example. `home-retry-receipt.json` exit 0; all six Home/chat
  viewport captures are now available across `frames-locked/` and
  `frames-home-retry/`. These are reference captures, not a Rust differential.
  All 12 raw files, hashes, label rows and run receipts were also preserved under
  the shared Git directory at
  `recovery-evidence/upstream-450e7b92412c780a7b5a75226bb8d49aaca24e22/rendered-home-chat/`.
  Repeated source-only claim that `ctrl+p commands` is absent is refuted by the
  actual upstream captured footer; do not remove it based on literal grep.
- Combined DeepSeek route returned `no eligible account` for sessions
  `ses_e88d55f6affe1nSMUg6fSHhOTM`, `ses_e88d69c44ffevqSB0bay7HDBvD`,
  `ses_e88d69c4affelIqL0fWyzjnAYe`. Pause NEW dispatches on that route; do not
  infer token exhaustion or balances. Existing workers may still return results.
  Replacements: GLM `ses_e88cf7e8cffeQQupeK9RWPjqgO` (Home send safety),
  Token Harbor `ses_e88cf7e88ffeq5jHM32xSWbaIM` (browser OAuth) and
  `ses_e88cf7e86ffemIND4Wyk4N2s7v` (output limits). GLM cumulative use UNKNOWN;
  stop dispatching it on any error as required by its budget rule.
- Horizontal F1v2 SHA6661c295... was independently rejected again by
  `ses_e88eb2ed1ffe6ExuTgL0v6GS6U`. Merely renaming a prompt-label registry
  does not fix actual `Chat` titles. Strategy note:
  `.../T/opencode/strategy-tabs-identity/TAB_IDENTITY_STRATEGY.md`, SHA256
  `15924b1a825573aa0e05b941270d9b8dbe2f4d9aa5899cc693584c0598e40ca2`.
  Next repair must use bounded rendered title occurrences checked against the
  authenticated UUID ledger, or genuinely created/renamed unique titles with
  verified propagation. No fixture acceptance from self-check/assertion counts.
- Original edge-auth negative reviewer approved SHA3a4d55... boundedness, but
  coordinator then found a missed EOF defect: `.expect(...)` on `read()` rejects
  errors, not `Ok(0)`, so the helper spins until timeout rather than failing EOF
  immediately. No test execution/integration yet. Token Harbor private author
  `ses_e88cea7e7ffeL1e1uHVpVJqm0U` owns explicit `n > 0` correction in a NEW
  external copy; original reviewer must evaluate that new hash again.
- Confirmed actual source defect: `daemon_client.rs::pid_alive` and
  `discover_presence` both use `/proc`, invalid on macOS. Server's existing safe
  rustix `kill(pid, 0)` implementation provides the repair pattern. Token Harbor
  author `ses_e88cde434ffe1Tk103SMKsVVrb` owns only a private daemon_client.rs
  candidate plus regression. No user daemon touched; do not call this the proven
  sole cause of the second-client EOF until the installed journey is rerun.
- Packaging audit `ses_e88d55f84ffe61W2pion4IU1Qz` reconciled old b24 receipt
  hashes, but its rebuild trigger list was incomplete: ANY linked product source
  change requires a new binary/release provenance for the new SHA, not only build
  scripts/lock/installer changes. Old b24 binary/archive remain evidence for b24;
  never relabel them as a newer verified release. Audit note:
  `.../T/opencode/auditor-b24-scratch/RELEASE_AUDIT_b24.md`, SHA256
  `10a925c2d9c6b2d1d15b06cab8bd7296fafe62636c44c53da419bb0528775e81`.
- Vertical repair author claimed `.../T/opencode/vertical-tabs-repair-v2/` but
  coordinator read confirmed that directory DOES NOT EXIST. Treat the handoff as
  INCOMPLETE, not an implementation. New Token Harbor owner
  `ses_e88ca554affeVqwp0ljN0bmuXS` owns real private horizontal/vertical fixture
  artifacts using the independent identity strategy and correct model labels.
- PTY reviewer `ses_e88d69c3dffeP2Guqpqw6bOIE1` approved the cancellation-test
  idea but reported the fix diff malformed. Do not apply a fragment or blindly
  change cleanup to return every existing state: review successful/running/error
  retry semantics and run a compiling RED/GREEN on a real candidate first.
- Edge-auth build-feature concern is now resolved by ACTUAL COMPILATION, not
  partial `cargo tree` output: exact test SHA3a4d55... compiled with Rust1.98.1,
  locked/offline `cargo test -p opencode-rk-providers --test
  openai_device_auth_edge_cases --no-run`, exit 0. Private source/receipt:
  `.../T/opencode/device-edge-compile-proof-ssxngo_a/`. No tests executed and
  no dependency changes; explicit EOF `n > 0` correction/re-review still pending.
- GLM Home audit `ses_e88cf7e8cffeQQupeK9RWPjqgO` refuted stale send for
  EMPTY Enter after last-tab `/exit`: draft is cleared and empty submit gate blocks
  the turn. It separately traced a real adjacent hazard: typing a NEW draft and
  submitting can use retained `live`/selected state and the closed tab's session.
  Do not report the empty-Enter result as proof that new-prompt routing is safe.
  Same GLM session reassigned to an actual installed new-prompt regression, private
  Python test only; shared `tui_entry.rs` implementation remains serialized.
  Research scratchpad SHA256
  `87e3f65b12bdddfd285c6b01324ddb19533740dafdac79d8bb8d5a9ec80be7da` at
  `.../T/opencode/review-stale-send-exit-lasttab-home-enter-2026-10-26.md`.
  GLM route working, no errors; cumulative tokens still UNKNOWN.
- Global CLI config audit reported that keybind overrides/leader timeout are not
  loaded (only tab layout parsed). Needs source validation before implementation;
  its returned scratchpad path points to a different `recovery-20261006` tree,
  so do not use that path as canonical evidence or claim a delivered artifact.
- Preservation inventory is available at
  `.../T/opencode/preservation-audit-scratchpad.md`, SHA256
  `5bf677988139e2f3d3d61cbbdbd0966d60569a9dd7d68d5995278241df8581e6`.
  This inventory is not authorization to delete user files. Its ETX-to-`/quit`
  copies remain REJECTED proposals, not valid test corrections; retain original
  Ctrl-C terminal-restoration assertions and do not integrate those copies.

## Latest handoffs and ownership — 2026-10-26

- Leader author `ses_e88d5600fffepyWKGmG4cZKga7` wrote canonical
  `crates/cli/src/tui_entry.rs` contrary to its private-copy grant. Actual diff
  matched its reported 171 insertions/6 deletions. Coordinator preserved exact
  file, base, binary diff, SHA256 hashes and receipt externally at
  `.../T/opencode/leader-canonical-delta-preserved-56sk_j24/`, then hash-checked
  and reversed only that completed worker's exact delta. Canonical product file
  restored to committed base; no unrelated files changed. Token Harbor evaluator
  `ses_e88bae68bffes0sGBMTdOX7Dqj` reviewing preserved candidate. No build/test
  results or integration approval exist yet.
- PTY researcher returned lifecycle evidence at
  `.../T/opencode/pty-research-20261026-9router/scratchpad.md`, reported SHA256
  `ee439e362b0c55d7b4779751680ba3ac8229a5849094cc8bb4aeb6636a00b495`.
  Distinguish pipe-backed shell streaming, in-process PTYs and persistent daemon
  PTYs; unused gateway state-machine code is not a wired runtime feature.
  The worker's assertion that persistent PTY is outside scope is NOT authorized:
  persistent daemon-owned work remains a user requirement. Revalidate precise
  source claims before implementation; no end-to-end continuity acceptance.
- Composer ACK audit `ses_e88d69cd4ffeXQ7i0yUzX5nXew` returned NO_PASS but
  its argument is internally inconsistent: differing drafts are preserved by
  the helper, and it did not account for `binding_active` around event draining
  (`tui_entry.rs:3645-3655`). Need a concrete same-text cross-tab/generation
  regression before accepting its proposed per-tab pending-submission redesign.
- GLM last-tab regression handoff confirmed no artifact was ever written; a tool
  command timed out during follow-up. Pause new GLM dispatches under stop-on-error
  rule; token balance UNKNOWN. Replacement Token Harbor author
  `ses_e88be529dffenPmnrU7qap3J5e` owns actual private Flow C artifact.
- Luna `ses_e88bd254fffeg0P1UZYgSytZK9` owns the exclusive heavy slot for a
  real compaction tail-retention RED/GREEN in private copies of
  `auto_compaction.rs` and `background_compaction_runtime.rs`. Preserve storage
  head-identity and retained-tail bounds; do not schedule concurrent Cargo/PTY.
- Combined-route completions continue arriving from previous launches, but new
  assignments remain paused after no-eligible-account failures. Current useful
  concurrency is not verified at 20; route availability and incomplete handoffs
  constrain replenishment. Record launches/completions rather than claiming all
  twenty original assignments remain active.
