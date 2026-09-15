use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_HOME: AtomicU64 = AtomicU64::new(0);

struct TestHome(PathBuf);

impl TestHome {
    fn new(label: &str) -> Self {
        let id = NEXT_HOME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-web006-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create isolated data directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn spawn(home: &TestHome, listen: SocketAddr) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
            .env_clear()
            .env("OPENCODE_RK_HOME", home.path())
            .args(["serve", "--listen", &listen.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start opencode-rk backend fixture");
        Self(Some(child))
    }

    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("child still owned")
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

fn free_loopback() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
    listener.local_addr().expect("reserved address")
}

fn wait_for_health(address: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(response) = request(address, "GET", "/health", None) {
            if response.contains("200 OK") && response.contains("\"status\":\"ok\"") {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "backend did not become healthy at {address}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn request(
    address: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let body = body.unwrap_or("");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("inspect child status") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn web_006_t01_and_t05_same_data_dir_has_one_backend_but_other_data_dir_isolated() {
    let home = TestHome::new("shared");
    let other_home = TestHome::new("other");
    let first_address = free_loopback();
    let second_address = free_loopback();

    let mut first = ChildGuard::spawn(&home, first_address);
    wait_for_health(first_address);

    let created = request(
        first_address,
        "POST",
        "/api/sessions",
        Some(r#"{"title":"singleton-visible"}"#),
    )
    .expect("create session through first client origin");
    assert!(
        created.contains("201 Created"),
        "create response: {created}"
    );

    let mut duplicate = ChildGuard::spawn(&home, second_address);
    let duplicate_status = wait_for_exit(duplicate.child_mut(), Duration::from_secs(1));
    assert!(
        duplicate_status.is_some_and(|status| status.success()),
        "a second serve for the same data directory must reuse the existing backend and exit successfully instead of opening another server"
    );
    duplicate.0.take();

    assert!(
        TcpStream::connect_timeout(&second_address, Duration::from_millis(150)).is_err(),
        "the duplicate invocation must not own a second HTTP listener"
    );
    let sessions = request(first_address, "GET", "/api/sessions", None)
        .expect("existing backend remains reachable");
    assert!(
        sessions.contains("singleton-visible"),
        "the reused backend must retain the same session state: {sessions}"
    );

    let mut isolated = ChildGuard::spawn(&other_home, second_address);
    wait_for_health(second_address);
    let isolated_sessions = request(second_address, "GET", "/api/sessions", None)
        .expect("isolated backend session list");
    assert!(
        !isolated_sessions.contains("singleton-visible"),
        "a different data directory must have isolated state"
    );

    isolated.stop();
    first.stop();
}
