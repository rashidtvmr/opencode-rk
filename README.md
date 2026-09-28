# opencode-rk

Native, local-first coding-agent harness. Rust + Tokio core, embedded SQLite, one singleton daemon per user/data-dir, many clients (CLI, TUI, web, headless).

**Status: `0.1.0-alpha.1`.** Source build today; `scripts/install-oc2.*` installs a release archive (artifacts pending). This README describes what runs today, not the full plan. Plan authority: `PLAN.md`, `FEATURES.md`, `FEATURES-COMPLETION.md`, `ralph.json` + `ralph.completion.json`.

## What it is

A lean OpenCode-V2-compatible agent runtime without requiring PostgreSQL, MongoDB, Redis, or a JS toolchain at runtime:

- One daemon owns sessions, execution, permissions, persistence, events.
- Thin clients attach over authenticated loopback HTTP: CLI, native TUI, embedded web UI, headless/CI runner.
- Model metadata comes from models.dev, served through our own versioned query API (`models sync` / `models search`).
- Safety is deterministic and capability-based: permission broker + denylists + execpolicy prefix rules. `*` does not bypass mandatory controls. Prompts/hooks never grant access.

Reference pins: OpenCode V2 `95daf90`, 9router `17c4cc7`. Full-file upstream audit still open (see `docs/SOURCE_AUDIT.md`, `docs/audits/`).

## What works today

Verified against `./target/debug/oc2` (see `docs/USER_GUIDE.md`):

- `doctor` — capability, auth, connectivity, tools, MCP status, `--json` for redacted bug reports.
- `session create/list/show/rename/archive`, `session message add/list`.
- `models sync` (fetches `https://models.dev/api.json`), `models search --query --provider --reasoning --tools --limit`.
- `serve` — foreground singleton daemon on `127.0.0.1:4096`. Second instance on same data-dir exits with existing origin (`AlreadyRunning`).
- `web` — serve + open browser (or `--no-open` print URL only). Production bundle embedded in Rust binary (`crates/server/web_dist`).
- `tui` — interactive client; `--once` bounded snapshot, `--origin` bind to live daemon, `--follow` requires `--origin`. Offline degrades to explicit `daemon offline`, never fabricated state.
- `run` — headless/CI execution (`--ci --prompt ... --max-steps --timeout`, JSONL events).
- Chat REPL (no subcommand on a TTY): `/new /sessions /open /models /model /help /exit` (+ `/quit /q`, `?` alias; `chat.rs:451-476`), plain text sends a turn.
- Server API: `/health`, `/api/capabilities`, `/api/workspaces`, `/api/models`, `/api/sessions` CRUD + `/messages /history /attachments /activity /artifacts /branch /retry-branch /fork /turns /turns/stream`. Authenticated router via bearer middleware; legacy unauthenticated router kept for frozen tests only.
- Turns: OpenAI Responses SSE consumed natively, exposed as bounded NDJSON. Dropping the browser response drops the upstream request. Completed JSON `/turns` kept; older daemons fall back to message-only endpoint.
- Web client: React 19 + Vite + TS + Tailwind 4 + shadcn (`aria-nova` base). Health, model picker, session create/rename/archive, bounded history, streaming turns, attachments UI, branch/retry, artifacts, workspace scoping, axe smoke test. Attachments/screenshots/voice/non-OpenAI adapters are explicit runtime gaps, not simulated.
- Providers: env keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GEMINI_API_KEY`) + Codex OAuth and Claude Code OAuth (PKCE/loopback) connectors, consent-based local credential import, redacted auth storage (keyring with `0600` fallback), usage/limits telemetry, versioned compatibility catalog, offline redacted fixtures.
- Storage: embedded SQLite (WAL), per-user global catalog + lazily opened workspace DBs, content-addressed blobs, JSONL rollout truth + sqlite snapshot, bounded events, quotas/retention. Never mutates the user's existing OpenCode DB; tests use disposable dirs.
- Subagents: fork/fresh-context spawn, multi-provider routing, resume/retry with backoff + circuit breaker, mid-session model swap, compression (thresholds + breaker), structured handoff, pool manager, observability counters, narrow-only permission inheritance, output aggregation, cancellation with real reclaim.
- Tools/MCP/ACP: typed tool contract + `ToolSearch`, per-MCP power toggle, lifecycle persistence, disabled tools excluded from payloads, MCP policy gate + elicitation, bounded hook bus (100, shift-drop, SSRF guard), ACP v1 JSONL bridge over stdio with explicit `Unsupported` surface, per-session runner (busy rejection, concurrent shell lane, owned cancellation).

## Quickstart

```sh
git clone <repo-url> opencode-rk
cd opencode-rk
cargo build -p opencode-rk-cli
./target/debug/oc2 --help
./target/debug/oc2 doctor
```

First run (data under `~/.local/share/opencode-rk`, override with `--data-dir` / `OPENCODE_RK_HOME`):

```sh
./target/debug/oc2 doctor
./target/debug/oc2 session create "My project"
./target/debug/oc2 session list
./target/debug/oc2 models sync
./target/debug/oc2 models search --limit 25
export OPENAI_API_KEY=...
./target/debug/oc2 serve --listen 127.0.0.1:4096  # terminal 1
./target/debug/oc2 web --no-open                  # terminal 2, or just `web`
./target/debug/oc2 tui --origin http://127.0.0.1:4096 --once
./target/debug/oc2 run --ci --prompt "doctor" --max-steps 5 --timeout 120
```

Frontend dev only:

```sh
cd web && pnpm install
VITE_API_ORIGIN=http://127.0.0.1:4096 pnpm dev
pnpm test && pnpm typecheck && pnpm build  # build embeds into crates/server/web_dist
```

## Architecture

```text
CLI / TUI / web / headless / SDK
  -> loopback HTTP (bearer) -> singleton daemon (Tokio, one runtime)
    -> session service -> agent loop -> provider adapter -> tools (allowlisted)
    -> permission broker -> SQLite + blobs + JSONL rollout -> event bus / SSE
