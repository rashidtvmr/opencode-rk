# User Guide — fresh-user setup, use, recovery

> Scope: HEAD `5af7884`. Binary is `opencode-rk` (`crates/cli/Cargo.toml:9-10`,
> `crates/cli/src/main.rs:24-28`). There is **no installed `opencode2` binary,
> installer, or package** in this repo: `Cargo.toml:2-13` lists workspace
> members only, no `[package]` install target named `opencode2`; `grep -rn
> "opencode2"` hits only plan/task/audit text, never build output. Every step
> below that needs `opencode2`, pairing, tunnel, or mobile is labeled **BLOCKED**
> with the exact missing piece. Verified steps were run against
> `./target/debug/opencode-rk` built from HEAD.

## 1. Build / install

No installer exists. The only verified path is a source checkout + Cargo build:

```sh
git clone <repo-url> opencode-rk
cd opencode-rk
cargo build -p opencode-rk-cli
./target/debug/opencode-rk --help
./target/debug/opencode-rk doctor
```

Expected `--help` output (verified):

```text
Commands:
  doctor
  session
  models
  serve
  web
  tui
```

Expected `doctor` baseline on a clean machine (verified, no keys set):

```text
OpenCode RK 0.1.0-alpha.1
native core: yes
embedded sqlite: yes
javascript compatibility host: disabled
os sandbox: not yet implemented
auth: unconfigured
connectivity: unconfigured
tools: ok
mcp: unconfigured
```

- **BLOCKED — `opencode2` install/launch:** no `opencode2` binary, package,
  or install script exists. SHIP-002 clean-machine acceptance
  (`tests/release/local_install`) is `not-started`. Do not type `opencode2`;
  use `./target/debug/opencode-rk`. Missing piece: SHIP-001/SHIP-002 release
  artifacts + packaging task.
- **BLOCKED — daemon/TUI first-run journey:** `crates/cli/src/main.rs:160-188`
  routes no-subcommand to `chat::run` and `tui` to `tui_entry::run`; the
  `app_start.rs` launch-orchestrator types (TTY probe, headless exit 2, RAII
  guards) are unit-tested library code, but the no-subcommand path does not
  wire them and there is no default-launch integration proof. Missing piece:
  APP-001 integration + SHIP-002 install evidence.

## 2. First run (verified commands only)

Data lives under `$HOME/.local/share/opencode-rk` unless overridden
(`main.rs:586-597`); override with `--data-dir` or `OPENCODE_RK_HOME`:

```sh
./target/debug/opencode-rk doctor            # capability + auth/connectivity/tools/mcp status
./target/debug/opencode-rk doctor --json     # same report as JSON (DiagnosticReport + checks)
./target/debug/opencode-rk session create "My project"
./target/debug/opencode-rk session list
./target/debug/opencode-rk models sync       # network: fetches https://models.dev/api.json
./target/debug/opencode-rk models search --limit 25
```

Provider setup is env keys only (`main.rs:258-284`): set one of
`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`, `GEMINI_API_KEY`,
then re-run `doctor` and confirm `auth: configured`. No in-app provider-setup
screen exists.

- **BLOCKED — in-app provider setup / consent screen:** `doctor_auth_check`
  reads env only; no UI flow stores keys. Missing piece: APP vertical slice
  for in-app setup.
- **BLOCKED — account pairing / phone use:** no pairing command, QR/code flow,
  account, or mobile client in the CLI surface or `crates/server`. SHIP-005
  (`tests/release/remote_devices`) is `not-started`. Missing pieces: MOB/NET
  slices, named Cloudflare Tunnel gateway, signed iOS/Android builds, pairing
  + revocation UI.
- **BLOCKED — operator self-host / tunnel deploy:** no tunnel, ingress, or
  hosted-deploy config in repo (`config/` holds lean defaults only). Missing
  piece: NET-015 + SHIP-005 staging evidence. One-time deployment docs cannot
  be written until that exists; there is no per-user setup distinct from it.

## 3. Service control (verified)

