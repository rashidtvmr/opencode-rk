# C1 compiler compatibility repair

Base: `37c4fe0f1c882e3049bbee9f1a9683845aff7e0f` (tree `713c23f57f5c821638890902beabc149e68ef935`).

Build evidence: `C1N7-clean-validation-20261002/logs/build.stderr`, source commit above. Native release build reached `crates/cli` and failed only on E0433 at `daemon_client.rs:774` (provider crate missing from CLI dependencies) and E0061 at `tui_entry.rs:1065` (missing third argument matching `run_with_dir` at line 1071). `main.rs:245` supplies `plan.view`; line 284 supplies `None` for explicit TUI, establishing existing default behavior.

Mechanical repair: declare existing path dependency `opencode-rk-providers`, add the corresponding CLI package dependency entry in `Cargo.lock`, pass `None` for default `StartupView` in `run`. No tests, startup logic, auth flow, fixtures or manifest pins changed.

Allowed changed paths: `crates/cli/Cargo.toml`, `Cargo.lock`, `crates/cli/src/tui_entry.rs`, this worklog.

Verification results are appended after candidate build and focused gates. Candidate verification is not integrated acceptance.
