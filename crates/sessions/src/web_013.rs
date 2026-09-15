//! Search / deep-research run state (WEB-013, REQ-040): pure state machine.
//!
//! No IO, no clock, no threads, no secrets. Caller supplies sources, plan
//! steps, progress events and the final cited result; this module enforces
//! adapter availability, claim-to-source binding, and resource bounds.

use thiserror::Error;

/// Max selectable research sources per run.
pub const MAX_SOURCES: usize = 8;
/// Max plan steps per run.
pub const MAX_PLAN_STEPS: usize = 16;
/// Max retained progress events per run (oldest evicted, flagged).
pub const MAX_PROGRESS_EVENTS: usize = 32;
/// Max concurrent child/tool work items per run.
pub const MAX_CHILD_WORK: usize = 4;
/// Max claims citable in one final result.
pub const MAX_CLAIMS: usize = 64;
/// Max final-result bytes retained.
pub const MAX_FINAL_BYTES: usize = 32_768;

/// Chat mode selector: plain chat vs native research adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchMode {
    Chat,
    Search,
    DeepResearch,
}

/// Lifecycle of one research run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunState {
    Draft,
    Planned,
    Running,
    Steered,
    Done,
    Cancelled,
}

/// Per-step plan state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepState {
    Pending,
    Active,
    Done,
    Aborted,
}

/// One reviewable plan step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanStep {
    pub title: String,
    pub state: StepState,
}

/// One retained progress event (bounded; oldest evicted past the cap).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressEvent {
    pub step: String,
    pub detail: String,
}

/// Binds one result claim index to a selected source name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Citation {
    pub claim: usize,
    pub source: String,
}

/// Durable record: plan, source set, activity summary and cited result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub mode: ResearchMode,
    pub adapter_available: bool,
    pub state: RunState,
    pub sources: Vec<String>,
    pub plan: Vec<PlanStep>,
    pub events: Vec<ProgressEvent>,
    pub event_count: usize,
    pub truncated: bool,
    pub claims: usize,
    pub citations: Vec<Citation>,
    pub final_text: String,
}

/// Failure modes for research runs.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ResearchError {
    #[error("search/research adapter is not installed or unavailable")]
    Unavailable,
    #[error("no sources selected")]
    NoSources,
    #[error("source name is empty")]
    EmptySourceName,
    #[error("too many sources: max {max}, actual {actual}")]
    TooManySources { max: usize, actual: usize },
    #[error("plan is empty")]
    EmptyPlan,
    #[error("plan step is empty")]
    EmptyStep,
    #[error("too many plan steps: max {max}, actual {actual}")]
    TooManySteps { max: usize, actual: usize },
    #[error("run is in state {state:?}; operation not allowed")]
    InvalidState { state: RunState },
    #[error("unknown plan step: {0}")]
    UnknownStep(String),
    #[error("unknown source: {0}")]
    UnknownSource(String),
    #[error("citation claim {claim} out of range for {claims} claims")]
    ClaimOutOfRange { claim: usize, claims: usize },
    #[error("claim {0} has no citation")]
    UncitedClaim(usize),
    #[error("final result is empty")]
    EmptyFinal,
    #[error("final result exceeds {MAX_FINAL_BYTES} bytes")]
    FinalTooLong,
    #[error("too many claims: max {MAX_CLAIMS}, actual {actual}")]
    TooManyClaims { actual: usize },
    #[error("too much concurrent child work: max {MAX_CHILD_WORK}")]
    TooManyChildren,
    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),
}

/// One search / deep-research run.
#[derive(Clone, Debug)]
pub struct ResearchRun {
    mode: ResearchMode,
    adapter_available: bool,
    state: RunState,
    sources: Vec<String>,
    plan: Vec<PlanStep>,
    events: Vec<ProgressEvent>,
    truncated: bool,
    children: usize,
    claims: usize,
    citations: Vec<Citation>,
    final_text: String,
}

impl ResearchRun {
    /// New run in `mode`. `adapter_available` reports whether the native
    /// search/research adapter is installed; without it the run can never
    /// reach `Done`.
    #[must_use]
    pub fn new(mode: ResearchMode, adapter_available: bool) -> Self {
        Self {
            mode,
            adapter_available,
            state: RunState::Draft,
            sources: Vec::new(),
            plan: Vec::new(),
            events: Vec::new(),
            truncated: false,
            children: 0,
            claims: 0,
            citations: Vec::new(),
            final_text: String::new(),
        }
    }

    /// True for research modes; a plain chat turn is never research.
    #[must_use]
    pub const fn is_research_turn(mode: ResearchMode) -> bool {
        !matches!(mode, ResearchMode::Chat)
    }

    #[must_use]
    pub const fn state(&self) -> RunState {
        self.state
    }

    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    #[must_use]
    pub fn plan(&self) -> &[PlanStep] {
        &self.plan
    }

    #[must_use]
    pub fn progress(&self) -> &[ProgressEvent] {
        &self.events
    }

    #[must_use]
    pub fn progress_truncated(&self) -> bool {
        self.truncated
    }

