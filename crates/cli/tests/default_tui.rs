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
        fs,
        io::{Read, Write},
        net::TcpStream,
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
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
            for part in ["home", "xdg-config", "xdg-data", "xdg-cache", "runtime"] {
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
        thread: Option<thread::JoinHandle<()>>,
    }

    impl Capture {
        fn spawn(pipe: impl Read + Send + 'static) -> Self {
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let overflow = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let dst = Arc::clone(&bytes);
            let full = Arc::clone(&overflow);
            let thread = thread::spawn(move || {
                let mut pipe = pipe;
                let mut buf = [0_u8; 4096];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
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
                thread: Some(thread),
            }
        }

        fn text(&self) -> String {
            String::from_utf8_lossy(&self.bytes.lock().expect("capture lock")).into_owned()
        }

        fn join(&mut self) {
            let thread = self.thread.take().expect("capture joined once");
            thread.join().expect("capture reader must terminate");
            assert!(
                !self.overflow.load(Ordering::Relaxed),
                "child output exceeded {PIPE_CAP} bytes"
            );
        }
    }

    struct OwnedServe {
        child: Child,
        stdout: Capture,
        stderr: Capture,
        cleaned: bool,
    }

    impl OwnedServe {}

    impl Drop for OwnedServe {
        fn drop(&mut self) {
            if self.cleaned {
                return;
            }
            self.cleaned = true;
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.stdout.join();
            self.stderr.join();
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
            .env("XDG_CACHE_HOME", home.root().join("xdg-cache"));
    }

    fn wait_descriptor(home: &TestHome) {
        let deadline = Instant::now() + EXIT_LIMIT;
        while !home.descriptor().exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        assert!(
            home.descriptor().exists(),
            "serve did not publish its descriptor"
        );
    }

    fn descriptor(home: &TestHome) -> (u32, String, String) {
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(home.descriptor()).expect("read published descriptor"),
        )
        .expect("published descriptor JSON");
        let pid = value["pid"].as_u64().expect("descriptor pid") as u32;
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
            origin.starts_with("http://127.0.0.1:"),
            "published origin must be loopback"
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
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("set write timeout");
        stream
            .write_all(request(origin, token, method, path, body).as_bytes())
            .expect("write owned request");
        let mut response = Vec::new();
        stream
            .take(PIPE_CAP as u64)
            .read_to_end(&mut response)
            .expect("read owned response");
        assert!(
            response.len() < PIPE_CAP,
            "HTTP response exceeded fixture cap"
        );
        String::from_utf8(response).expect("owned response UTF-8")
    }

    fn spawn_serve(home: &TestHome) -> (OwnedServe, u32, String, String) {
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
        let mut child = command.spawn().expect("spawn owned serve");
        let stdout = Capture::spawn(child.stdout.take().expect("serve stdout pipe"));
        let stderr = Capture::spawn(child.stderr.take().expect("serve stderr pipe"));
        let mut serve = OwnedServe {
            child,
            stdout,
            stderr,
            cleaned: false,
        };
        wait_descriptor(home);
        let (pid, origin, token) = descriptor(home);
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
            created.starts_with("HTTP/1.1 201") || created.starts_with("HTTP/1.1 200"),
            "session creation failed: {created}"
        );

        let mut command = Command::new(env!("CARGO_BIN_EXE_opencode-rk"));
        command_env(&mut command, &home);
        let output = command
            .args(["tui", "--once", "--origin", &origin])
            .output()
            .expect("run native tui once");
        assert!(
            output.status.success(),
            "tui --once must exit successfully: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
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
        drop(serve);
    }
}
