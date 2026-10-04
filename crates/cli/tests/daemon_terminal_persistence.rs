#![cfg(unix)]

use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const DRIVER: &str = r#"
import json, os, pty, select, signal, socket, subprocess, sys, time
binary, root, address, foreground_address = sys.argv[1:]
data_dir = root
home = os.path.join(root, "home")
env = {"HOME": home, "OPENCODE_RK_HOME": data_dir,
       "OPENCODE_RK_DAEMON_ADDR": address,
       "PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "TERM": "xterm"}
leader = None
cli = None
master = None
second = None
second_master = None
foreground = None
daemon = None

def alarm(_signum, _frame):
    raise RuntimeError("bounded lifecycle test exceeded 25 seconds")
signal.signal(signal.SIGALRM, alarm)
signal.alarm(25)

def request(addr, path, token=None):
    host, port = addr.rsplit(":", 1)
    with socket.create_connection((host, int(port)), timeout=0.5) as conn:
        conn.settimeout(1)
        headers = "Host: " + addr + "\r\nConnection: close\r\n"
        if token is not None:
            headers += "Authorization: Bearer " + token + "\r\n"
        conn.sendall(("GET " + path + " HTTP/1.1\r\n" + headers + "\r\n").encode())
        chunks = []
        total = 0
        while total < 65536:
            block = conn.recv(min(4096, 65536 - total))
            if not block:
                break
            chunks.append(block)
            total += len(block)
        return b"".join(chunks).split(b"\r\n", 1)[0]

def healthy(addr, token):
    try:
        return request(addr, "/health").startswith(b"HTTP/1.1 200") and \
               request(addr, "/api/sessions", token).startswith(b"HTTP/1.1 200")
    except OSError:
        return False

def read_descriptor():
    with open(os.path.join(data_dir, "runtime", "backend.json"), encoding="utf-8") as f:
        value = json.load(f)
    assert value["http_origin"] == "http://" + address, value
    assert isinstance(value["pid"], int) and value["pid"] > 0, value
    token = value["auth_token"]
    assert len(token) == 64 and all(c in "0123456789abcdefABCDEF" for c in token), "invalid daemon bearer"
    return value

def launch_pty(args):
    pid, fd = pty.fork()
    if pid == 0:
        try:
            os.execve(binary, [binary] + args, env)
        except BaseException as exc:
            os.write(2, ("exec failed: " + repr(exc) + chr(10)).encode())
            os._exit(127)
    return pid, fd

def read_until(pid, fd, marker, timeout):
    output = bytearray()
    deadline = time.monotonic() + timeout
    while marker not in output and time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if ready:
            try:
                block = os.read(fd, 4096)
            except OSError:
                block = b""
            if block:
                output.extend(block[:max(0, 65536 - len(output))])
        waited, status = os.waitpid(pid, os.WNOHANG)
        if waited:
            raise AssertionError("PTY client exited before marker; status=%r output=%r" % (status, output[-2048:]))
    assert marker in output, "PTY client did not print %r; output=%r" % (marker, output[-2048:])
    return output

def wait_exit(pid, fd, timeout):
    output = bytearray()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if ready:
            try:
                block = os.read(fd, 4096)
            except OSError:
                block = b""
            if block:
                output.extend(block[:max(0, 65536 - len(output))])
        waited, status = os.waitpid(pid, os.WNOHANG)
        if waited:
            return status, output
    return None, output

def wait_health(addr, token, timeout=4):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if healthy(addr, token):
            return
        time.sleep(0.025)
    raise AssertionError("authenticated daemon health/session API unavailable at " + addr)

def cleanup_pty(pid, fd):
    if pid is None:
        return
    try:
        os.killpg(pid, signal.SIGTERM)
    except (ProcessLookupError, PermissionError):
        pass
    end = time.monotonic() + 0.5
    while time.monotonic() < end:
        waited, _ = os.waitpid(pid, os.WNOHANG)
        if waited:
            pid = None
            break
        time.sleep(0.01)
    if pid is not None:
        try:
            os.killpg(pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
    if fd is not None:
        os.close(fd)

try:
    os.mkdir(home, 0o700)
    cli, master = launch_pty(["--data-dir", data_dir])
    leader = cli
    read_until(cli, master, b"OpenCode RK", 12)
    descriptor = read_descriptor()
    daemon = (descriptor["pid"], descriptor["auth_token"], cli)
    assert descriptor["pid"] != cli, "daemon PID aliases client"
    assert os.getpgid(descriptor["pid"]) == cli, "autospawned daemon changed its owner's process group"
    wait_health(address, descriptor["auth_token"])

    os.write(master, bytes([3]))
    status, _ = wait_exit(cli, master, 4)
    assert status is not None, "Ctrl-C did not exit the controlling-PTY client"
    cli = None
    os.close(master)
    master = None
    assert os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGINT, "client did not retain ordinary Ctrl-C exit: %r" % status
    assert healthy(address, descriptor["auth_token"]), "autospawned daemon died with its Ctrl-C client; authenticated health and sessions must survive"

    for sig in (signal.SIGHUP, signal.SIGINT):
        os.kill(descriptor["pid"], sig)
        wait_health(address, descriptor["auth_token"])
        assert os.getpgid(descriptor["pid"]) == leader, "daemon left its original process group"

    second, second_master = launch_pty(["--data-dir", data_dir])
    text = read_until(second, second_master, ("daemon: http://" + address).encode(), 5)
    assert b"OpenCode RK" in text
    os.write(second_master, b"/exit" + bytes([10]))
    second_status, _ = wait_exit(second, second_master, 4)
    assert second_status is not None and os.WIFEXITED(second_status) and os.WEXITSTATUS(second_status) == 0, "second attached client failed to exit normally"
    second = None
    os.close(second_master)
    second_master = None
    after = read_descriptor()
    assert after["pid"] == descriptor["pid"] and after["auth_token"] == descriptor["auth_token"], "second client replaced daemon identity"
    assert healthy(address, descriptor["auth_token"]), "daemon failed authenticated second-client attachment"

    foreground_dir = os.path.join(root, "foreground")
    os.mkdir(foreground_dir, 0o700)
    foreground_env = dict(env, OPENCODE_RK_HOME=foreground_dir)
    foreground = subprocess.Popen(
        [binary, "--data-dir", foreground_dir, "serve", "--listen", foreground_address],
        env=foreground_env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    end = time.monotonic() + 5
    while time.monotonic() < end:
        try:
            if request(foreground_address, "/health").startswith(b"HTTP/1.1 200"):
                break
        except OSError:
            pass
        if foreground.poll() is not None:
            raise AssertionError("foreground serve exited before readiness")
        time.sleep(0.025)
    else:
        raise AssertionError("foreground serve did not become healthy")
    foreground.send_signal(signal.SIGINT)
    assert foreground.wait(timeout=3) == -signal.SIGINT, "foreground serve signal behavior changed"
    foreground = None
finally:
    signal.alarm(0)
    if foreground is not None and foreground.poll() is None:
        foreground.terminate()
        try:
            foreground.wait(timeout=1)
        except subprocess.TimeoutExpired:
            foreground.kill()
            foreground.wait(timeout=1)
    cleanup_pty(second, second_master)
    cleanup_pty(cli, master)
    if daemon is not None:
        pid, token, pgid = daemon
        try:
            current = read_descriptor()
            if current["pid"] == pid and current["auth_token"] == token and healthy(address, token) and os.getpgid(pid) == pgid:
                os.killpg(pgid, signal.SIGTERM)
        except (OSError, AssertionError, ProcessLookupError, PermissionError):
            pass
"#;

fn free_loopback() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
    listener
        .local_addr()
        .expect("read reserved address")
        .to_string()
}

#[test]
fn autospawned_daemon_outlives_terminal_signal_and_attaches_second_client() {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = PathBuf::from(format!("/tmp/oc2-term-{}-{id}", std::process::id()));
    fs::create_dir(&root).expect("create isolated data directory");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .expect("restrict fixture directory");
    let output = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(DRIVER)
        .arg(env!("CARGO_BIN_EXE_opencode-rk"))
        .arg(&root)
        .arg(free_loopback())
        .arg(free_loopback())
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .output()
        .expect("run bounded controlling-PTY fixture");
    let _ = fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "daemon terminal-lifetime fixture failed (status {}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