    #[must_use]
    pub fn citations(&self) -> &[Citation] {
        &self.citations
    }

    #[must_use]
    pub const fn child_work(&self) -> usize {
        self.children
    }

    /// Select the real sources backing this run. Replaces any prior set.
    pub fn set_sources(&mut self, sources: &[&str]) -> Result<(), ResearchError> {
        if sources.len() > MAX_SOURCES {
            return Err(ResearchError::TooManySources {
                max: MAX_SOURCES,
                actual: sources.len(),
            });
        }
        let mut out = Vec::with_capacity(sources.len());
        for name in sources {
            if name.is_empty() {
                return Err(ResearchError::EmptySourceName);
            }
            if !out.contains(&name.to_string()) {
                out.push(name.to_string());
            }
        }
        if out.is_empty() {
            return Err(ResearchError::NoSources);
        }
        self.sources = out;
        Ok(())
    }

    /// Build the reviewable plan. Requires the native adapter.
    pub fn build_plan(&mut self, steps: &[&str]) -> Result<(), ResearchError> {
        if !self.adapter_available {
            return Err(ResearchError::Unavailable);
        }
        if !matches!(self.state, RunState::Draft | RunState::Planned) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        if self.sources.is_empty() {
            return Err(ResearchError::NoSources);
        }
        if steps.is_empty() {
            return Err(ResearchError::EmptyPlan);
        }
        if steps.len() > MAX_PLAN_STEPS {
            return Err(ResearchError::TooManySteps {
                max: MAX_PLAN_STEPS,
                actual: steps.len(),
            });
        }
        let mut plan = Vec::with_capacity(steps.len());
        for title in steps {
            if title.is_empty() {
                return Err(ResearchError::EmptyStep);
            }
            plan.push(PlanStep {
                title: title.to_string(),
                state: StepState::Pending,
            });
        }
        self.plan = plan;
        self.state = RunState::Planned;
        Ok(())
    }

