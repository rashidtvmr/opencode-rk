#![forbid(unsafe_code)]

//! Installed-client boundary probe.  A healthy foreign listener is not daemon
//! ownership evidence and must never receive this workspace's bearer.

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        fs::{self, File},
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Duration, Instant},
    };

    const IO: Duration = Duration::from_millis(200);
    const CAP: usize = 8 * 1024;

    struct ChildGuard(Option<Child>);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                }
                let _ = child.wait();
            }
        }
    }

    struct ForeignServer {
        stop: Arc<AtomicBool>,
        health: Arc<AtomicUsize>,
        bearer: Arc<Mutex<bool>>,
        join: Option<thread::JoinHandle<()>>,
        addr: String,
    }
    impl ForeignServer {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("foreign listener");
            listener
                .set_nonblocking(true)
                .expect("nonblocking listener");
            let addr = listener.local_addr().expect("foreign address").to_string();
            let stop = Arc::new(AtomicBool::new(false));
            let health = Arc::new(AtomicUsize::new(0));
            let bearer = Arc::new(Mutex::new(false));
            let (stop2, health2, bearer2) =
                (Arc::clone(&stop), Arc::clone(&health), Arc::clone(&bearer));
            let join = thread::spawn(move || {
                while !stop2.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    let _ = stream.set_read_timeout(Some(IO));
                    let _ = stream.set_write_timeout(Some(IO));
                    let mut bytes = [0_u8; CAP];
                    let n = stream.read(&mut bytes).unwrap_or(0).min(CAP);
                    let request = String::from_utf8_lossy(&bytes[..n]);
                    if request.starts_with("GET /health ") {
                        health2.fetch_add(1, Ordering::Relaxed);
                    }
                    if request.lines().any(|line| {
                        line.to_ascii_lowercase()
                            .starts_with("authorization: bearer")
                    }) {
                        *bearer2.lock().expect("capture lock") = true;
                    }
                    let (status, body) = if request.starts_with("GET /health ") {
                        (
                            "200 OK",
                            r#"{"schema_version":1,"status":"ok","runtime":"foreign"}"#,
                        )
                    } else {
                        ("403 Forbidden", "forbidden")
                    };
                    let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            Self {
                stop,
                health,
                bearer,
                join: Some(join),
                addr,
            }
        }
    }
    impl Drop for ForeignServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            // Wake accept so the join is bounded even if the nonblocking loop
            // is between polls.
            let _ = TcpStream::connect(&self.addr);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    fn fixture_root() -> tempfile::TempDir {
        let base = std::env::var_os("OPENCODE_TEST_TMP_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        tempfile::Builder::new()
            .prefix("oc2-discovery-")
            .tempdir_in(base)
            .expect("fixture root")
    }

    fn write_descriptor(data: &Path, pid: u32, addr: &str) {
        fs::create_dir_all(data.join("runtime")).expect("runtime");
        let token = "a".repeat(64);
        let value = serde_json::json!({"schema_version": 1, "pid": pid, "http_origin": format!("http://{addr}"), "auth_token": token});
        fs::write(data.join("runtime/backend.json"), value.to_string()).expect("descriptor");
        let parsed = opencode_rk_server::daemon::read_backend_descriptor(data)
            .expect("descriptor read")
            .expect("descriptor present");
        assert_eq!(parsed.pid, pid);
        assert_eq!(parsed.http_origin, format!("http://{addr}"));
        assert_eq!(parsed.auth_token.len(), 64);
    }

    #[test]
    fn default_entrypoint_rejects_live_foreign_health_descriptor() {
        let root = fixture_root();
        let data = root.path().join("data");
        let workspace = root.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let server = ForeignServer::new();
        let foreign = ChildGuard(Some(
            Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .expect("foreign pid"),
        ));
        write_descriptor(&data, foreign.0.as_ref().unwrap().id(), &server.addr);
        let binary = std::env::var_os("OC2_E2E_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")));
        let bundled = std::env::var_os("OC2_E2E_BIN").is_some();
        let out = File::create(root.path().join("stdout.log")).expect("stdout log");
        let err = File::create(root.path().join("stderr.log")).expect("stderr log");
        let mut command = Command::new("/usr/bin/script");
        command
            .args(["-q", "/dev/null"])
            .arg(binary)
            .args(["--data-dir", data.to_str().unwrap()])
            .current_dir(&workspace)
            .env_clear()
            .env("HOME", root.path().join("home"))
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("OPENCODE_RK_HOME", &data)
            .env("OPENCODE_RK_CONFIG_DIR", root.path().join("config"))
            .env("PATH", "/usr/bin:/bin")
            .env("TERM", "xterm-256color")
            .env("LANG", "C")
            .env("OPENCODE_RK_DAEMON_ADDR", &server.addr)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        if !bundled {
            if let Some(value) = std::env::var_os("DYLD_LIBRARY_PATH") {
                command.env("DYLD_LIBRARY_PATH", value);
            }
        }
        let mut child = ChildGuard(Some(command.spawn().expect("installed entrypoint")));
        let mut stdin = child.0.as_mut().unwrap().stdin.take().expect("stdin");
        let deadline = Instant::now() + Duration::from_secs(8);
        while server.health.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
        }
        if server.health.load(Ordering::Relaxed) > 0 {
            thread::sleep(Duration::from_millis(300));
        }
        let _ = stdin.write_all(b"\x04");
        let _ = stdin.flush();
        let exit_deadline = Instant::now() + Duration::from_secs(2);
        while child
            .0
            .as_mut()
            .unwrap()
            .try_wait()
            .expect("poll child")
            .is_none()
            && Instant::now() < exit_deadline
        {
            thread::sleep(Duration::from_millis(25));
        }
        if child
            .0
            .as_mut()
            .unwrap()
            .try_wait()
            .expect("poll child")
            .is_none()
        {
            let _ = child.0.as_mut().unwrap().kill();
            let _ = child.0.as_mut().unwrap().wait();
        }
        let health_reached = server.health.load(Ordering::Relaxed) > 0;
        let bearer_seen = *server.bearer.lock().expect("capture lock");
        drop(child);
        drop(foreign);
        drop(server);
        let failure = !health_reached || bearer_seen;
        if failure {
            let read_log = |name: &str| {
                let path = root.path().join(name);
                let bytes = fs::read(path).unwrap_or_default();
                String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned()
            };
            let stdout = read_log("stdout.log");
            let stderr = read_log("stderr.log");
            let preserved = root.keep();
            panic!(
                "discovery boundary failed (health_reached={health_reached}, bearer_seen={bearer_seen}); fixture={}\nstdout:\n{stdout}\nstderr:\n{stderr}",
                preserved.display()
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn discovery_identity_boundary_is_macOS_installed_probe() {}
