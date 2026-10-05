#![forbid(unsafe_code)]
//! Native default-entrypoint renderer controls.
//!
//! The four former pipe-driven chat tests belonged to the legacy line-chat
//! contract.  They are intentionally not retained here: current default
//! launch refuses redirected interactive input/output before daemon discovery
//! or raw-mode setup.  The current default-entrypoint startup/ownership
//! contract is frozen in `native_daemon_flow.rs` and is run as a separate
//! four-test target.
//!
//! Negative cleanup tests (Cases A, B, C) are RED checkpoints: they expose
//! missing owned-group cleanup in the relay when fault paths are triggered.
//! Do NOT fix the relay cleanup logic before these are independently validated
//! as genuinely RED on the current relay code.

#[cfg(feature = "native")]
mod native_controls {
    use std::{
        fs::{self, File},
        io::{Read, Write},
        net::TcpStream,
        path::{Path, PathBuf},
        process::{Child, Command, ExitStatus, Stdio},
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    use opencode_rk_opentui_bridge::Renderer;

    const PIPE_CAP: usize = 256 * 1024;
    const EXIT_LIMIT: Duration = Duration::from_secs(20);
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestHome(PathBuf);

    impl TestHome {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            // Leave room for runtime/opencode-rk.sock below the platform's
            // Unix-domain socket path bound, including a long macOS TMPDIR.
            let root = std::env::temp_dir().join(format!("dn-{}-{id}", std::process::id()));
            for part in [
                "home",
                "xdg-config",
                "xdg-data",
                "xdg-state",
                "xdg-cache",
                "runtime",
            ] {
                fs::create_dir_all(root.join(part)).expect("create disposable fixture directory");
            }
            Self(root)
        }

        fn root(&self) -> &Path {
            &self.0
        }
        fn descriptor(&self) -> PathBuf {
            self.0.join("runtime/backend.json")
        }
        fn models(&self) -> PathBuf {
            self.0.join("models.json")
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Capture {
        bytes: Arc<Mutex<Vec<u8>>>,
        overflow: Arc<std::sync::atomic::AtomicBool>,
        failed: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Capture {
        fn spawn(pipe: impl Read + Send + 'static) -> Self {
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let overflow = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dst = Arc::clone(&bytes);
            let full = Arc::clone(&overflow);
            let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_failed = Arc::clone(&failed);
            let thread = thread::spawn(move || {
                let mut pipe = pipe;
                let mut buf = [0_u8; 4096];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) => break,
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            worker_failed.store(true, Ordering::Release);
                            break;
                        }
                        Ok(n) => {
                            let mut out = dst.lock().expect("capture lock");
                            let room = PIPE_CAP.saturating_sub(out.len());
                            if room == 0 {
                                full.store(true, Ordering::Relaxed);
                            } else {
                                out.extend_from_slice(&buf[..n.min(room)]);
                                if n > room {
                                    full.store(true, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
            });
            Self {
                bytes,
                overflow,
                failed,
                thread: Some(thread),
            }
        }

        fn text(&self) -> String {
            String::from_utf8_lossy(&self.bytes.lock().expect("capture lock")).into_owned()
        }

        fn join(&mut self) -> std::io::Result<()> {
            if let Some(reader) = self.thread.take() {
                let deadline = Instant::now() + Duration::from_secs(2);
                while !reader.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                if !reader.is_finished() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "output reader join deadline",
                    ));
                }
                if reader.join().is_err() || self.failed.load(Ordering::Acquire) {
                    return Err(std::io::Error::other("output reader failed"));
                }
            }
            if self.overflow.load(Ordering::Acquire) {
                return Err(std::io::Error::other(
                    "child output exceeded fixture byte bound",
                ));
            }
            Ok(())
        }
    }

    struct OwnedChild {
        child: Child,
        stdout: Option<Capture>,
        stderr: Option<Capture>,
        native_finished: Option<(PathBuf, PathBuf)>,
    }

    impl OwnedChild {
        fn spawn(command: &mut Command) -> Self {
            let child = command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn owned fixture child");
            let mut owned = Self {
                child,
                stdout: None,
                stderr: None,
                native_finished: None,
            };
            owned.stdout = Some(Capture::spawn(
                owned.child.stdout.take().expect("child stdout"),
            ));
            owned.stderr = Some(Capture::spawn(
                owned.child.stderr.take().expect("child stderr"),
            ));
            owned
        }

        fn wait_native_natural(&mut self) -> std::io::Result<()> {
            let (finished, _) = self.native_finished.as_ref().expect("native checkpoint");
            let deadline = Instant::now() + EXIT_LIMIT;
            while !finished.is_file() {
                if Instant::now() >= deadline {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "native checkpoint deadline",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            if fs::read_to_string(finished)?.trim() != "natural" {
                return Err(std::io::Error::other("invalid native checkpoint"));
            }
            Ok(())
        }

        fn acknowledge_native(&mut self) -> std::io::Result<ExitStatus> {
            let (_, ack) = self
                .native_finished
                .as_ref()
                .expect("native acknowledgement");
            fs::write(ack, b"A")?;
            self.wait()
        }

        fn wait(&mut self) -> std::io::Result<ExitStatus> {
            let deadline = Instant::now() + EXIT_LIMIT;
            loop {
                if let Some(status) = self.child.try_wait()? {
                    self.drain()?;
                    return Ok(status);
                }
                if Instant::now() >= deadline {
                    self.stop()?;
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "child exit deadline",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
        }

        fn drain(&mut self) -> std::io::Result<()> {
            let stdout = self.stdout.as_mut().map(Capture::join).unwrap_or(Ok(()));
            let stderr = self.stderr.as_mut().map(Capture::join).unwrap_or(Ok(()));
            stdout.and(stderr)
        }

        fn stop(&mut self) -> std::io::Result<()> {
            if let Some((_, ack)) = &self.native_finished {
                let _ = fs::write(ack, b"A");
            }
            let grace = Instant::now() + Duration::from_secs(5);
            while self.child.try_wait()?.is_none() && Instant::now() < grace {
                thread::sleep(Duration::from_millis(10));
            }
            if self.child.try_wait()?.is_none() {
                self.child.kill()?;
                let deadline = Instant::now() + Duration::from_secs(2);
                while self.child.try_wait()?.is_none() {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "child reaping deadline",
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            self.drain()
        }
    }

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if let Err(error) = self.stop() {
                if thread::panicking() {
                    eprintln!("default TUI fixture failure cleanup: {error}");
                } else {
                    panic!("default TUI fixture cleanup failed: {error}");
                }
            }
        }
    }

    fn command_env(command: &mut Command, home: &TestHome) {
        command
            .env_clear()
            .current_dir(home.root())
            .env("OPENCODE_RK_HOME", home.root())
            .env("HOME", home.root().join("home"))
            .env("XDG_CONFIG_HOME", home.root().join("xdg-config"))
            .env("XDG_DATA_HOME", home.root().join("xdg-data"))
            .env("XDG_STATE_HOME", home.root().join("xdg-state"))
            .env("XDG_CACHE_HOME", home.root().join("xdg-cache"));
        #[cfg(target_os = "macos")]
        {
            // Cargo supplies its native search paths to the test process.
            // env_clear deliberately removes them from the fixture children,
            // so restore only the pinned repository library directory.
            let library_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../opentui-bridge/native/lib")
                .join(format!("{}-apple-darwin", std::env::consts::ARCH));
            assert!(library_dir.join("libopentui.dylib").is_file());
            command.env("DYLD_LIBRARY_PATH", library_dir);
        }
    }

    fn wait_descriptor(home: &TestHome, child: &mut OwnedChild) {
        let deadline = Instant::now() + EXIT_LIMIT;
        while !home.descriptor().exists() && Instant::now() < deadline {
            if let Some(status) = child.child.try_wait().expect("poll fixture daemon") {
                child.drain().expect("drain failed daemon output");
                panic!(
                    "fixture daemon exited before readiness: {status}; stderr: {}",
                    child.stderr.as_ref().unwrap().text()
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(
            home.descriptor().exists(),
            "serve did not publish its descriptor"
        );
    }

    fn descriptor(home: &TestHome) -> (u32, String, String) {
        let mut bytes = Vec::new();
        File::open(home.descriptor())
            .expect("open published descriptor")
            .take(8193)
            .read_to_end(&mut bytes)
            .expect("bounded descriptor read");
        assert!(bytes.len() <= 8192, "descriptor byte bound");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("published descriptor JSON");
        assert_eq!(value["schema_version"], 1);
        let pid = u32::try_from(value["pid"].as_u64().expect("descriptor pid"))
            .expect("descriptor PID bound");
        let origin = value["http_origin"]
            .as_str()
            .expect("descriptor origin")
            .to_owned();
        let token = value["auth_token"]
            .as_str()
            .expect("descriptor token")
            .to_owned();
        assert_eq!(token.len(), 64, "published bearer length");
        assert!(
            token.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "published bearer shape"
        );
        assert!(
            origin
                .strip_prefix("http://127.0.0.1:")
                .is_some_and(|port| port.parse::<u16>().is_ok_and(|port| port != 0)),
            "published origin must be numeric loopback with nonzero port"
        );
        (pid, origin, token)
    }

    fn address(origin: &str) -> &str {
        origin
            .strip_prefix("http://")
            .expect("fixture only uses http origin")
    }

    fn request(origin: &str, token: &str, method: &str, path: &str, body: Option<&str>) -> String {
        let body = body.unwrap_or("");
        format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            address(origin),
            body.len()
        )
    }

    fn authenticated_request(
        origin: &str,
        token: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> String {
        let mut stream = TcpStream::connect_timeout(
            &address(origin).parse().expect("socket address"),
            Duration::from_secs(5),
        )
        .expect("connect owned serve");
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("set read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("set write timeout");
        stream
            .write_all(request(origin, token, method, path, body).as_bytes())
            .expect("write owned request");
        let mut response = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut buffer = [0_u8; 4096];
        loop {
            assert!(Instant::now() < deadline, "owned HTTP response deadline");
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    assert!(
                        response.len() <= PIPE_CAP.saturating_sub(n),
                        "HTTP response exceeded fixture cap"
                    );
                    response.extend_from_slice(&buffer[..n]);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) => panic!("owned HTTP response read failed: {error}"),
            }
        }
        String::from_utf8(response).expect("owned response UTF-8")
    }

    fn spawn_serve(home: &TestHome) -> (OwnedChild, u32, String, String) {
        fs::write(&home.models(), r#"{"openai":{"name":"OpenAI","models":{"gpt-5.6":{"name":"GPT-5.6","tool_call":true,"reasoning":true,"limit":{"context":200000}}}}}"#)
            .expect("write offline model catalogue");
        let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
        command_env(&mut command, home);
        command
            .args(["serve", "--listen", "127.0.0.1:0", "--models-file"])
            .arg(home.models())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut serve = OwnedChild::spawn(&mut command);
        wait_descriptor(home, &mut serve);
        let (pid, origin, token) = descriptor(home);
        assert_eq!(
            pid,
            serve.child.id(),
            "descriptor must name the actual owned daemon child"
        );
        let health = authenticated_request(&origin, &token, "GET", "/health", None);
        assert!(
            health.starts_with("HTTP/1.1 200"),
            "owned daemon health failed: {health}"
        );
        let models = authenticated_request(
            &origin,
            &token,
            "GET",
            "/api/models?provider=openai&limit=500",
            None,
        );
        assert!(
            models.starts_with("HTTP/1.1 200"),
            "owned daemon model catalogue failed: {models}"
        );
        assert!(
            serve.child.try_wait().expect("poll owned serve").is_none(),
            "serve exited before attachment"
        );
        (serve, pid, origin, token)
    }

    // ── Python relay DRIVER ───────────────────────────────────────────────────
    //
    // Embedded driver for the native-PTY relay.  OC2_TEST_FAULT controls
    // test-only fault injection (never touches product code):
    //   ""               → normal cooperative exit
    //   "pre_ready_fail" → relay raises before fixture_group is set (Case A)
    //   "sigterm_ignore" → relay forks a SIGTERM-ignoring member, then exits
    //                       with nonzero so KILL path is exercised (Case C)
    //
    // The DRIVER must compile to a single -c argument with no shell escaping
    // issues.  Triple-quote strings are used for the embedded Python.
    const DRIVER: &str = r#"
import errno, fcntl, os, pty, select, signal, sys, termios, time, traceback

SUPERVISOR = 12.0
CLEANUP = 2.0
CAP = 256 * 1024
master = slave = child = None
ready_r = ready_w = release_r = release_w = None
member_ready_r = member_ready_w = None
anchor_ready_r = anchor_ready_w = None
anchor = None
anchor_reaped = None
member_pid = None
member_reaped = None
forced = False
child_forced = False
fixture_group = None
captured = bytearray()
pending_out = bytearray()
pending_in = bytearray()
reaped = None
foreground_before = None
foreground_assigned = False
foreground_restored = False

FAULT = os.environ.get("OC2_TEST_FAULT", "")

def deadline_after(seconds):
    return time.monotonic() + seconds

def remaining(deadline):
    value = deadline - time.monotonic()
    if value <= 0:
        raise TimeoutError("native relay deadline")
    return value

def poll_child():
    global reaped
    if reaped is None:
        got, status = os.waitpid(child, os.WNOHANG)
        if got == child:
            reaped = status
    return reaped

def terminate_owned():
    global forced, child_forced, reaped, anchor_reaped, member_reaped
    if child is None or child <= 0: raise RuntimeError("missing native child")
    group = fixture_group
    # The child is positively owned immediately after fork, while the group
    # oracle is only READY after the child has established its process group.
    # Faults can therefore arrive in the interval between those two events.
    # Never let a missing group turn an owned child into an orphan: reclaim the
    # direct child first, and use killpg only once the group identity has been
    # validated by the normal READY path.
    valid_group = group is not None and group > 0 and group != os.getpgrp()

    def send_child(sig):
        if reaped is not None:
            return
        try:
            os.kill(child, sig)
        except ProcessLookupError:
            pass

    def reap_child():
        end = deadline_after(CLEANUP)
        while reaped is None and time.monotonic() < end:
            poll_child()
            if reaped is None:
                time.sleep(0.01)
        if reaped is not None:
            return
        send_child(signal.SIGKILL)
        end = deadline_after(CLEANUP)
        while reaped is None and time.monotonic() < end:
            poll_child()
            if reaped is None:
                time.sleep(0.01)
        if reaped is None: raise TimeoutError("native direct-child reap deadline")

    if not valid_group:
        send_child(signal.SIGTERM)
        reap_child()
        return

    def gone():
        try: os.killpg(group, 0); return False
        except ProcessLookupError: return True
        except OSError as error:
            if error.errno == errno.ESRCH: return True
            if error.errno == errno.EPERM: return False
            raise
    def send(sig):
        try: os.killpg(group, sig)
        except (ProcessLookupError, PermissionError): pass
    send(signal.SIGTERM)
    end=deadline_after(CLEANUP)
    group_gone_after_term = False
    while time.monotonic()<end:
        poll_child()
        if gone():
            group_gone_after_term = True
            break
        time.sleep(0.01)
    if not group_gone_after_term:
        send(signal.SIGKILL)
        end=deadline_after(CLEANUP)
        while time.monotonic()<end:
            poll_child()
            time.sleep(0.01)
    if reaped is None:
        end=deadline_after(CLEANUP)
        while reaped is None and time.monotonic()<end: poll_child(); time.sleep(0.01)
    if reaped is None: raise TimeoutError("native relay reap deadline")

    # The anchor and any directly-forked fault member deliberately remain
    # unreaped until both group signals have been sent.  Their zombie state
    # pins the owned PGID and prevents a numeric PGID reuse race.  Only now do
    # we reap our own direct children, then use killpg(..., 0) as observation.
    def reap_owned(pid, label):
        if pid is None: return
        end=deadline_after(CLEANUP)
        while time.monotonic()<end:
            got, status = os.waitpid(pid, os.WNOHANG)
            if got == pid: return status
            time.sleep(0.01)
        raise TimeoutError("native %s reap deadline" % label)

    anchor_reaped = reap_owned(anchor, "group anchor")
    member_reaped = reap_owned(member_pid, "fault member")
    end=deadline_after(CLEANUP)
    while time.monotonic()<end:
        if gone(): return
        time.sleep(0.01)
    raise TimeoutError("native group cleanup deadline")

def write_deadline(fd, data, deadline):
    while data:
        _, writable, _ = select.select([], [fd], [], remaining(deadline))
        if not writable:
            raise TimeoutError("relay write deadline")
        try:
            n = os.write(fd, data)
        except OSError as error:
            if error.errno in (errno.EAGAIN, errno.EWOULDBLOCK, errno.EINTR):
                continue
            raise
        if n <= 0:
            raise OSError("relay short write")
        del data[:n]

def restore_harness_foreground():
    global foreground_restored
    if not foreground_assigned or foreground_restored:
        return
    if reaped is None:
        raise RuntimeError("cannot restore fixture foreground while child is live")
    # This is fixture shell/job-control cleanup, not product terminal
    # restoration. The product never owned the parent shell foreground group.
    os.tcsetpgrp(slave, foreground_before)
    if os.tcgetpgrp(slave) != foreground_before:
        raise RuntimeError("fixture foreground process-group restoration failed")
    foreground_restored = True

def child_main():
    try:
        # The relay remains the session leader/controlling-terminal owner.
        # The application is only a foreground process group, so its exit
        # cannot revoke the controller's terminal vnode on macOS.
        os.setpgid(0, 0)
        os.dup2(slave, 0); os.dup2(slave, 1); os.dup2(slave, 2)
        if master is not None: os.close(master)
        if slave > 2: os.close(slave)
        if ready_r is not None: os.close(ready_r)
        if release_w is not None: os.close(release_w)
        write_deadline(ready_w, bytearray(b"R"), deadline_after(2.0))
        os.close(ready_w)
        if FAULT == "pre_ready_fail":
            # Keep the positively-owned fork child alive after readiness. The
            # parent deliberately faults before recording fixture_group; this
            # makes the missing pre-READY cleanup observable rather than an
            # EOF-driven cooperative child exit.
            time.sleep(60)
            os._exit(0)
        release_deadline = deadline_after(2.0)
        while True:
            readable, _, _ = select.select([release_r], [], [], remaining(release_deadline))
            if release_r not in readable:
                raise TimeoutError("native child release barrier")
            try:
                release = os.read(release_r, 1)
                if release != b"L": raise RuntimeError("native child release barrier EOF or corrupt")
                break
            except OSError as error:
                if error.errno in (errno.EAGAIN, errno.EWOULDBLOCK, errno.EINTR):
                    continue
                raise
        os.close(release_r)
        signal.signal(signal.SIGTTOU, signal.SIG_DFL)
        signal.signal(signal.SIGTTIN, signal.SIG_DFL)
        signal.signal(signal.SIGHUP, signal.SIG_DFL)
        child_env = os.environ.copy()
        # macOS strips inherited DYLD_* before entering this test wrapper.
        # Rebuild it at the last possible point before the real exec.
        child_env["DYLD_LIBRARY_PATH"] = child_env["OC2_TEST_NATIVE_LIBRARY_DIR"]
        os.execve(child_env["OC2_TEST_BINARY"],
                  [child_env["OC2_TEST_BINARY"]] + sys.argv[1:], child_env)
    except BaseException as error:
        print("native child PTY/exec failure: " + repr(error), file=sys.stderr)
        os._exit(125)

super_deadline = deadline_after(SUPERVISOR)
try:
    master, slave = pty.openpty()
    ready_r, ready_w = os.pipe()
    release_r, release_w = os.pipe()
    for fd in (ready_r, ready_w, release_r, release_w):
        os.set_blocking(fd, False)
    # Rust's normal subprocess launch is expected not to make this relay a
    # process-group leader. Fail closed rather than hiding that topology fact
    # behind a second controller fork.
    if os.getpgrp() == os.getpid():
        raise RuntimeError("relay unexpectedly started as process-group leader")
    os.setsid()
    fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    # This is the complete pre-spawn oracle: inspect the actual slave before
    # the child exists and never restore/mask it with tcsetattr.
    attrs_before = termios.tcgetattr(slave)
    foreground_before = os.tcgetpgrp(slave)
    os.set_blocking(master, False)
    os.set_blocking(1, False)

    child = os.fork()
    if child == 0:
        try:
            child_main()
        finally:
            # execve success never returns. Any wrapper failure must terminate
            # this forked child and can never fall through into parent cleanup.
            os._exit(125)
    os.close(ready_w); ready_w = None
    os.close(release_r); release_r = None
    # Fault after the native child is positively forked and alive, but before
    # fixture_group is recorded.  This is the real pre-READY orphan case: the
    # except-path cleanup sees no group and therefore cannot reclaim child.
    if FAULT == "pre_ready_fail":
        if "OC2_TEST_RELAY_PID_FILE" in os.environ:
            with open(os.environ["OC2_TEST_RELAY_PID_FILE"], "w") as f:
                f.write(str(child))
        if "OC2_TEST_GROUP_FILE" in os.environ:
            with open(os.environ["OC2_TEST_GROUP_FILE"], "w") as f:
                f.write(str(child))
        raise RuntimeError("injected pre-ready fault: fixture_group never set")
    if os.environ.get("OC2_TEST_AUTO_START") == "1":
        with open(os.environ["OC2_TEST_GROUP_FILE"], "w", encoding="ascii") as group_file:
            group_file.write(str(child))
            group_file.flush()
    os.set_blocking(ready_r, False)
    ready_deadline = deadline_after(2.0)
    while True:
        readable, _, _ = select.select([ready_r], [], [], remaining(ready_deadline))
        if ready_r in readable:
            try: ready = os.read(ready_r, 1)
            except OSError as error:
                if error.errno in (errno.EAGAIN, errno.EWOULDBLOCK, errno.EINTR): continue
                raise
            if ready != b"R": raise RuntimeError("native child PTY readiness barrier EOF or corrupt")
            break
    if child <= 0 or os.getpgid(child) != child or os.getpgid(child) == os.getpgrp():
        raise RuntimeError("native child process-group ownership validation failed")
    fixture_group = os.getpgid(child)
    # Keep an independently-owned direct child in the validated group until
    # the final group signal has been sent.  A live/zombie group member pins
    # the numeric PGID while cleanup is in flight, so a later PID/PGID reuse
    # cannot redirect a cached killpg call at an unrelated process group.
    anchor_ready_r, anchor_ready_w = os.pipe()
    os.set_blocking(anchor_ready_r, False)
    anchor = os.fork()
    if anchor == 0:
        try:
            os.close(anchor_ready_r)
            os.setpgid(0, fixture_group)
            for fd in (master, slave, ready_r, release_w, release_r, member_ready_r, member_ready_w):
                if fd is not None:
                    try: os.close(fd)
                    except OSError: pass
            for fd in (0, 1, 2):
                try: os.close(fd)
                except OSError: pass
            os.write(anchor_ready_w, b"R")
            os.close(anchor_ready_w)
            while True:
                signal.pause()
        except BaseException:
            os._exit(125)
    os.close(anchor_ready_w); anchor_ready_w = None
    anchor_deadline = deadline_after(2.0)
    while True:
        readable, _, _ = select.select([anchor_ready_r], [], [], remaining(anchor_deadline))
        if anchor_ready_r in readable:
            if os.read(anchor_ready_r, 1) != b"R":
                raise RuntimeError("native group anchor readiness barrier corrupt")
            break
    os.close(anchor_ready_r); anchor_ready_r = None
    if os.getpgid(anchor) != fixture_group or os.getsid(anchor) != os.getsid(child):
        raise RuntimeError("native group anchor ownership validation failed")

    # FAULT: sigterm_ignore — fork a SIGTERM-ignoring member into the validated
    # group, record its PID for cleanup proof, then force a nonzero relay exit
    # so the SIGKILL path is exercised.  This happens AFTER fixture_group is
    # set so terminate_owned has a valid group.
    if FAULT == "sigterm_ignore":
        member_ready_r, member_ready_w = os.pipe()
        os.set_blocking(member_ready_r, False)
        # Fork a child that ignores SIGTERM and sleeps.
        member_pid = os.fork()
        if member_pid == 0:
            # Move into the fixture group so killpg hits it.
            os.setpgid(0, fixture_group)
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            os.write(member_ready_w, b"R")
            os.close(member_ready_r); os.close(member_ready_w)
            # Sleep long enough for the test harness to act.
            time.sleep(60)
            os._exit(0)
        os.close(member_ready_w)
        member_ready_deadline = deadline_after(2.0)
        while True:
            readable, _, _ = select.select([member_ready_r], [], [], remaining(member_ready_deadline))
            if member_ready_r in readable:
                if os.read(member_ready_r, 1) != b"R":
                    raise RuntimeError("SIGTERM-ignore member readiness barrier corrupt")
                break
        os.close(member_ready_r)
        if os.getpgid(member_pid) != fixture_group:
            raise RuntimeError("SIGTERM-ignore member group ownership validation failed")
        # Record member PID so test can verify reap.
        if "OC2_TEST_MEMBER_PID_FILE" in os.environ:
            with open(os.environ["OC2_TEST_MEMBER_PID_FILE"], "w") as f:
                f.write(str(member_pid))
        if "OC2_TEST_GROUP_FILE" in os.environ:
            with open(os.environ["OC2_TEST_GROUP_FILE"], "w", encoding="ascii") as gf:
                gf.write(str(fixture_group))
        # Release the real child so it can exec and eventually exit.
        signal.signal(signal.SIGTTOU, signal.SIG_IGN)
        os.tcsetpgrp(slave, child)
        foreground_assigned = True
        write_deadline(release_w, bytearray(b"L"), super_deadline)
        os.close(release_w); release_w = None
        # Raise so terminate_owned is called from the except block.
        raise RuntimeError("injected sigterm_ignore fault: forced relay failure")

    # The native child is the foreground process group of the controller's
    # controlling terminal. Ignore SIGTTOU only for this supervisor ioctl;
    # child_main restores the default disposition before exec.
    signal.signal(signal.SIGTTOU, signal.SIG_IGN)
    os.tcsetpgrp(slave, child)
    if os.tcgetpgrp(slave) != child:
        raise RuntimeError("native child foreground process-group validation failed")
    foreground_assigned = True
    write_deadline(release_w, bytearray(b"L"), super_deadline)
    os.close(release_w); release_w = None
    # Keep the parent slave descriptor open through the post-exit oracle.
    seen = bytearray()
    sent = False
    triggered = False
    while True:
        status = poll_child()
        readable, writable, _ = select.select([master], [1] if pending_out else [], [], min(0.05, remaining(super_deadline)))
        if master in readable:
            try:
                data = os.read(master, 65536)
            except OSError as error:
                if error.errno in (errno.EAGAIN, errno.EWOULDBLOCK, errno.EINTR):
                    data = b""
                elif error.errno == errno.EIO:
                    data = b""  # PTY EOF is not required for completion.
                else:
                    raise
            if data:
                if len(captured) + len(data) > CAP:
                    raise RuntimeError("native PTY output exceeded fixture cap")
                captured.extend(data); seen.extend(data); pending_out.extend(data)
                if not sent and b"session:" in seen:
                    pending_in.extend(b"native default turn\r"); sent = True
                if sent and not triggered and (
                    b"native default reply" in seen
                    or (
                        os.environ.get("OC2_TEST_AUTO_START") == "1"
                        and b"OpenCode RK" in seen
                    )
                ):
                    pending_in.extend(b"\x03"); triggered = True
        if pending_out and 1 in writable:
            try:
                n = os.write(1, pending_out); del pending_out[:n]
            except OSError as error:
                if error.errno not in (errno.EAGAIN, errno.EWOULDBLOCK, errno.EINTR):
                    raise
        if pending_in:
            write_deadline(master, pending_in, super_deadline); pending_in.clear()
        if reaped is not None:
            if not pending_out:
                break
        if time.monotonic() >= super_deadline:
            raise TimeoutError("native relay supervisor deadline")
    # Child exit is observed naturally; forced cleanup is never converted to
    # success. Keep slave open while taking the complete post-exit snapshot.
    attrs_after = termios.tcgetattr(slave)
    if attrs_after != attrs_before:
        raise AssertionError("native child changed full PTY slave attributes")
    if child_forced: raise RuntimeError("native child required forced termination")
    checkpoint_tmp = os.environ["OC2_TEST_NATIVE_FINISHED"] + ".tmp"
    with open(checkpoint_tmp, "w", encoding="ascii") as marker:
        marker.write("natural")
        marker.flush()
    os.replace(checkpoint_tmp, os.environ["OC2_TEST_NATIVE_FINISHED"])
    deadline = deadline_after(CLEANUP + 20.0)
    while not os.path.exists(os.environ["OC2_TEST_NATIVE_ACK"]):
        if time.monotonic() >= deadline: raise TimeoutError("native cleanup acknowledgement deadline")
        time.sleep(0.01)
    terminate_owned()
    if not os.waitstatus_to_exitcode(reaped) == 0:
        raise RuntimeError("native child exited unsuccessfully")
    if os.environ.get("OC2_TEST_AUTO_START") == "1":
        try:
            os.getpgid(child)
        except ProcessLookupError:
            with open(os.environ["OC2_TEST_GROUP_FILE"] + ".gone", "w", encoding="ascii") as group_file:
                group_file.write("gone")
        else:
            raise RuntimeError("native foreground process group remained after natural exit")
    restore_harness_foreground()
except BaseException as error:
    try:
        terminate_owned()
    except BaseException as cleanup_error:
        print("native relay cleanup failed: " + repr(cleanup_error), file=sys.stderr)
    traceback.print_exc(file=sys.stderr)
    print("native relay failure: " + repr(error), file=sys.stderr)
    sys.exit(1)
finally:
    if reaped is not None and foreground_assigned and not foreground_restored:
        try:
            restore_harness_foreground()
        except BaseException as restore_error:
            print("native relay foreground cleanup failed: " + repr(restore_error), file=sys.stderr)
    # The slave is deliberately closed only after post-exit tcgetattr.
    for fd in (master, slave, ready_r, ready_w, release_r, release_w, member_ready_r, member_ready_w, anchor_ready_r, anchor_ready_w):
        if fd is not None:
            try: os.close(fd)
            except OSError: pass
if reaped is None or forced:
    sys.exit(1)
sys.exit(0 if os.waitstatus_to_exitcode(reaped) == 0 else 1)
"#;

    fn command_env_with_stub(command: &mut Command, home: &TestHome, stub_url: &str) {
        command_env(command, home);
        command.env("OPENAI_BASE_URL", stub_url);
        // Daemon request path reads OPENAI_API_KEY only as a fallback;
        // the persisted auth file is the primary credential under test.
        // Keep ambient empty so the stub assertion proves file usage.
        command.env_remove("OPENAI_API_KEY");
    }

    fn spawn_serve_with_stub(home: &TestHome, stub_url: &str) -> (OwnedChild, u32, String, String) {
        fs::write(&home.models(), r#"{"openai":{"name":"OpenAI","models":{"gpt-5.6":{"name":"GPT-5.6","tool_call":true,"reasoning":true,"limit":{"context":200000}}}}}"#)
            .expect("write offline model catalogue");
        let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
        command_env_with_stub(&mut command, home, stub_url);
        command
            .args(["serve", "--listen", "127.0.0.1:0", "--models-file"])
            .arg(home.models())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut serve = OwnedChild::spawn(&mut command);
        wait_descriptor(home, &mut serve);
        let (pid, origin, token) = descriptor(home);
        assert_eq!(
            pid,
            serve.child.id(),
            "descriptor must name the actual owned daemon child"
        );
        let health = authenticated_request(&origin, &token, "GET", "/health", None);
        assert!(
            health.starts_with("HTTP/1.1 200"),
            "owned daemon health failed: {health}"
        );
        (serve, pid, origin, token)
    }

    fn start_local_provider_stub() -> std::net::TcpListener {
        // Minimal OpenAI-Responses-shaped stub on loopback. Serves exactly
        // the requests the test drives via blocking accept with a bounded
        // deadline, so the spine can prove the daemon forwarded the
        // persisted credential and selected model over a client-controlled
        // path, with no outbound network. No thread is spawned.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback stub");
        listener
            .set_nonblocking(true)
            .expect("stub nonblocking accept");
        listener
    }

    fn find_headers_end(raw: &[u8]) -> Option<usize> {
        raw.windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|pos| pos + 4)
    }

    fn stub_base_url(listener: &std::net::TcpListener) -> String {
        format!("http://{}/v1", listener.local_addr().expect("stub address"))
    }

    fn status_code(response: &str) -> &str {
        response
            .split_whitespace()
            .nth(1)
            .expect("HTTP status line")
    }

    fn response_body(response: &str) -> &str {
        let start = response.find("\r\n\r\n").expect("HTTP header boundary") + 4;
        &response[start..]
    }

    fn serve_stub_once(
        listener: &std::net::TcpListener,
        seen: &Arc<Mutex<Vec<(String, String)>>>,
        expected_key: &str,
        expected_model: &str,
        expected_prompt: &str,
        expected_stream: Option<bool>,
        assistant_text: &str,
    ) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline, "stub saw no provider request");
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .expect("stub read timeout");
                    stream
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .expect("stub write timeout");
                    let mut raw = Vec::new();
                    let mut buffer = [0_u8; 4096];
                    let headers_end = loop {
                        assert!(Instant::now() < deadline, "stub headers deadline");
                        match stream.read(&mut buffer) {
                            Ok(0) => panic!("stub connection closed before headers"),
                            Ok(n) => {
                                raw.extend_from_slice(&buffer[..n]);
                                if let Some(end) = find_headers_end(&raw) {
                                    break end;
                                }
                                assert!(raw.len() <= 64 * 1024, "stub headers bound");
                            }
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(10));
                                continue;
                            }
                            Err(error) => panic!("stub read failed: {error}"),
                        }
                    };
                    let header_text = String::from_utf8_lossy(&raw[..headers_end]).into_owned();
                    let request_line = header_text.lines().next().expect("stub request line");
                    assert_eq!(
                        request_line, "POST /v1/responses HTTP/1.1",
                        "provider must receive the Responses endpoint"
                    );
                    let content_length = header_text
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            (name.trim().eq_ignore_ascii_case("content-length"))
                                .then(|| value.trim().parse::<usize>().ok())?
                        })
                        .expect("stub request has content-length");
                    assert!(content_length <= 128 * 1024, "stub body bound");
                    while raw.len() < headers_end + content_length {
                        assert!(Instant::now() < deadline, "stub body deadline");
                        match stream.read(&mut buffer) {
                            Ok(0) => panic!("stub connection closed before body"),
                            Ok(n) => raw.extend_from_slice(&buffer[..n]),
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(10));
                                continue;
                            }
                            Err(error) => panic!("stub read failed: {error}"),
                        }
                    }
                    let body = &raw[headers_end..headers_end + content_length];
                    let value: serde_json::Value =
                        serde_json::from_slice(body).expect("stub request JSON");
                    let auth_header = header_text
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            (name.trim().eq_ignore_ascii_case("authorization"))
                                .then(|| value.trim().to_owned())
                        })
                        .expect("stub request has authorization");
                    let model = value["model"].as_str().expect("stub model").to_owned();
                    seen.lock()
                        .expect("stub seen lock")
                        .push((auth_header.clone(), model.clone()));
                    assert_eq!(
                        auth_header,
                        format!("Bearer {expected_key}"),
                        "daemon must forward the persisted provider key"
                    );
                    assert_eq!(
                        model, expected_model,
                        "daemon must request the configured model"
                    );
                    let actual_stream = value.get("stream").and_then(serde_json::Value::as_bool);
                    assert!(
                        expected_stream.is_none_or(|expected| actual_stream == Some(expected)),
                        "Responses stream mode must match the actual turn: expected={expected_stream:?} actual={actual_stream:?}"
                    );
                    let input = value["input"].as_array().expect("Responses input array");
                    assert!(
                        input.iter().any(|item| {
                            item["role"] == "user"
                                && (item["content"].as_str() == Some(expected_prompt)
                                    || item["content"].as_array().is_some_and(|content| {
                                        content.iter().any(|part| {
                                            part["type"] == "input_text"
                                                && part["text"] == expected_prompt
                                        })
                                    }))
                        }),
                        "Responses input must contain the native/user prompt: {value}"
                    );
                    let (content_type, body) = if assistant_text == "structure stub reply" {
                        let payload = serde_json::json!({
                            "id": "stub-fixture", "object": "response", "status": "completed",
                            "output": [{"type":"message", "role":"assistant",
                                "content":[{"type":"output_text", "text":assistant_text}]}],
                        });
                        (
                            "application/json",
                            serde_json::to_vec(&payload).expect("stub response JSON"),
                        )
                    } else {
                        let payload = serde_json::json!({"type":"response.completed", "response":{"status":"completed"}});
                        (
                            "text/event-stream",
                            format!(
                                "event: response.output_text.delta\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
                                serde_json::json!({"type":"response.output_text.delta","delta":assistant_text}),
                                payload
                            )
                            .into_bytes(),
                        )
                    };
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(head.as_bytes()).expect("stub write head");
                    stream.write_all(&body).expect("stub write body");
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("stub accept failed: {error}"),
            }
        }
    }

    fn providers_auth_path(home: &TestHome) -> PathBuf {
        // persisted_auth::auth_path resolves XDG_DATA_HOME first, then
        // HOME/.local/share. command_env sets XDG_DATA_HOME to the
        // disposable xdg-data dir, so auth lands under opencode/auth.json.
        home.root().join("xdg-data/opencode/auth.json")
    }

    fn spawn_native_entrypoint_with_addr(
        home: &TestHome,
        stub_url: &str,
        daemon_addr: Option<&str>,
        auto_start: bool,
        fault: Option<&str>,
        relay_pid_file: Option<&Path>,
        member_pid_file: Option<&Path>,
        group_file: Option<&Path>,
    ) -> OwnedChild {
        // Run the actual no-subcommand binary behind a real PTY.  The small
        // relay is intentionally in the fixture (rather than a shell): it
        // gives the child a controlling terminal while retaining bounded,
        // continuously drained output in OwnedChild.
        let mut command = Command::new("/usr/bin/python3");
        command.args([
            "-c",
            DRIVER,
            "--data-dir",
            home.root().to_str().expect("fixture data directory UTF-8"),
        ]);
        command_env_with_stub(&mut command, home, stub_url);
        if let Some(daemon_addr) = daemon_addr {
            command.env("OPENCODE_RK_DAEMON_ADDR", daemon_addr);
        } else {
            command.env_remove("OPENCODE_RK_DAEMON_ADDR");
        }
        if auto_start {
            command.env("OC2_TEST_AUTO_START", "1");
        }
        if let Some(f) = fault {
            command.env("OC2_TEST_FAULT", f);
        } else {
            command.env_remove("OC2_TEST_FAULT");
        }
        if let Some(p) = relay_pid_file {
            command.env("OC2_TEST_RELAY_PID_FILE", p);
        }
        if let Some(p) = member_pid_file {
            command.env("OC2_TEST_MEMBER_PID_FILE", p);
        }
        let group_path = group_file
            .map(|p| p.to_owned())
            .unwrap_or_else(|| home.root().join("runtime/native-group"));
        command.env("OC2_TEST_GROUP_FILE", &group_path);
        command.env("OC2_TEST_BINARY", env!("CARGO_BIN_EXE_opencode-rk"));
        command.env("TERM", "xterm-256color");
        command.env("XDG_RUNTIME_DIR", home.root().join("runtime"));
        command.env("TMPDIR", home.root());
        let library_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../opentui-bridge/native/lib")
            .join("aarch64-apple-darwin");
        command.env("OC2_TEST_NATIVE_LIBRARY_DIR", library_dir);
        command.env(
            "OC2_TEST_NATIVE_FINISHED",
            home.root().join("runtime/native-finished"),
        );
        command.env(
            "OC2_TEST_NATIVE_ACK",
            home.root().join("runtime/native-ack"),
        );
        let mut relay = OwnedChild::spawn(&mut command);
        relay.native_finished = Some((
            home.root().join("runtime/native-finished"),
            home.root().join("runtime/native-ack"),
        ));
        relay
    }

    fn spawn_native_entrypoint(home: &TestHome, stub_url: &str, daemon_addr: &str) -> OwnedChild {
        spawn_native_entrypoint_with_addr(
            home,
            stub_url,
            Some(daemon_addr),
            false,
            None,
            None,
            None,
            None,
        )
    }

    // ── Unix-specific OS query helpers for negative tests ─────────────────────

    /// Poll whether PID `pid` exists in the OS process table.
    /// Returns `true` if kill(pid, 0) == 0 (process exists).
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    fn pid_exists(pid: u32) -> bool {
        // Safety: kill(pid, 0) never sends a signal; it only checks existence.
        // This is standard POSIX and safe from a correctness standpoint.
        // We do not use libc directly; we shell to `kill -0` to stay safe-only.
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Bounded wait until `pid` disappears from the OS process table.
    #[cfg(unix)]
    fn wait_pid_gone(pid: u32, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !pid_exists(pid) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        !pid_exists(pid)
    }

    /// Check whether a process group `pgid` has any living members using
    /// `kill -0` on the process group (negative PID).
    #[cfg(unix)]
    fn pgrp_exists(pgid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &format!("-{pgid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Bounded wait until no process in group `pgid` is alive.
    #[cfg(unix)]
    fn wait_pgrp_gone(pgid: u32, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if !pgrp_exists(pgid) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        !pgrp_exists(pgid)
    }

    // ── Positive tests (canonical, preserved unchanged) ────────────────────────

    #[test]
    fn native_render_once_snapshot_contains_frame_content() {
        let frame_lines = vec![
            "OpenCode RK TUI".to_owned(),
            "status: ready".to_owned(),
            "> ".to_owned(),
        ];
        let output =
            Renderer::render_once(80, 24, &frame_lines).expect("native memory renderer snapshot");
        assert!(
            !output.trim().is_empty(),
            "render_once snapshot must be non-empty"
        );
        assert!(
            output.contains("OpenCode RK"),
            "render_once output must contain frame text: {output}"
        );
        assert!(
            output.contains("status: ready"),
            "render_once output must preserve status text: {output}"
        );
    }

    #[test]
    fn once_mode_with_bearer_auth_shows_native_or_fallback() {
        let home = TestHome::new();
        let (mut serve, pid, origin, token) = spawn_serve(&home);
        let title = "Native Once Live Probe";
        let body = serde_json::json!({"title": title}).to_string();
        let created = authenticated_request(&origin, &token, "POST", "/api/sessions", Some(&body));
        assert!(
            created.starts_with("HTTP/1.1 201"),
            "session creation failed: {created}"
        );

        let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
        command_env(&mut command, &home);
        command.args(["tui", "--once", "--origin", &origin]);
        let mut once = OwnedChild::spawn(&mut command);
        let status = once.wait().expect("bounded native tui once exit");
        assert!(
            status.success(),
            "tui --once must exit successfully: {}",
            once.stderr.as_ref().unwrap().text()
        );
        let stdout = once.stdout.as_ref().unwrap().text();
        assert!(
            stdout.contains("OpenCode RK TUI"),
            "frame banner missing: {stdout}"
        );
        assert!(
            stdout.contains("(live)"),
            "live frame marker missing: {stdout}"
        );
        assert!(
            stdout.contains(title),
            "actual authenticated session title missing: {stdout}"
        );
        assert_eq!(
            descriptor(&home).0,
            pid,
            "published daemon identity changed during attachment"
        );
        assert!(
            serve
                .child
                .try_wait()
                .expect("poll owned serve after tui")
                .is_none(),
            "owned serve exited during attachment"
        );
        drop(once);
        serve
            .stop()
            .expect("reap owned daemon and join output readers");
        assert!(serve
            .child
            .try_wait()
            .expect("verify reaped daemon")
            .is_some());
    }

    #[test]
    fn bare_launch_opens_chat_tui() {
        // Genuine E2E spine for the default entrypoint's daemon-owned path:
        // bare launch refuses redirected stdio without touching raw mode or
        // starting a daemon (headless exit 2 with the documented message);
        // an owned daemon then proves provider setup persistence via PUT
        // /auth/openai with daemon bearer auth, a real provider turn uses
        // the persisted key against a local stub (OPENAI_BASE_URL override,
        // no outbound network) with auth-denial checks, and typed history
        // survives daemon restart. The native OpenTUI renderer proves the
        // real frame path.
        let home = TestHome::new();

        // Bare launch with redirected stdio: headless, no daemon started.
        let mut bare = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
        command_env(&mut bare, &home);
        bare.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut bare_child = OwnedChild::spawn(&mut bare);
        let bare_status = bare_child.wait().expect("bounded bare launch exit");
        assert_eq!(
            bare_status.code(),
            Some(2),
            "redirected bare launch must take the headless path"
        );
        let bare_stderr = bare_child.stderr.as_ref().unwrap().text();
        assert!(
            bare_stderr.contains("raw mode is refused"),
            "headless message must refuse raw mode: {bare_stderr}"
        );
        assert!(
            !home.descriptor().exists(),
            "headless bare launch must not start a daemon"
        );

        // Local provider stub first: the daemon inherits OPENAI_BASE_URL at
        // spawn, so the stub must be listening before serve starts.
        let stub = start_local_provider_stub();
        let stub_url = stub_base_url(&stub);
        let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let (mut serve, pid, origin, token) = spawn_serve_with_stub(&home, &stub_url);
        let identity_before = descriptor(&home);
        // Authenticated provider setup: persist a fixture key through the
        // real daemon route with the daemon bearer token.
        let fixture_key = "sk-test-structure-0123456789";
        let setup_body = serde_json::json!({"type":"api","key":fixture_key}).to_string();
        let saved =
            authenticated_request(&origin, &token, "PUT", "/auth/openai", Some(&setup_body));
        assert!(
            saved.starts_with("HTTP/1.1 200"),
            "provider setup persist failed: {saved}"
        );
        assert!(
            response_body(&saved).trim() == "true",
            "provider setup must acknowledge persistence: {saved}"
        );
        // Persisted shape: upstream auth.json with type/key, owner-only.
        let auth_path = providers_auth_path(&home);
        assert!(auth_path.is_file(), "auth.json must be persisted");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&auth_path)
                .expect("auth metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "auth.json must be owner-only");
        }
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(&auth_path).expect("read persisted auth"))
                .expect("persisted auth JSON");
        assert_eq!(persisted["openai"]["type"], "api");
        assert_eq!(persisted["openai"]["key"], fixture_key);

        // A denied different-key update must be rejected without touching auth.
        let denied_body = serde_json::json!({"type":"api","key":"sk-different-key"}).to_string();
        let denied_before = fs::read(&auth_path).expect("read auth before denied PUT");
        let denied_token = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let denied = request(
            &origin,
            denied_token,
            "PUT",
            "/auth/openai",
            Some(&denied_body),
        );
        let mut denied_stream = TcpStream::connect_timeout(
            &address(&origin).parse().expect("socket address"),
            Duration::from_secs(5),
        )
        .expect("connect owned serve");
        denied_stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        denied_stream
            .write_all(denied.as_bytes())
            .expect("write denied request");
        let mut denied_raw = Vec::new();
        let mut buffer = [0_u8; 4096];
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "denied response deadline");
            match denied_stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    assert!(
                        denied_raw.len() <= PIPE_CAP.saturating_sub(n),
                        "denied response exceeded fixture cap"
                    );
                    denied_raw.extend_from_slice(&buffer[..n]);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) => panic!("denied response read failed: {error}"),
            }
        }
        let denied_text = String::from_utf8(denied_raw).expect("denied UTF-8");
        assert!(
            status_code(&denied_text) == "403",
            "provider setup without bearer must be denied: {denied_text}"
        );
        assert_eq!(
            fs::read(&auth_path).expect("read auth after denied PUT"),
            denied_before,
            "denied provider PUT must not modify persisted auth bytes"
        );
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&auth_path)
                .expect("auth metadata after denied PUT")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "denied provider PUT must preserve owner-only auth mode"
        );

        // Real provider turn: local stub proves the daemon uses the
        // persisted credential and the configured model. The stub serves
        // the single request on a helper thread while the daemon performs
        // the outbound request; bounded accept avoids hangs.
        let title = "Structure Provider Spine";
        let body = serde_json::json!({"title": title}).to_string();
        let created = authenticated_request(&origin, &token, "POST", "/api/sessions", Some(&body));
        assert!(
            created.starts_with("HTTP/1.1 201"),
            "session creation failed: {created}"
        );
        let session_id = serde_json::from_str::<serde_json::Value>(response_body(&created))
            .expect("session JSON")["session"]["id"]
            .as_str()
            .expect("session id")
            .to_owned();
        let turn_body =
            serde_json::json!({"text":"structure provider turn","model":"openai/gpt-5.6"})
                .to_string();
        let stub_seen = Arc::clone(&seen);
        let first_stub_listener = stub.try_clone().expect("clone provider stub listener");
        let stub_thread = thread::spawn(move || {
            serve_stub_once(
                &first_stub_listener,
                &stub_seen,
                fixture_key,
                "gpt-5.6",
                "structure provider turn",
                None,
                "structure stub reply",
            );
        });
        let turn = authenticated_request(
            &origin,
            &token,
            "POST",
            &format!("/api/sessions/{session_id}/turns"),
            Some(&turn_body),
        );
        stub_thread.join().expect("stub thread");
        let first_seen = seen.lock().expect("seen lock").clone();
        assert!(
            turn.starts_with("HTTP/1.1 201"),
            "provider turn failed: {turn}"
        );
        let turn_value =
            serde_json::from_str::<serde_json::Value>(response_body(&turn)).expect("turn JSON");
        let assistant = turn_value["assistant_message"]["body"]["text"]
            .as_str()
            .expect("assistant text");
        assert!(
            assistant.contains("structure stub reply"),
            "turn must return stub provider text: {turn}"
        );
        assert_eq!(
            first_seen.len(),
            1,
            "exactly one stub request: {first_seen:?}"
        );
        assert_eq!(first_seen[0].1, "gpt-5.6");
        assert_eq!(first_seen[0].0, format!("Bearer {fixture_key}"));
        assert!(
            !turn.contains(fixture_key),
            "provider key leaked in turn response"
        );

        // The actual default interactive journey: attach the no-subcommand
        // binary through a PTY to the daemon above, let it bind the recent
        // session, submit a second user turn, and observe the real provider
        // response. This is deliberately not Renderer::render_once or a
        // disconnected HTTP-only assertion.
        let second_stub_seen = Arc::clone(&seen);
        let second_stub_listener = stub.try_clone().expect("clone provider stub listener");
        let second_stub = thread::spawn(move || {
            serve_stub_once(
                &second_stub_listener,
                &second_stub_seen,
                fixture_key,
                "gpt-5.6",
                "native default turn",
                Some(true),
                "native default reply",
            );
        });
        let daemon_addr = address(&origin).to_owned();
        let mut native = spawn_native_entrypoint(&home, &stub_url, &daemon_addr);
        native
            .wait_native_natural()
            .expect("native natural checkpoint");
        let native_output = native.stdout.as_ref().unwrap().text();
        let native_error = native.stderr.as_ref().unwrap().text();
        assert!(
            second_stub.join().is_ok(),
            "native provider stub saw no request; captured PTY output: {native_output}; relay stderr: {native_error}"
        );
        let cumulative_seen = seen
            .lock()
            .expect("cumulative provider observations")
            .clone();
        assert_eq!(
            cumulative_seen.len(),
            2,
            "the two turns must produce two captured Responses requests: {cumulative_seen:?}"
        );
        assert_eq!(cumulative_seen[1].0, format!("Bearer {fixture_key}"));
        assert_eq!(cumulative_seen[1].1, "gpt-5.6");
        assert!(
            native_output.contains("OpenCode RK"),
            "native entrypoint did not reach startup frame: stderr={native_error}"
        );
        assert!(
            native_output.contains("OpenCode RK"),
            "native PTY did not reach startup frame: {native_output}"
        );
        assert!(
            native_output.contains("native default reply"),
            "default native journey must render provider response: {native_output}"
        );
        assert!(
            !native_output.contains(fixture_key),
            "provider key leaked in native output"
        );
        let identity_after = descriptor(&home);
        assert_eq!(
            identity_after, identity_before,
            "default client must not replace daemon descriptor identity"
        );
        let native_status = native
            .acknowledge_native()
            .expect("native relay cleanup and reap");
        assert!(
            native_status.success(),
            "native relay cleanup failed: {native_status}; stderr: {}",
            native.stderr.as_ref().unwrap().text()
        );

        fn message_records(value: &serde_json::Value) -> Vec<serde_json::Value> {
            value["messages"]
                .as_array()
                .expect("messages array")
                .iter()
                .map(|message| {
                    serde_json::json!({
                        "role": message["role"],
                        "body": message["body"],
                    })
                })
                .collect()
        }

        let native_history = authenticated_request(
            &origin,
            &token,
            "GET",
            &format!("/api/sessions/{session_id}/messages?limit=10"),
            None,
        );
        let native_history_value =
            serde_json::from_str::<serde_json::Value>(response_body(&native_history))
                .expect("native history JSON");
        let native_records = message_records(&native_history_value);
        assert!(
            native_records.iter().any(|message| {
                message["role"] == "user" && message["body"]["text"] == "native default turn"
            }),
            "native user message must persist in the bound session: {native_records:?}"
        );
        assert!(
            native_records.iter().any(|message| {
                message["role"] == "assistant" && message["body"]["text"] == "native default reply"
            }),
            "native assistant message must persist in the bound session: {native_records:?}"
        );

        // Restart/resume: typed history persists across daemon restart. Remove
        // only the descriptor owned by the daemon we just stopped; readiness
        // must never accept the previous descriptor as the new daemon's.
        let history = authenticated_request(
            &origin,
            &token,
            "GET",
            &format!("/api/sessions/{session_id}/messages?limit=10"),
            None,
        );
        assert!(
            history.starts_with("HTTP/1.1 200"),
            "history read failed: {history}"
        );
        let history_value = serde_json::from_str::<serde_json::Value>(response_body(&history))
            .expect("history JSON");
        let texts: Vec<&str> = history_value["messages"]
            .as_array()
            .expect("messages array")
            .iter()
            .filter_map(|message| message["body"]["text"].as_str())
            .collect();
        assert!(
            texts
                .iter()
                .any(|text| text.contains("structure provider turn")),
            "user turn must persist: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("structure stub reply")),
            "assistant turn must persist: {texts:?}"
        );
        serve.stop().expect("reap owned daemon");
        fs::remove_file(home.descriptor()).expect("remove stopped daemon descriptor");
        let (mut serve2, pid2, origin2, token2) = spawn_serve_with_stub(&home, &stub_url);
        assert_ne!(pid2, pid, "restart must start a fresh daemon");
        assert_eq!(
            descriptor(&home).0,
            pid2,
            "descriptor must track the restarted daemon"
        );
        let resumed = authenticated_request(
            &origin2,
            &token2,
            "GET",
            &format!("/api/sessions/{session_id}/messages?limit=10"),
            None,
        );
        assert!(
            resumed.starts_with("HTTP/1.1 200"),
            "resumed history read failed: {resumed}"
        );
        let resumed_value = serde_json::from_str::<serde_json::Value>(response_body(&resumed))
            .expect("resumed JSON");
        assert_eq!(
            message_records(&resumed_value),
            message_records(&history_value),
            "typed session messages must remain exactly equal across daemon restart"
        );
        let resumed_texts: Vec<&str> = resumed_value["messages"]
            .as_array()
            .expect("resumed array")
            .iter()
            .filter_map(|message| message["body"]["text"].as_str())
            .collect();
        assert!(
            resumed_texts
                .iter()
                .any(|text| text.contains("structure provider turn")),
            "user turn must survive restart: {resumed_texts:?}"
        );
        assert!(
            resumed_texts
                .iter()
                .any(|text| text.contains("structure stub reply")),
            "assistant turn must survive restart: {resumed_texts:?}"
        );
        // Native renderer additionally proves the real OpenTUI frame path.
        let frame = Renderer::render_once(
            80,
            24,
            &[
                "OpenCode RK TUI".to_owned(),
                "model: openai/gpt-5.6 (ctrl-p)".to_owned(),
                format!("session: {title} (live)"),
            ],
        )
        .expect("native renderer snapshot");
        assert!(frame.contains("OpenCode RK"), "frame banner: {frame}");
        assert!(
            frame.contains("openai/gpt-5.6"),
            "frame model status: {frame}"
        );
        assert!(frame.contains(title), "frame session title: {frame}");
        assert!(
            serve2
                .child
                .try_wait()
                .expect("poll restarted serve")
                .is_none(),
            "restarted serve exited during attachment"
        );
        drop(serve2);
    }

    #[test]
    fn bare_launch_auto_starts_owned_daemon_from_fresh_home() {
        let home = TestHome::new();
        let stub = start_local_provider_stub();
        let stub_url = stub_base_url(&stub);
        let native =
            spawn_native_entrypoint_with_addr(&home, &stub_url, None, true, None, None, None, None);
        let mut native = native;
        let descriptor_deadline = Instant::now() + Duration::from_secs(5);
        while !home.descriptor().is_file() && Instant::now() < descriptor_deadline {
            thread::sleep(Duration::from_millis(25));
        }
        let (owned_pid, owned_origin, owned_token) = descriptor(&home);
        let native_group = home.root().join("runtime/native-group");
        let group_deadline = Instant::now() + Duration::from_secs(2);
        while !native_group.is_file() && Instant::now() < group_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let native_group_id = fs::read_to_string(&native_group)
            .expect("native group marker")
            .trim()
            .parse::<u32>()
            .expect("native group PID");
        assert!(native_group_id > 0, "native group marker must be positive");
        native
            .wait_native_natural()
            .expect("auto-start native natural checkpoint");
        let output = native.stdout.as_ref().expect("auto-start stdout").text();
        assert!(
            output.contains("OpenCode RK"),
            "auto-start frame missing: {output}"
        );
        assert!(
            home.descriptor().is_file(),
            "bare launch must publish daemon descriptor"
        );
        let (pid, origin, token) = descriptor(&home);
        assert_eq!(
            origin, "http://127.0.0.1:4096",
            "bare launch uses product default address"
        );
        let health = authenticated_request(&origin, &token, "GET", "/health", None);
        assert!(
            health.starts_with("HTTP/1.1 200"),
            "auto-start daemon health failed: {health}"
        );
        assert_eq!(
            pid,
            descriptor(&home).0,
            "descriptor PID must remain stable"
        );
        let survived = authenticated_request(&origin, &token, "GET", "/health", None);
        assert!(
            survived.starts_with("HTTP/1.1 200"),
            "daemon must survive client exit: {survived}"
        );
        let native_status = native
            .acknowledge_native()
            .expect("auto-start relay cleanup and reap");
        assert!(
            native_status.success(),
            "auto-start relay cleanup failed: {native_status}"
        );
        assert_eq!(pid, owned_pid);
        assert_eq!(origin, owned_origin);
        assert_eq!(token, owned_token);
    }

    // ── Case A: pre-READY failure with positively owned fork child alive ───────
    //
    // The relay forks and readies a native child, then faults after fork but
    // before recording fixture_group. The OS PID/group oracle must observe the
    // resulting orphan; the harness cleanup below is bounded and independent.
    #[cfg(unix)]
    #[test]
    fn case_a_pre_ready_failure_orphan_detection_and_harness_cleanup() {
        let home = TestHome::new();
        let stub = start_local_provider_stub();
        let stub_url = stub_base_url(&stub);

        // ── Launch the relay with pre_ready_fail fault.
        let relay_pid_file = home.root().join("runtime/relay-pid");
        let group_file = home.root().join("runtime/native-group");
        let mut relay = spawn_native_entrypoint_with_addr(
            &home,
            &stub_url,
            None,
            false,
            Some("pre_ready_fail"),
            Some(&relay_pid_file),
            None,
            Some(&group_file),
        );
        // The relay must exit nonzero: fault was injected.
        let relay_status = relay.wait().expect("relay bounded exit");
        let relay_stderr = relay.stderr.as_ref().unwrap().text();
        assert!(
            !relay_status.success(),
            "relay must exit nonzero on pre-ready fault; stderr: {relay_stderr}"
        );
        assert!(
            relay_stderr.contains("injected pre-ready fault")
                || relay_stderr.contains("fixture_group never set")
                || relay_stderr.contains("native relay failure"),
            "relay stderr must mention the injected fault: {relay_stderr}"
        );

        // No native-finished checkpoint should exist (relay never reached it).
        assert!(
            !home.root().join("runtime/native-finished").exists(),
            "no native checkpoint must exist after pre-ready fault"
        );

        // Actual relay child oracle: the driver forked it and proved its
        // process-group identity before injecting the fault.  This assertion
        // is intentionally before any harness cleanup and is the compiling RED
        // for the missing pre-READY cleanup behavior.
        let owned_pid: u32 = fs::read_to_string(&relay_pid_file)
            .expect("relay must publish owned fork child PID")
            .trim()
            .parse()
            .expect("owned fork child PID");
        let owned_group: u32 = fs::read_to_string(&group_file)
            .expect("relay must publish owned fork group")
            .trim()
            .parse()
            .expect("owned fork group");
        assert_eq!(
            owned_pid, owned_group,
            "owned child must lead its native group"
        );
        let orphaned_before_harness_cleanup = pid_exists(owned_pid) && pgrp_exists(owned_group);

        if orphaned_before_harness_cleanup {
            let _ = Command::new("kill")
                .args(["-TERM", &format!("-{owned_group}")])
                .status();
            thread::sleep(Duration::from_millis(100));
            if pgrp_exists(owned_group) {
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{owned_group}")])
                    .status();
            }
        }
        assert!(
            wait_pid_gone(owned_pid, Duration::from_secs(3)),
            "harness must reclaim actual orphaned child: pid={owned_pid}"
        );
        assert!(
            wait_pgrp_gone(owned_group, Duration::from_secs(3)),
            "harness must reclaim actual orphaned group: pgid={owned_group}"
        );

        assert!(
            !orphaned_before_harness_cleanup,
            "pre-READY failure left a positively-owned native child/group alive"
        );
    }

    // ── Case B: descriptor corruption / assertion unwind after auto-start ──────
    //
    // The product daemon auto-starts and publishes a real descriptor.  The test
    // then corrupts the in-memory descriptor string and calls a function that
    // panics via assertion.  catch_unwind catches the panic so Drop cleanup
    // runs; the test then proves the relay and daemon are cleanly reaped and
    // the group is gone — HOME is kept alive outside the panic boundary.
    //
    // This test may PASS on the current baseline (cleanup is cooperative when
    // descriptor assertion fires outside the relay lifecycle).  It must NOT
    // fabricate RED: if cleanup actually succeeds, the test passes.
    //
    // The auto-start path (OC2_TEST_AUTO_START=1) launches the real product
    // daemon; we do NOT pre-start serve separately.
    #[cfg(unix)]
    #[test]
    fn case_b_descriptor_corruption_unwind_cleanup_and_group_gone() {
        // Keep HOME alive outside the catch_unwind scope.
        let home = TestHome::new();
        let stub = start_local_provider_stub();
        let stub_url = stub_base_url(&stub);
        let group_file = home.root().join("runtime/native-group");

        // Spawn the relay in auto-start mode (no pre-existing daemon).
        // The relay will auto-start the product daemon.
        let mut relay = spawn_native_entrypoint_with_addr(
            &home,
            &stub_url,
            None,
            true,
            None,
            None,
            None,
            Some(&group_file),
        );

        // Wait for the descriptor to appear (daemon started by the relay).
        let desc_deadline = Instant::now() + Duration::from_secs(15);
        while !home.descriptor().is_file() && Instant::now() < desc_deadline {
            // Also check relay hasn't exited prematurely.
            if let Some(status) = relay.child.try_wait().expect("poll relay") {
                let relay_stderr = relay.stderr.as_ref().unwrap().text();
                panic!("relay exited before descriptor appeared: {status}; stderr: {relay_stderr}");
            }
            thread::sleep(Duration::from_millis(25));
        }

        if !home.descriptor().is_file() {
            let relay_stderr = relay.stderr.as_ref().unwrap().text();
            panic!("native auto-start failed to publish descriptor: {relay_stderr}");
        }

        // Read real descriptor before corruption.
        let (daemon_pid, _daemon_origin, _daemon_token) = descriptor(&home);

        // Read the group file written by the relay (child PID = group leader).
        let native_group_pid: Option<u32> = fs::read_to_string(&group_file)
            .ok()
            .and_then(|s| s.trim().parse().ok());

        // ── Simulate a descriptor assertion unwind by corrupting the
        // descriptor and calling descriptor() which will panic.
        //
        // We use catch_unwind around the corrupt-and-read cycle, but keep
        // HOME and relay alive outside. This mirrors the real scenario where
        // a test assertion fires mid-test and Drop cleanup must still work.
        let desc_path = home.descriptor();

        // Corrupt: overwrite descriptor with invalid JSON.
        fs::write(&desc_path, b"{{not valid json}}").expect("corrupt descriptor");

        // This should panic inside the assertion (serde_json parse fails).
        let unwind_result = std::panic::catch_unwind(|| {
            // Exercise the actual descriptor helper and its original panic.
            let _ = descriptor(&home);
        });
        assert!(
            unwind_result.is_err(),
            "corrupt descriptor must cause a panic/unwind"
        );

        // Restore a valid descriptor so Stop/Drop can proceed cleanly.
        // (The real daemon is still alive; its descriptor was corrupted by us.)
        // We reconstruct a minimal valid descriptor using daemon_pid.
        // The cleanup path must NOT be gated on descriptor validity.
        let _ = fs::remove_file(&desc_path);
        // Relay cleanup via explicit stop (simulates Drop after unwind).
        let stop_result = relay.stop();

        // After stop, assert relay process is reaped.
        assert!(
            relay
                .child
                .try_wait()
                .expect("poll relay after stop")
                .is_some(),
            "relay must be reaped after stop"
        );

        // If we have a native group PID, verify it's gone (group cleaned up).
        if let Some(pgid) = native_group_pid {
            if pgid > 0 {
                // Give the relay cleanup 3s to kill the group.
                let group_gone = wait_pgrp_gone(pgid, Duration::from_secs(3));
                // We report whether cleanup worked — this is the RED/GREEN marker.
                // On unfixed code: relay may not clean up the group after a
                // descriptor-path fault; assert group_gone to make it RED.
                assert!(
                    group_gone,
                    "native group must be gone after relay stop/Drop: pgid={pgid}; stop_result={stop_result:?}"
                );
            }
        }

        // Daemon PID should also be gone.
        if daemon_pid > 0 {
            assert!(
                wait_pid_gone(daemon_pid, Duration::from_secs(3)),
                "daemon must be reaped after relay stop: pid={daemon_pid}"
            );
        }

        // stop_result may be Ok or Err; both are acceptable as long as the
        // process is reaped.  We don't assert success because the relay may
        // have already exited via the descriptor corruption path.
        let _ = stop_result;
    }

    // ── Case C: TERM-ignoring member directly forked by relay into valid group ──
    //
    // The relay forks a SIGTERM-ignoring member into the validated native group
    // AFTER fixture_group is set (so terminate_owned has a valid group).
    // A barrier ensures the member has installed SIGTERM ignore before cleanup.
    // The relay then raises (injected fault) so terminate_owned is called.
    //
    // Expected path: SIGTERM sent → ignored by member → timeout → SIGKILL sent
    // → group killed → relay reaped.
    //
    // Forced/fault status nonzero: expected (relay exits 1 due to injected
    // fault).  Nonzero alone is not cleanup proof.  We assert:
    //  1. Relay exits nonzero (fault injected).
    //  2. Member PID from OC2_TEST_MEMBER_PID_FILE is gone (not via marker
    //     alone — via OS query).
    //  3. Group (pgid = fixture_group) is gone (OS query, not marker).
    //
    // If current cleanup already sends SIGKILL correctly, this baseline may
    // PASS.  Do NOT pretend RED if the cleanup actually works.
    //
    // We use direct member waitpid only when the member is a direct child of
    // the relay; we do NOT waitpid grandchild daemons.  The relay's terminate_owned
    // uses killpg which covers all group members, so we verify the group is gone.
    #[cfg(unix)]
    #[test]
    fn case_c_sigterm_ignoring_member_killed_by_cleanup() {
        let home = TestHome::new();
        let stub = start_local_provider_stub();
        let stub_url = stub_base_url(&stub);

        let member_pid_file = home.root().join("runtime/member-pid");
        let group_file = home.root().join("runtime/native-group");

        // Spawn relay with sigterm_ignore fault.
        let mut relay = spawn_native_entrypoint_with_addr(
            &home,
            &stub_url,
            None,
            false,
            Some("sigterm_ignore"),
            None,
            Some(&member_pid_file),
            Some(&group_file),
        );

        // Wait for relay to exit (it injects fault and exits nonzero).
        let relay_status = relay.wait().expect("relay bounded exit");
        let relay_stderr = relay.stderr.as_ref().unwrap().text();

        // Relay must exit nonzero (injected fault).
        assert!(
            !relay_status.success(),
            "relay must exit nonzero on sigterm_ignore fault; stderr: {relay_stderr}"
        );

        // Member PID must be recorded.
        let member_pid_str = fs::read_to_string(&member_pid_file).unwrap_or_default();
        let member_pid: u32 = member_pid_str.trim().parse().unwrap_or(0);

        // Group file must be recorded.
        let group_str = fs::read_to_string(&group_file).unwrap_or_default();
        let group_pid: u32 = group_str.trim().parse().unwrap_or(0);

        // Validate both IDs are nonzero before OS assertions.
        assert!(
            member_pid > 0 && group_pid > 0,
            "SIGTERM-ignore fixture must publish member/group identities: member={member_pid} group={group_pid}; stderr={relay_stderr}"
        );

        // ── OS-level cleanup proof ────────────────────────────────────────────
        // Give the relay's terminate_owned up to 5s to kill the group after
        // SIGTERM+SIGKILL (CLEANUP=2.0 in the DRIVER, plus margin).
        let member_gone = wait_pid_gone(member_pid, Duration::from_secs(5));
        let group_gone = wait_pgrp_gone(group_pid, Duration::from_secs(5));

        // The relay's terminate_owned is called from the except block.
        // On unfixed code: if the relay cleanup is correct, SIGKILL is sent
        // after SIGTERM timeout, and the group disappears.  This PASSES
        // baseline.  Do NOT assert failure artificially.
        // If either assertion fails, it means cleanup did NOT send SIGKILL
        // after SIGTERM ignore — that is the genuine RED condition.
        assert!(
            member_gone,
            "SIGTERM-ignoring member must be gone after relay cleanup (SIGKILL): pid={member_pid}; relay_stderr={relay_stderr}"
        );
        assert!(
            group_gone,
            "native group must be gone after relay cleanup: pgid={group_pid}; relay_stderr={relay_stderr}"
        );

        // Relay stderr must mention the fault and cleanup.
        assert!(
            relay_stderr.contains("injected sigterm_ignore fault")
                || relay_stderr.contains("native relay failure"),
            "relay must report the injected fault: {relay_stderr}"
        );
    }
}
