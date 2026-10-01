#![forbid(unsafe_code)]
//! Native default-entrypoint renderer controls.
//!
//! The four former pipe-driven chat tests belonged to the legacy line-chat
//! contract.  They are intentionally not retained here: current default
//! launch refuses redirected interactive input/output before daemon discovery
//! or raw-mode setup.  The current default-entrypoint startup/ownership
//! contract is frozen in `native_daemon_flow.rs` and is run as a separate
//! four-test target.

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
            let root = std::env::temp_dir().join(format!(
                "opencode-rk-default-native-{}-{id}",
                std::process::id()
            ));
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
            };
            owned.stdout = Some(Capture::spawn(
                owned.child.stdout.take().expect("child stdout"),
            ));
            owned.stderr = Some(Capture::spawn(
                owned.child.stderr.take().expect("child stderr"),
            ));
            owned
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
    }

    fn wait_descriptor(home: &TestHome, child: &mut OwnedChild) {
        let deadline = Instant::now() + EXIT_LIMIT;
        while !home.descriptor().exists() && Instant::now() < deadline {
            assert!(
                child
                    .child
                    .try_wait()
                    .expect("poll fixture daemon")
                    .is_none(),
                "fixture daemon exited before readiness"
            );
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
        format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", address(origin), body.len())
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
                    continue
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
}
