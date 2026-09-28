# User Guide — fresh-user setup, use, recovery

> Scope: HEAD `96087d1`. Binaries are `oc2` (primary) and `opencode-rk`
> (compat alias, same source: `crates/cli/Cargo.toml:9-18`). Release artifacts
> ship only `oc2` (`scripts/install-oc2.*`). There is **no installer, package,
> or pairing/tunnel/mobile client** in this repo: workspace `Cargo.toml:2-13`
> lists members only; `config/` holds lean defaults. Every step below needing
> pairing, tunnel, or mobile is labeled **BLOCKED** with the exact missing
> piece. Verified steps were run against `./target/debug/oc2` built from HEAD.

## 1. Build / install

No installer exists. The only verified path is a source checkout + Cargo build:

```sh
git clone <repo-url> opencode-rk
cd opencode-rk
cargo build -p opencode-rk-cli
./target/debug/oc2 --help
./target/debug/oc2 doctor
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
  run
```

Expected `doctor` baseline on a clean machine (verified, no keys set):

```text
OpenCode RK 0.1.0-alpha.1
native core: yes
embedded sqlite: yes
javascript compatibility host: disabled
os sandbox: unavailable on this platform
auth: unconfigured
connectivity: unconfigured
tools: ok
mcp: unconfigured
```

- **BLOCKED — packaged release artifacts:** `scripts/install-oc2.*` exists but no signed release archive is published; SHIP-002 clean-machine acceptance (`tests/release/local_install`) is `not-started`. Until then build from source (`cargo build -p opencode-rk-cli`). Missing piece: SHIP-001/SHIP-002 published artifacts + install evidence.
- **BLOCKED — default-launch integration proof:** no-subcommand routes to `chat::run` and `tui` to `tui_entry::run` (`crates/cli/src/main.rs:72,222-254`), and `app_start.rs` launch-orchestrator types (TTY probe, headless exit 2, RAII guards) are unit-tested library code, but the no-subcommand path has no default-launch integration test. Missing piece: APP-001 integration + SHIP-002 install evidence.

## 2. First run (verified commands only)

Data lives under `$HOME/.local/share/opencode-rk` unless overridden
(`main.rs:748-759`); override with `--data-dir` or `OPENCODE_RK_HOME`:

```sh
./target/debug/oc2 doctor            # capability + auth/connectivity/tools/mcp status
./target/debug/oc2 doctor --json     # same report as JSON (DiagnosticReport + checks)
./target/debug/oc2 session create "My project"
./target/debug/oc2 session list
./target/debug/oc2 models sync       # network: fetches https://models.dev/api.json
./target/debug/oc2 models search --limit 25
```

Provider setup is env keys only (`main.rs:408-434`): set one of
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

The chat entry (`crates/cli/src/chat.rs:4-13,135`) attaches to the singleton
daemon (auto-spawning one bound to this data dir when the probe fails) on
`127.0.0.1:4096` (override: `OPENCODE_RK_DAEMON_ADDR`). It kills only a daemon
it spawned; a pre-existing daemon is left running. Singleton lock: PID file +
Unix socket + `backend.json` descriptor under `<data-dir>/runtime/`
(`crates/server/src/daemon.rs:139-143`: `opencode-rk.pid`, `opencode-rk.sock`,
`backend.json`; socket itself lives under a short per-user runtime root,
`daemon.rs:150-153`).

```sh
./target/debug/oc2 serve --listen 127.0.0.1:4096   # foreground singleton daemon
./target/debug/oc2 web --no-open                   # serve + print origin, no browser
./target/debug/oc2 web                             # serve + open browser at origin
./target/debug/oc2 tui --once                      # one bounded snapshot frame, no daemon needed
./target/debug/oc2 tui --origin http://127.0.0.1:4096 --once   # frame bound to live daemon
```

Notes:

- Second `serve`/`web` on the same data dir prints the existing origin and
  exits (`main.rs:665-674`, `DaemonError::AlreadyRunning`). Stop the daemon
  with Ctrl-C in its terminal; there is no `stop`/`restart` subcommand.
- `tui --follow` requires `--origin`; without it `tui_entry.rs:827`
  errors `--follow requires --origin`. Offline `tui` degrades to an explicit
  `daemon offline:` line, never fabricated state.
- Chat commands (stdin loop, `chat.rs:176-181`): `/new [title]`, `/sessions`,
  `/open <id-prefix>`, `/models`, `/model <provider/model>`, `/exit`.
  Plain text sends a turn to the bound session. Offline, every mutating action
  prints `[error]`/`[offline]` and suggests `oc2 serve`.

## 4. Limits that match `doctor` output

- `javascript compatibility host: disabled` — JS plugin/compat hosts do not
  run; optional compatibility is not zero-overhead native.
- `os sandbox: unavailable on this platform` — no OS isolation backend here; do not treat
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
./target/debug/oc2 doctor --json 2>doctor-err.log | sed -E 's/sk-[A-Za-z0-9_-]+/[REDACTED]/g'
echo "exit=$?"
OPENCODE_RK_HOME=/tmp/rk-repro ./target/debug/oc2 doctor
OPENCODE_RK_HOME=/tmp/rk-repro ./target/debug/oc2 session list
```

Common cases (all messages verified in source):

| Symptom | Cause | Fix |
|---|---|---|
| `auth: unconfigured` | no provider env key | `export OPENAI_API_KEY=...` (or Anthropic/Google/Gemini key), re-run `doctor` |
| `[offline] daemon unavailable (port ... unwinnable)` | nothing on daemon addr | start `oc2 serve` in another terminal, or set `OPENCODE_RK_DAEMON_ADDR` |
| `backend is already running but its endpoint descriptor is unavailable` (`main.rs:665-668`) | stale PID/socket under `<data-dir>/runtime/` | stop the real daemon process, remove only `runtime/opencode-rk.pid` + `opencode-rk.sock` for that data dir, restart `serve` |
| `daemon offline: ...` from `tui --origin` | wrong origin / daemon down | verify `curl http://127.0.0.1:4096/health`, match `--origin` to the printed origin |
| `--follow requires --origin` | flag without origin | add `--origin http://127.0.0.1:4096` |
| `response exceeds 1048576 byte bound` | oversized daemon reply | retry with a smaller `messages?limit=` / narrower query; file a bug with the redacted `doctor --json` |

When reporting: attach redacted `doctor --json` output, the exact command,
exit code, and the last 20 lines of stderr. State binary (`oc2`,
`0.1.0-alpha.1`), OS, and whether `OPENCODE_RK_HOME` was overridden.
