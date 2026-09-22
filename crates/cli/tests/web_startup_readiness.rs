use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use opencode_rk_server::daemon::{publish_backend_descriptor_with_auth, DaemonPaths, PidLock};

struct TempData(std::path::PathBuf);

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

impl TempData {
    fn new() -> Self {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-web006-readiness-{}-{}",
            std::process::id(),
            id
        ));
        fs::create_dir_all(&path).expect("create disposable data directory");
        Self(path)
    }
}

struct FixtureThreads {
    publisher: Option<thread::JoinHandle<String>>,
    health: Option<thread::JoinHandle<()>>,
}

impl Drop for FixtureThreads {
    fn drop(&mut self) {
        if let Some(handle) = self.publisher.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.health.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TempData {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn serve_health(listener: TcpListener, deadline: Instant) {
    listener
        .set_nonblocking(true)
        .expect("set fixture listener nonblocking");
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut request = [0_u8; 512];
                let _ = stream.read(&mut request);
                let response =
                    b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                let _ = stream.write_all(response);
                let _ = stream.flush();
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
}

fn wait_for_exit(mut child: Child, bound: Duration) -> std::process::Output {
    let deadline = Instant::now() + bound;
    loop {
        if child.try_wait().expect("inspect serve child").is_some() {
            return child
                .wait_with_output()
                .expect("collect serve output after exit");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("readiness wait exceeded 3 seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn test_binary() -> std::path::PathBuf {
    std::env::var_os("OPENCODE_RK_TEST_BIN")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())
        .expect("OPENCODE_RK_TEST_BIN must name the built opencode-rk binary")
}

#[test]
fn serve_waits_for_authenticated_publication_while_singleton_lock_is_held() {
    let data = TempData::new();
    let paths = DaemonPaths::for_data_dir(&data.0);
    let _owner = PidLock::acquire(&paths.pid).expect("fixture owns real singleton PID lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture health endpoint");
    let address: SocketAddr = listener.local_addr().expect("fixture address");
    let origin = format!("http://{address}");
    let health_deadline = Instant::now() + Duration::from_secs(3);
    let health = thread::spawn(move || serve_health(listener, health_deadline));

    let publish_data = data.0.clone();
    let publish_origin = origin.clone();
    let publisher = thread::spawn(move || {
        thread::sleep(Duration::from_millis(250));
        // The lock remains held by the parent test process while publication is delayed.
        publish_backend_descriptor_with_auth(
            &publish_data,
            address,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        )
        .expect("publish authenticated descriptor");
        publish_origin
    });

    let mut fixture_threads = FixtureThreads {
        publisher: Some(publisher),
        health: Some(health),
    };
    let mut second = Command::new(test_binary())
        .env_clear()
        .env("OPENCODE_RK_HOME", &data.0)
        .args(["serve", "--listen", "127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start second serve caller");
    thread::sleep(Duration::from_millis(100));
    if second
        .try_wait()
        .expect("inspect pre-publication caller")
        .is_some()
    {
        let output = second
            .wait_with_output()
            .expect("collect early-exited serve output");
        panic!(
            "second caller exited during publication window: status={:?}, stderr={:?}",
            output.status, output.stderr
        );
    }

    let output = wait_for_exit(second, Duration::from_secs(3));
    assert!(output.status.success(), "serve stderr: {:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), origin);
    assert_eq!(
        fixture_threads
            .publisher
            .take()
            .expect("publisher handle")
            .join()
            .expect("publisher child thread")
            .as_str(),
        origin
    );
    fixture_threads
        .health
        .take()
        .expect("health handle")
        .join()
        .expect("health fixture thread reaped");
}
