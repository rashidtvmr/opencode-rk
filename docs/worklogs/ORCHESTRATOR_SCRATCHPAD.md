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
  `52a89c156a6e6e30268c49d2587b131836c11233` (`main-v2`).
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
  package and existing-assertion changes. Ready for scoped integration after
  checking current base/path hashes. Not yet integrated at this snapshot.

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
