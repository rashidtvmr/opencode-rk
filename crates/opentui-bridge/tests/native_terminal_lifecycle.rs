//! Runtime lifecycle coverage for the real native renderer.
//!
//! The parent is deliberately a small Rust harness around a Python stdlib PTY
//! supervisor.  Python owns the slave descriptor and process group; Rust child
//! tests use only the public bridge API.

#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::io::Write;
use std::process::Command;

const PYTHON_DRIVER: &str = r#"
import json, os, pty, signal, subprocess, tempfile, termios, time

exe, case = os.environ["TUI015_EXE"], os.environ["TUI015_CASE"]
master, slave = pty.openpty()
before = termios.tcgetattr(slave)
env = os.environ.copy()
env["TUI015_CHILD"] = "1"
env["HOME"] = tempfile.mkdtemp(prefix="tui015-home-")
env.pop("RUST_BACKTRACE", None)
proc = subprocess.Popen(
    [exe, "--exact", case, "--nocapture"], stdin=slave, stdout=slave,
    stderr=slave, env=env, cwd=env["HOME"], start_new_session=True)
os.close(slave)
data = bytearray(); deadline = time.monotonic() + 8.0
try:
    while time.monotonic() < deadline:
        try:
            chunk = os.read(master, 65536)
            if chunk: data.extend(chunk)
            if len(data) > 1024 * 1024:
                raise RuntimeError("native PTY output exceeded 1 MiB")
        except BlockingIOError: pass
        if proc.poll() is not None: break
        time.sleep(.01)
    if proc.poll() is None:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.wait(timeout=2)
        raise RuntimeError("native child exceeded PTY deadline")
    after = termios.tcgetattr(master)
    if before != after:
        raise RuntimeError("PTY termios was not restored")
    if proc.returncode != 0:
        raise RuntimeError("child exit %d: %r" % (proc.returncode, bytes(data[-4096:])))
    print(json.dumps({"case": case, "bytes": len(data), "exit": proc.returncode}))
finally:
    if proc.poll() is None:
        try: os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError: pass
        proc.wait()
    os.close(master)
"#;

#[cfg(feature = "native")]
fn run_case(case: &str) {
    let exe = std::env::current_exe().expect("current test executable");
    let output = Command::new("python3")
        .arg("-c")
        .arg(PYTHON_DRIVER)
        .env("TUI015_EXE", exe)
        .env("TUI015_CASE", case)
        .output()
        .expect("python3 stdlib PTY supervisor");
    assert!(
        output.status.success(),
        "PTY supervisor failed for {case}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"exit\": 0"));
}

#[cfg(feature = "native")]
#[test]
fn native_setup_raw_and_restore() { run_case("child_setup_raw_and_restore"); }

#[cfg(feature = "native")]
#[test]
fn native_normal_close_and_drop() { run_case("child_normal_close_and_drop"); }

#[cfg(feature = "native")]
#[test]
fn native_error_scope_drop_restores() { run_case("child_error_scope_drop_restores"); }

#[cfg(feature = "native")]
#[test]
fn native_resize_and_closed_handle_errors() { run_case("child_resize_and_closed_handle_errors"); }

#[cfg(feature = "native")]
#[test]
fn native_input_modes_and_singleton_reacquire() { run_case("child_input_modes_and_singleton_reacquire"); }

#[cfg(not(feature = "native"))]
#[test]
fn native_feature_is_required() {
    panic!("native_terminal_lifecycle requires --features native");
}

#[cfg(feature = "native")]
#[test]
fn child_setup_raw_and_restore() {
    if std::env::var_os("TUI015_CHILD").is_none() { return; }
    let mut renderer = opentui_bridge::Renderer::create(20, 8).unwrap();
    renderer.setup_terminal().unwrap();
    renderer.frame(|buffer| { let _ = buffer; }).unwrap();
    renderer.restore_terminal_modes().unwrap();
}

#[cfg(feature = "native")]
#[test]
fn child_normal_close_and_drop() {
    if std::env::var_os("TUI015_CHILD").is_none() { return; }
    let mut renderer = opentui_bridge::Renderer::create(20, 8).unwrap();
    renderer.setup_terminal().unwrap();
    renderer.close();
    assert_eq!(renderer.snapshot_text(), Err(opentui_bridge::BridgeError::InvalidHandle));
    drop(renderer);
    let _ = opentui_bridge::Renderer::create(20, 8).unwrap();
}

#[cfg(feature = "native")]
#[test]
fn child_error_scope_drop_restores() {
    if std::env::var_os("TUI015_CHILD").is_none() { return; }
    let result = std::panic::catch_unwind(|| {
        let renderer = opentui_bridge::Renderer::create(20, 8).unwrap();
        renderer.setup_terminal().unwrap();
        panic!("caller-owned failure");
    });
    assert!(result.is_err());
    let _ = opentui_bridge::Renderer::create(20, 8).unwrap();
}

#[cfg(feature = "native")]
#[test]
fn child_resize_and_closed_handle_errors() {
    if std::env::var_os("TUI015_CHILD").is_none() { return; }
    let mut renderer = opentui_bridge::Renderer::create(20, 8).unwrap();
    assert_eq!(renderer.resize(0, 8), Err(opentui_bridge::BridgeError::ZeroSize));
    assert_eq!(renderer.resize(8, 0), Err(opentui_bridge::BridgeError::ZeroSize));
    renderer.resize(24, 10).unwrap();
    assert_eq!((renderer.cols(), renderer.rows()), (24, 10));
    renderer.close();
    assert_eq!(renderer.frame(|_| {}), Err(opentui_bridge::BridgeError::InvalidHandle));
    assert_eq!(renderer.draw_text(0, 0, "closed"), Err(opentui_bridge::BridgeError::InvalidHandle));
    assert_eq!(renderer.snapshot_text(), Err(opentui_bridge::BridgeError::InvalidHandle));
}

#[cfg(feature = "native")]
#[test]
fn child_input_modes_and_singleton_reacquire() {
    if std::env::var_os("TUI015_CHILD").is_none() { return; }
    let renderer = opentui_bridge::Renderer::create(20, 8).unwrap();
    renderer.enable_mouse(true).unwrap();
    renderer.enable_kitty_keyboard(1).unwrap();
    renderer.setup_terminal().unwrap();
    renderer.suspend().unwrap();
    renderer.resume().unwrap();
    drop(renderer);
    let mut reacquired = opentui_bridge::Renderer::create(20, 8).unwrap();
    reacquired.close();
}
