# APP-005-SECURE-RED

Claim: session ses_f2e9f871cffeHOJ3OZ8fi4L24J owns task, scratchpad worklog/APP-005-SECURE-RED.md, owned file crates/cli/tests/app005_setup_secret_red.rs.

Source evidence:
- crates/cli/src/tui_entry.rs:739 `lines.push(format!("> {draft}"))` echoes raw draft in Chat page, including setup_mode (setup header at 715-720 does not mask).
- crates/cli/src/tui_entry.rs:954-963, 986-996 paint path repaints `composer.draft()` verbatim.
- crates/cli/src/tui_entry.rs:1432-1450 setup_mode enters native_interactive_loop with live=None.
- crates/cli/src/daemon_client.rs:776-791 creds_configured false when no provider env.
- crates/cli/src/main.rs:227-259 no-subcommand bare launch plans Setup view, prepare_daemon then run_default.
- Helper pattern adapted from crates/cli/tests/app001_repair_e2e.rs (macOS /usr/bin/script PTY, 64KiB cap, disposable HOME/data).

Observed scenario: bare native oc2 with cleared provider env opens provider setup. Typing a credential-like sentinel (no submit) paints raw bytes via `> {draft}`.

Target boundary: setup renderer must never emit raw secret bytes; masked marker (e.g. `[hidden]`) shown instead. Ctrl-C from nonempty draft exits success with bounded alternate-screen restore (`ESC [?1049l`).

Tests: crates/cli/tests/app005_setup_secret_red.rs, one PTY test. Compiling RED confirmed `0 passed, 1 failed` at the secrecy assertion because the captured setup repaint contains the raw sentinel; the recorded excerpt replaces it with `[REDACTED-SENTINEL]`.

Frozen test SHA-256: `5c7e9efdb72f209dd47429d9a7a391d83ef5686e7c8d5806c1773983544f3a19`.

Exact RED command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test app005_setup_secret_red --features native -- --test-threads=1`; exit 101, test compiled, intended product assertion failed at line 244.

Decisions: no Enter submit (avoids `you: {text}` transcript side effect); Ctrl-C exit; descriptor PID cleanup best-effort; all failure excerpts redacted (sentinel never printed).

Remaining unknowns: exact masked-marker string the GREEN renderer will use (`[hidden]` assumed per task); whether GREEN also masks transcript echoes.
