# Worklog SHIP-007-guide (docs/USER_GUIDE.md)

Claim: fresh-user guide matching actual HEAD behavior; unverified steps BLOCKED.
Source evidence (HEAD 5af7884):
- Binary `opencode-rk`, no `opencode2` target: crates/cli/Cargo.toml:9-10, main.rs:24-28, workspace Cargo.toml:2-13.
- No-subcommand -> chat::run; tui arm -> tui_entry::run: main.rs:160-188.
- Chat auto-spawn/kill-owned daemon, 127.0.0.1:4096, OPENCODE_RK_DAEMON_ADDR: chat.rs:33-73.
- Singleton runtime files opencode-rk.pid/.sock/backend.json: server/src/daemon.rs:116-128.
- doctor env-only auth (4 keys), endpoint probe, MCP JSON: main.rs:218-382.
- TUI --follow requires --origin, offline degrades: tui_entry.rs:469-498.
- SHIP-002/SHIP-005 status not-started via completion_plan.py --card.
Observed: built -p opencode-rk-cli (0 errors), ran --help and doctor; outputs quoted verbatim.
Target boundary: owned file only docs/USER_GUIDE.md. No other edits.
Tests: `timeout 30 python3 tools/completion_plan.py --check` -> SPEC OK (not product acceptance).
Decisions: every opencode2/daemon-TUI-journey/pairing/tunnel/mobile/in-app-setup step BLOCKED with missing piece; no invented commands; redacted diagnostics only.
Remaining: guide goes stale unless re-verified after APP-001/SHIP-001/SHIP-002/SHIP-005 land.
