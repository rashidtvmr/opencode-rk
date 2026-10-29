//! TUI entrypoint lane (UI-014..UI-018 observability through the real CLI).
//!
//! Binary-driven tests. Snapshot tests use pipes; the native interactive journey
//! is delegated to the bounded provider fixture, which owns a real PTY.

use std::{
    fs,
    process::{Command, Stdio},
    io::Read,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn bin() -> Command {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let home = std::env::temp_dir().join(format!("opencode-rk-tui-{}-{id}", std::process::id()));
    fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
    cmd.env_clear()
        .env("OPENCODE_RK_HOME", home)
        .env("NO_COLOR", "1");
    cmd
}

fn run(mut cmd: Command, args: &[&str], stdin_text: Option<&str>) -> (i32, String, String) {
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

// UI-014/UI-018: the tui subcommand exists and documents its keymap options.
#[test]
fn t01_tui_help_lists_subcommand_and_keymap() {
    let (code, stdout, _stderr) = run(bin(), &["tui", "--help"], None);
    assert_eq!(code, 0, "tui --help must exit 0");
    assert!(stdout.contains("--submit-keymap"), "keymap flag: {stdout}");
    assert!(stdout.contains("--once"), "single-frame flag: {stdout}");
    assert!(stdout.contains("--memory"), "memory pane flag: {stdout}");
}

// UI-015: single frame renders the status bar items and footer hints.
#[test]
fn t02_tui_once_renders_status_bar_and_footer() {
    let (code, stdout, stderr) = run(bin(), &["tui", "--once"], None);
    assert_eq!(code, 0, "tui --once must exit 0 (stderr: {stderr})");
    assert!(stdout.contains("OpenCode RK TUI"), "banner: {stdout}");
    assert!(stdout.contains("[model:"), "model item: {stdout}");
    assert!(stdout.contains("[context:"), "context item: {stdout}");
    assert!(stdout.contains("submit"), "footer submit hint: {stdout}");
    assert!(
        stdout.contains("Ctrl+P") && stdout.contains("Ctrl+T"),
        "keyboard fallback hints: {stdout}"
    );
}

// UI-018: composer submit keymap is configurable by flag and env.
#[test]
fn t03_submit_keymap_flag_and_env_change_footer() {
    let (code, stdout, _) = run(bin(), &["tui", "--once", "--submit-keymap", "ctrl-j"], None);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Ctrl+J: submit"),
        "ctrl-j keymap advertises Ctrl+J submit: {stdout}"
    );
    assert!(
        !stdout.contains("Enter: submit"),
        "default Enter submit hint must be replaced: {stdout}"
    );

    let mut env_cmd = bin();
    env_cmd.env("OPENCODE_RK_TUI_SUBMIT_KEY", "ctrl-j");
    let (code_env, stdout_env, _) = run(env_cmd, &["tui", "--once"], None);
    assert_eq!(code_env, 0);
    assert!(
        stdout_env.contains("Ctrl+J: submit"),
        "env keymap override: {stdout_env}"
    );
}

// UI-017: memory pane lists real files with byte counts.
#[test]
fn t04_memory_pane_lists_real_files() {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opencode-rk-tui-mem-{}-{id}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    fs::write(&a, "x".repeat(40)).unwrap();
    fs::write(&b, "y".repeat(8)).unwrap();

    let (code, stdout, stderr) = run(
        bin(),
        &[
            "tui",
            "--once",
            "--memory",
            a.to_str().unwrap(),
            "--memory",
            b.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("a.md"), "memory list shows a.md: {stdout}");
    assert!(stdout.contains("b.md"), "memory list shows b.md: {stdout}");
    assert!(stdout.contains("40"), "byte count for a.md: {stdout}");
    let _ = fs::remove_dir_all(&dir);
}

// UI-018: invalid keymap is a typed failure, not a silent default.
#[test]
fn t05_invalid_keymap_fails_closed() {
    let (code, _stdout, stderr) = run(bin(), &["tui", "--once", "--submit-keymap", "bogus"], None);
    assert_ne!(code, 0, "bogus keymap must fail");
    assert!(
        stderr.contains("bogus") || stderr.contains("invalid"),
        "error names the bad value: {stderr}"
    );
}

fn drain_bounded<R: Read>(mut reader: R) -> String {
    const CAP: usize = 256 * 1024;
    let mut kept = Vec::with_capacity(CAP);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let remaining = CAP.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..n.min(remaining)]);
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

fn bounded_drain<R: Read + Send + 'static>(reader: R) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || { let _ = sender.send(drain_bounded(reader)); });
    receiver
}

