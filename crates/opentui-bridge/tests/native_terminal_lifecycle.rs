//! Native renderer lifecycle tests.  The parent retains the same PTY slave
//! descriptor throughout; the child is allowed to clean up only after the
//! parent has observed its live state.

use std::io::{Read, Write};
use std::process::Command;
use opencode_rk_opentui_bridge as opentui_bridge;

const READY: &[u8] = b"TUI015_READY_7f31\n";
const RESTORED: &[u8] = b"TUI015_RESTORED_7f31\n";
const ACK: u8 = b'K';
const PYTHON_DRIVER: &str = r#"
import fcntl, json, os, pathlib, pty, signal, subprocess, tempfile, termios, time
exe, case, native_dir = os.environ["TUI015_EXE"], os.environ["TUI015_CASE"], os.environ["TUI015_NATIVE_LIB_DIR"]
native_path = pathlib.Path(native_dir) / "libopentui.dylib"
if not native_path.is_file(): raise RuntimeError("native fixture missing: %s" % native_path)
master, slave = pty.openpty()
flags = fcntl.fcntl(master, fcntl.F_GETFL)
fcntl.fcntl(master, fcntl.F_SETFL, flags | os.O_NONBLOCK)
before = termios.tcgetattr(slave)
env = {key: os.environ[key] for key in ("PATH", "TERM") if key in os.environ}
env["TUI015_CHILD"] = "1"; env["HOME"] = tempfile.mkdtemp(prefix="tui015-home-")
env["DYLD_LIBRARY_PATH"] = native_dir; env["DYLD_FALLBACK_LIBRARY_PATH"] = native_dir
def controlling_tty():
    # Popen(start_new_session=True) performs setsid exactly once; this hook
    # performs the required POSIX TIOCSCTTY in that new session.
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
proc = subprocess.Popen([exe, "--exact", case, "--nocapture"], stdin=slave,
    stdout=slave, stderr=slave, env=env, cwd=env["HOME"],
    start_new_session=True, preexec_fn=controlling_tty)
data = bytearray(); deadline = time.monotonic() + 8.0
def read_until(needle):
    while needle not in data:
        if time.monotonic() >= deadline: raise RuntimeError("PTY handshake deadline")
        try:
            chunk = os.read(master, 65536)
            if chunk: data.extend(chunk)
        except BlockingIOError:
            time.sleep(.001)
        if len(data) > 1024 * 1024: raise RuntimeError("PTY output exceeded 1 MiB")
        if proc.poll() is not None:
            raise RuntimeError("child exited %d before handshake: %r" % (proc.returncode, bytes(data[-4096:])))
def raw_flags(attrs):
    lflag = attrs[3]
    return (lflag & (termios.ICANON | termios.ECHO | termios.ISIG)) == 0
try:
    if case in ("child_setup_raw_and_restore", "child_normal_close_and_drop",
                "child_error_scope_drop_restores", "child_input_modes_and_singleton_reacquire"):
        read_until(b"TUI015_READY_7f31\n")
        live = termios.tcgetattr(slave)
        if not raw_flags(live): raise RuntimeError("live PTY was not raw")
        os.write(master, bytes([ord('K')]))
        read_until(b"TUI015_RESTORED_7f31\n")
        restored = termios.tcgetattr(slave)
        if restored != before: raise RuntimeError("live PTY not restored before exit")
    else:
        while proc.poll() is None:
            if time.monotonic() >= deadline: raise RuntimeError("PTY deadline")
            try:
                chunk = os.read(master, 65536)
                if chunk: data.extend(chunk)
            except BlockingIOError: pass
            if len(data) > 1024 * 1024: raise RuntimeError("PTY output exceeded 1 MiB")
            time.sleep(.01)
    if case == "child_input_modes_and_singleton_reacquire":
        # These are the pinned native terminal protocol bytes: SGR mouse
        # (1000/1002 + 1006) and Kitty keyboard (>[>1u, disable [<u).
        required = [b"\x1b[?1002h", b"\x1b[?1006h", b"\x1b[?1002l",
                    b"\x1b[?1006l", b"\x1b[>1u", b"\x1b[<u"]
        for marker in required:
            if marker not in data: raise RuntimeError("missing native protocol %r" % marker)
        if not (data.index(b"\x1b[?1002h") < data.index(b"\x1b[?1002l")):
            raise RuntimeError("mouse enable/disable ordering")
        if not (data.index(b"\x1b[>1u") < data.index(b"\x1b[<u")):
            raise RuntimeError("kitty enable/disable ordering")
    proc.wait(timeout=max(0, deadline-time.monotonic()))
    if proc.returncode != 0: raise RuntimeError("child exit %d: %r" % (proc.returncode, bytes(data[-4096:])))
    print(json.dumps({"case": case, "bytes": len(data), "exit": proc.returncode}))
finally:
    if proc.poll() is None:
        try: os.killpg(proc.pid, signal.SIGTERM); proc.wait(timeout=1)
        except (ProcessLookupError, subprocess.TimeoutExpired):
            try: os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError: pass
            proc.wait()
    os.close(slave); os.close(master)
"#;

