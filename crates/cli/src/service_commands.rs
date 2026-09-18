#![forbid(unsafe_code)]
//! Service control types (APP-009 slice).
//!
//! Pure state only: no syscalls, no threads, no clock, no IO. The native host
//! owns real FDs/watchdogs/listeners; these types record the control contract
//! so misuse fails loudly and every resource stays bounded.
//!
//! Invariants:
//! - [`ServiceController::stop`] with [`StopPolicy::Drain`] completes pending
//!   work up to the policy bound and reports the remainder as cancelled; with
//!   [`StopPolicy::Cancel`] it completes nothing and reports all as cancelled.
//! - Detaching one client never kills the daemon or its owned session; only an
//!   explicit [`ServiceAction::Stop`] through the controller stops the daemon.
//! - [`ResourceLedger`] pairs every acquire with a release; [`assert_no_leak`]
//!   diffs a snapshot pair so repeated start/stop cycles must return to
//!   baseline (no leaked FDs, watchdogs, or port reservations).

use std::collections::{HashMap, VecDeque};
use std::fmt;

/// Max pending work items retained by [`ServiceController`] (bounded queue).
pub const MAX_PENDING_WORK: usize = 1024;
/// Default drain bound when the caller wants "drain everything currently held".
pub const DRAIN_ALL: usize = MAX_PENDING_WORK;

/// Service control verbs (APP-009 journey entrypoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceAction {
    Status,
    Start,
    Stop,
    Restart,
}

impl ServiceAction {
    /// Parse a CLI verb; `None` for unknown input (caller surfaces usage).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "status" => Some(Self::Status),
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "restart" => Some(Self::Restart),
            _ => None,
        }
    }

    /// Canonical CLI spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

/// Explicit-stop work policy: drain in-flight work or cancel it outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopPolicy {
    /// Complete up to `max_items` pending items, report rest as cancelled.
    Drain { max_items: usize },
    /// Complete nothing, report all pending as cancelled.
    Cancel,
}

impl StopPolicy {
    /// `Drain { max_items: DRAIN_ALL }` shorthand.
    pub fn drain_all() -> Self {
        Self::Drain { max_items: DRAIN_ALL }
    }

    /// True for [`StopPolicy::Drain`].
    pub fn wants_drain(self) -> bool {
        matches!(self, Self::Drain { .. })
    }
}

/// Outcome receipt for an explicit service stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReceipt {
    /// IDs completed under a drain policy, in submission order.
    pub drained: Vec<u64>,
    /// IDs dropped without running (cancel policy + drain overflow).
    pub cancelled: Vec<u64>,
    /// Policy that produced this receipt.
    pub policy: StopPolicy,
    /// Always false after an explicit stop: the daemon is down.
    pub daemon_running: bool,
}

/// Receipt for closing one TUI client: proves the daemon survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseReceipt {
    pub closed_client: u64,
    pub remaining_clients: usize,
    pub daemon_running: bool,
    pub session_preserved: bool,
}

/// Control errors. No side effects accompany any of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceError {
    AlreadyRunning,
    NotRunning,
    QueueFull,
    UnknownClient(u64),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "service already running"),
            Self::NotRunning => write!(f, "service not running"),
            Self::QueueFull => write!(f, "pending work queue full"),
            Self::UnknownClient(id) => write!(f, "unknown client {id}"),
        }
    }
}

impl std::error::Error for ServiceError {}

/// Owns daemon lifetime + bounded pending-work queue.
#[derive(Debug)]
pub struct ServiceController {
    queue: VecDeque<u64>,
    completed: Vec<u64>,
    running: bool,
}

impl Default for ServiceController {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceController {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            completed: Vec::new(),
            running: false,
        }
    }

    /// Start the daemon. `Err(AlreadyRunning)` if live; no state change.
    pub fn start(&mut self) -> Result<(), ServiceError> {
        if self.running {
            return Err(ServiceError::AlreadyRunning);
        }
        self.running = true;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Enqueue one work item. `Err(NotRunning)` when down, `Err(QueueFull)`
    /// past [`MAX_PENDING_WORK`]; neither mutates the queue.
    pub fn submit(&mut self, id: u64) -> Result<(), ServiceError> {
        if !self.running {
            return Err(ServiceError::NotRunning);
        }
        if self.queue.len() >= MAX_PENDING_WORK {
            return Err(ServiceError::QueueFull);
        }
        self.queue.push_back(id);
        Ok(())
    }

    /// Explicit stop: apply `policy`, shut the daemon down, return receipt.
    pub fn stop(&mut self, policy: StopPolicy) -> StopReceipt {
        let (drained, cancelled) = match policy {
            StopPolicy::Drain { max_items } => {
                let n = max_items.min(self.queue.len());
                let drained: Vec<u64> = self.queue.drain(..n).collect();
                let cancelled: Vec<u64> = self.queue.drain(..).collect();
                (drained, cancelled)
            }
            StopPolicy::Cancel => (Vec::new(), self.queue.drain(..).collect()),
        };
        self.completed.extend_from_slice(&drained);
        // ponytail: no wall-clock timeout on drain; add when stuck-work SLO exists.
        self.running = false;
        StopReceipt {
            drained,
            cancelled,
            policy,
            daemon_running: false,
        }
    }

    /// IDs completed by drain stops, in completion order.
    pub fn completed(&self) -> &[u64] {
        &self.completed
    }
}

