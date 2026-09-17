//! TUI entrypoint lane (UI-014..UI-018 observability through the real CLI).
//!
//! Binary-driven only: no TTY required, stdio pipes are the transport. The
//! interactive loop is line-based so the same code path runs in tests and in
//! a real terminal without a rendering dependency.

use std::{
    fs,
    process::{Command, Stdio},
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

// UI-014: interactive line composer submits drafts, help toggles, quit exits.
#[test]
fn t06_interactive_pipe_submit_help_quit() {
    let (code, stdout, stderr) = run(bin(), &["tui"], Some("hello\n?\n:q\n"));
    assert_eq!(code, 0, "clean quit must exit 0 (stderr: {stderr})");
    assert!(stdout.contains("you: hello"), "echoed submit: {stdout}");
    assert!(
        stdout.contains("Keybindings"),
        "? shows keybinding help: {stdout}"
    );
}
