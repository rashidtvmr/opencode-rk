//! RED test: AUTHWEB-CLI-HANDOFF-RED
//!
//! Verifies that `oc2 web` (without `--no-open`) passes the bearer credential
//! to the browser as a URL fragment `#oc2-token=<64hex>` rather than the bare
//! origin.
//!
//! Source evidence:
//! - `crates/server/src/daemon.rs:376-416` `read_backend_descriptor`: reads
//!   `<data-dir>/runtime/backend.json` (schema_version 1, live pid, loopback
//!   origin, 64-hex auth_token).
//! - `crates/cli/src/main.rs:668-686` `web()`: reads descriptor, prints
//!   `descriptor.http_origin`, calls `open_web_browser(&descriptor.http_origin)`.
//! - `crates/cli/src/main.rs:761-789` `open_web_browser(origin)`: spawns
//!   `open`/`xdg-open` with `origin` as the sole arg -- does NOT append the
//!   `#oc2-token=<token>` fragment.  This is the RED gap.
//!
//! Compiles with only `std` (no external deps) so `rustc --test` works.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A disposable temp directory removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = std::process::id();
        let path = env::temp_dir().join(format!(
            "authweb-handoff-{prefix}-{id}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path { &self.0 }
}

impl Drop for TempDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

/// Locate the `oc2` binary via `OC2_BIN` env var.
fn oc2_binary() -> Option<PathBuf> {
    env::var_os("OC2_BIN").map(PathBuf::from)
}

/// Find a free loopback TCP port, returning `127.0.0.1:<port>`.
fn free_loopback_addr() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("127.0.0.1:{}", addr.port())
}

/// Write a `backend.json` descriptor with a fake 64-hex token (never real).
fn write_descriptor(home: &Path, origin: &str, token: &str) {
    let runtime = home.join("runtime");
    fs::create_dir_all(&runtime).unwrap();
    let path = runtime.join("backend.json");
    let mut f = fs::File::create(&path).unwrap();
    write!(
        f,
        r#"{{"pid":{},"http_origin":"{}","schema_version":1,"auth_token":"{}"}}"#,
        std::process::id(), origin, token,
    ).unwrap();
    f.sync_all().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms).unwrap();
    }
}

/// Poll a child for up to `timeout`; returns exit status if it exited.
fn wait_child(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(e) => panic!("try_wait failed: {e}"),
        }
        if Instant::now() >= deadline { return None; }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Read a piped stdio stream into a bounded String (max 64 KiB).
fn read_bounded<R: Read>(stream: &mut R) -> String {
    let mut buf = String::new();
    let _ = stream.take(65536).read_to_string(&mut buf);
    buf
}

/// `oc2 web` (without `--no-open`) must deliver the bearer credential as a URL
/// fragment `#oc2-token=<64hex>` to the browser launcher.
///
/// RED: `open_web_browser` (main.rs:761-789) passes only the bare `http_origin`
/// to `open`/`xdg-open` with no fragment. The browser probe therefore receives
/// `http://127.0.0.1:<port>` and NOT `http://127.0.0.1:<port>#oc2-token=<token>`.
#[test]
fn web_command_delivers_token_fragment_to_browser() {
    let oc2 = match oc2_binary() {
        Some(p) => p,
        None => panic!(
            "OC2_BIN env var not set; build `oc2` and set OC2_BIN=/path/to/oc2 \
             (compilation-only check passes without it, execution requires it)"
        ),
    };

    let home = TempDir::new("handoff-red");
    let origin = free_loopback_addr();
    let token = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    write_descriptor(home.path(), &format!("http://{origin}"), token);

    // Pick the right browser executable name for this platform.
    #[cfg(target_os = "macos")]
    let exe_name = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let exe_name = "xdg-open";
    #[cfg(not(unix))]
    let exe_name = "open";

    // Fake browser script that records its argv (the URL) to a probe file.
    let probe = home.path().join("probe_url.txt");
    let probe_str = probe.to_string_lossy().replace('\'', "'\\''");
    let script = home.path().join(exe_name);
    let content = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{probe_str}'\n",
    );
    let mut f = fs::File::create(&script).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.sync_all().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }

    // Path: probe dir first, then standard system dirs. No leading `:`.
    let new_path = format!("{}:/usr/bin:/bin", home.path().to_string_lossy());

    let mut child = Command::new(&oc2)
        .env_clear()
        .env("PATH", &new_path)
        .env("TMPDIR", env::temp_dir())
        .args(["--data-dir", home.path().to_str().unwrap()])
        .args(["web", "--listen", &origin])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn oc2 web: {e}"));

    // Wait up to 5s; kill+wait if still alive (the fast path exits quickly).
    let exited = wait_child(&mut child, Duration::from_secs(5));
    if exited.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }

    // Read bounded output (child must outlive stdout/stderr takes).
    let stdout = child.stdout.take()
        .map(|mut s| read_bounded(&mut s))
        .unwrap_or_default();
    let stderr = child.stderr.take()
        .map(|mut s| read_bounded(&mut s))
        .unwrap_or_default();

    // Assertion: bearer token must never leak to stdout/stderr.
    assert!(!stdout.contains(token), "token leaked to stdout");
    assert!(!stderr.contains(token), "token leaked to stderr");

    // Give the browser-launch path a moment to write the probe record.
    std::thread::sleep(Duration::from_millis(200));

    let probe_url = fs::read_to_string(&probe).ok();
    assert!(
        probe_url.is_some(),
        "browser probe was not invoked — open_web_browser (main.rs:761) \
         does not pass #oc2-token fragment to the browser"
    );
    let probe_url = probe_url.unwrap();
    let probe_url = probe_url.lines().next().unwrap_or("").trim().to_owned();

    let expected = format!("http://{origin}#oc2-token={token}");
    assert_eq!(
        probe_url, expected,
        "captured URL does not carry the credential fragment; \
         expected exactly `{expected}`, got `{probe_url}`"
    );
    assert!(
        !probe_url.contains('?'),
        "token must not appear in URL query string; got: {probe_url}"
    );
    assert_ne!(
        probe_url, format!("http://{origin}"),
        "probe received bare origin with no #oc2-token fragment — RED gap"
    );
}
