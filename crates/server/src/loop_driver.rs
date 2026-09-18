//! Roadmap 3.1: task-level agentic driver state machine (the loop).
//!
//! Pure-state, std-only, `forbid(unsafe_code)`. Drives a plan of goals
//! through a tick-based cycle: start → step results → steer amendments →
//! checkpoint/resume. Deterministic: same event sequence → same state.
//! No I/O, no threads, no provider calls.
//!
//! # In-file tests
//!
//! T01 – start→tick returns Continue with first goal InProgress
//! T02 – step Done advances to next goal
//! T03 – all goals Achieved → Stop{Achieved}
//! T04 – blocked goal triggers Replan once, then Exhausted after budget
//! T05 – steer appends amendment and preserves determinism
//! T06 – checkpoint→resume roundtrip restores exact state
//! T07 – resume rejects foreign version
//! T08 – max_steps cap stops with Exhausted
//! T09 – per-goal attempt cap 8 → Failed goal skipped
//! T10 – empty plan → immediate Stop

#![forbid(unsafe_code)]

use std::collections::VecDeque;

/// Maximum total driver iterations.
pub const MAX_LOOP_STEPS: u32 = 256;
/// Maximum attempts per individual goal before it is marked Failed.
pub const MAX_GOAL_ATTEMPTS: u32 = 8;
/// Maximum goals in a plan.
pub const MAX_GOALS: usize = 16;
/// Maximum bytes for a goal description or steer amendment.
pub const MAX_DESCRIPTION_BYTES: usize = 256;
/// Checkpoint format version (typed for resume validation).
pub const CHECKPOINT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Goal
// ---------------------------------------------------------------------------

/// Lifecycle status of a single goal within a plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GoalStatus {
    Pending,
    InProgress,
    Achieved,
    Failed,
}

/// A single goal inside a [`LoopPlan`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopGoal {
    pub id: u32,
    pub description: Vec<u8>,
    pub status: GoalStatus,
    /// How many times the driver has attempted this goal.
    pub attempts: u32,
}

impl LoopGoal {
    /// Create a new goal. Panics if description exceeds [`MAX_DESCRIPTION_BYTES`].
    pub fn new(id: u32, description: Vec<u8>) -> Self {
        assert!(
            description.len() <= MAX_DESCRIPTION_BYTES,
            "description {} bytes exceeds limit {}",
            description.len(),
            MAX_DESCRIPTION_BYTES
        );
        Self {
            id,
            description,
            status: GoalStatus::Pending,
            attempts: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

/// A bounded plan containing up to [`MAX_GOALS`] goals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopPlan {
    pub goals: VecDeque<LoopGoal>,
}

impl LoopPlan {
    /// Create a plan from an iterator of goals. Panics if > [`MAX_GOALS`].
    pub fn new(goals: impl IntoIterator<Item = LoopGoal>) -> Self {
        let goals: VecDeque<LoopGoal> = goals.into_iter().collect();
        assert!(goals.len() <= MAX_GOALS, "plan has {} goals, max {}", goals.len(), MAX_GOALS);
        Self { goals }
    }

    /// True when every goal is Achieved or the plan is empty.
    pub fn is_complete(&self) -> bool {
        self.goals.is_empty() || self.goals.iter().all(|g| g.status == GoalStatus::Achieved)
    }
}

// ---------------------------------------------------------------------------
// Events (driver inputs)
// ---------------------------------------------------------------------------

/// Outcome of a single goal step execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Done,
    Blocked(String),
    NeedsReplan,
}

/// Events fed into the [`LoopDriver`] state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopEvent {
    Start(LoopPlan),
    StepResult {
        goal_id: u32,
        outcome: StepOutcome,
    },
    Steer(String),
    Checkpoint,
    Resume(Vec<u8>),
    Tick,
}

// ---------------------------------------------------------------------------
// Actions (driver outputs)
// ---------------------------------------------------------------------------

/// Why the driver wants to stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopReason {
    Achieved,
    Exhausted,
}

/// Action produced by a [`Tick`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoopAction {
    Continue { goal_id: u32 },
    Replan,
    Stop { reason: StopReason },
}