#[cfg(feature = "native")]
fn run_case(case: &str) {
    let native_dir = std::env::var("TUI015_NATIVE_LIB_DIR")
        .expect("TUI015_NATIVE_LIB_DIR must name the staged native fixture directory");
    assert!(std::path::Path::new(&native_dir).join("libopentui.dylib").is_file(),
        "native fixture missing at {native_dir}/libopentui.dylib");
    let output = Command::new("python3").arg("-c").arg(PYTHON_DRIVER)
        .env("TUI015_EXE", std::env::current_exe().expect("current test executable"))
        .env("TUI015_CASE", case).env("TUI015_NATIVE_LIB_DIR", native_dir)
        .output().expect("python3 PTY supervisor");
    assert!(output.status.success(), "PTY supervisor failed for {case}: {}{}",
        String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"exit\": 0"));
}

#[cfg(feature = "native")]
#[test] fn native_setup_raw_and_restore() { run_case("child_setup_raw_and_restore"); }
#[cfg(feature = "native")]
#[test] fn native_normal_close_and_drop() { run_case("child_normal_close_and_drop"); }
#[cfg(feature = "native")]
#[test] fn native_error_scope_drop_restores() { run_case("child_error_scope_drop_restores"); }
#[cfg(feature = "native")]
#[test] fn native_resize_and_closed_handle_errors() { run_case("child_resize_and_closed_handle_errors"); }
#[cfg(feature = "native")]
#[test] fn native_input_modes_and_singleton_reacquire() { run_case("child_input_modes_and_singleton_reacquire"); }
#[cfg(not(feature = "native"))]
#[test] fn native_feature_is_required() { panic!("native_terminal_lifecycle requires --features native"); }

#[cfg(feature = "native")]
fn child_only() -> bool { std::env::var_os("TUI015_CHILD").is_some() }
#[cfg(feature = "native")]
fn handshake() {
    let mut out = std::io::stdout(); out.write_all(READY).unwrap(); out.flush().unwrap();
    let mut ack = [0u8; 1]; std::io::stdin().read_exact(&mut ack).unwrap(); assert_eq!(ack[0], ACK);
}
#[cfg(feature = "native")]
fn restored() { let mut out = std::io::stdout(); out.write_all(RESTORED).unwrap(); out.flush().unwrap(); }

#[cfg(feature = "native")]
#[test] fn child_setup_raw_and_restore() {
    if !child_only() { return; }
    let mut r = opentui_bridge::Renderer::create(20, 8).unwrap(); r.setup_terminal().unwrap();
    r.frame(|_| {}).unwrap(); handshake(); r.restore_terminal_modes().unwrap(); restored();
}
#[cfg(feature = "native")]
#[test] fn child_normal_close_and_drop() {
    if !child_only() { return; }
    let mut r = opentui_bridge::Renderer::create(20, 8).unwrap(); r.setup_terminal().unwrap(); handshake();
    r.close(); assert_eq!(r.snapshot_text(), Err(opentui_bridge::BridgeError::InvalidHandle)); restored();
    drop(r); let _ = opentui_bridge::Renderer::create(20, 8).unwrap();
}
#[cfg(feature = "native")]
#[test] fn child_error_scope_drop_restores() {
    if !child_only() { return; }
    let result = std::panic::catch_unwind(|| { let r = opentui_bridge::Renderer::create(20, 8).unwrap();
        r.setup_terminal().unwrap(); handshake(); panic!("caller-owned failure"); });
    assert!(result.is_err()); restored(); let _ = opentui_bridge::Renderer::create(20, 8).unwrap();
}
#[cfg(feature = "native")]
#[test] fn child_resize_and_closed_handle_errors() {
    if !child_only() { return; }
    let mut r = opentui_bridge::Renderer::create(20, 8).unwrap();
    assert_eq!(r.resize(0, 8), Err(opentui_bridge::BridgeError::ZeroSize));
    assert_eq!(r.resize(8, 0), Err(opentui_bridge::BridgeError::ZeroSize)); r.resize(24, 10).unwrap();
    assert_eq!((r.cols(), r.rows()), (24, 10)); r.close();
    assert_eq!(r.frame(|_| {}), Err(opentui_bridge::BridgeError::InvalidHandle));
    assert_eq!(r.draw_text(0, 0, "closed"), Err(opentui_bridge::BridgeError::InvalidHandle));
    assert_eq!(r.snapshot_text(), Err(opentui_bridge::BridgeError::InvalidHandle));
}
#[cfg(feature = "native")]
#[test] fn child_input_modes_and_singleton_reacquire() {
    if !child_only() { return; }
    let mut pre = opentui_bridge::Renderer::create(20, 8).unwrap(); pre.enable_mouse(true).unwrap();
    pre.enable_kitty_keyboard(1).unwrap(); pre.close(); drop(pre);
    let r = opentui_bridge::Renderer::create(20, 8).unwrap(); r.enable_mouse(true).unwrap();
    r.enable_kitty_keyboard(1).unwrap(); r.setup_terminal().unwrap(); handshake(); r.suspend().unwrap();
    r.resume().unwrap(); drop(r); restored(); let _ = opentui_bridge::Renderer::create(20, 8).unwrap();
}
