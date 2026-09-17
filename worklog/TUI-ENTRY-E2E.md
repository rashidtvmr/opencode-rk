# TUI-ENTRY-E2E - TUI CLI entrypoint lane (UI-014..UI-018 observability)

Status: IMPLEMENTED (lane evidence; verifier decides acceptance).
Base rev: b60ceda worktree (product bytes 1be93d3). Date: 2026-09-17.
Bounds: crates/cli/src/tui_entry.rs (NEW), crates/cli/src/main.rs (wiring),
crates/cli/tests/tui_entrypoint.rs (NEW). No control-plane, task, or
ralph.json/FEATURES.md edits. No test outside this lane modified.

## Claim

The binary now exposes a `tui` subcommand binding the existing pure
`opencode-rk-sessions::tui_state` machines (UI-014..UI-018) to process IO:
snapshot frame, interactive line composer, memory pane over real files,
configurable submit keymap (flag > env > default, unknown env fails closed).

## RED / GREEN

- RED: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 300 cargo test -p
  opencode-rk-cli --test tui_entrypoint` -> 6 failed for the missing
  behavior (`error: unrecognized subcommand 'tui'`, exit 2), suite compiled.
- Frozen test hash (tui_entrypoint.rs):
  07e6b021e6dcdbce3fbb606fdfcc6af9f0f2301bbdd3df94dc6c5b0ac3b2c47a
- GREEN: same command after implementation -> 6 passed; full CLI crate
  `timeout 300 cargo test -p opencode-rk-cli` -> 29 passed (7 suites).
- Regressions: sessions TUI-adjacent suites (tui_state, tui_info_panel,
  runner, part_events) 20 passed; `timeout 420 cargo check --workspace`
  0 errors (11 pre-existing warnings, none in this lane).

## E2E (real binary, temp OPENCODE_RK_HOME)

- `--help` lists doctor/session/models/serve/web/tui.
- `tui --once` renders banner, status bar, composer, footer hints.
- `--submit-keymap ctrl-j` and env `OPENCODE_RK_TUI_SUBMIT_KEY=ctrl-j`
  both flip the footer; `--submit-keymap bogus` exits 2 naming the value.
- `--memory <file>` lists real files with byte counts (11-byte probe shown).
- Pipe session `hello` / `?` / `:q` echoes `you: hello`, keybindings help,
  exit 0.
- `doctor`, `session create/message add/list/rename/show/archive` all pass
  against a disposable SQLite home.
- `serve` owns the singleton: GET / returns the embedded app
  (`<title>OpenCode RK</title>`), GET /session 200; `web` attach prints the
  owner origin and exits 0; after owner kill, `web` starts its own daemon.
- First serve attempt used a nonexistent `--no-open` flag (serve never opens
  a browser); corrected flags and re-run clean. No orphan processes.

## Guard state (pre-existing, unchanged by this lane)

- `python3 tools/validate_repository.py` exit 1, 51 errors: the pre-session
  worktree flip of all 82 remaining stories to `accepted` in ralph.json/
  FEATURES.md made the fail-closed ledger unsatisfiable by design
  (`backlog-exhaustion.json` and the 8 ownership-gap records pin those
  stories as non-accepted; validate_backlog_exhaustion.py:2510-2524 rejects
  both removing and keeping them). Reconciliation is controller/verifier
  territory (owner-protected paths).
- Bootstrap unittests: 177 ran, 18 failures + 1 error, all
  ledger/plan/controller classes matching GUARD-TRIAGE-20 pre-session
  baseline; zero failures reference this lane.
- Stub scan (`todo!`/`unimplemented!`/`#[ignore]` in crates/) 0 hits;
  `git diff --check` clean; my lane files fmt-clean
  (repo fmt debt 58 files carried, untouched per INTEGRATION-19).

## Follow-up lane: TUI-LIVE (composer/status bound to daemon state)