/// Tracks attached TUI clients plus the daemon-owned session. Closing clients
/// never stops the daemon; only [`ServiceController::stop`] does.
#[derive(Debug)]
pub struct ClientRegistry {
    clients: HashMap<u64, ()>,
    daemon_running: bool,
    session_live: bool,
}

impl ClientRegistry {
    /// Fresh daemon: running, owned session live, zero clients attached.
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            daemon_running: true,
            session_live: true,
        }
    }

    /// Attach a client. Idempotent: re-attaching returns the current count.
    pub fn attach(&mut self, id: u64) -> usize {
        self.clients.insert(id, ());
        self.clients.len()
    }

    /// Detach one client. `Err(UnknownClient)` leaves all state untouched.
    /// The daemon and its session survive regardless of remaining count.
    pub fn detach(&mut self, id: u64) -> Result<CloseReceipt, ServiceError> {
        if self.clients.remove(&id).is_none() {
            return Err(ServiceError::UnknownClient(id));
        }
        // Detach never touches daemon/session ownership: only an explicit
        // ServiceController::stop brings the daemon down.
        Ok(CloseReceipt {
            closed_client: id,
            remaining_clients: self.clients.len(),
            daemon_running: self.daemon_running,
            session_preserved: self.session_live,
        })
    }

    pub fn remaining(&self) -> usize {
        self.clients.len()
    }

    pub fn daemon_running(&self) -> bool {
        self.daemon_running
    }

    pub fn session_live(&self) -> bool {
        self.session_live
    }
}

/// Point-in-time resource counts (FDs, watchdog handles, port reservations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResourceCounts {
    pub fds: usize,
    pub watchdogs: usize,
    pub ports: usize,
}

impl ResourceCounts {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn total(self) -> usize {
        self.fds + self.watchdogs + self.ports
    }
}

/// Mismatch report from [`assert_no_leak`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeakReport {
    pub baseline: ResourceCounts,
    pub current: ResourceCounts,
}

impl fmt::Display for LeakReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "resource leak: fds {}->{} watchdogs {}->{} ports {}->{}",
            self.baseline.fds,
            self.current.fds,
            self.baseline.watchdogs,
            self.current.watchdogs,
            self.baseline.ports,
            self.current.ports,
        )
    }
}

impl std::error::Error for LeakReport {}

/// No-leak assertion helper: `Ok` iff every counter returned to baseline.
pub fn assert_no_leak(
    baseline: &ResourceCounts,
    current: &ResourceCounts,
) -> Result<(), LeakReport> {
    if baseline == current {
        Ok(())
    } else {
        Err(LeakReport {
            baseline: *baseline,
            current: *current,
        })
    }
}

/// Test/ledger stand-in for host resources: every acquire must pair with a
/// release, mirroring the host's FD/watchdog/listener bookkeeping.
#[derive(Debug, Default)]
pub struct ResourceLedger {
    counts: ResourceCounts,
}

