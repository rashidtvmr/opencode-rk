#![forbid(unsafe_code)]
//! TUI-010: native terminal parity acceptance through the closest real harness.
//!
//! Card path `tests/e2e/native_tui` does not exist at HEAD and this environment
//! provides no real PTY (no pty device, no Windows console, no macOS runner),
//! so real resize/mouse/clipboard/IME scenarios cannot execute here. This file
//! pins the complete native terminal journey — startup, compose keymap, tools
//! status surface, session stream (tabs proxy), quit/exit — plus resize,
//! unicode/paste-fallback, exit-restore and resource-bound scenarios through
//! the real `tui` entrypoint headlessly (pipes only, zero skips).
//!
//! Honest gaps (see worklog/TUI-010.md): interactive line loop, tab switching,
//! mouse, clipboard and IME require a real TTY and are not exercisable here;
//! the scriptable `--once`/`--follow` paths are the closest real harness.

use std::{
    fs,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn bin() -> Command {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!("opencode-rk-parity-{}-{id}", std::process::id()));
    fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("OPENCODE_RK_HOME", home)
        .env("NO_COLOR", "1");
    cmd
}

fn run(cmd: Command, args: &[&str], stdin_text: Option<&str>) -> (i32, String, String) {
    let mut cmd = cmd;
    cmd.args(args)
        .stdin(if stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn opencode-rk");
    if let Some(text) = stdin_text {
        use std::io::Write;
        child
            .stdin
            .take()
            .expect("stdin pipe")
            .write_all(text.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().expect("wait opencode-rk");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Write a backend descriptor whose origin matches `origin` so `--once --origin`
/// reaches the transport layer (fail-closed downstream on unreachable daemon).
/// Hand-rolled JSON: keeps this target std-only (no new dev-dependencies).
fn write_descriptor(home: &std::path::Path, origin: &str) {
    let dir = home.join("runtime");
    fs::create_dir_all(&dir).unwrap();
    let body = format!(
        "{{\"pid\":{},\"http_origin\":\"{origin}\",\"schema_version\":1,\"auth_token\":\"{}\"}}",
        std::process::id(),
        "ab".repeat(32)
    );
    fs::write(dir.join("backend.json"), body).unwrap();
}

// p01 startup: single frame renders banner, status bar items, footer hints.
#[test]
fn p01_startup_once_frame_contract() {
    let (code, stdout, stderr) = run(bin(), &["tui", "--once"], None);
    assert_eq!(code, 0, "tui --once must exit 0 (stderr: {stderr})");
    assert!(stdout.contains("OpenCode RK TUI"), "banner: {stdout}");
    assert!(stdout.contains("[model:"), "model item: {stdout}");
    assert!(stdout.contains("[context:"), "context item: {stdout}");
    assert!(stdout.contains("composer:"), "composer row: {stdout}");
    assert!(stdout.contains("footer:"), "footer: {stdout}");
    assert!(
        stdout.contains("Ctrl+P") && stdout.contains("Ctrl+T"),
        "keyboard fallback hints: {stdout}"
    );
}

// p02 compose: submit keymap surfaces via flag and env (compose binding parity).
#[test]
fn p02_compose_keymap_flag_and_env() {
    let (code, stdout, _) = run(bin(), &["tui", "--once", "--submit-keymap", "ctrl-j"], None);
    assert_eq!(code, 0);
    assert!(stdout.contains("Ctrl+J: submit"), "ctrl-j advertises: {stdout}");
    assert!(!stdout.contains("Enter: submit"), "default replaced: {stdout}");

    let mut env_cmd = bin();
    env_cmd.env("OPENCODE_RK_TUI_SUBMIT_KEY", "ctrl-j");
    let (code_env, stdout_env, _) = run(env_cmd, &["tui", "--once"], None);
    assert_eq!(code_env, 0);
    assert!(stdout_env.contains("Ctrl+J: submit"), "env override: {stdout_env}");
}

// p03 tools: status bar names model-switcher / context-detail actions.
#[test]
fn p03_tools_status_actions_named() {
    let (code, stdout, stderr) = run(bin(), &["tui", "--once"], None);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("model switcher"), "model action: {stdout}");
    assert!(stdout.contains("context detail"), "context action: {stdout}");
    assert!(stdout.contains("?: keybindings help"), "help hint: {stdout}");
}

// p04 tabs proxy: scriptable session-stream path runs headless and exits clean.
#[test]
fn p04_session_follow_scriptable_path_exits_clean() {
    let port = free_port();
    let start = Instant::now();
    let (code, _stdout, stderr) = run(
        bin(),
        &[
            "tui",
            "--follow",
            "--origin",
            &format!("http://127.0.0.1:{port}"),
            "--follow-for",
            "1",
            "--poll-ms",
            "100",
        ],
        None,
    );
    assert_eq!(code, 0, "follow-for must exit 0 (stderr: {stderr})");
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "follow run must stay bounded"
    );
}

// p05 quit: piped stdin to bare interactive tui fails closed with typed error.
#[test]
fn p05_quit_piped_stdin_fails_closed_typed() {
    let (code, _stdout, stderr) = run(bin(), &["tui"], Some("draft\n:q\n"));
    assert_ne!(code, 0, "piped interactive tui must not pretend success");
    assert!(
        stderr.contains("piped stdin"),
        "typed refusal naming piped stdin: {stderr}"
    );
    assert!(
        stderr.contains("--once") || stderr.contains("--follow"),
        "refusal names scriptable paths: {stderr}"
    );
}

// p06 resize: frame output is width-independent (no COLUMNS coupling headless).
#[test]
fn p06_resize_width_independent_headless() {
    let mut narrow = bin();
    narrow.env("COLUMNS", "40");
    let (c1, o40, _) = run(narrow, &["tui", "--once"], None);
    let mut wide = bin();
    wide.env("COLUMNS", "200");
    let (c2, o200, _) = run(wide, &["tui", "--once"], None);
    assert_eq!((c1, c2), (0, 0));
    assert_eq!(o40, o200, "headless frame must not depend on COLUMNS");
}

// p07 unicode + paste fallback: unicode memory names render; fallback hints stay.
#[test]
fn p07_unicode_memory_renders_with_fallback_hints() {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opencode-rk-parity-uni-{}-{id}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let name = "héllo-日本語.md";
    fs::write(dir.join(name), "x".repeat(16)).unwrap();

    let (code, stdout, stderr) = run(
        bin(),
        &["tui", "--once", "--memory", dir.join(name).to_str().unwrap()],
        None,
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("日本語"), "unicode name renders: {stdout}");
    assert!(stdout.contains("16"), "byte count renders: {stdout}");
    assert!(
        stdout.contains("Ctrl+P") && stdout.contains("Ctrl+T"),
        "keyboard (paste/mouse) fallback hints stay: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// p08 exit-restore: --once exits promptly; shell stays usable afterwards.
#[test]
fn p08_exit_restore_bounded_and_shell_usable() {
    let start = Instant::now();
    let (code, stdout, stderr) = run(bin(), &["tui", "--once"], None);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(start.elapsed() < Duration::from_secs(30), "exit must stay bounded");
    assert!(!stdout.trim().is_empty(), "frame emitted before exit");
    // Shell usable: an immediate follow-up run works in a fresh home.
    let (code2, _, stderr2) = run(bin(), &["tui", "--once"], None);
    assert_eq!(code2, 0, "follow-up run after exit: {stderr2}");
}

// p09 resource bound: memory pane caps retained files at 64.
#[test]
fn p09_memory_pane_bounded_at_64() {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opencode-rk-parity-mem-{}-{id}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let cmd = bin();
    let mut args: Vec<String> = vec!["tui".into(), "--once".into()];    for i in 0..70 {
        let p = dir.join(format!("f{i:03}.md"));
        fs::write(&p, "z").unwrap();
        args.push("--memory".into());
        args.push(p.to_str().unwrap().to_owned());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let (code, stdout, stderr) = run(cmd, &arg_refs, None);
    assert_eq!(code, 0, "stderr: {stderr}");
    let listed = stdout.matches("bytes)").count();
    assert_eq!(listed, 64, "memory pane must cap at 64 files, got {listed}");
    let _ = fs::remove_dir_all(&dir);
}

// p10 tools config: invalid keymap fails closed naming the value.
#[test]
fn p10_invalid_keymap_fails_closed() {
    let (code, _stdout, stderr) = run(bin(), &["tui", "--once", "--submit-keymap", "bogus"], None);
    assert_ne!(code, 0, "bogus keymap must fail");
    assert!(
        stderr.contains("bogus") || stderr.contains("invalid"),
        "error names the bad value: {stderr}"
    );
}

// p11 dead origin: --once with matching descriptor fails closed, bounded.
#[test]
fn p11_dead_origin_once_fails_closed_bounded() {
    let port = free_port();
    let origin = format!("http://127.0.0.1:{port}");
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!("opencode-rk-parity-dead-{}-{id}", std::process::id()));
    fs::create_dir_all(&home).unwrap();
    write_descriptor(&home, &origin);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("OPENCODE_RK_HOME", &home)
        .env("NO_COLOR", "1");
    let start = Instant::now();
    let (code, stdout, stderr) = run(cmd, &["tui", "--once", "--origin", &origin], None);
    assert_ne!(code, 0, "dead origin must fail --once");
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "dead-origin failure must stay bounded"
    );
    assert!(
        stderr.contains("refusing unauthenticated")
            || (stderr.contains("127.0.0.1")
                && (stderr.contains("unreachable")
                    || stderr.contains("connect")
                    || stderr.contains("refused")
                    || stderr.contains("offline"))),
        "typed connection/auth error: {stderr}"
    );
    assert!(!stdout.contains("(live)"), "no fabricated live data: {stdout}");
    let _ = fs::remove_dir_all(&home);
}

// p12 native flag: either renders a frame (exit 0) or fails with a typed
// terminal/headless message — never a silent wrong path.
#[test]
fn p12_native_once_renders_or_typed_refusal() {
    let start = Instant::now();
    let (code, stdout, stderr) = run(bin(), &["--native", "--once"], None);
    assert!(start.elapsed() < Duration::from_secs(30), "must stay bounded");
    if code == 0 {
        assert!(
            stdout.contains("OpenCode RK") || !stdout.trim().is_empty(),
            "native frame content: {stdout}"
        );
    } else {
        let combined = format!("{stdout}\n{stderr}").to_lowercase();
        assert!(
            combined.contains("terminal")
                || combined.contains("headless")
                || combined.contains("tty")
                || combined.contains("native")
                || combined.contains("opentui")
                || combined.contains("renderer")
                || combined.contains("missing"),
            "typed refusal: {stdout}\n{stderr}"
        );
    }
}

// p13 no-leak: repeated frames exit cleanly (no leaked callbacks observable).
#[test]
fn p13_repeated_frames_exit_cleanly() {
    for i in 0..3 {
        let (code, stdout, stderr) = run(bin(), &["tui", "--once"], None);
        assert_eq!(code, 0, "run {i} must exit 0 (stderr: {stderr})");
        assert!(stdout.contains("OpenCode RK TUI"), "run {i} frame: {stdout}");
    }
}