    /// Start executing the reviewed plan.
    pub fn start(&mut self) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Planned) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        self.state = RunState::Running;
        if let Some(step) = self.plan.iter_mut().find(|s| s.state == StepState::Pending) {
            step.state = StepState::Active;
        }
        Ok(())
    }

    /// Record one bounded progress event against a plan step.
    pub fn push_progress(&mut self, step: &str, detail: &str) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Running | RunState::Steered) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        let slot = self
            .plan
            .iter_mut()
            .find(|s| s.title == step)
            .ok_or_else(|| ResearchError::UnknownStep(step.to_owned()))?;
        if slot.state == StepState::Pending {
            slot.state = StepState::Active;
        }
        if self.events.len() >= MAX_PROGRESS_EVENTS {
            self.events.remove(0);
            self.truncated = true;
        }
        self.events.push(ProgressEvent {
            step: step.to_owned(),
            detail: detail.to_owned(),
        });
        Ok(())
    }

    /// Steer the live run. A note appends a reviewable plan step.
    pub fn steer(&mut self, note: Option<&str>) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Running | RunState::Steered) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        if let Some(note) = note {
            if !note.is_empty() {
                if self.plan.len() >= MAX_PLAN_STEPS {
                    return Err(ResearchError::TooManySteps {
                        max: MAX_PLAN_STEPS,
                        actual: self.plan.len() + 1,
                    });
                }
                self.plan.push(PlanStep {
                    title: note.to_owned(),
                    state: StepState::Pending,
                });
            }
        }
        self.state = RunState::Steered;
        Ok(())
    }

    /// Start one bounded child/tool work item owned by this run.
    pub fn spawn_child(&mut self) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Running | RunState::Steered) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        if self.children >= MAX_CHILD_WORK {
            return Err(ResearchError::TooManyChildren);
        }
        self.children += 1;
        Ok(())
    }

    /// Finish with a cited result. Every claim must bind to a selected
    /// source; unknown sources, out-of-range or uncited claims fail and
    /// leave the run unfinished. Plain chat text is never accepted here.
    pub fn finish(
        &mut self,
        claims: usize,
        citations: Vec<Citation>,
        final_text: &str,
    ) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Running | RunState::Steered) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        if claims > MAX_CLAIMS {
            return Err(ResearchError::TooManyClaims { actual: claims });
        }
        if final_text.is_empty() {
            return Err(ResearchError::EmptyFinal);
        }
        if final_text.len() > MAX_FINAL_BYTES {
            return Err(ResearchError::FinalTooLong);
        }
        for citation in &citations {
            if !self.sources.iter().any(|s| s == &citation.source) {
                return Err(ResearchError::UnknownSource(citation.source.clone()));
            }
            if citation.claim >= claims {
                return Err(ResearchError::ClaimOutOfRange {
                    claim: citation.claim,
                    claims,
                });
            }
        }
        for claim in 0..claims {
            if !citations.iter().any(|c| c.claim == claim) {
                return Err(ResearchError::UncitedClaim(claim));
            }
        }
        for step in &mut self.plan {
            step.state = StepState::Done;
        }
        self.children = 0;
        self.claims = claims;
        self.citations = citations;
        self.final_text = final_text.to_owned();
        self.state = RunState::Done;
        Ok(())
    }

    /// Cancel the run and tear down owned child/tool work.
    pub fn cancel(&mut self) {
        for step in &mut self.plan {
            if step.state == StepState::Active {
                step.state = StepState::Aborted;
            }
        }
        self.children = 0;
        if !matches!(self.state, RunState::Done) {
            self.state = RunState::Cancelled;
        }
    }

    /// Retry a cancelled run: keep sources and plan, drop progress.
    pub fn retry(&mut self) -> Result<(), ResearchError> {
        if !matches!(self.state, RunState::Cancelled) {
            return Err(ResearchError::InvalidState { state: self.state });
        }
        for step in &mut self.plan {
            step.state = StepState::Pending;
        }
        self.events.clear();
        self.truncated = false;
        self.state = RunState::Planned;
        Ok(())
    }

    /// Keyboard-reachable actions for the current state. Every state keeps
    /// mode/source choice reachable; live runs expose steer/cancel;
    /// finished runs expose retry. No pointer-only control.
    #[must_use]
    pub fn keyboard_actions(&self) -> Vec<&'static str> {
        match self.state {
            RunState::Draft => vec!["choose-mode", "choose-sources"],
            RunState::Planned => vec![
                "choose-mode",
                "choose-sources",
                "review-plan",
                "start",
                "cancel",
            ],
            RunState::Running | RunState::Steered => vec![
                "choose-mode",
                "choose-sources",
                "review-plan",
                "steer",
                "cancel",
            ],
            RunState::Done | RunState::Cancelled => vec![
                "choose-mode",
                "choose-sources",
                "review-plan",
                "show-references",
                "retry",
            ],
        }
    }

    /// Polite live-region status: phase state only, never per-event detail
    /// spam. Detail text is intentionally excluded.
    #[must_use]
    pub fn polite_status(&self) -> String {
        let phase = match self.state {
            RunState::Draft => "draft",
            RunState::Planned => "plan ready",
            RunState::Running => "researching",
            RunState::Steered => "steered, researching",
            RunState::Done => "done",
            RunState::Cancelled => "cancelled",
        };
        match self.events.last() {
            Some(last) => format!(
                "{phase}: step {} ({} of {} updates{})",
                last.step,
                self.events.len(),
                MAX_PROGRESS_EVENTS,
                if self.truncated { ", older dropped" } else { "" },
            ),
            None => format!("{phase}: {} sources, {} steps", self.sources.len(), self.plan.len()),
        }
    }

    /// One-line activity summary for the durable record.
    #[must_use]
    pub fn activity_summary(&self) -> String {
        format!(
            "{} sources, {} steps, {} progress updates, {} citations",
            self.sources.len(),
            self.plan.len(),
            self.events.len(),
            self.citations.len(),
        )
    }

    /// Capture the durable record: plan, source set, activity and result.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            mode: self.mode,
            adapter_available: self.adapter_available,
            state: self.state,
            sources: self.sources.clone(),
            plan: self.plan.clone(),
            events: self.events.clone(),
            event_count: self.events.len(),
            truncated: self.truncated,
            claims: self.claims,
            citations: self.citations.clone(),
            final_text: self.final_text.clone(),
        }
    }

    /// Reload a run from its durable record.
    pub fn rehydrate(snapshot: Snapshot) -> Result<Self, ResearchError> {
        if snapshot.sources.len() > MAX_SOURCES {
            return Err(ResearchError::InvalidSnapshot("too many sources".to_owned()));
        }
        if snapshot.plan.len() > MAX_PLAN_STEPS {
            return Err(ResearchError::InvalidSnapshot("too many plan steps".to_owned()));
        }
        if snapshot.events.len() > MAX_PROGRESS_EVENTS || snapshot.event_count > MAX_PROGRESS_EVENTS
        {
            return Err(ResearchError::InvalidSnapshot("too many progress events".to_owned()));
        }
        if snapshot.event_count != snapshot.events.len() {
            return Err(ResearchError::InvalidSnapshot(
                "event count mismatch".to_owned(),
            ));
        }
        if snapshot.claims > MAX_CLAIMS {
            return Err(ResearchError::InvalidSnapshot("too many claims".to_owned()));
        }
        for citation in &snapshot.citations {
            if !snapshot.sources.iter().any(|s| s == &citation.source) {
                return Err(ResearchError::InvalidSnapshot("dangling citation".to_owned()));
            }
            if citation.claim >= snapshot.claims {
                return Err(ResearchError::InvalidSnapshot(
                    "citation out of range".to_owned(),
                ));
            }
        }
        Ok(Self {
            mode: snapshot.mode,
            adapter_available: snapshot.adapter_available,
            state: snapshot.state,
            sources: snapshot.sources,
            plan: snapshot.plan,
            events: snapshot.events,
            truncated: snapshot.truncated,
            children: 0,
            claims: snapshot.claims,
            citations: snapshot.citations,
            final_text: snapshot.final_text,
        })
    }
}
