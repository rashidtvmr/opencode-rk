//! Installed default-entrypoint journey (CONVERGENCE steps 1-4 + 10).
//! Frozen parent-journey spec: `opencode-rk` (binary alias) with no subcommand
//! from a fresh disposable HOME must discover/start exactly one authenticated
//! daemon with Bearer auth, render the real native OpenTUI frame via
//! `render_once` bridge, and be rerunnable on the exact integrated commit.
//!
//! Source evidence (current code, 2026-09-19):
//! - `crates/cli/src/main.rs:218-250` — `None` arm calls
//!   `plan_default_launch` + `daemon_client::discover_presence` + `creds_configured`.
//! - `crates/cli/src/main.rs:252-265` — NativeTui/LaunchMode::Headless/Error routing
//!   via `app_start::plan_default_launch`.
//! - `crates/cli/src/tui_entry.rs:510-530` — Bearer routing via
//!   `resolve_origin_bearer` → `fetch_snapshot` → `http_request` with auth.
//! - `crates/cli/src/tui_entry.rs:575-586` — `print_native_or_legacy` calls
//!   `Renderer::render_once` from `opencode_rk_opentui_bridge`.
//! - `crates/server/src/lib.rs:96-97` — `opencode_rk_agents` imported.
//! - `crates/server/src/lib.rs:840-845` — `agent_plan: AgentExecutor` field.
//!
//! Std-only so the file compiles as a root integration target with no new
//! deps. RED tests fail by design until spine lands; GREEN tests pin
//! behavior already true.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Fresh disposable HOME, isolated per test (mirrors `TestHome` in
/// `crates/cli/tests/default_tui.rs:20-42`).
fn fresh_home(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "opencode2-e2e-{}-{}",
        std::process::id(),
        tag
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create disposable HOME");
    path
}

/// Installed binary under test. In dev, `opencode-rk` is the test harness;
/// release artifacts ship only `oc2` (installed as `opencode2`).
fn installed_binary() -> Command {
    // Prefer the development binary alias for test runs
    if std::env::var_os("CARGO_BIN_EXE_opencode-rk").is_some() {
        Command::new(std::env::var("CARGO_BIN_EXE_opencode-rk").unwrap())
    } else {
        Command::new("opencode-rk")
    }
}

/// CONVERGENCE-10-T01 (GREEN): disposable HOME starts empty and is fully
/// removed on teardown, so reruns never inherit state.
#[test]
fn rerun_starts_from_empty_disposable_home() {
    let home = fresh_home("empty");
    assert!(home.is_dir());
    assert!(fs::read_dir(&home).unwrap().next().is_none());
    fs::remove_dir_all(&home).unwrap();
    assert!(!home.exists());
}

/// CONVERGENCE-1-T02 (RED): bare `opencode2` (no subcommand) routes through
/// the `plan_default_launch`/daemon-discovery path instead of jumping
/// straight to the chat loop. Fails today: `main.rs:218-234` bypasses it.
#[test]
fn no_subcommand_launches_plan_path() {
    let home = fresh_home("plan");
    let _ = (home, installed_binary());
    todo!("RED: spawn `opencode2` with no args under disposable HOME, expect plan/daemon-discovery markers before chat");
}

/// CONVERGENCE-2-T03 (RED): exactly one authenticated daemon is discovered
/// (or started) and attached with a bearer, with no manual `serve`.
/// Fails today: offline path prints a manual-`serve` hint (`chat.rs:65-70`).
#[test]
fn single_authenticated_daemon_without_manual_serve() {
    let home = fresh_home("daemon");
    let _ = home;
    todo!("RED: bare launch attaches one bearer-authed daemon; second probe finds same PID; stdout never asks for manual `serve`");
}

/// CONVERGENCE-3-T04 (RED): absent provider credentials open in-app setup
/// instead of a bare turn error. Fails today: no setup screen exists.
#[test]
fn missing_credentials_open_inapp_setup() {
    let home = fresh_home("setup");
    let _ = home;
    todo!("RED: bare launch with no provider key renders in-app setup, not `[error` turn failure");
}

/// CONVERGENCE-4-T05 (RED): default launch renders a non-empty native
/// OpenTUI `render_once` snapshot rather than the line fallback.
/// Fails today: CLI has no `opencode_rk_opentui_bridge` caller.
#[test]
fn native_render_once_snapshot_nonempty() {
    let home = fresh_home("native");
    let _ = home;
    todo!("RED: `--native --once` snapshot contains frame content (non-empty after trim), exit 0 with renderer artifact present");
}

/// CONVERGENCE-10-T06 (RED): frozen suite reruns green on the exact
/// integrated commit and records the revision. Fails today: no
/// rerun-receipt mechanism exists.
#[test]
fn frozen_rerun_pinned_to_integrated_commit() {
    todo!("RED: rerun emits revision receipt (`git rev-parse HEAD` + pass/fail) on the integrated tree");
}
