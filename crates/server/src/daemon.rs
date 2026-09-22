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
            socket: short_socket_path(data_dir.as_ref()),
            descriptor: runtime.join("backend.json"),
        }
    }
}

/// Darwin `sockaddr_un.sun_path` fits 104 bytes including its NUL, so a
/// portable socket pathname must stay below 104 bytes. The socket therefore
/// lives under a short per-user runtime root, never under the data dir.
pub const MAX_SOCKET_PATH_BYTES: usize = 104;

/// Short per-user runtime root for the socket: a fixed system directory,
/// never `TMPDIR` (untrusted environment) and never the data dir
/// (unbounded length). The per-user leaf is created 0700 by `bind`.
#[cfg(target_os = "macos")]
fn short_root() -> PathBuf {
    PathBuf::from("/private/tmp")
}
#[cfg(all(unix, not(target_os = "macos")))]
fn short_root() -> PathBuf {
    PathBuf::from("/tmp")
}
#[cfg(not(unix))]
fn short_root() -> PathBuf {
    std::env::temp_dir()
}

/// Deterministic socket path for a data directory:
/// `<root>/rk-<uid>/rk-<hex>.sock`. The hex digest is the first 128 bits of
/// SHA-256 over the caller euid, a separator, and the lexically normalized
/// path. Equivalent spellings map together; distinct paths are cryptographically
/// collision-resistant within the remaining filesystem and euid trust boundary.
fn short_socket_path(data_dir: &Path) -> PathBuf {
    let mut key = uid_tag().into_bytes();
    key.push(0xff);
    key.extend_from_slice(&normalized_key(data_dir));
    let digest = sha256(&key);
    let name = format!("rk-{}.sock", hex_lower(&digest[..16]));
    let path = short_root().join(format!("rk-{}", uid_tag())).join(name);
    debug_assert!(path.as_os_str().as_encoded_bytes().len() < MAX_SOCKET_PATH_BYTES);
    path
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// FIPS 180-4 SHA-256, kept local to avoid a dependency for this small digest.
fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut data = input.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = (*slot).wrapping_add(value);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.into_iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Euid tag for per-user isolation of the socket root. `current_uid` only
/// fails when the platform cannot report an euid at all; the digest still
/// isolates data directories in that case.
fn uid_tag() -> String {
    current_uid()
        .map(|uid| uid.to_string())
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// Lexically normalized path bytes: `.` skipped, `..` pops, duplicate
/// separators collapsed by `components`, so logically equivalent spellings
/// share one digest. Symlinks are not resolved here; callers pass the
/// canonical directory when identity must survive links.
fn normalized_key(data_dir: &Path) -> Vec<u8> {
    use std::path::Component;
    let mut root: Vec<u8> = Vec::new();
    let mut parts: Vec<&[u8]> = Vec::new();
    for component in data_dir.components() {
        match component {
            Component::Prefix(prefix) => {
                root.extend_from_slice(prefix.as_os_str().as_encoded_bytes());
            }
            Component::RootDir => root.push(b'/'),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part.as_encoded_bytes()),
        }
    }
    let mut out = Vec::with_capacity(root.len() + parts.len() * 2 + 16);
    out.extend_from_slice(&root);
    for part in parts {
        out.push(b'/');
        out.extend_from_slice(part);
    }
    out
}

/// Fail-closed gate for the socket parent, run inside `bind` after the PID
/// lock is held: the parent must be a real directory (never a symlink) and,
/// on unix, owned by the caller euid. Missing parents are created with
/// private 0700 mode; a managed `rk-<uid>` leaf that lost its private mode
/// is refused. Pre-existing unrelated directories keep their mode.
fn ensure_socket_parent(socket_path: &Path) -> Result<()> {
    let Some(parent) = socket_path.parent() else {
        return Err(DaemonError::Io(
            "socket path has no parent directory".to_owned(),
        ));
    };
    if parent.as_os_str().is_empty() {
        return Err(DaemonError::Io(
            "socket path has no parent directory".to_owned(),
        ));
    }
    match std::fs::symlink_metadata(parent) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Err(error) => return Err(error.into()),
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(DaemonError::Io(format!(
                    "socket parent {} is a symlink; refusing",
                    parent.display()
                )));
            }
            if !meta.file_type().is_dir() {
                return Err(DaemonError::Io(format!(
                    "socket parent {} is not a directory; refusing",
                    parent.display()
                )));
            }
        }
    }
    #[cfg(unix)]
    {
        let meta = std::fs::symlink_metadata(parent)?;
        let caller = current_uid()?;
        if owner_uid(&meta) != caller {
            return Err(DaemonError::Io(format!(
                "socket parent {} owned by uid {}, caller is uid {caller}; refusing",
                parent.display(),
                owner_uid(&meta)
            )));
        }
        if is_managed_leaf(parent) && !mode_is_private(&meta) {
            return Err(DaemonError::Io(format!(
                "socket parent {} lost private mode; refusing",
                parent.display()
            )));
        }
    }
    Ok(())
}

/// True when `parent` is the `rk-<uid>` leaf directly under our short root.
#[cfg(unix)]
fn is_managed_leaf(parent: &Path) -> bool {
    parent.parent().is_some_and(|grand| grand == short_root())
        && parent
            .file_name()
            .is_some_and(|leaf| leaf.as_encoded_bytes().starts_with(b"rk-"))
}