```

- `ADR-001`: one Tokio runtime per daemon; bounded CPU pool only where profiled. No process-per-subagent.
- `ADR-002`: singleton per OS user + data dir, lazy location activation, LRU caps. Remote multi-user is separate opt-in profile.
- `ADR-004`: SQLite canonical state + content-addressed blobs; live deltas vs durable history vs replay windows vs audit vs disposable caches separated.
- `ADR-006`: all agent ops go through trusted policy broker; OS sandbox backend unavailable on this platform (`doctor` reports `os sandbox: unavailable on this platform` — config text is not a sandbox).
- `ADR-007`: implementer never self-accepts; controller + independent verifier own frozen tests and exact-tree reruns.

Crates (12): `agents catalog cli contracts foundation opentui-bridge providers security server sessions storage tools`. Plan detail: `PLAN.md` §3.

## Trajectory

Where this repo came from and where it goes. Every row is still mandatory; lean milestones never delete remaining work (`PLAN.md` §1).

| Phase | Milestone | Result |
|---|---|---|
| Done (foundation) | M0–M2 | Pinned sources, RED/GREEN pipeline, daemon + sessions + models + one coding turn with streaming |
| Done (wiring) | M3–M4 | Delegation/routing, fork/rewind, skills/plugins/commands, MCP/ACP, full history |
| In progress | M5 | Optional compat (web/parity contracts with cost attribution), native TUI over owned OpenTUI fork (`rashidtvmr/opentui`) |
| Open | M6 + completion backlog | Hardening, resource evidence, reproducible artifacts, release certificate |

Completion backlog (`FEATURES-COMPLETION.md`, `ralph.completion.json`): **90 vertical parents, 450 concrete scenarios**, all `not-started` until independently verified:

| Family | Slices | Outcome |
|---|---|---|
| AUD-001..020 | 20 | Independent source/runtime re-audits, legacy-ID reconciliation, dev-delta |
| APP-001..012 | 12 | Installed `oc2` one-command app, secure daemon, onboarding, golden journey |
| TUI-001..011 | 11 | Owned Zig ABI/Rust bridge, real renderer, composer/transcript/tabs/approvals/settings, packaging |
| PAR-001..010 | 10 | A-to-Z upstream parity + provider/routing/agent/security/plugin/tool/web/protocol extras |
| NET-001..015 | 15 | Owned identity/control plane, pairing, scoped protocol, named tunnel, outbound PC connector, files/PTY, replay/revoke |
| MOB-001..006 | 6 | Real iOS/Android installs, login, secure tokens, multi-PC tabs, lifecycle, device acceptance |
| COORD-001..008 | 8 | Native adapter, rolling 20-worker scheduling, one-file children, frozen tests, durable leases/budgets |
| SHIP-001..008 | 8 | Platform artifacts, clean-machine/provider/source/remote/device/resource evidence, release decision |

Upcoming-feature research already mined into the backlog (`docs/upcoming-features/`): dangerous-pattern denylist + execpolicy, turn submission state machine, granular approvals, bounded hook bus + SSRF guard, provider tap + redaction + debug export, dual rollout + dedupe, fork/revert/rollback split, auto-compact + cheap-model chores + cost counters, plan mode + structured review + `request_permissions`, safe skill extract + `/doctor`.

Known blockers (honest, see `docs/USER_GUIDE.md` §BLOCKED): no published release archive yet (`scripts/install-oc2.*` present, SHIP-001/002 pending), no in-app provider setup screen, no pairing/tunnel/mobile, no operator self-host deploy, OS sandbox backend unavailable on this platform, JS compatibility host disabled. Tunnel transport is not auth; Cloudflare terminates TLS unless app-layer encryption lands separately.

## Repo map

| Path | Purpose |
|---|---|
| `PLAN.md` / `AGENTS.md` | Architecture + non-negotiable worker contract |
| `FEATURES.md` / `ralph.json` / `prd.json` | 258-story backlog, requirements map, flat export (export is not acceptance authority) |
| `FEATURES-COMPLETION.md` / `ralph.completion.json` | 90-slice completion backlog + cards (`python3 tools/completion_plan.py --card <ID>`) |
| `requirements/` | Explicit requirement checklist |
| `sources/` | Pins, inventory, behavior-surface rules, audit limits |
| `crates/` | Rust workspace (see above) |
| `web/` | React client, embedded at build time |
| `docs/` | `SECURITY.md STORAGE.md TDD.md ADAPTER_PROTOCOL.md AUTONOMOUS_EXECUTION.md REPOSITORY_PROTECTION.md SOURCE_AUDIT.md USER_GUIDE.md upcoming-features/ audits/` |
| `config/` | Lean defaults, resource targets, example controller settings |
| `tasks/` / `worklog/` | Task cards + session scratchpads (advisory, not proof) |
| `tools/` / `tests/` / `validation/` | Controller, guards, bootstrap tests, recorded logs |

## Develop / verify

```sh
python3 tools/validate_repository.py
python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v
python3 tools/completion_plan.py --check
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p <crate> --test <target>
```

Rules: 8 GiB host budget (2 GiB reserve), one heavy command at a time, disposable fixtures only, never touch the live OpenCode DB. Frozen tests are immutable — fix code, not tests. See `AGENTS.md`, `docs/TDD.md`, `docs/CONVERGENCE.md`.

## Reporting issues

Attach: redacted `doctor --json`, exact command, exit code, last 20 stderr lines, binary version (`0.1.0-alpha.1`), OS, `OPENCODE_RK_HOME` override state. Never paste full keys, MCP secrets, or your live data dir. Recovery table: `docs/USER_GUIDE.md` §5.