fn verified_session_group(pid: u32) -> Option<u32> {
    let output = Command::new("/bin/ps").args(["-o", "pgid=", "-p", &pid.to_string()]).output().ok()?;
    if !output.status.success() { return None; }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn run_native_provider_t06() -> (i32, String, String) {
    let artifact_dir = std::env::temp_dir().join(format!(
        "opencode-rk-t06-fixture-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&artifact_dir).expect("t06 artifact directory");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/native_provider_setup.py");
    let native_library = std::env::var_os("OC2_TEST_NATIVE_LIBRARY")
        .expect("OC2_TEST_NATIVE_LIBRARY must name the staged native library");
    let build_json = std::env::var_os("OC2_TEST_BUILD_JSON")
        .expect("OC2_TEST_BUILD_JSON must name the generated build manifest");
    // A tiny Python exec supervisor is portable across the supported POSIX
    // runners: it calls setsid, then execs the fixture so the direct child is
    // the session/group leader. The fixture's native child inherits this group.
    let supervisor = r#"import os,sys; os.setsid(); os.execv(sys.argv[1],sys.argv[1:])"#;
    let mut child = Command::new("/usr/bin/python3")
        .arg("-c").arg(supervisor).arg("/usr/bin/python3")
        .arg(script).arg("--t06")
        .arg("--binary").arg(env!("CARGO_BIN_EXE_opencode-rk"))
        .arg("--native-library").arg(native_library)
        .arg("--build-json").arg(build_json)
        .arg("--artifact-dir").arg(&artifact_dir)
        .stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("spawn bounded T06 supervisor");
    let supervisor_pgid = child.id();
    let stdout = child.stdout.take().expect("T06 stdout pipe");
    let stderr = child.stderr.take().expect("T06 stderr pipe");
    let out_reader = bounded_drain(stdout);
    let err_reader = bounded_drain(stderr);
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if child.try_wait().expect("poll T06 supervisor").is_some() { break; }
        if Instant::now() >= deadline {
            // Verify the live direct child still owns the intentionally-created
            // session group immediately before any group signal.
            let live = child.try_wait().expect("poll T06 timeout cleanup").is_none();
            let verified = live && verified_session_group(child.id()) == Some(supervisor_pgid);
            if verified {
                let _ = Command::new("/bin/kill").args(["-TERM", &format!("-{}", supervisor_pgid)]).status();
            }
            let grace = Instant::now() + Duration::from_secs(3);
            while Instant::now() < grace && child.try_wait().expect("poll T06 timeout cleanup").is_none() {
                thread::sleep(Duration::from_millis(25));
            }
            if child.try_wait().expect("poll T06 timeout cleanup").is_none()
                && verified_session_group(child.id()) == Some(supervisor_pgid) {
                let _ = Command::new("/bin/kill").args(["-KILL", &format!("-{}", supervisor_pgid)]).status();
            }
            // Direct Child ownership remains valid even if session verification
            // failed. Do not wait indefinitely for a failed group signal.
            if child.try_wait().expect("poll T06 direct cleanup").is_none() {
                let _ = child.kill();
            }
            let reap_deadline = Instant::now() + Duration::from_secs(2);
            let reaped = loop {
                if child.try_wait().expect("poll T06 bounded reap").is_some() {
                    break true;
                }
                if Instant::now() >= reap_deadline { break false; }
                thread::sleep(Duration::from_millis(25));
            };
            let _ = out_reader.recv_timeout(Duration::from_secs(2));
            let _ = err_reader.recv_timeout(Duration::from_secs(2));
            if !reaped { panic!("T06 timeout cleanup could not reap its direct child within two seconds"); }
            if !verified { panic!("T06 supervisor group identity could not be verified; cleanup failed closed"); }
            panic!("native provider fixture exceeded 45 second bound");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let status = child.wait().expect("reap bounded T06 supervisor");
    let stdout = out_reader.recv_timeout(Duration::from_secs(2)).expect("bounded T06 stdout drain");
    let stderr = err_reader.recv_timeout(Duration::from_secs(2)).expect("bounded T06 stderr drain");
    if status.success() {
        let _ = fs::remove_dir_all(&artifact_dir);
    } else {
        eprintln!("T06 failure artifacts retained: {}", artifact_dir.display());
    }
    (status.code().unwrap_or(-1), stdout, stderr)
}

// UI-014: native interactive composer submits a real provider-backed turn,
// opens help, and exits through the native :q route.
#[test]
fn t06_interactive_pty_submit_help_quit() {
    let (code, stdout, stderr) = run_native_provider_t06();
    assert_eq!(code, 0, "native provider fixture must exit 0 (stderr: {stderr})");
    assert!(stdout.contains("native-provider-result.json"), "fixture evidence: {stdout}");
}
