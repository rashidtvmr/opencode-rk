# E2E-APP — Default-to-TUI chat, actionable doctor, web UI repair

Status: IMPLEMENTED (lane evidence; verifier decides acceptance).
Base rev: 165b076 (worktree). Date: 2026-09-17.
Bounds: `crates/cli/src/chat.rs` (NEW), `crates/cli/src/main.rs` (wiring +
doctor next-steps), `crates/cli/tests/default_tui.rs` (NEW),
`crates/cli/tests/doctor_next_steps.rs` (NEW), `web/src/**`, `web/package.json`,
`web/pnpm-workspace.yaml` (NEW), `web/vitest.config.ts`,
`crates/server/web_dist/**` (rebuilt bundle). No control-plane or other-lane
files; no frozen test edited to pass.

## Claim

1. Bare `opencode-rk` (no subcommand) opens an interactive chat TUI. It probes
   the singleton daemon via real `GET /health`, auto-spawns and owns one if
   absent (`--listen` on a free loopback port; killed on exit — no detached
   processes), then submits real turns via `POST /api/sessions/{id}/turns`
   with honest offline degradation (`[offline]` hint when not attached).
   Commands: `/new [title]`, `/model [id]`, `/models`, `/help`, `/exit`.
2. `doctor` now emits a per-check `next_step` (human line + JSON field)
   naming the exact command/flag to fix each unconfigured item.
3. Web UI: served bundle was stale (source had turn-stream + history endpoints;
   bundle predated them). Fixed source defects blocking the frozen suites:
   `listHistoryPage` 404-fallback page size (50→200 contract size), composer
   `innerText` crash on detached nodes, dropdowns hidden from the a11y tree
   (`isNonModal` routed through the shadcn wrapper to react-aria Popover),
   branch-then-execute clobber race (turn-seeded session latch on the load
   effect), and deterministic Escape→trigger focus restore (WAI-ARIA menu
   pattern) in the DropdownMenu wrapper.

## Source evidence

- `crates/server/src/lib.rs:81` health route is `/health`; `:86-93` session
  create/list/message/history routes; `:248-266` create body `{title}`;
  `:284-310` turn endpoints — verified before implementation.
- `crates/contracts/src/lib.rs:183-192` `PayloadRef` serializes internally
  tagged `{"storage":"inline","text":...}` — TUI/web extraction matches wire.
- `crates/server/tests/session_turn_api.rs` — sanctioned fake-provider seam
  (`OPENAI_BASE_URL`/`OPENAI_API_KEY`) reused by the chat E2E fixture.
- react-aria FocusScope restores focus on a `requestAnimationFrame` during
  scope cleanup (`react-aria/dist/private/focus/FocusScope.mjs:530-560`),
  which loses the trigger in jsdom; hence the synchronous Escape handler.

## RED→GREEN

- `crates/cli/tests/default_tui.rs` — RED 0/4 confirmed against the real
  binary (clap usage, empty stdout) before implementation; GREEN 4/4.
  Frozen hash: d8feb83738ca4e5060d1f9561492cbd5e438a307aa144f8e710d2c81aa28e09e
- `crates/cli/tests/doctor_next_steps.rs` — RED 3/4 (one trivially-true)
  before the `next_step` implementation; GREEN 4/4.
  Frozen hash: 6893c162625f2cb21ca17167f0442f973a1909e2036da8a1cc075ab70f2b2689
- Web frozen suites were failing pre-session (10 of 22); all fixes were made
  in product code (`App.tsx`, `composer.tsx`, `api.ts`, ui wrappers), never
  in tests. Final: 22/22 vitest, `tsc -b` exit 0 (repo's first green typecheck;
  fixed pre-existing `title`/OverlayArrow type debt in shared wrappers).

## Verification

- `cargo test -p opencode-rk-cli` — 46 passed (11 suites), incl. all prior
  TUI/live-state/follow lanes.
- `cargo build --release` — 0 errors, 10 warnings (all pre-existing, other
  crates; my lane clean).
- Live E2E: release binary `serve --listen 127.0.0.1:4601` → `/health` ok,
  `POST /api/sessions` persists; served bundle = fresh `index-XBArXcct.js`
  (web_dist is `include_dir!`-embedded, confirmed served bytes 506192).
- Chat E2E with the fake provider: `/new` → turn streamed → assistant reply
  rendered → persisted (verified by the default_tui suite against the real
  binary and daemon).

## Decisions / deviations

- Daemon auto-spawn uses a per-process free port, not fixed 4096, so tests
  and parallel users never cross-attach; spawned daemon is explicitly owned
  (killed on chat exit) per the no-detached-task rule.
- `web/pnpm-workspace.yaml` is pnpm 11's generated build-gate file; completed
  it with `esbuild: true` (its postinstall links the platform binary needed
  by `vite build`) so installs are non-interactive.
- Remaining upstream gap (not fixable in-product): only the `openai` provider
  is wired to real turns server-side; chat without `OPENAI_*` configured
  degrades to message-append + honest notice (web) / `[offline]`-style hints
  (TUI).

## Remaining unknowns

- Repository guard remains RED from the pre-existing acceptance flip
  (`ralph.json`/`FEATURES.md`); owner reconciliation per
  `worklog/ACCEPTANCE-FLIP-PROPOSAL.md`. Independent of product behavior.