/// True when no group/other permission bit is set (0700 satisfies this).
#[cfg(unix)]
fn mode_is_private(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o077 == 0
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
    token.len() == crate::daemon_auth::TOKEN_HEX_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
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
/// output fails closed. Non-unix targets have no euid: fail closed and let
/// callers fall back to the shared tag.
#[cfg(not(unix))]
fn current_uid() -> Result<u32> {
    Err(DaemonError::Descriptor(
        "no effective uid on this platform".to_owned(),
    ))
}
#[cfg(unix)]
fn current_uid() -> Result<u32> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").map_err(DaemonError::from)?;
        let field = status
            .lines()
            .find_map(|line| {
                line.strip_prefix("Uid:")
                    .and_then(|rest| rest.split_whitespace().nth(1))
            })
            .ok_or_else(|| {
                DaemonError::Descriptor("no Uid line in /proc/self/status".to_owned())
            })?;
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

/// Publish the descriptor with a freshly minted bearer credential.
///
/// Secure-by-default: mints a real `DaemonAuth` credential via the existing
/// same-crate API and delegates to `publish_backend_descriptor_with_auth`.
/// Mint failure maps to `DaemonError::Descriptor` without leaking entropy
/// source details. The returned descriptor is immediately consumable by
/// `read_backend_descriptor`; legacy empty-token descriptors read from disk
/// are still refused by the reader.
pub fn publish_backend_descriptor(
    data_dir: impl AsRef<Path>,
    address: SocketAddr,
) -> Result<BackendDescriptor> {
    let credential = crate::daemon_auth::DaemonAuth::mint().map_err(|_| {
        DaemonError::Descriptor("cannot mint daemon bearer credential".to_owned())
    })?;
    publish_backend_descriptor_with_auth(data_dir, address, credential.token().to_owned())
}

/// Publish the descriptor with the daemon's bearer token. Callers that
/// already hold their minted credential pass it here. Empty or malformed
/// tokens still publish a descriptor that authenticated readers refuse, so
/// they must never be used for fresh publication; use
/// `publish_backend_descriptor` (which mints) instead.
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
        // Lock authority first: the PID lock decides who may delete/rebind
        // the socket. A live owner's socket is never removed; `acquire`
        // fails with `AlreadyRunning` before any filesystem mutation here.
        let lock = PidLock::acquire(pid_path)?;
        // Fail-closed gate on the short socket parent (directory, not a
        // symlink, owned by this euid, private mode on our managed leaf).
        // Never `TMPDIR`: the path above is derived, not environmental.
        ensure_socket_parent(&socket_path)?;
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
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
            tokio::select! {
                biased;
                _ = self.shutdown.notify.notified() => break,
                res = self.listener.accept() => {
                    match res {
                        Ok((stream, _)) => {
                            let id = self.next_id.fetch_add(1, Ordering::SeqCst);
                            self.clients.fetch_add(1, Ordering::SeqCst);
                            let clients = Arc::clone(&self.clients);
                            tokio::spawn(async move {
                                hold_client(ClientConnection { id, stream }).await;
                                clients.fetch_sub(1, Ordering::SeqCst);
                            });
                        }
                        Err(_) if self.is_shutdown() => break,
                        Err(_) => {}
                    }
                }
            }
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
    #[test]
    fn sha256_known_vector_abc() {
        assert_eq!(
            hex_lower(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn socket_paths_isolated_and_bounded() {
        // BASE-004: socket derivation is deterministic, bounded, isolated.
        // These unit tests cover the private helpers; the frozen
        // `daemon_long_path` integration test owns the acceptance contract.
        let a = std::path::PathBuf::from("/tmp/data/profile-a");
        let first = short_socket_path(&a);
        assert_eq!(short_socket_path(&a), first);
        assert_eq!(
            short_socket_path(&std::path::PathBuf::from("/tmp/data/./profile-a")),
            first
        );
        let other = short_socket_path(&std::path::PathBuf::from("/tmp/data/profile-b"));
        assert_ne!(first, other);
        let len = if cfg!(unix) {
            short_socket_path(&std::path::PathBuf::from(
                "/tmp/data-with-a-deliberately-very-long-name-0123456789/Library/Application Support/OpenCode RK/profile-a",
            ))
        } else {
            first.clone()
        }
        .as_os_str()
        .len();
        assert!(
            !cfg!(unix) || len < MAX_SOCKET_PATH_BYTES,
            "short socket must stay below Darwin SUN_LEN: {len}"
        );
        let paths = DaemonPaths::for_data_dir(&a);
        assert!(paths.pid.starts_with(&a));
        assert!(paths.descriptor.starts_with(&a));
        assert!(!paths.socket.starts_with(&a));
    }
    #[test]
    fn socket_parent_gate_refuses_symlink_not_dir() {
        let base = test_dir("sockgate");
        let target = base.join("real");
        std::fs::create_dir_all(&target).unwrap();
        let link = base.join("linkdir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(not(unix))]
        std::fs::create_dir_all(&link).unwrap();
        assert!(
            ensure_socket_parent(&link.join("rk.sock")).is_err(),
            "symlink socket parent must be refused"
        );
        let file = base.join("afile");
        std::fs::write(&file, b"x").unwrap();
        assert!(
            ensure_socket_parent(&file.join("rk.sock")).is_err(),
            "non-directory socket parent must be refused"
        );
        let _ = std::fs::remove_dir_all(&base);
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
