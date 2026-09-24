# INSTALLED-DEFAULT-CONTRACT

- Task: `INSTALLED-DEFAULT-CONTRACT`
- Session: `ses_f2e91daabffeTGGBirK2pY2wVp`
- Claim: ledger claimed before edits; owned files are `crates/cli/tests/installed_default_entrypoint.rs`, this scratchpad, own ledger row.
- Candidate base: `20F4D` (`lane/INSTALLED-DEFAULT-CONTRACT`); old test blob SHA-1 `22ded29842fd765d45d3a5bbc92dd146ad24c85e`.
- Source evidence:
  - `crates/cli/src/main.rs:225-271`: no-subcommand plans startup, then native/default route.
  - `crates/cli/src/app_start.rs:287-320`: TTY plan routes native owner/attacher and setup/main view.
  - `crates/cli/src/chat.rs:99-137,657-698`: authenticated singleton daemon discovery/spawn; detached client lease.
  - `crates/cli/src/daemon_client.rs:661-743`: descriptor bearer/PID/loopback validation; bounded descriptor read.
  - `crates/cli/src/tui_entry.rs:1393-1455,1470-1522,1606-1625`: default setup/main, once frame, native `render_once` caller with legacy fallback.
  - `crates/cli/tests/app001_repair_e2e.rs:18-124`: macOS PTY, 64KiB capture, bounded waits, disposable HOME/data, loopback, descriptor bearer/PID helpers.
- Contract: five executable macOS journeys. No shell git invocation in test. PTY captures <=64KiB. Cleanup bounded, TERM then KILL, terminal restoration exact sequence. Revision receipt accepts `OC2_E2E_REVISION`, else compile-time `option_env!("GIT_COMMIT")`; absent receipt fails honestly.
- Existing blocker: native build may fail at vendored OpenTUI linkage; no product edits authorized.
- Implementation: replaced all five literal `todo!` bodies with executable macOS
  PTY journeys. New test SHA-256 `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317`.
- Focused command without receipt:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test installed_default_entrypoint --features native -- --test-threads=1`
  compiled; 4/5 passed; deterministic rerun failed at the required missing
  revision receipt assertion. Exact blocker: `revision receipt missing: set
  OC2_E2E_REVISION in packaging or provide compile-time GIT_COMMIT`.
- Focused command with explicit packaging receipt:
  `OC2_E2E_REVISION=installed-test-receipt CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test installed_default_entrypoint --features native -- --test-threads=1`
  GREEN 5/5. Receipt path is tested without invoking git.
- Old test SHA-256 `c16fea143dcd7444ced26289704925ddd878b1c52028cfa2efc23ef9a738cb6`.
- Status: blocked. Product journeys pass with an explicit receipt, but the
  required cargo-test invocation has no `OC2_E2E_REVISION` and no compile-time
  `GIT_COMMIT`; no receipt mechanism was fabricated.
- Decisions: use `OC2_E2E_BIN`, cargo fallback `CARGO_BIN_EXE_oc2`; use `script -q /dev/null` PTY; preserve macOS-only scope; no fallback claim accepted for native journey.
- Remaining: implement, run exact focused command; status `blocked` on genuine receipt/product RED, else `completed` only all five green.
