//! SDK-002: bounded SDK backend spawner with kill-on-drop reaping.
//!
//! Caller-owned [`Spawner`] starts Server/Process/Tui backends through a
//! caller-supplied [`SpawnPort`] under an 8-slot cap with a 30s startup
//! deadline. Every live child is owned by a kill-on-drop [`ChildHandle`]
//! that reaps exactly once via `wait`/`kill`/drop, so no zombies escape.
//!
//! Performs no direct `Command::spawn`, no socket I/O, no logging, and no
//! persistence. Startup deadlines are observed through the caller port;
//! this module only propagates them while killing and reaping the partial
//! child. Errors carry variant names only, never argv/dir bytes.

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

/// Maximum concurrent live children per spawner.
pub const MAX_SPAWNS: usize = 8;
/// Startup deadline (seconds) observed through the caller port.
pub const STARTUP_TIMEOUT_SECS: u64 = 30;

/// Maximum argv entries per spawn request.
const MAX_ARGV_ENTRIES: usize = 32;
/// Maximum chars per argv entry.
const MAX_ARG_CHARS: usize = 1024;
/// Maximum chars for the workspace directory.
const MAX_DIR_CHARS: usize = 1024;

/// Closed set of SDK backend kinds; every spawn names exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum BackendKind {
    Server = 0,
    Process = 1,
    Tui = 2,
}

/// Spawn request: argv only, never a shell line.
pub struct SpawnRequest {
    pub kind: BackendKind,
    pub argv: Vec<String>,
    pub dir: String,
}

impl Clone for SpawnRequest {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            argv: self.argv.clone(),
            dir: self.dir.clone(),
        }
    }
}

// Manual `Debug`: argv/dir may carry secrets; emit lengths only.
impl std::fmt::Debug for SpawnRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnRequest")
            .field("kind", &self.kind)
            .field("argv_len", &self.argv.len())
            .field("dir_len", &self.dir.chars().count())
            .finish_non_exhaustive()
    }
}

impl SpawnRequest {
    /// Build a spawn request; validated on [`Spawner::spawn`], not here.
    pub fn new(kind: BackendKind, argv: Vec<String>, dir: String) -> Self {
        Self { kind, argv, dir }
    }
}

/// Typed spawn failures. Variants carry no argv, dir, or env bytes by
/// construction, so `Debug`/`Display` output is secret-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    SpawnFull,
    StartupTimeout,
    InvalidInput,
    StartFailed,
    AlreadyReaped,
    UnknownChild,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnFull => write!(f, "spawn table full"),
            Self::StartupTimeout => write!(f, "startup timeout"),
            Self::InvalidInput => write!(f, "invalid input"),
            Self::StartFailed => write!(f, "backend start failed"),
            Self::AlreadyReaped => write!(f, "child already reaped"),
            Self::UnknownChild => write!(f, "unknown child"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Reaped exit status: numeric code only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    code: i32,
}

impl ExitStatus {
    /// Wrap a caller-port exit code.
    pub fn new(code: i32) -> Self {
        Self { code }
    }

    /// Raw exit code.
    #[must_use]
    pub fn code(&self) -> i32 {
        self.code
    }

    /// True for exit code 0.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.code == 0
    }
}

/// Caller-supplied spawn backend. Owns all OS handles; kills and reaps on
/// demand. `spawn`/`await_startup` failures report typed errors; `kill` and
/// `reap` are infallible by contract (fixture asserts counts instead).
pub trait SpawnPort {
    fn spawn(
        &self,
        id: u64,
        kind: BackendKind,
        argv: &[String],
        dir: &str,
    ) -> Result<(), SpawnError>;
    fn await_startup(&self, id: u64) -> Result<(), SpawnError>;
    fn kill(&self, id: u64);
    fn reap(&self, id: u64) -> ExitStatus;
}

/// Caller-owned live table shared with outstanding [`ChildHandle`]s so a
/// spawner drop can kill and reap every live child exactly once, and late
/// handle drops become no-ops instead of double reaps.
#[derive(Debug, Default)]
struct Shared {
    live: BTreeMap<u64, BackendKind>,
    reaped: HashSet<u64>,
    next_id: u64,
}

/// Bounded spawner over one caller-supplied port.
pub struct Spawner<'a> {
    port: &'a dyn SpawnPort,
    shared: Rc<RefCell<Shared>>,
}

// Manual `Debug`: never emits argv/dir bytes (the table holds kinds only).
impl<'a> std::fmt::Debug for Spawner<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spawner")
            .field("live", &self.shared.borrow().live.len())
            .finish_non_exhaustive()
    }
}

impl<'a> Spawner<'a> {
    /// Wrap the caller-supplied port. Allocates only the empty table.
    pub fn new(port: &'a dyn SpawnPort) -> Self {
        Self {
            port,
            shared: Rc::new(RefCell::new(Shared::default())),
        }
    }

    /// Number of currently live (unreaped) children, `0..=MAX_SPAWNS`.
    #[must_use]
    pub fn live_count(&self) -> usize {
        self.shared.borrow().live.len()
    }

