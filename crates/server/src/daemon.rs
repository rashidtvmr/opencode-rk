//! Singleton daemon: PID-file lock plus Unix socket listener.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::Notify,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaemonError {
    AlreadyRunning(PathBuf),
    Io(String),
    Descriptor(String),
}
impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning(p) => {
                write!(f, "daemon already running (pid file {})", p.display())
            }
            Self::Io(e) => write!(f, "daemon io: {e}"),
            Self::Descriptor(e) => write!(f, "daemon descriptor: {e}"),
        }
    }
}
impl std::error::Error for DaemonError {}
impl From<std::io::Error> for DaemonError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, DaemonError>;
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }
}
pub struct PidLock {
    path: PathBuf,
    file: File,
}
impl PidLock {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(DaemonError::AlreadyRunning(path));
            }
            Err(error) => return Err(DaemonError::Io(error.to_string())),
        }
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(std::process::id().to_string().as_bytes())?;
        file.sync_data()?;
        Ok(Self { path, file })
    }
    #[must_use]
    pub fn is_held(path: impl AsRef<Path>) -> bool {
        let Ok(file) = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.as_ref())
        else {
            return false;
        };
        match file.try_lock_exclusive() {
            Ok(()) => {
                let _ = FileExt::unlock(&file);
                false
            }
            Err(error) => error.kind() == std::io::ErrorKind::WouldBlock,
        }
    }
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for PidLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BackendDescriptor {
    pub pid: u32,
    pub http_origin: String,
    pub schema_version: u16,
    /// Bearer token the live daemon expects on `/api/*`. Empty means a
    /// legacy descriptor: readers refuse it with an error, never as
    /// authenticated and never as silently absent.
    #[serde(default)]
    pub auth_token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonPaths {
    pub pid: PathBuf,
    pub socket: PathBuf,
    pub descriptor: PathBuf,
}

impl DaemonPaths {
    #[must_use]
    pub fn for_data_dir(data_dir: impl AsRef<Path>) -> Self {
        let runtime = data_dir.as_ref().join("runtime");
        Self {
            pid: runtime.join("opencode-rk.pid"),
            socket: runtime.join("opencode-rk.sock"),
            descriptor: runtime.join("backend.json"),
        }
    }
}

pub fn read_backend_descriptor(data_dir: impl AsRef<Path>) -> Result<Option<BackendDescriptor>> {
    let path = DaemonPaths::for_data_dir(data_dir).descriptor;
    // RC-02: bound resources and refuse path-swap/redirect attacks BEFORE
    // reading. symlink_metadata never follows links, so a symlink (even
    // dangling, which plain read maps to Ok(None)) is observable here.
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate_descriptor_file(&meta, &path)?;
    let bytes = std::fs::read(&path)?;
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DaemonError::Descriptor(format!(
            "descriptor too large: {} bytes (max {MAX_DESCRIPTOR_BYTES})",
            bytes.len()
        )));
    }
    let descriptor: BackendDescriptor = serde_json::from_slice(&bytes)
        .map_err(|error| DaemonError::Descriptor(error.to_string()))?;
    if descriptor.schema_version != opencode_rk_contracts::WIRE_SCHEMA_VERSION
        || !pid_alive(descriptor.pid)
        || parse_loopback_port(&descriptor.http_origin).is_none()
    {
        return Ok(None);
    }
    // A live, well-formed endpoint without a usable bearer must never look
    // absent: Ok(None) would let callers spawn a second daemon or report no
    // daemon while a live one runs. It must not look ready either: a bad
    // bearer would attach then fail at `/api/*`. Refuse loudly instead.
    if descriptor.auth_token.is_empty() {
        return Err(DaemonError::Descriptor(
            "descriptor predates bearer auth (empty auth_token); refusing to attach".to_owned(),
        ));
    }
    if !is_wellformed_token(&descriptor.auth_token) {
        return Err(DaemonError::Descriptor(
            "descriptor auth_token is malformed (want 64 hex chars); refusing to attach".to_owned(),
        ));
    }
    Ok(Some(descriptor))
}