Same bounds, added files: crates/cli/tests/tui_live_state.rs (NEW),
crates/cli/src/tui_entry.rs extended with --origin/--session.

- RED: `cargo test -p opencode-rk-cli --test tui_live_state` -> 4 failed
  (`unexpected argument '--origin'`), suite compiled. Frozen hash
  (tui_live_state.rs): 6c95f595b62f51940cf51f3c814a0d503318a5dd05eb424bf022e268be4a7c44
  (pre wire-shape fix; see note below).
- Implementation: std-only bounded HTTP client (1 MiB cap, 2 s timeouts) over
  GET /api/sessions + GET /api/sessions/{id}/messages (server routes at
  crates/server/src/lib.rs:86,89); POST /api/sessions/{id}/messages persists
  submits (lib.rs:428-440). Default session = most recently updated; --session
  pins an id. Dead origin: --once fails closed with a typed error; interactive
  degrades to an explicit `daemon offline:` banner and marks local submits
  `[offline: not persisted]`. Live data is never fabricated.
- Wire-shape fix found by E2E: PayloadRef serializes internally tagged
  (`{"storage":"inline","text":...}`, contracts/src/lib.rs:188-190), not
  externally tagged; snapshot parser and test extraction updated accordingly
  (test file hash above predates that one-line fix).
- GREEN: tui_live_state 4 passed (spawns a real daemon, persists through it,
  asserts durable visibility); full CLI crate 33 passed (8 suites).
- E2E (real binary): `tui --once --origin` renders live title/state/updated/
  message count/last message tagged `(live)`; interactive submit shows
  `[persisted]` and the daemon's message list contains the text; dead origin
  --once exits non-zero naming the origin; fmt-clean (private_interfaces fixed
  by narrowing render_frame visibility); `cargo check --workspace` 0 errors,
  warnings unchanged (11 in CLI crate, none from this lane).
- Honest scope: status frame still shows placeholder model/context labels;
  live turn streaming (POST /turns) is not wired into the composer, so no
  token counts or provider state are invented.

## Follow-up lane: TUI-FOLLOW (bounded live polling)

Added file: crates/cli/tests/tui_follow.rs (NEW); tui_entry.rs gains
--follow/--poll-ms/--follow-for.

- RED: `cargo test -p opencode-rk-cli --test tui_follow` -> 5 failed
  (unexpected argument '--follow'), suite compiled. Frozen hash
  (tui_follow.rs): ddd4fe4f697561aca71c8659492b7ef0e116257b2b9fe0e99cf080401e3f8032
- Implementation: follow_loop polls fetch_snapshot every --poll-ms (floor
  50 ms), re-renders render_live only when the updated_at/count/state/title/
  last fingerprint changes, marks changes `--- update ---`, stops after
  --follow-for seconds when bounded, degrades offline polls to a
  `daemon offline:` line without exiting. --follow without --origin is a
  typed usage error. Read-only: stdin ignored.
- GREEN: tui_follow 5 passed (other-client message detected mid-run,
  bounded no-change exit 0 with no marker, dead-origin degrade, missing
  --origin error, pinned-session isolation); full CLI crate 38 passed
  (9 suites); my files fmt-clean; `cargo check --workspace` 0 errors.
- Mandated serial gates re-run on this tree (JOBS=2 THREADS=2,
  --test-threads=1, timeout 120): session_turn_stream_api 2 passed;
  ext_manifest_lane 5 passed; codex_oauth + bounds + bounds2 17 passed.
- Guard after all lanes: validate_repository exit 1, unchanged 51 errors
  (pre-existing acceptance-flip class; zero lane-caused). git diff --check
  clean. Track-file delta: only crates/cli/src/main.rs (+6 lines wiring);
  new files untracked by design (lane-owned).

## Remaining unknowns

- Guard reconciliation flip ownership (controller, blocked on policy).
- `serve`/`web` subcommands intentionally lack `--no-open` on `serve` only;
  harmless asymmetry, unchanged here.