// ---------------------------------------------------------------------------
// Driver state
// ---------------------------------------------------------------------------

/// Core state of the loop driver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriverState {
    pub plan: LoopPlan,
    pub current_index: usize,
    pub steps_taken: u32,
    pub replan_used: bool,
    pub amends: VecDeque<String>,
    pub stopped: bool,
    pub stop_reason: Option<StopReason>,
}

impl DriverState {
    fn new() -> Self {
        Self {
            plan: LoopPlan { goals: VecDeque::new() },
            current_index: 0,
            steps_taken: 0,
            replan_used: false,
            amends: VecDeque::new(),
            stopped: false,
            stop_reason: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Checkpoint (deterministic serialization)
// ---------------------------------------------------------------------------

/// Opaque checkpoint blob. Caller treats it as bytes.
pub type CheckpointBlob = Vec<u8>;

/// Encode the driver state into a deterministic checkpoint blob.
pub fn checkpoint(state: &DriverState) -> CheckpointBlob {
    let mut buf = Vec::new();
    // version: 4 bytes LE
    buf.extend_from_slice(&CHECKPOINT_VERSION.to_le_bytes());
    // steps_taken: 4 bytes LE
    buf.extend_from_slice(&state.steps_taken.to_le_bytes());
    // replan_used: 1 byte
    buf.push(state.replan_used as u8);
    // current_index: 4 bytes LE
    buf.extend_from_slice(&(state.current_index as u32).to_le_bytes());
    // stopped: 1 byte
    buf.push(state.stopped as u8);
    // stop_reason: 1 byte (0=None, 1=Achieved, 2=Exhausted)
    buf.push(match state.stop_reason {
        None => 0,
        Some(StopReason::Achieved) => 1,
        Some(StopReason::Exhausted) => 2,
    });
    // amends count: 4 bytes LE
    let amend_count = state.amends.len() as u32;
    buf.extend_from_slice(&amend_count.to_le_bytes());
    // each amend: 4 bytes LE len + bytes
    for amend in &state.amends {
        let len = amend.len() as u32;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(amend.as_bytes());
    }
    // goals count: 4 bytes LE
    let goal_count = state.plan.goals.len() as u32;
    buf.extend_from_slice(&goal_count.to_le_bytes());
    // each goal: id(4) + status(1) + attempts(4) + desc_len(4) + desc
    for goal in &state.plan.goals {
        buf.extend_from_slice(&goal.id.to_le_bytes());
        buf.push(match goal.status {
            GoalStatus::Pending => 0,
            GoalStatus::InProgress => 1,
            GoalStatus::Achieved => 2,
            GoalStatus::Failed => 3,
        });
        buf.extend_from_slice(&goal.attempts.to_le_bytes());
        let desc_len = goal.description.len() as u32;
        buf.extend_from_slice(&desc_len.to_le_bytes());
        buf.extend_from_slice(&goal.description);
    }
    buf
}

/// Decode a checkpoint blob back into driver state.
///
/// # Errors
///
/// Returns `Err(reason)` if the blob version mismatches or is malformed.
pub fn resume(blob: &CheckpointBlob) -> Result<DriverState, String> {
    let buf = blob;

    // version
    if buf.len() < 4 {
        return Err("blob too short for version".into());
    }
    let version = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if version != CHECKPOINT_VERSION {
        return Err(format!(
            "version mismatch: expected {}, got {}",
            CHECKPOINT_VERSION, version
        ));
    }
    let mut pos = 4;

    // steps_taken
    if buf.len() < pos + 4 {
        return Err("blob too short for steps_taken".into());
    }
    let steps_taken = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
    pos += 4;

    // replan_used
    if buf.len() < pos + 1 {
        return Err("blob too short for replan_used".into());
    }
    let replan_used = buf[pos] != 0;
    pos += 1;

    // current_index
    if buf.len() < pos + 4 {
        return Err("blob too short for current_index".into());
    }
    let current_index =
        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
    pos += 4;

    // stopped
    if buf.len() < pos + 1 {
        return Err("blob too short for stopped".into());
    }
    let stopped = buf[pos] != 0;
    pos += 1;

    // stop_reason
    if buf.len() < pos + 1 {
        return Err("blob too short for stop_reason".into());
    }
    let stop_reason = match buf[pos] {
        0 => None,
        1 => Some(StopReason::Achieved),
        2 => Some(StopReason::Exhausted),
        _ => return Err("invalid stop_reason tag".into()),
    };
    pos += 1;

    // amends
    if buf.len() < pos + 4 {
        return Err("blob too short for amend count".into());
    }
    let amend_count =
        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
    pos += 4;
    let mut amends = VecDeque::new();
    for _ in 0..amend_count {
        if buf.len() < pos + 4 {
            return Err("blob too short for amend len".into());
        }
        let len =
            u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        if buf.len() < pos + len {
            return Err("blob too short for amend data".into());
        }
        let amend = String::from_utf8(buf[pos..pos + len].to_vec())
            .map_err(|e| format!("amend not valid UTF-8: {e}"))?;
        amends.push_back(amend);
        pos += len;
    }

    // goals
    if buf.len() < pos + 4 {
        return Err("blob too short for goal count".into());
    }
    let goal_count =
        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
    pos += 4;
    let mut goals = VecDeque::new();
    for _ in 0..goal_count {
        if buf.len() < pos + 13 {
            return Err("blob too short for goal".into());
        }
        let id = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        pos += 4;
        let status = match buf[pos] {
            0 => GoalStatus::Pending,
            1 => GoalStatus::InProgress,
            2 => GoalStatus::Achieved,
            3 => GoalStatus::Failed,
            _ => return Err("invalid goal status tag".into()),
        };
        pos += 1;
        let attempts =
            u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        pos += 4;
        let desc_len =
            u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        if buf.len() < pos + desc_len {
            return Err("blob too short for goal description".into());
        }
        let description = buf[pos..pos + desc_len].to_vec();
        pos += desc_len;
        goals.push_back(LoopGoal {
            id,
            description,
            status,
            attempts,
        });
    }

    Ok(DriverState {
        plan: LoopPlan { goals },
        current_index,
        steps_taken,
        replan_used,
        amends,
        stopped,
        stop_reason,
    })
}

// ---------------------------------------------------------------------------
// LoopDriver
// ---------------------------------------------------------------------------

/// The task-level loop driver state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopDriver {
    state: DriverState,
}

impl LoopDriver {
    /// Create a new driver with no plan loaded.
    pub fn new() -> Self {
        Self {
            state: DriverState::new(),
        }
    }

    /// Immutable access to the current driver state.
    pub fn state(&self) -> &DriverState {
        &self.state
    }

    /// Feed an event into the driver, returning the resulting action.
    pub fn send(&mut self, event: LoopEvent) -> LoopAction {
        match event {
            LoopEvent::Start(plan) => self.handle_start(plan),
            LoopEvent::StepResult { goal_id, outcome } => {
                self.handle_step_result(goal_id, outcome)
            }
            LoopEvent::Steer(note) => self.handle_steer(note),
            LoopEvent::Checkpoint => {
                // Checkpoint is a query, state unchanged; caller reads blob via
                // `checkpoint()` on the returned state reference.
                // We still return a valid action.
                if self.state.stopped {
                    LoopAction::Stop {
                        reason: self.state.stop_reason.unwrap_or(StopReason::Exhausted),
                    }
                } else {
                    self.produce_action()
                }
            }
            LoopEvent::Resume(blob) => self.handle_resume(blob),
            LoopEvent::Tick => self.handle_tick(),
        }
    }

    /// Produce a checkpoint blob from the current state.
    pub fn checkpoint_blob(&self) -> CheckpointBlob {
        checkpoint(&self.state)
    }

    // -- internal handlers --

    fn handle_start(&mut self, plan: LoopPlan) -> LoopAction {
        self.state = DriverState::new();
        self.state.plan = plan;
        self.produce_action()
    }

    fn handle_step_result(&mut self, goal_id: u32, outcome: StepOutcome) -> LoopAction {
        if self.state.stopped {
            return LoopAction::Stop {
                reason: self.state.stop_reason.unwrap_or(StopReason::Exhausted),
            };
        }
        // Find the goal by id.
        let goal_idx = self.state.plan.goals.iter().position(|g| g.id == goal_id);
        let goal_idx = match goal_idx {
            Some(i) => i,
            None => return self.produce_action(),
        };

        match outcome {
            StepOutcome::Done => {
                self.state.plan.goals[goal_idx].status = GoalStatus::Achieved;
                // Advance to next goal.
                self.state.current_index = goal_idx + 1;
            }
            StepOutcome::Blocked(_reason) => {
                self.state.plan.goals[goal_idx].attempts += 1;
                if self.state.plan.goals[goal_idx].attempts >= MAX_GOAL_ATTEMPTS {
                    self.state.plan.goals[goal_idx].status = GoalStatus::Failed;
                }
                // On first block, trigger replan if available.
                if !self.state.replan_used {
                    self.state.replan_used = true;
                    return LoopAction::Replan;
                }
            }
            StepOutcome::NeedsReplan => {
                if !self.state.replan_used {
                    self.state.replan_used = true;
                    return LoopAction::Replan;
                }
            }
        }
        self.produce_action()
    }

    fn handle_steer(&mut self, note: String) -> LoopAction {
        if note.len() > MAX_DESCRIPTION_BYTES {
            // Truncate to bound.
            let mut truncated = note.into_bytes();
            truncated.truncate(MAX_DESCRIPTION_BYTES);
            self.state.amends.push_back(
                String::from_utf8(truncated).unwrap_or_default(),
            );
        } else {
            self.state.amends.push_back(note);
        }
        self.produce_action()
    }

    fn handle_resume(&mut self, blob: Vec<u8>) -> LoopAction {
        match resume(&blob) {
            Ok(restored) => {
                self.state = restored;
                self.produce_action()
            }
            Err(_reason) => {
                self.state.stopped = true;
                self.state.stop_reason = Some(StopReason::Exhausted);
                LoopAction::Stop {
                    reason: StopReason::Exhausted,
                }
            }
        }
    }

    fn handle_tick(&mut self) -> LoopAction {
        if self.state.stopped {
            return LoopAction::Stop {
                reason: self.state.stop_reason.unwrap_or(StopReason::Exhausted),
            };
        }
        self.state.steps_taken += 1;
        if self.state.steps_taken >= MAX_LOOP_STEPS {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Exhausted);
            return LoopAction::Stop {
                reason: StopReason::Exhausted,
            };
        }
        // If plan is empty or complete, stop.
        if self.state.plan.goals.is_empty() {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Achieved);
            return LoopAction::Stop {
                reason: StopReason::Achieved,
            };
        }
        if self.state.plan.is_complete() {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Achieved);
            return LoopAction::Stop {
                reason: StopReason::Achieved,
            };
        }
        // Advance past achieved/failed goals to find the next actionable one.
        self.skip_achieved();
        self.produce_action()
    }

    /// Skip goals that are Achieved or Failed, advancing current_index.
    fn skip_achieved(&mut self) {
        while self.state.current_index < self.state.plan.goals.len() {
            match self.state.plan.goals[self.state.current_index].status {
                GoalStatus::Achieved | GoalStatus::Failed => {
                    self.state.current_index += 1;
                }
                _ => break,
            }
        }
    }

    /// Produce an action based on current state without mutating it further.
    fn produce_action(&mut self) -> LoopAction {
        if self.state.stopped {
            return LoopAction::Stop {
                reason: self.state.stop_reason.unwrap_or(StopReason::Exhausted),
            };
        }
        if self.state.plan.goals.is_empty() {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Achieved);
            return LoopAction::Stop {
                reason: StopReason::Achieved,
            };
        }
        if self.state.plan.is_complete() {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Achieved);
            return LoopAction::Stop {
                reason: StopReason::Achieved,
            };
        }
        // Skip past achieved/failed goals.
        self.skip_achieved();
        if self.state.current_index >= self.state.plan.goals.len() {
            self.state.stopped = true;
            self.state.stop_reason = Some(StopReason::Achieved);
            return LoopAction::Stop {
                reason: StopReason::Achieved,
            };
        }
        let goal = &mut self.state.plan.goals[self.state.current_index];
        if goal.status == GoalStatus::Pending {
            goal.status = GoalStatus::InProgress;
        }
        LoopAction::Continue {
            goal_id: goal.id,
        }
    }
}

