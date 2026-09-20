# LANE-DEFAULTTUI-FIX scratchpad

Claim: LANE-DEFAULTTUI-FIX, session ses_f423ccebfffeIe1a310sv3QDnH, status in-progress.
Prior LANE-CLI-ONCE fenced by ses_f425545f9ffeLHmVe806FTqcEX — new row used, that lane untouched.

Source evidence:
- crates/cli/tests/default_tui.rs:403-433 — fake token "test-bearer-token-12345", top-level `--once --native` flags.
- crates/cli/src/daemon_client.rs:632-636 — is_wellformed_token: 64-char hex only.
- crates/cli/src/ci_run.rs:46-48 — same 64-hex mirror.
- crates/cli/src/main.rs:61-86 — no top-level --once; Tui(TuiArgs) subcommand owns --once (tui_entry.rs:62-63).
- crates/cli/src/tui_entry.rs:534-537 — no --origin + --once renders local frame directly.

Target boundary: own default_tui.rs only. No src edits, no other lanes.

Fix: descriptor auth_token -> "ab".repeat(32) (64 hex); invoke ["tui","--once"]; assert success + stdout contains "OpenCode RK TUI"; create runtime dir first.

Remaining: run check cmd, set completed, commit+push lane files.