The chat entry (`crates/cli/src/chat.rs:33-73`) auto-spawns `serve` on
`127.0.0.1:4096` (override: `OPENCODE_RK_DAEMON_ADDR`) when `GET /health`
fails, and kills only a daemon it spawned; a pre-existing daemon is left
running. Singleton lock: PID file + Unix socket under
`<data-dir>/runtime/` (`crates/server/src/daemon.rs:116-128`:
`opencode-rk.pid`, `opencode-rk.sock`, `backend.json` descriptor).

```sh
./target/debug/opencode-rk serve --listen 127.0.0.1:4096   # foreground singleton daemon
./target/debug/opencode-rk web --no-open                   # serve + print origin, no browser
./target/debug/opencode-rk web                             # serve + open browser at origin
./target/debug/opencode-rk tui --once                      # one bounded snapshot frame, no daemon needed
./target/debug/opencode-rk tui --origin http://127.0.0.1:4096 --once   # frame bound to live daemon
```

Notes:

- Second `serve`/`web` on the same data dir prints the existing origin and
  exits (`main.rs:513-526`, `DaemonError::AlreadyRunning`). Stop the daemon
  with Ctrl-C in its terminal; there is no `stop`/`restart` subcommand.
- `tui --follow` requires `--origin`; without it `tui_entry.rs:469-470`
  errors `--follow requires --origin`. Offline `tui` degrades to an explicit
  `daemon offline:` line, never fabricated state.
- Chat commands (stdin loop, `chat.rs:350-382`): `/new [title]`, `/sessions`,
  `/open <id-prefix>`, `/models`, `/model <provider/model>`, `/help`, `/exit`.
  Plain text sends a turn to the bound session. Offline, every mutating action
  prints `[error]`/`[offline]` and suggests `opencode-rk serve`.

## 4. Limits that match `doctor` output

- `javascript compatibility host: disabled` — JS plugin/compat hosts do not
  run; optional compatibility is not zero-overhead native.
- `os sandbox: not yet implemented` — no OS isolation backend; do not treat
  config text as a sandbox.
- `connectivity` stays `unconfigured` until `OPENCODE_RK_DOCTOR_ENDPOINT` is
  set to an https URL to probe.
- `mcp` stays `unconfigured` until `OPENCODE_RK_MCP_CONFIG` holds JSON like
  `{"servers":{"name":{"command":"..."}}}`.

## 5. Recovery with redacted diagnostics

Rules: never paste full API keys, full `OPENCODE_RK_MCP_CONFIG` secrets, or
your live data dir into a report. Never delete or hand-edit the data dir to
"fix" history; use disposable dirs for repros.

```sh
./target/debug/opencode-rk doctor --json 2>doctor-err.log | sed -E 's/sk-[A-Za-z0-9_-]+/[REDACTED]/g'
echo "exit=$?"
OPENCODE_RK_HOME=/tmp/rk-repro ./target/debug/opencode-rk doctor
OPENCODE_RK_HOME=/tmp/rk-repro ./target/debug/opencode-rk session list
```

Common cases (all messages verified in source):

| Symptom | Cause | Fix |
|---|---|---|
| `auth: unconfigured` | no provider env key | `export OPENAI_API_KEY=...` (or Anthropic/Google/Gemini key), re-run `doctor` |
| `[offline] daemon unavailable (port ... unwinnable)` | nothing on daemon addr | start `opencode-rk serve` in another terminal, or set `OPENCODE_RK_DAEMON_ADDR` |
| `backend is already running but its endpoint descriptor is unavailable` (`main.rs:516-518`) | stale PID/socket under `<data-dir>/runtime/` | stop the real daemon process, remove only `runtime/opencode-rk.pid` + `opencode-rk.sock` for that data dir, restart `serve` |
| `daemon offline: ...` from `tui --origin` | wrong origin / daemon down | verify `curl http://127.0.0.1:4096/health`, match `--origin` to the printed origin |
| `--follow requires --origin` | flag without origin | add `--origin http://127.0.0.1:4096` |
| `response exceeds 1048576 byte bound` | oversized daemon reply | retry with a smaller `messages?limit=` / narrower query; file a bug with the redacted `doctor --json` |

When reporting: attach redacted `doctor --json` output, the exact command,
exit code, and the last 20 lines of stderr. State binary (`opencode-rk`,
`0.1.0-alpha.1`), OS, and whether `OPENCODE_RK_HOME` was overridden.