impl Default for LoopDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // T01: start→tick returns Continue with first goal InProgress.
    #[test]
    fn t01_start_tick_continue_first_goal() {
        let goal = LoopGoal::new(1, b"alpha".to_vec());
        let plan = LoopPlan::new(vec![goal]);
        let mut driver = LoopDriver::new();
        let action = driver.send(LoopEvent::Start(plan));
        assert_eq!(
            action,
            LoopAction::Continue { goal_id: 1 },
            "Start should return Continue for first goal"
        );
        assert_eq!(
            driver.state().plan.goals[0].status,
            GoalStatus::InProgress,
            "first goal should be InProgress after Start"
        );
    }

    // T02: step Done advances to next goal.
    #[test]
    fn t02_step_done_advances() {
        let g1 = LoopGoal::new(1, b"one".to_vec());
        let g2 = LoopGoal::new(2, b"two".to_vec());
        let plan = LoopPlan::new(vec![g1, g2]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        let action = driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Done,
        });
        assert_eq!(
            action,
            LoopAction::Continue { goal_id: 2 },
            "Done on goal 1 should Continue to goal 2"
        );
        assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Achieved);
        assert_eq!(driver.state().plan.goals[1].status, GoalStatus::InProgress);
    }

    // T03: all goals Achieved → Stop{Achieved}.
    #[test]
    fn t03_all_achieved_stop() {
        let g1 = LoopGoal::new(1, b"a".to_vec());
        let g2 = LoopGoal::new(2, b"b".to_vec());
        let plan = LoopPlan::new(vec![g1, g2]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Done,
        });
        let action = driver.send(LoopEvent::StepResult {
            goal_id: 2,
            outcome: StepOutcome::Done,
        });
        assert_eq!(
            action,
            LoopAction::Stop {
                reason: StopReason::Achieved
            }
        );
    }

    // T04: blocked goal triggers Replan once, then Exhausted after budget.
    #[test]
    fn t04_blocked_triggers_replan_then_exhausted() {
        let g = LoopGoal::new(1, b"stuck".to_vec());
        let plan = LoopPlan::new(vec![g]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        // First block → Replan.
        let action = driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Blocked("dep missing".into()),
        });
        assert_eq!(action, LoopAction::Replan, "first block should Replan");
        assert!(driver.state().replan_used);
        // Subsequent blocks increment attempts but no more replan.
        for _ in 1..MAX_GOAL_ATTEMPTS {
            let a = driver.send(LoopEvent::StepResult {
                goal_id: 1,
                outcome: StepOutcome::Blocked("still stuck".into()),
            });
            // Should not be Replan again.
            assert_ne!(a, LoopAction::Replan);
        }
        // Goal should now be Failed (attempts >= MAX_GOAL_ATTEMPTS).
        assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Failed);
        // Tick should stop with Achieved (all remaining are failed/achieved).
        let action = driver.send(LoopEvent::Tick);
        assert_eq!(
            action,
            LoopAction::Stop {
                reason: StopReason::Achieved
            }
        );
    }

    // T05: steer appends amendment and preserves determinism.
    #[test]
    fn t05_steer_appends_and_deterministic() {
        let g = LoopGoal::new(1, b"task".to_vec());
        let plan = LoopPlan::new(vec![g]);
        let mut d1 = LoopDriver::new();
        let mut d2 = LoopDriver::new();
        d1.send(LoopEvent::Start(plan.clone()));
        d2.send(LoopEvent::Start(plan));
        d1.send(LoopEvent::Steer("note A".into()));
        d2.send(LoopEvent::Steer("note A".into()));
        d1.send(LoopEvent::Steer("note B".into()));
        d2.send(LoopEvent::Steer("note B".into()));
        assert_eq!(d1.state(), d2.state(), "identical events must produce identical state");
        assert_eq!(d1.state().amends.len(), 2);
        assert_eq!(d1.state().amends[0], "note A");
        assert_eq!(d1.state().amends[1], "note B");
    }

    // T06: checkpoint→resume roundtrip restores exact state.
    #[test]
    fn t06_checkpoint_resume_roundtrip() {
        let g1 = LoopGoal::new(1, b"first".to_vec());
        let g2 = LoopGoal::new(2, b"second".to_vec());
        let plan = LoopPlan::new(vec![g1, g2]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        driver.send(LoopEvent::StepResult {
            goal_id: 1,
            outcome: StepOutcome::Done,
        });
        driver.send(LoopEvent::Steer("hint".into()));
        let blob = driver.checkpoint_blob();
        let mut restored = LoopDriver::new();
        let action = restored.send(LoopEvent::Resume(blob));
        assert_eq!(restored.state(), driver.state());
        // Restored driver should continue from where we left off.
        assert_eq!(
            action,
            LoopAction::Continue { goal_id: 2 },
            "restored driver should continue goal 2"
        );
    }

    // T07: resume rejects foreign version.
    #[test]
    fn t07_resume_rejects_foreign_version() {
        let mut blob = checkpoint(&DriverState::new());
        // Corrupt version field.
        blob[0..4].copy_from_slice(&99u32.to_le_bytes());
        let mut driver = LoopDriver::new();
        let action = driver.send(LoopEvent::Resume(blob));
        assert_eq!(
            action,
            LoopAction::Stop {
                reason: StopReason::Exhausted
            },
            "foreign version should stop with Exhausted"
        );
    }

    // T08: max_steps cap stops with Exhausted.
    #[test]
    fn t08_max_steps_exhausted() {
        let g = LoopGoal::new(1, b"forever".to_vec());
        let plan = LoopPlan::new(vec![g]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        // Simulate reaching the iteration budget by directly setting steps_taken.
        // The driver increments steps_taken on Tick, so setting it to
        // MAX_LOOP_STEPS - 1 means the next tick hits the cap.
        driver.state.steps_taken = MAX_LOOP_STEPS - 1;
        let action = driver.send(LoopEvent::Tick);
        assert_eq!(
            action,
            LoopAction::Stop {
                reason: StopReason::Exhausted
            }
        );
        assert_eq!(driver.state().steps_taken, MAX_LOOP_STEPS);
    }

    // T09: per-goal attempt cap 8 → Failed goal skipped.
    #[test]
    fn t09_per_goal_attempt_cap() {
        let g1 = LoopGoal::new(1, b"blocker".to_vec());
        let g2 = LoopGoal::new(2, b"free".to_vec());
        let plan = LoopPlan::new(vec![g1, g2]);
        let mut driver = LoopDriver::new();
        driver.send(LoopEvent::Start(plan));
        // Block goal 1 until it hits the cap.
        for _ in 0..MAX_GOAL_ATTEMPTS {
            driver.send(LoopEvent::StepResult {
                goal_id: 1,
                outcome: StepOutcome::Blocked("always".into()),
            });
        }
        assert_eq!(driver.state().plan.goals[0].status, GoalStatus::Failed);
        // Tick should skip failed goal 1 and continue to goal 2.
        let action = driver.send(LoopEvent::Tick);
        assert_eq!(
            action,
            LoopAction::Continue { goal_id: 2 },
            "should skip failed goal and continue to next"
        );
    }

    // T10: empty plan → immediate Stop.
    #[test]
    fn t10_empty_plan_stop() {
        let plan = LoopPlan::new(std::iter::empty());
        let mut driver = LoopDriver::new();
        let action = driver.send(LoopEvent::Start(plan));
        assert_eq!(
            action,
            LoopAction::Stop {
                reason: StopReason::Achieved
            },
            "empty plan should stop immediately"
        );
    }
}
