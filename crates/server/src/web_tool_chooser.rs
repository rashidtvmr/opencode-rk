//! WEB-012: plugin/app/tool chooser with approvals (pure selection contract).
//!
//! std-only, no I/O, no clock, no network, no threads. Every collection is
//! bounded: registry/selection/pending by hard caps, audit by a retention
//! quota (oldest drops), rehydrate input by a byte budget.

pub const MAX_CAPABILITIES: usize = 64;
pub const MAX_SEARCH_RESULTS: usize = 32;
pub const MAX_SELECTION: usize = 8;
pub const MAX_PENDING_APPROVALS: usize = 16;
pub const MAX_SNAPSHOT_BYTES: usize = 4096;
/// Retention quota for the audit trail; oldest records drop first.
pub const MAX_AUDIT_RECORDS: usize = 64;

const SNAPSHOT_HEADER: &str = "rk-chooser-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    Tool,
    Plugin,
    App,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityState {
    Enabled,
    Disabled,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub id: String,
    pub label: String,
    pub kind: CapabilityKind,
    pub state: CapabilityState,
    pub permission: String,
    pub needs_approval: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooserError {
    Unknown,
    Disabled,
    Denied,
    SelectionFull,
    PendingFull,
    ApprovalNotRequired,
    ApprovalUnknown,
    MalformedSnapshot,
    SnapshotTooLarge,
}

impl std::fmt::Display for ChooserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Unknown => "unknown capability",
            Self::Disabled => "capability disabled",
            Self::Denied => "capability denied",
            Self::SelectionFull => "selection full",
            Self::PendingFull => "pending approvals full",
            Self::ApprovalNotRequired => "approval not required",
            Self::ApprovalUnknown => "unknown approval",
            Self::MalformedSnapshot => "malformed snapshot",
            Self::SnapshotTooLarge => "snapshot too large",
        };
        write!(f, "{s}")
    }
}

impl std::error::Error for ChooserError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decision {
    Approved,
    Denied,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecord {
    pub tool_id: String,
    pub decision: Decision,
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    caps: Vec<Capability>,
}

impl Registry {
    pub fn new(caps: Vec<Capability>) -> Result<Self, ChooserError> {
        if caps.len() > MAX_CAPABILITIES {
            return Err(ChooserError::SelectionFull);
        }
        Ok(Self { caps })
    }

    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.caps.iter().find(|c| c.id == id)
    }

    pub fn capabilities(&self) -> &[Capability] {
        &self.caps
    }