impl ResourceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn acquire_fd(&mut self) {
        self.counts.fds += 1;
    }

    pub fn release_fd(&mut self) {
        self.counts.fds = self.counts.fds.saturating_sub(1);
    }

    pub fn acquire_watchdog(&mut self) {
        self.counts.watchdogs += 1;
    }

    pub fn release_watchdog(&mut self) {
        self.counts.watchdogs = self.counts.watchdogs.saturating_sub(1);
    }

    pub fn acquire_port(&mut self) {
        self.counts.ports += 1;
    }

    pub fn release_port(&mut self) {
        self.counts.ports = self.counts.ports.saturating_sub(1);
    }

    pub fn snapshot(&self) -> ResourceCounts {
        self.counts
    }

    /// `Ok` iff all counters are back at zero (fully released).
    pub fn assert_balanced(&self) -> Result<(), LeakReport> {
        assert_no_leak(&ResourceCounts::zero(), &self.counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_action_roundtrip() {
        for action in [
            ServiceAction::Status,
            ServiceAction::Start,
            ServiceAction::Stop,
            ServiceAction::Restart,
        ] {
            assert_eq!(ServiceAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(ServiceAction::parse("status"), Some(ServiceAction::Status));
        assert_eq!(ServiceAction::parse("stop"), Some(ServiceAction::Stop));
        assert_eq!(ServiceAction::parse("explode"), None);
        assert_eq!(ServiceAction::parse(""), None);
    }

    #[test]
    fn stop_drain_completes_pending() {
        let mut svc = ServiceController::new();
        svc.start().unwrap();
        for id in [1, 2, 3] {
            svc.submit(id).unwrap();
        }
        let receipt = svc.stop(StopPolicy::drain_all());
        assert_eq!(receipt.drained, vec![1, 2, 3]);
        assert!(receipt.cancelled.is_empty());
        assert!(!receipt.daemon_running);
        assert!(!svc.is_running());
        assert_eq!(svc.completed(), &[1, 2, 3]);
    }

    #[test]
    fn stop_cancel_drops_pending() {
        let mut svc = ServiceController::new();
        svc.start().unwrap();
        for id in [7, 8, 9] {
            svc.submit(id).unwrap();
        }
        let receipt = svc.stop(StopPolicy::Cancel);
        assert!(receipt.drained.is_empty());
        assert_eq!(receipt.cancelled, vec![7, 8, 9]);
        assert!(!receipt.daemon_running);
        assert!(svc.completed().is_empty());
        assert_eq!(svc.pending(), 0);
    }

    #[test]
    fn stop_drain_respects_bound() {
        let mut svc = ServiceController::new();
        svc.start().unwrap();
        for id in [1, 2, 3] {
            svc.submit(id).unwrap();
        }
        let policy = StopPolicy::Drain { max_items: 2 };
        assert!(policy.wants_drain());
        assert!(!StopPolicy::Cancel.wants_drain());
        let receipt = svc.stop(policy);
        assert_eq!(receipt.drained, vec![1, 2]);
        assert_eq!(receipt.cancelled, vec![3]);
        assert_eq!(receipt.policy, policy);
    }

    #[test]
    fn submit_rejected_when_stopped_or_full() {
        let mut svc = ServiceController::new();
        assert_eq!(svc.submit(1), Err(ServiceError::NotRunning));
        assert_eq!(svc.pending(), 0);
        svc.start().unwrap();
        assert_eq!(svc.start(), Err(ServiceError::AlreadyRunning));
        for id in 0..MAX_PENDING_WORK as u64 {
            svc.submit(id).unwrap();
        }
        assert_eq!(svc.submit(9999), Err(ServiceError::QueueFull));
        assert_eq!(svc.pending(), MAX_PENDING_WORK);
    }

    #[test]
    fn close_one_of_two_preserves_daemon_and_session() {
        let mut reg = ClientRegistry::new();
        reg.attach(1);
        reg.attach(2);
        let receipt = reg.detach(1).unwrap();
        assert_eq!(
            receipt,
            CloseReceipt {
                closed_client: 1,
                remaining_clients: 1,
                daemon_running: true,
                session_preserved: true,
            }
        );
        assert_eq!(reg.remaining(), 1);
        assert!(reg.daemon_running());
        assert!(reg.session_live());
    }

    #[test]
    fn close_last_client_keeps_daemon_owned_session() {
        let mut reg = ClientRegistry::new();
        reg.attach(1);
        let receipt = reg.detach(1).unwrap();
        assert_eq!(receipt.remaining_clients, 0);
        assert!(receipt.daemon_running);
        assert!(receipt.session_preserved);
        assert!(reg.daemon_running());
        assert!(reg.session_live());
        assert_eq!(reg.detach(1), Err(ServiceError::UnknownClient(1)));
        assert!(reg.session_live());
    }

    #[test]
    fn leak_helper_reports_growth() {
        let base = ResourceCounts::zero();
        assert_eq!(base.total(), 0);
        assert!(assert_no_leak(&base, &base).is_ok());
        let grown = ResourceCounts {
            fds: 1,
            watchdogs: 0,
            ports: 0,
        };
        let err = assert_no_leak(&base, &grown).unwrap_err();
        assert_eq!(
            err,
            LeakReport {
                baseline: base,
                current: grown,
            }
        );
        assert!(format!("{err}").contains('1'));
    }

    #[test]
    fn repeated_start_stop_cycles_no_leak() {
        let mut ledger = ResourceLedger::new();
        let mut svc = ServiceController::new();
        let baseline = ledger.snapshot();
        assert!(ledger.assert_balanced().is_ok());
        for cycle in 0..50u64 {
            svc.start().unwrap();
            ledger.acquire_fd();
            ledger.acquire_watchdog();
            ledger.acquire_port();
            svc.submit(cycle).unwrap();
            // Alternate policies per cycle: even drains, odd cancels.
            let policy = if cycle % 2 == 0 {
                StopPolicy::drain_all()
            } else {
                StopPolicy::Cancel
            };
            svc.stop(policy);
            ledger.release_fd();
            ledger.release_watchdog();
            ledger.release_port();
        }
        assert!(!svc.is_running());
        assert_eq!(svc.pending(), 0);
        let now = ledger.snapshot();
        assert_eq!(now, baseline);
        assert!(assert_no_leak(&baseline, &now).is_ok());
        assert!(ledger.assert_balanced().is_ok());
    }
}
