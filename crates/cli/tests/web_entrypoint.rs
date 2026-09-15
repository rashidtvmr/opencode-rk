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
    fn new() -> Self {
        let id = NEXT_HOME.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-web-entrypoint-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
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

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_loopback() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap()
}

fn get(address: SocketAddr, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(200)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    stream.flush().ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

fn wait_for_page(address: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if get(address, "/").is_some_and(|response| {
            response.contains("200 OK") && response.contains("<title>OpenCode RK</title>")
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "web UI did not start at {address}"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn web_006_t03_web_command_owns_or_reuses_the_singleton_without_vite() {
    let home = TestHome::new();
    let first_address = free_loopback();
    let unused_address = free_loopback();

    let child = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["web", "--no-open", "--listen", &first_address.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start web-owned singleton daemon");
    let _owner = ChildGuard(child);
    wait_for_page(first_address);

    let reused = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", home.path())
        .args(["web", "--no-open", "--listen", &unused_address.to_string()])
        .output()
        .expect("reuse already-running web backend");
    assert!(reused.status.success());
    assert_eq!(
        String::from_utf8(reused.stdout).unwrap().trim(),
        format!("http://{first_address}")
    );
    assert!(
        TcpStream::connect_timeout(&unused_address, Duration::from_millis(150)).is_err(),
        "reusing the singleton must not bind the requested second port"
    );
}