    pub fn len(&self) -> usize {
        self.caps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Deterministic substring search over id and label in registry order,
    /// honoring `limit` and the `MAX_SEARCH_RESULTS` hard cap.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Capability> {
        let q = query.to_lowercase();
        let cap = limit.min(MAX_SEARCH_RESULTS);
        let mut out = Vec::new();
        for c in &self.caps {
            if out.len() >= cap {
                break;
            }
            if c.id.to_lowercase().contains(&q) || c.label.to_lowercase().contains(&q) {
                out.push(c);
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    ids: Vec<String>,
}

impl Selection {
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }

    /// Select an enabled capability. Idempotent reselect. Disabled/denied
    /// are rejected and leave no selection behind; nothing is substituted.
    pub fn select(&mut self, reg: &Registry, id: &str) -> Result<(), ChooserError> {
        if self.ids.iter().any(|s| s == id) {
            return Ok(());
        }
        let cap = reg.get(id).ok_or(ChooserError::Unknown)?;
        match cap.state {
            CapabilityState::Denied => return Err(ChooserError::Denied),
            CapabilityState::Disabled => return Err(ChooserError::Disabled),
            CapabilityState::Enabled => {}
        }
        if self.ids.len() >= MAX_SELECTION {
            return Err(ChooserError::SelectionFull);
        }
        self.ids.push(id.to_string());
        Ok(())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|s| s == id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

/// Turn contract: selected ids in registry order, enabled only. Never injects
/// a fallback tool; denied/disabled can never reactivate into the contract.
pub fn turn_contract(reg: &Registry, sel: &Selection) -> Vec<String> {
    reg.capabilities()
        .iter()
        .filter(|c| c.state == CapabilityState::Enabled && sel.contains(&c.id))
        .map(|c| c.id.clone())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub id: u64,
    pub tool_id: String,
    pub permission: String,
    pub return_to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focus {
    pub return_to: String,
}

#[derive(Debug, Clone, Default)]
pub struct ApprovalQueue {
    pending: Vec<PendingApproval>,
    audit: Vec<DecisionRecord>,
    next_id: u64,
    next_seq: u64,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            audit: Vec::new(),
            next_id: 1,
            next_seq: 0,
        }
    }

    /// Mint a pending approval. Tools needing no approval, and denied or
    /// disabled tools, never mint one.
    pub fn request(
        &mut self,
        reg: &Registry,
        tool_id: &str,
        return_to: &str,
    ) -> Result<u64, ChooserError> {
        let cap = reg.get(tool_id).ok_or(ChooserError::Unknown)?;
        match cap.state {
            CapabilityState::Denied => return Err(ChooserError::Denied),
            CapabilityState::Disabled => return Err(ChooserError::Disabled),
            CapabilityState::Enabled => {}
        }
        if !cap.needs_approval {
            return Err(ChooserError::ApprovalNotRequired);
        }
        if self.pending.len() >= MAX_PENDING_APPROVALS {
            return Err(ChooserError::PendingFull);
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.pending.push(PendingApproval {
            id,
            tool_id: tool_id.to_string(),
            permission: cap.permission.clone(),
            return_to: return_to.to_string(),
        });
        Ok(id)
    }

    pub fn get(&self, id: u64) -> Option<&PendingApproval> {
        self.pending.iter().find(|p| p.id == id)
    }

    pub fn resolve(&mut self, id: u64, approved: bool) -> Result<Focus, ChooserError> {
        let pos = self
            .pending
            .iter()
            .position(|p| p.id == id)
            .ok_or(ChooserError::ApprovalUnknown)?;
        let p = self.pending.remove(pos);
        let focus = Focus {
            return_to: p.return_to.clone(),
        };
        let decision = if approved {
            Decision::Approved
        } else {
            Decision::Denied
        };
        self.push_audit(p.tool_id, decision);
        Ok(focus)
    }

    pub fn cancel(&mut self, id: u64) -> Result<Focus, ChooserError> {
        let pos = self
            .pending
            .iter()
            .position(|p| p.id == id)
            .ok_or(ChooserError::ApprovalUnknown)?;
        let p = self.pending.remove(pos);
        let focus = Focus {
            return_to: p.return_to.clone(),
        };
        self.push_audit(p.tool_id, Decision::Cancelled);
        Ok(focus)
    }

    /// Cancel every pending approval, auditing each as cancelled. Returns the
    /// count dropped; cancelled approvals own nothing afterwards.
    pub fn cancel_all(&mut self) -> usize {
        let items = std::mem::take(&mut self.pending);
        let n = items.len();
        for p in items {
            self.push_audit(p.tool_id, Decision::Cancelled);
        }
        n
    }

    pub fn live_approvals(&self) -> usize {
        self.pending.len()
    }

    pub fn audit(&self) -> &[DecisionRecord] {
        &self.audit
    }

    fn push_audit(&mut self, tool_id: String, decision: Decision) {
        if self.audit.len() >= MAX_AUDIT_RECORDS {
            self.audit.remove(0);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.audit.push(DecisionRecord {
            tool_id,
            decision,
            seq,
        });
    }
}

fn decision_word(d: Decision) -> &'static str {
    match d {
        Decision::Approved => "approved",
        Decision::Denied => "denied",
        Decision::Cancelled => "cancelled",
    }
}

/// Persist selection plus audit trail. Carries ids, decisions and seqs only:
/// labels, permissions and return-to targets never leave memory, so the
/// snapshot holds no credentials.
pub fn snapshot(sel: &Selection, queue: &ApprovalQueue) -> String {
    let mut out = String::from(SNAPSHOT_HEADER);
    out.push('\n');
    for id in sel.ids() {
        out.push_str("selected ");
        out.push_str(id);
        out.push('\n');
    }
    for r in queue.audit() {
        out.push_str("decision ");
        out.push_str(&r.tool_id);
        out.push(' ');
        out.push_str(decision_word(r.decision));
        out.push(' ');
        out.push_str(&r.seq.to_string());
        out.push('\n');
    }
    out
}

/// Rehydrate a snapshot. Atomic: any malformed line fails the whole input.
/// Well-formed lines for unknown ids drop; disabled/denied ids never
/// reactivate into the selection. Oversize input fails before parsing.
pub fn rehydrate(text: &str, reg: &Registry) -> Result<(Selection, Vec<DecisionRecord>), ChooserError> {
    if text.len() > MAX_SNAPSHOT_BYTES {
        return Err(ChooserError::SnapshotTooLarge);
    }
    let mut lines = text.lines();
    match lines.next() {
        Some(h) if h == SNAPSHOT_HEADER => {}
        _ => return Err(ChooserError::MalformedSnapshot),
    }
    let mut sel = Selection::new();
    let mut audit = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["selected", id] => {
                let cap = match reg.get(id) {
                    Some(c) => c,
                    None => continue,
                };
                if cap.state != CapabilityState::Enabled {
                    continue;
                }
                if sel.contains(id) {
                    continue;
                }
                if sel.len() >= MAX_SELECTION {
                    return Err(ChooserError::SelectionFull);
                }
                sel.ids.push(id.to_string());
            }
            ["decision", tool, word, seq] => {
                if reg.get(tool).is_none() {
                    continue;
                }
                let decision = match *word {
                    "approved" => Decision::Approved,
                    "denied" => Decision::Denied,
                    "cancelled" => Decision::Cancelled,
                    _ => return Err(ChooserError::MalformedSnapshot),
                };
                let seq: u64 = seq.parse().map_err(|_| ChooserError::MalformedSnapshot)?;
                audit.push(DecisionRecord {
                    tool_id: tool.to_string(),
                    decision,
                    seq,
                });
            }
            _ => return Err(ChooserError::MalformedSnapshot),
        }
    }
    Ok((sel, audit))
}
