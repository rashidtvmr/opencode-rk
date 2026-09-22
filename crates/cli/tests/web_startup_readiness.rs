use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use opencode_rk_server::daemon::{publish_backend_descriptor_with_auth, DaemonPaths, PidLock};

struct TempData(std::path::PathBuf);

impl TempData {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "opencode-rk-web006-readiness-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&path).expect("create disposable data directory");
        Self(path)
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
        assert!(
            Instant::now() < deadline,
            "readiness wait exceeded 3 seconds"
        );
        thread::sleep(Duration::from_millis(10));
    }
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

    let mut second = Command::new(env!("CARGO_BIN_EXE_opencode-rk"))
        .env_clear()
        .env("OPENCODE_RK_HOME", &data.0)
        .args(["serve", "--listen", "127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start second serve caller");
    thread::sleep(Duration::from_millis(100));
    assert!(
        second
            .try_wait()
            .expect("inspect pre-publication caller")
            .is_none(),
        "second caller must remain alive during the publication window"
    );

    let output = wait_for_exit(second, Duration::from_secs(3));
    assert!(output.status.success(), "serve stderr: {:?}", output.stderr);
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), origin);
    assert_eq!(
        publisher.join().expect("publisher child thread").as_str(),
        origin
    );
    health.join().expect("health fixture thread reaped");
}
