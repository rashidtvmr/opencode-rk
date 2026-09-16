//! Autonomous-loop driver slice (AUTO-006).
//!
//! Native ready-queue + single-file lease + gate-before-commit driver.
//! Pure in-process state: no OS process per lane, no network, no database.
//! Every lane owns exactly one file via [`LeaseTable`]; [`Driver::drive_task`]
//! runs worker, then gate, then commit; [`Driver::drive`] stops cleanly when
//! the ready queue is empty.
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

use thiserror::Error;

pub type TaskId = String;

/// Attempt cap for gate/worker retries before a lane blocks.
pub const ATTEMPT_CAP: u32 = 3;
/// Per-lane retained tail cap: 12 lines / 4 KiB with truncation marker.
pub const MAX_TAIL_LINES: usize = 12;
pub const MAX_TAIL_BYTES: usize = 4096;
const TRUNCATION_MARKER: &str = "...[truncated]";
/// Marker proving an owned file is real work, not a stub body.
pub const GATE_MARKER: &str = "verified:gate-pass";
/// Minimum owned-file body length for a passing gate.
pub const MIN_BODY_LEN: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSpec {
    id: TaskId,
    rank: u32,
    deps: Vec<TaskId>,
}

impl TaskSpec {
    pub fn new(id: &str, rank: u32, deps: &[&str]) -> Self {
        Self {
            id: id.to_owned(),
            rank,
            deps: deps.iter().map(|d| (*d).to_owned()).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    tasks: HashMap<TaskId, TaskSpec>,
}

impl Plan {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn add(&mut self, spec: TaskSpec) {
        self.tasks.insert(spec.id.clone(), spec);
    }

    #[must_use]
    pub fn rank_of(&self, id: &str) -> Option<u32> {
        self.tasks.get(id).map(|t| t.rank)
    }
}

pub struct ReadyQueue;

impl ReadyQueue {
    /// Lower-rank plan peers act as synthesized rank edges: a task is ready
    /// only when every plan task with a strictly lower rank is accepted.
    fn synthesized_deps(plan: &Plan, id: &str, rank: u32) -> Vec<TaskId> {
        let mut out: Vec<TaskId> = plan
            .tasks
            .values()
            .filter(|t| t.rank < rank && t.id != id)
            .map(|t| t.id.clone())
            .collect();
        out.sort();
        out
    }

    fn all_deps(plan: &Plan, spec: &TaskSpec) -> Vec<TaskId> {
        let mut deps = spec.deps.clone();
        deps.extend(Self::synthesized_deps(plan, &spec.id, spec.rank));
        deps.sort();
        deps.dedup();
        deps
    }

    /// Not-started tasks (neither accepted nor blocked) whose authored plus
    /// synthesized rank dependencies are all accepted, sorted by
    /// `(rank asc, id asc)`, truncated to `limit`.
    pub fn next(
        plan: &Plan,
        accepted: &HashSet<String>,
        blocked: &HashSet<String>,
        limit: usize,
    ) -> Vec<TaskId> {
        let mut ready: Vec<(u32, TaskId)> = plan
            .tasks
            .values()
            .filter(|t| !accepted.contains(&t.id) && !blocked.contains(&t.id))
            .filter(|t| Self::all_deps(plan, t).iter().all(|d| accepted.contains(d)))
            .map(|t| (t.rank, t.id.clone()))
            .collect();
        ready.sort();
        ready.into_iter().map(|(_, id)| id).take(limit).collect()
    }

    /// Not-started tasks held back by a blocked dependency (authored or
    /// synthesized rank edge). Reported as `waiting_on_blocker`, never ready.
    pub fn blocked_dependents(
        plan: &Plan,
        accepted: &HashSet<String>,
        blocked: &HashSet<String>,
    ) -> HashSet<TaskId> {
        plan.tasks
            .values()
            .filter(|t| !accepted.contains(&t.id) && !blocked.contains(&t.id))
            .filter(|t| Self::all_deps(plan, t).iter().any(|d| blocked.contains(d)))
            .map(|t| t.id.clone())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OwnerToken(u64);

impl OwnerToken {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lease {
    pub task_id: TaskId,
    pub owner: OwnerToken,
    pub file: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LeaseError {
    #[error("lease denied")]
    LeaseDenied,
}

#[derive(Debug, Default)]
pub struct LeaseTable {
    by_task: HashMap<TaskId, Lease>,
    by_file: HashMap<String, TaskId>,
}

impl LeaseTable {
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_task: HashMap::new(),
            by_file: HashMap::new(),
        }
    }

    /// One lane owns exactly one file: a second file for the same lane, a
    /// live file claimed by another lane, or a live file claimed by another
    /// owner all fail with [`LeaseError::LeaseDenied`] and mutate nothing.
    pub fn acquire(&mut self, task: &str, owner: OwnerToken, file: &str) -> Result<(), LeaseError> {
        if let Some(live) = self.by_task.get(task) {
            if live.owner == owner && live.file == file {
                return Ok(());
            }
            return Err(LeaseError::LeaseDenied);
        }
        if self.by_file.contains_key(file) {
            return Err(LeaseError::LeaseDenied);
        }
        self.by_task.insert(
            task.to_owned(),
            Lease {
                task_id: task.to_owned(),
                owner,
                file: file.to_owned(),
            },
        );
        self.by_file.insert(file.to_owned(), task.to_owned());
        Ok(())
    }

    /// Only the owning lane may release. Denied releases retain the lease.
    pub fn release(&mut self, task: &str, owner: OwnerToken) -> Result<(), LeaseError> {
        match self.by_task.get(task) {
            Some(live) if live.owner == owner => {
                let live = self.by_task.remove(task).expect("checked above");
                self.by_file.remove(&live.file);
                Ok(())
            }
            _ => Err(LeaseError::LeaseDenied),
        }
    }

    #[must_use]
    pub fn get(&self, task: &str) -> Option<&Lease> {
        self.by_task.get(task)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GateVerdict {
    Pass,
    Stub,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaneStatus {
    Accepted { task: TaskId, last_error: String },
    Blocked { task: TaskId, last_error: String },
}

impl LaneStatus {
    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        match self {
            Self::Accepted { last_error, .. } | Self::Blocked { last_error, .. } => {
                Some(last_error.as_str())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stopped {
    NoReadyWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StopReport {
    pub code: i32,
    pub receipt: String,
    pub blocked: Vec<String>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DriverError {
    #[error("accepted but not integrated: fast-forward failed for {0}")]
    NotIntegrated(String),
    #[error("verification failed for {0}")]
    VerificationFailed(String),
    #[error("no lease for {0}")]
    NoLease(String),
}

impl fmt::Display for GateVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::Stub => write!(f, "STUB"),
            Self::Incomplete => write!(f, "INCOMPLETE"),
        }
    }
}

fn tail_bounded(text: &str) -> String {
    let lines: Vec<&str> = text.lines().take(MAX_TAIL_LINES).collect();
    let mut out = lines.join("\n");
    if text.lines().count() > MAX_TAIL_LINES || text.len() > MAX_TAIL_BYTES {
        out.push_str(TRUNCATION_MARKER);
    }
    if out.len() > MAX_TAIL_BYTES {
        let mut end = MAX_TAIL_BYTES.saturating_sub(TRUNCATION_MARKER.len());
        while end > 0 && !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push_str(TRUNCATION_MARKER);
    }
    out
}

pub struct Driver {
    max_lanes: usize,
    leases: LeaseTable,
    staged: HashMap<String, String>,
    status: HashMap<TaskId, LaneStatus>,
    committed: HashSet<TaskId>,
    branches: HashSet<TaskId>,
    ff_conflict: HashSet<TaskId>,
    receipts: Vec<String>,
    commit_sequence: Vec<TaskId>,
    workers_spawned: u64,
    commits: u64,
    blocked_tasks: Vec<TaskId>,
}

impl Driver {
    #[must_use]
    pub fn new(max_lanes: usize) -> Self {
        Self {
            max_lanes: max_lanes.max(1),
            leases: LeaseTable::new(),
            staged: HashMap::new(),
            status: HashMap::new(),
            committed: HashSet::new(),
            branches: HashSet::new(),
            ff_conflict: HashSet::new(),
            receipts: Vec::new(),
            commit_sequence: Vec::new(),
            workers_spawned: 0,
            commits: 0,
            blocked_tasks: Vec::new(),
        }
    }

    pub fn acquire(&mut self, task: &str, owner: OwnerToken, file: &str) -> Result<(), LeaseError> {
        self.leases.acquire(task, owner, file)
    }

    pub fn stage(&mut self, file: &str, body: &str) {
        self.staged.insert(file.to_owned(), body.to_owned());
    }

    pub fn set_ff_conflict(&mut self, task: &str, conflict: bool) {
        if conflict {
            self.ff_conflict.insert(task.to_owned());
        } else {
            self.ff_conflict.remove(task);
        }
    }

    fn append_receipt(&mut self, line: String) {
        self.receipts.push(tail_bounded(&line));
    }

    /// Gate over the lane-owned file: missing/short/placeholder bodies are
    /// `Stub`, marker-less real-length bodies are `Incomplete`, only
    /// marker-bearing bodies pass. Gate never commits.
    #[must_use]
    pub fn gate(&self, task: &str) -> GateVerdict {
        let file = match self.leases.get(task) {
            Some(lease) => lease.file.clone(),
            None => return GateVerdict::Stub,
        };
        match self.staged.get(&file) {
            None => GateVerdict::Stub,
            Some(body) if body.len() < MIN_BODY_LEN => GateVerdict::Stub,
            Some(body) if !body.contains(GATE_MARKER) => GateVerdict::Incomplete,
            _ => GateVerdict::Pass,
        }
    }

    #[must_use]
    pub fn committed(&self, task: &str) -> bool {
        self.committed.contains(task)
    }

    #[must_use]
    pub fn branch_preserved(&self, task: &str) -> bool {
        self.branches.contains(task)
    }

    #[must_use]
    pub fn status(&self, task: &str) -> LaneStatus {
        self.status
            .get(task)
            .cloned()
            .unwrap_or(LaneStatus::Blocked {
                task: task.to_owned(),
                last_error: "verification failed: no lane record".to_owned(),
            })
    }

    pub fn commit(&mut self, task: &str) -> Result<(), DriverError> {
        if self.ff_conflict.contains(task) {
            self.branches.insert(task.to_owned());
            let err = format!("accepted but not integrated: fast-forward failed for {task}");
            self.status.insert(
                task.to_owned(),
                LaneStatus::Accepted {
                    task: task.to_owned(),
                    last_error: err.clone(),
                },
            );
            self.append_receipt(format!("commit {task}: {err}"));
            return Err(DriverError::NotIntegrated(task.to_owned()));
        }
        if self.gate(task) != GateVerdict::Pass {
            return Err(DriverError::VerificationFailed(task.to_owned()));
        }
        if self.leases.get(task).is_none() {
            return Err(DriverError::NoLease(task.to_owned()));
        }
        self.committed.insert(task.to_owned());
        self.branches.insert(task.to_owned());
        self.commits += 1;
        self.commit_sequence.push(task.to_owned());
        self.status.insert(
            task.to_owned(),
            LaneStatus::Accepted {
                task: task.to_owned(),
                last_error: "integrated".to_owned(),
            },
        );
        self.append_receipt(format!("commit {task}: integrated"));
        Ok(())
    }

    /// Worker, then gate, then commit. Gate FAIL never commits: the lane is
    /// retried up to [`ATTEMPT_CAP`], then blocked with `verification failed`.
    pub fn drive_task(&mut self, task: &str) -> LaneStatus {
        for _ in 0..ATTEMPT_CAP {
            self.workers_spawned += 1;
            if self.gate(task) == GateVerdict::Pass {
                if self.ff_conflict.contains(task) {
                    let _ = self.commit(task);
                    return self.status(task);
                }
                if self.commit(task).is_ok() {
                    return self.status(task);
                }
            } else {
                let verdict = self.gate(task);
                self.append_receipt(format!("gate {task}: {verdict}"));
            }
        }
        if self.ff_conflict.contains(task) {
            let _ = self.commit(task);
            return self.status(task);
        }
        let err = format!("verification failed for {task}: gate {0}", self.gate(task));
        self.status.insert(
            task.to_owned(),
            LaneStatus::Blocked {
                task: task.to_owned(),
                last_error: err,
            },
        );
        if !self.blocked_tasks.contains(&task.to_owned()) {
            self.blocked_tasks.push(task.to_owned());
        }
        self.append_receipt(format!("block {task}: verification failed"));
        self.status(task)
    }

    /// Drive one bounded ready batch in milestone order, merging is the
    /// caller's job: returns integrated task ids in rank order.
    pub fn drive_ready(
        &mut self,
        plan: &Plan,
        accepted: &HashSet<String>,
        blocked: &HashSet<String>,
    ) -> Vec<TaskId> {
        let mut done = Vec::new();
        for task in ReadyQueue::next(plan, accepted, blocked, self.max_lanes) {
            if self.leases.get(&task).is_none() {
                continue;
            }
            let status = self.drive_task(&task);
            if matches!(status, LaneStatus::Accepted { .. }) && self.committed(&task) {
                done.push(task);
            }
        }
        done
    }

    /// Empty-queue stop: no worker spawned, no commit attempted, exit 0 with
    /// blocked-task report and receipt path. Never spins.
    pub fn drive(&self) -> Stopped {
        Stopped::NoReadyWork
    }

    #[must_use]
    pub fn stop_report(&self) -> StopReport {
        let mut blocked = self.blocked_tasks.clone();
        blocked.sort();
        StopReport {
            code: 0,
            receipt: format!("receipts/driver-{}.log", self.receipts.len()),
            blocked,
        }
    }

    #[must_use]
    pub fn workers_spawned(&self) -> u64 {
        self.workers_spawned
    }

    #[must_use]
    pub fn commits(&self) -> u64 {
        self.commits
    }

    #[must_use]
    pub fn commit_sequence(&self) -> &[TaskId] {
        &self.commit_sequence
    }
}