/// True only for a 64-char hex bearer (mirrors
/// `daemon_auth::from_published`: empty/short/non-hex never authenticates).
fn is_wellformed_token(token: &str) -> bool {
    token.len() == crate::daemon_auth::TOKEN_HEX_LEN
        && token.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Byte budget for `backend.json`. Anything larger is forged/broken and is
/// refused before (and after) the read, never buffered unbounded.
pub const MAX_DESCRIPTOR_BYTES: usize = 8 * 1024;

/// Fail-closed pre-read gate for the descriptor file: size cap, symlink
/// refusal, and owner check against the caller's euid.
fn validate_descriptor_file(meta: &std::fs::Metadata, path: &Path) -> Result<()> {
    if meta.len() > MAX_DESCRIPTOR_BYTES as u64 {
        return Err(DaemonError::Descriptor(format!(
            "descriptor too large: {} bytes (max {MAX_DESCRIPTOR_BYTES})",
            meta.len()
        )));
    }
    if meta.file_type().is_symlink() {
        return Err(DaemonError::Descriptor(
            "descriptor is a symlink; refusing to follow".to_owned(),
        ));
    }
    check_owner_uid(owner_uid(meta), current_uid()?, path)
}

/// Owner of the descriptor file (Unix UID). std-only via MetadataExt.
#[cfg(unix)]
fn owner_uid(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.uid()
}

/// Effective UID of this process. Linux exposes it in `/proc`; Darwin has no
/// `/proc`, so use the platform `id -u` utility without a shell. Unparseable
/// output fails closed.
#[cfg(unix)]
fn current_uid() -> Result<u32> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").map_err(DaemonError::from)?;
        let field = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:").and_then(|rest| rest.split_whitespace().nth(1)))
            .ok_or_else(|| DaemonError::Descriptor("no Uid line in /proc/self/status".to_owned()))?;
        return field.parse::<u32>().map_err(|_| {
            DaemonError::Descriptor("cannot parse euid from /proc/self/status".to_owned())
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let output = std::process::Command::new("/usr/bin/id")
            .arg("-u")
            .output()
            .map_err(DaemonError::from)?;
        if !output.status.success() {
            return Err(DaemonError::Descriptor("id -u failed".to_owned()));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| DaemonError::Descriptor("id -u returned non-UTF-8 output".to_owned()))?;
        text.trim().parse::<u32>().map_err(|_| {
            DaemonError::Descriptor("cannot parse effective uid from id -u".to_owned())
        })
    }
}

/// Pure owner comparison, unit-testable without filesystem privileges.
fn check_owner_uid(owner: u32, caller: u32, path: &Path) -> Result<()> {
    if owner != caller {
        return Err(DaemonError::Descriptor(format!(
            "descriptor {} owned by uid {owner}, caller is uid {caller}; refusing",
            path.display()
        )));
    }
    Ok(())
}

/// Accept exactly `http://127.0.0.1:<port>` with numeric port 1..=65535.
/// Rejects prefix-smuggled hosts (`...:4096.evil.com`), trailing paths,
/// `localhost`/`0.0.0.0`/LAN IPs, `https`, missing/zero/out-of-range ports,
/// and leading-zero padding.
fn parse_loopback_port(origin: &str) -> Option<u16> {
    let rest = origin.strip_prefix("http://127.0.0.1:")?;
    if rest.is_empty() || rest.len() > 5 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if rest.len() > 1 && rest.starts_with('0') {
        return None;
    }
    let port: u32 = rest.parse().ok()?;
    if port == 0 || port > u16::MAX as u32 {
        return None;
    }
    Some(port as u16)
}

pub fn publish_backend_descriptor(
    data_dir: impl AsRef<Path>,
    address: SocketAddr,
) -> Result<BackendDescriptor> {
    publish_backend_descriptor_with_auth(data_dir, address, String::new())
}

/// Publish the descriptor with the daemon's bearer token. Empty token marks
/// a legacy descriptor: `DaemonAuth::from_published` rejects it, so readers
/// treat it as stale.
pub fn publish_backend_descriptor_with_auth(
    data_dir: impl AsRef<Path>,
    address: SocketAddr,
    auth_token: String,
) -> Result<BackendDescriptor> {
    let paths = DaemonPaths::for_data_dir(data_dir);
    if let Some(parent) = paths.descriptor.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let descriptor = BackendDescriptor {
        pid: std::process::id(),
        http_origin: format!("http://{address}"),
        schema_version: opencode_rk_contracts::WIRE_SCHEMA_VERSION,
        auth_token,
    };
    let bytes = serde_json::to_vec(&descriptor)
        .map_err(|error| DaemonError::Descriptor(error.to_string()))?;
    let temporary = paths
        .descriptor
        .with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, &paths.descriptor)?;
    Ok(descriptor)
}
pub struct ClientConnection {
    pub id: u64,
    pub stream: UnixStream,
}
// ponytail: local flag+Notify shutdown instead of tokio-util CancellationToken (server Cargo.toml lacks tokio-util, no new deps allowed); swap type when dep approved.
struct ShutdownState {
    flag: AtomicBool,
    notify: Notify,
}
pub struct SingletonDaemon {
    _lock: PidLock,
    socket_path: PathBuf,
    listener: Arc<UnixListener>,
    shutdown: Arc<ShutdownState>,
    clients: Arc<AtomicUsize>,
    next_id: AtomicU64,
}
impl SingletonDaemon {
    pub fn bind(socket_path: impl AsRef<Path>, pid_path: impl AsRef<Path>) -> Result<Self> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let lock = PidLock::acquire(pid_path)?;
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let listener = UnixListener::bind(&socket_path)?;
        Ok(Self {
            _lock: lock,
            socket_path,
            listener: Arc::new(listener),
            shutdown: Arc::new(ShutdownState {
                flag: AtomicBool::new(false),
                notify: Notify::new(),
            }),
            clients: Arc::new(AtomicUsize::new(0)),
            next_id: AtomicU64::new(1),
        })
    }
    pub async fn accept_clients(&self) {
        loop {
            tokio::select! {biased;_ = self.shutdown.notify.notified() => break,res=self.listener.accept()=>{match res{Ok((stream,_))=>{let id=self.next_id.fetch_add(1,Ordering::SeqCst);self.clients.fetch_add(1,Ordering::SeqCst);let clients=Arc::clone(&self.clients);tokio::spawn(async move{hold_client(ClientConnection{id,stream}).await;clients.fetch_sub(1,Ordering::SeqCst);});}Err(_)=>{if self.is_shutdown(){break;}}}}}
        }
    }
    pub fn shutdown(&self) {
        self.shutdown.flag.store(true, Ordering::SeqCst);
        self.shutdown.notify.notify_waiters();
    }
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.flag.load(Ordering::SeqCst)
    }
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.load(Ordering::SeqCst)
    }
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}
impl Drop for SingletonDaemon {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
async fn hold_client(conn: ClientConnection) {
    let ClientConnection { stream: ref s, .. } = conn;
    let mut buf = [0u8; 512];
    loop {
        match s.readable().await {
            Ok(()) => match s.try_read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            },
            Err(_) => break,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::net::UnixStream as ClientStream;
    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);
    fn test_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "rk-daemon-{name}-{}-{}",
            std::process::id(),
            TEST_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::create_dir_all(&p);
        p
    }
    async fn wait_for(cond: impl Fn() -> bool) -> bool {
        for _ in 0..100 {
            if cond() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        cond()
    }
    #[test]
    fn pid_lock_acquire_release() {
        let d = test_dir("acq");
        let p = d.join("rk.pid");
        {
            let lock = PidLock::acquire(&p).unwrap();
            assert!(PidLock::is_held(&p));
            assert_eq!(lock.path(), p);
        }
        assert!(!PidLock::is_held(&p));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn pid_lock_double_acquire_fails() {
        let d = test_dir("double");
        let p = d.join("rk.pid");
        let _first = PidLock::acquire(&p).unwrap();
        assert!(matches!(
            PidLock::acquire(&p),
            Err(DaemonError::AlreadyRunning(_))
        ));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn pid_lock_stale_reclaim() {
        let d = test_dir("stale");
        let p = d.join("rk.pid");
        std::fs::write(&p, u32::MAX.to_string()).unwrap();
        assert!(!PidLock::is_held(&p));
        let lock = PidLock::acquire(&p).unwrap();
        assert!(PidLock::is_held(&p));
        drop(lock);
        assert!(!PidLock::is_held(&p));
        let _ = std::fs::remove_dir_all(&d);
    }
    // --- RC-02 hardened descriptor validation (frozen RED) ---
    fn rc02_write(name: &str, bytes: &[u8]) -> PathBuf {
        let d = test_dir(name);
        let runtime = d.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::write(runtime.join("backend.json"), bytes).unwrap();
        d
    }
    fn rc02_valid_json(pid: u32, origin: &str) -> Vec<u8> {
        format!(r#"{{"pid":{pid},"http_origin":{origin:?},"schema_version":1}}"#).into_bytes()
    }
    #[test]
    fn rc02_spoofed_origin_ignored() {
        let pid = std::process::id();
        for origin in [
            "http://127.0.0.1:4096.evil.example",
            "http://127.0.0.1:4096/health",
            "http://127.0.0.1:4096/",
            "http://127.0.0.1:0",
            "http://127.0.0.1:99999",
            "http://127.0.0.1:04096",
            "http://localhost:4096",
            "https://127.0.0.1:4096",
        ] {
            let d = rc02_write("spoof", &rc02_valid_json(pid, origin));
            let got = read_backend_descriptor(&d);
            assert!(
                matches!(got, Ok(None)),
                "spoofed origin must be ignored: {origin:?} got {got:?}"
            );
            let _ = std::fs::remove_dir_all(&d);
        }
    }
    #[test]
    fn rc02_symlink_refused() {
        let pid = std::process::id();
        let d = test_dir("symlink");
        let runtime = d.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let target = runtime.join("real.json");
        std::fs::write(&target, rc02_valid_json(pid, "http://127.0.0.1:4096")).unwrap();
        std::os::unix::fs::symlink(&target, runtime.join("backend.json")).unwrap();
        let got = read_backend_descriptor(&d);
        assert!(
            matches!(got, Err(DaemonError::Descriptor(_))),
            "symlink descriptor must be refused pre-read, got {got:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn rc02_dangling_symlink_refused() {
        let d = test_dir("dangling");
        let runtime = d.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        std::os::unix::fs::symlink(runtime.join("nope.json"), runtime.join("backend.json"))
            .unwrap();
        let got = read_backend_descriptor(&d);
        assert!(
            matches!(got, Err(DaemonError::Descriptor(_))),
            "dangling symlink must be refused pre-read, got {got:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn rc02_oversized_refused() {
        let pid = std::process::id();
        let mut bytes = rc02_valid_json(pid, "http://127.0.0.1:4096");
        bytes.resize(8 * 1024 + 1, b' ');
        let d = rc02_write("oversize", &bytes);
        let got = read_backend_descriptor(&d);
        assert!(
            matches!(got, Err(DaemonError::Descriptor(_))),
            "oversized descriptor must be refused pre-read, got {got:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn rc02_occupied_port_never_kills() {
        use std::process::Command;
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        let d = rc02_write("occupied", &rc02_valid_json(pid, "http://10.0.0.9:4096"));
        let got = read_backend_descriptor(&d);
        assert!(
            matches!(got, Ok(None)),
            "forged descriptor must be ignored, got {got:?}"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "reader must never kill the described process"
        );
        child.kill().unwrap();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn rc02_foreign_owner_refused() {
        let d = test_dir("owner");
        assert_eq!(
            check_owner_uid(0, 1000, &d),
            Err(DaemonError::Descriptor(format!(
                "descriptor {} owned by uid 0, caller is uid 1000; refusing",
                d.display()
            ))),
        );
        assert_eq!(check_owner_uid(1000, 1000, &d), Ok(()));
        let _ = std::fs::remove_dir_all(&d);
    }
    // --- LANE-DESC-STALE: live descriptor without a usable bearer is Err ---
    fn stale_write_token(name: &str, pid: u32, origin: &str, token: &str) -> PathBuf {
        let d = test_dir(name);
        let runtime = d.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let body = format!(
            r#"{{"pid":{pid},"http_origin":{origin:?},"schema_version":{},"auth_token":{token:?}}}"#,
            opencode_rk_contracts::WIRE_SCHEMA_VERSION
        );
        std::fs::write(runtime.join("backend.json"), body).unwrap();
        d
    }
    #[test]
    fn desc_stale_empty_token_errors() {
        let d = stale_write_token("emptytok", std::process::id(), "http://127.0.0.1:4096", "");
        let got = read_backend_descriptor(&d);
        assert!(
            matches!(got, Err(DaemonError::Descriptor(_))),
            "live descriptor with empty token must error, got {got:?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn desc_stale_malformed_token_errors() {
        for token in [String::from("short"), "zz".repeat(32), "0".repeat(63)] {
            let d = stale_write_token(
                "badtok",
                std::process::id(),
                "http://127.0.0.1:4096",
                &token,
            );
            let got = read_backend_descriptor(&d);
            assert!(
                matches!(got, Err(DaemonError::Descriptor(_))),
                "malformed token must error, got {got:?}"
            );
            let _ = std::fs::remove_dir_all(&d);
        }
    }
    #[test]
    fn desc_stale_valid_token_attaches() {
        let token = "ab".repeat(32);
        let d = stale_write_token(
            "goodtok",
            std::process::id(),
            "http://127.0.0.1:4096",
            &token,
        );
        let got = read_backend_descriptor(&d).expect("read");
        assert_eq!(got.map(|desc| desc.auth_token), Some(token));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn rc02_current_uid_matches_filesystem() {
        let d = test_dir("selfuid");
        std::fs::write(d.join("probe"), b"x").unwrap();
        let meta = std::fs::symlink_metadata(d.join("probe")).unwrap();
        assert_eq!(owner_uid(&meta), current_uid().unwrap());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[tokio::test]
    async fn daemon_accept_client() {
        let d = test_dir("accept");
        let sock = d.join("rk.sock");
        let daemon = Arc::new(SingletonDaemon::bind(&sock, d.join("rk.pid")).unwrap());
        let worker = Arc::clone(&daemon);
        let handle = tokio::spawn(async move {
            worker.accept_clients().await;
        });
        let client = ClientStream::connect(&sock).await.unwrap();
        assert!(wait_for(|| daemon.client_count() == 1).await);
        drop(client);
        assert!(wait_for(|| daemon.client_count() == 0).await);
        daemon.shutdown();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
    #[tokio::test]
    async fn daemon_shutdown_closes() {
        let d = test_dir("shutdown");
        let daemon = Arc::new(SingletonDaemon::bind(d.join("rk.sock"), d.join("rk.pid")).unwrap());
        let worker = Arc::clone(&daemon);
        let handle = tokio::spawn(async move {
            worker.accept_clients().await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        daemon.shutdown();
        assert!(daemon.is_shutdown());
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("accept loop did not end")
            .unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }
}
