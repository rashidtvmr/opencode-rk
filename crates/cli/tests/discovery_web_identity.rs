#![forbid(unsafe_code)]

//! Installed macOS regression coverage for web descriptor identity and reuse.

#[cfg(target_os = "macos")]
mod macos {
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc},
        thread,
        time::{Duration, Instant},
    };

    const IO: Duration = Duration::from_millis(200);
    const CAP: usize = 8 * 1024;
    const CHILD_LIMIT: Duration = Duration::from_secs(8);

    struct ChildGuard(Option<Child>);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                if child.try_wait().ok().flatten().is_none() { let _ = child.kill(); }
                let _ = child.wait();
            }
        }
    }

    struct ForeignServer {
        stop: Arc<AtomicBool>,
        proof: Arc<AtomicUsize>,
        authorization: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
        addr: String,
    }
    impl ForeignServer {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("foreign listener");
            listener.set_nonblocking(true).expect("nonblocking listener");
            let addr = listener.local_addr().expect("foreign address").to_string();
            let stop = Arc::new(AtomicBool::new(false));
            let proof = Arc::new(AtomicUsize::new(0));
            let authorization = Arc::new(AtomicBool::new(false));
            let (s, p, a) = (stop.clone(), proof.clone(), authorization.clone());
            let join = thread::spawn(move || {
                while !s.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = listener.accept() else { thread::sleep(Duration::from_millis(5)); continue; };
                    let _ = stream.set_read_timeout(Some(IO));
                    let _ = stream.set_write_timeout(Some(IO));
                    let mut bytes = [0_u8; CAP];
                    let n = stream.read(&mut bytes).unwrap_or(0).min(CAP);
                    let request = String::from_utf8_lossy(&bytes[..n]);
                    if request.starts_with("GET /discovery-proof?") { p.fetch_add(1, Ordering::Relaxed); }
                    if request.lines().any(|line| line.to_ascii_lowercase().starts_with("authorization:")) { a.store(true, Ordering::Relaxed); }
                    let (status, body) = if request.starts_with("GET /health ") {
                        ("200 OK", r#"{"schema_version":1,"status":"ok","runtime":"foreign"}"#)
                    } else { ("404 Not Found", "not found") };
                    let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            Self { stop, proof, authorization, join: Some(join), addr }
        }
    }
    impl Drop for ForeignServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(&self.addr);
            if let Some(join) = self.join.take() { let _ = join.join(); }
        }
    }

    fn fixture_root() -> tempfile::TempDir {
        let base = std::env::var_os("OPENCODE_TEST_TMP_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        tempfile::Builder::new().prefix("oc2-web-identity-").tempdir_in(base).expect("fixture root")
    }
    fn binary() -> PathBuf {
        std::env::var_os("OC2_E2E_BIN").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_oc2")))
    }
    fn write_descriptor(data: &Path, pid: u32, addr: &str) {
        fs::create_dir_all(data.join("runtime")).expect("runtime");
        fs::write(data.join("runtime/backend.json"), serde_json::json!({"schema_version":1,"pid":pid,"http_origin":format!("http://{addr}"),"auth_token":"a".repeat(64)}).to_string()).expect("descriptor");
    }
    fn command(root: &Path, data: &Path, marker: Option<&Path>) -> Command {
        let mut c = Command::new(binary());
        c.current_dir(root).env_clear().env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config")).env("OPENCODE_RK_HOME", data)
            .env("OPENCODE_RK_CONFIG_DIR", root.join("config")).env("PATH", "/usr/bin:/bin")
            .env("TERM", "xterm-256color").env("LANG", "C");
        if let Some(marker) = marker { c.env("OC2_TEST_OPEN_MARKER", marker); }
        c
    }
    fn bounded_exit(child: &mut Child) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while child.try_wait().expect("poll child").is_none() && Instant::now() < deadline { thread::sleep(Duration::from_millis(20)); }
        if child.try_wait().expect("poll child").is_none() { let _ = child.kill(); let _ = child.wait(); }
    }
    fn run_bounded(mut command: Command) -> std::process::ExitStatus {
        let mut child = ChildGuard(Some(command.spawn().expect("spawn command")));
        bounded_exit(child.0.as_mut().unwrap());
        child.0.as_mut().unwrap().try_wait().expect("wait").expect("bounded command exit")
    }
    fn logs(root: &Path) -> String {
        let mut out = String::new();
        for name in ["stdout.log", "stderr.log"] { let bytes = fs::read(root.join(name)).unwrap_or_default(); out.push_str(&String::from_utf8_lossy(&bytes[..bytes.len().min(CAP)])); }
        out
    }
    fn complete_marker_lines(path: &Path) -> Vec<String> {
        let Ok(bytes) = fs::read(path) else { return Vec::new() };
        if !bytes.ends_with(b"\n") { return Vec::new(); }
        String::from_utf8_lossy(&bytes).lines().map(str::to_owned).collect()
    }
    fn wait_for_marker_lines(path: &Path, count: usize, deadline: Instant) -> Vec<String> {
        loop {
            let lines = complete_marker_lines(path);
            if lines.len() >= count || Instant::now() >= deadline { return lines; }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn web_rejects_live_foreign_healthy_descriptor_without_open_or_bearer() {
        let root = fixture_root(); let data = root.path().join("data");
        let foreign = ForeignServer::new();
        let pid = Command::new("/bin/sleep").arg("30").spawn().expect("foreign pid");
        let _foreign_pid = ChildGuard(Some(pid));
        write_descriptor(&data, _foreign_pid.0.as_ref().unwrap().id(), &foreign.addr);
        let marker = root.path().join("open.marker");
        let fake = root.path().join("bin"); fs::create_dir_all(&fake).expect("fake bin");
        fs::write(fake.join("open"), "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$OC2_TEST_OPEN_MARKER\"\n").expect("fake open");
        let _ = Command::new("/bin/chmod").args(["+x", fake.join("open").to_str().unwrap()]).status();
        let out = fs::File::create(root.path().join("stdout.log")).expect("stdout"); let err = fs::File::create(root.path().join("stderr.log")).expect("stderr");
        let mut c = command(root.path(), &data, Some(&marker)); c.env("PATH", format!("{}:/usr/bin:/bin", fake.display())).arg("--data-dir").arg(&data).arg("web").stdout(Stdio::from(out)).stderr(Stdio::from(err));
        let mut child = ChildGuard(Some(c.spawn().expect("installed oc2"))); bounded_exit(child.0.as_mut().unwrap());
        assert!(foreign.proof.load(Ordering::Relaxed) > 0, "fresh discovery proof was not attempted: {}", logs(root.path()));
        assert!(!foreign.authorization.load(Ordering::Relaxed), "foreign endpoint received Authorization");
        assert!(!marker.exists() || fs::read_to_string(&marker).unwrap_or_default().is_empty(), "fake browser invoked");
        assert!(child.0.as_mut().unwrap().try_wait().expect("wait").map(|s| !s.success()).unwrap_or(false), "foreign descriptor must fail closed");
    }

    #[test]
    fn web_reuses_genuine_daemon_and_opens_exact_same_origin_token() {
        let root = fixture_root(); let data = root.path().join("data");
        let marker = root.path().join("open.marker"); let fake = root.path().join("bin"); fs::create_dir_all(&fake).expect("fake bin");
        fs::write(fake.join("open"), "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$OC2_TEST_OPEN_MARKER\"\n").expect("fake open");
        let _ = Command::new("/bin/chmod").args(["+x", fake.join("open").to_str().unwrap()]).status();
        let mut first = command(root.path(), &data, Some(&marker)); first.env("PATH", format!("{}:/usr/bin:/bin", fake.display())).arg("--data-dir").arg(&data).arg("web").arg("--listen").arg("127.0.0.1:0").stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
        let mut daemon = ChildGuard(Some(first.spawn().expect("real web daemon")));
        let initial_lines = wait_for_marker_lines(&marker, 1, Instant::now() + CHILD_LIMIT);
        let descriptor = opencode_rk_server::daemon::read_backend_descriptor(&data).expect("descriptor read").expect("descriptor");
        let expected = format!("{}#oc2-token={}", descriptor.http_origin, descriptor.auth_token);
        assert_eq!(initial_lines.len(), 1, "initial browser proof/open was not reached");
        assert!(descriptor.http_origin.starts_with("http://127.0.0.1:"), "descriptor origin must be loopback: {}", descriptor.http_origin);
        assert!(descriptor.http_origin.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()).is_some_and(|p| p != 0), "descriptor port must be valid");
        let mut second = command(root.path(), &data, Some(&marker)); second.env("PATH", format!("{}:/usr/bin:/bin", fake.display())).arg("--data-dir").arg(&data).arg("web").stdout(Stdio::null()).stderr(Stdio::null());
        let status = run_bounded(second); assert!(status.success(), "genuine web reuse failed");
        let lines = wait_for_marker_lines(&marker, 2, Instant::now() + CHILD_LIMIT);
        assert_eq!(lines.len(), 2, "unexpected browser invocation count: {lines:?}"); assert_eq!(lines[1], expected, "reuse opened a different origin/token");
        bounded_exit(daemon.0.as_mut().unwrap());
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn discovery_web_identity_is_macOS_installed_probe() {}