    /// Spawn one backend. Validates before any port call; rejects past the
    /// cap without a port call; kills and reaps the partial child when
    /// startup misses the caller-port deadline.
    pub fn spawn(&mut self, req: SpawnRequest) -> Result<ChildHandle<'a>, SpawnError> {
        validate(&req)?;
        let id = {
            let mut shared = self.shared.borrow_mut();
            if shared.live.len() >= MAX_SPAWNS {
                return Err(SpawnError::SpawnFull);
            }
            shared.next_id += 1;
            shared.next_id
        };
        self.port.spawn(id, req.kind, &req.argv, &req.dir)?;
        if self.port.await_startup(id).is_err() {
            self.port.kill(id);
            self.port.reap(id);
            return Err(SpawnError::StartupTimeout);
        }
        self.shared.borrow_mut().live.insert(id, req.kind);
        Ok(ChildHandle {
            id,
            kind: req.kind,
            port: self.port,
            shared: Rc::clone(&self.shared),
        })
    }

    /// Reap a live child by id without its handle.
    pub fn wait_child(&mut self, id: u64) -> Result<ExitStatus, SpawnError> {
        let mut shared = self.shared.borrow_mut();
        if shared.live.remove(&id).is_some() {
            let status = self.port.reap(id);
            shared.reaped.insert(id);
            return Ok(status);
        }
        if shared.reaped.contains(&id) {
            return Err(SpawnError::AlreadyReaped);
        }
        Err(SpawnError::UnknownChild)
    }

    /// Terminate and reap a live child by id without its handle.
    pub fn kill_child(&mut self, id: u64) -> Result<ExitStatus, SpawnError> {
        let mut shared = self.shared.borrow_mut();
        if shared.live.remove(&id).is_some() {
            self.port.kill(id);
            let status = self.port.reap(id);
            shared.reaped.insert(id);
            return Ok(status);
        }
        if shared.reaped.contains(&id) {
            return Err(SpawnError::AlreadyReaped);
        }
        Err(SpawnError::UnknownChild)
    }
}

impl<'a> Drop for Spawner<'a> {
    /// Kill and reap every still-live child so none is orphaned or left
    /// as a zombie. Handles dropped later observe the reaped set and do
    /// nothing (no double reap).
    fn drop(&mut self) {
        let ids: Vec<u64> = self.shared.borrow().live.keys().copied().collect();
        for id in ids {
            self.port.kill(id);
            self.port.reap(id);
            let mut shared = self.shared.borrow_mut();
            shared.live.remove(&id);
            shared.reaped.insert(id);
        }
    }
}

/// Owned live-child handle. `wait`/`kill` reap exactly once; drop kills
/// and reaps when still live.
pub struct ChildHandle<'a> {
    pub id: u64,
    pub kind: BackendKind,
    port: &'a dyn SpawnPort,
    shared: Rc<RefCell<Shared>>,
}

// Manual `Debug`: ids and kinds only, never argv/dir bytes.
impl<'a> std::fmt::Debug for ChildHandle<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChildHandle")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl<'a> ChildHandle<'a> {
    /// Reap the child and report its exit status. Second call fails with
    /// `AlreadyReaped`; the table is unchanged by the failed call.
    pub fn wait(&mut self) -> Result<ExitStatus, SpawnError> {
        let mut shared = self.shared.borrow_mut();
        if shared.live.remove(&self.id).is_some() {
            let status = self.port.reap(self.id);
            shared.reaped.insert(self.id);
            return Ok(status);
        }
        Err(SpawnError::AlreadyReaped)
    }

    /// Terminate and reap the child. Second call fails with
    /// `AlreadyReaped`; the table is unchanged by the failed call.
    pub fn kill(&mut self) -> Result<ExitStatus, SpawnError> {
        let mut shared = self.shared.borrow_mut();
        if shared.live.remove(&self.id).is_some() {
            self.port.kill(self.id);
            let status = self.port.reap(self.id);
            shared.reaped.insert(self.id);
            return Ok(status);
        }
        Err(SpawnError::AlreadyReaped)
    }
}

impl<'a> Drop for ChildHandle<'a> {
    /// Kill-on-drop: terminate and reap when still live; no-op when the
    /// child was already reaped via `wait`/`kill` or a spawner drop.
    fn drop(&mut self) {
        let mut shared = self.shared.borrow_mut();
        if shared.live.remove(&self.id).is_some() {
            self.port.kill(self.id);
            self.port.reap(self.id);
            shared.reaped.insert(self.id);
        }
    }
}

/// argv 1..=32 entries of 1..=1024 chars with no NUL/control bytes;
/// dir 1..=1024 chars with no NUL/control bytes.
fn validate(req: &SpawnRequest) -> Result<(), SpawnError> {
    if req.argv.is_empty() || req.argv.len() > MAX_ARGV_ENTRIES {
        return Err(SpawnError::InvalidInput);
    }
    for arg in &req.argv {
        let len = arg.chars().count();
        if len == 0 || len > MAX_ARG_CHARS || arg.chars().any(|c| c == '\0' || c.is_control()) {
            return Err(SpawnError::InvalidInput);
        }
    }
    let dir_len = req.dir.chars().count();
    if dir_len == 0
        || dir_len > MAX_DIR_CHARS
        || req.dir.chars().any(|c| c == '\0' || c.is_control())
    {
        return Err(SpawnError::InvalidInput);
    }
    Ok(())
}
