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
#[derive(Debug)]
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
    Path::new(&format!("/proc/{pid}")).exists()
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
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let descriptor: BackendDescriptor = serde_json::from_slice(&bytes)
        .map_err(|error| DaemonError::Descriptor(error.to_string()))?;
    if descriptor.schema_version != opencode_rk_contracts::WIRE_SCHEMA_VERSION
        || !pid_alive(descriptor.pid)
        || !descriptor.http_origin.starts_with("http://127.0.0.1:")
    {
        return Ok(None);
    }
    Ok(Some(descriptor))
}

pub fn publish_backend_descriptor(
    data_dir: impl AsRef<Path>,
    address: SocketAddr,
) -> Result<BackendDescriptor> {
    let paths = DaemonPaths::for_data_dir(data_dir);
    if let Some(parent) = paths.descriptor.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let descriptor = BackendDescriptor {
        pid: std::process::id(),
        http_origin: format!("http://{address}"),
        schema_version: opencode_rk_contracts::WIRE_SCHEMA_VERSION,
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
