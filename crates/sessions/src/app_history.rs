//! PAR-003 history workflow types (RED: behavior missing).
//!
//! Owned file: `crates/sessions/src/app_history.rs`. std only.
#![forbid(unsafe_code)]

use std::fmt;

/// Immutable fork provenance: parent session + boundary message. No setters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForkProvenance {
    parent_session_id: String,
    fork_message_seq: u64,
    boundary_message_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvenanceError {
    EmptyParent,
    EmptyBoundary,
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyParent => write!(f, "parent session id must be non-empty"),
            Self::EmptyBoundary => write!(f, "boundary message id must be non-empty"),
        }
    }
}

impl std::error::Error for ProvenanceError {}

impl ForkProvenance {
    pub fn new(
        parent_session_id: String,
        fork_message_seq: u64,
        boundary_message_id: Option<String>,
    ) -> Result<Self, ProvenanceError> {
        if parent_session_id.is_empty() {
            return Err(ProvenanceError::EmptyParent);
        }
        if matches!(&boundary_message_id, Some(b) if b.is_empty()) {
            return Err(ProvenanceError::EmptyBoundary);
        }
        Ok(Self {
            parent_session_id,
            fork_message_seq,
            boundary_message_id,
        })
    }

    /// Borrow the immutable parent session id.
    #[must_use]
    pub fn parent_session_id(&self) -> &str {
        &self.parent_session_id
    }

    /// Sequence in the parent history through which the child was forked.
    #[must_use]
    pub fn fork_message_seq(&self) -> u64 {
        self.fork_message_seq
    }

    /// Boundary message id when the fork cut at a known message.
    #[must_use]
    pub fn boundary_message_id(&self) -> Option<&str> {
        self.boundary_message_id.as_deref()
    }
}

/// Rewind scope: history-only never touches workspace effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewindScope {
    HistoryOnly,
    HistoryAndWorkspace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RewindCursor {
    message_seq: u64,
    scope: RewindScope,
}

impl RewindCursor {
    /// Rewind that truncates message history only; workspace effects stay.
    #[must_use]
    pub fn history_only(message_seq: u64) -> Self {
        Self {
            message_seq,
            scope: RewindScope::HistoryOnly,
        }
    }

    /// Rewind that also rolls back workspace effects (explicit opt-in).
    #[must_use]
    pub fn with_workspace(message_seq: u64) -> Self {
        Self {
            message_seq,
            scope: RewindScope::HistoryAndWorkspace,
        }
    }

    /// Target message sequence to rewind to.
    #[must_use]
    pub fn message_seq(&self) -> u64 {
        self.message_seq
    }

    /// Whether the cursor covers workspace effects or history alone.
    #[must_use]
    pub fn scope(&self) -> RewindScope {
        self.scope
    }

    /// True only for [`RewindScope::HistoryAndWorkspace`].
    #[must_use]
    pub fn touches_workspace(&self) -> bool {
        self.scope == RewindScope::HistoryAndWorkspace
    }
}

/// Compact checker: retained slice must keep tool call/result pairs together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactMessage {
    pub id: String,
    pub kind: CompactKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactKind {
    User,
    Assistant,
    ToolCall { call_id: String },
    ToolResult { call_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactError {
    OrphanResult { call_id: String },
    SplitPair { call_id: String },
}

impl fmt::Display for CompactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OrphanResult { call_id } => {
                write!(f, "tool result without retained call: {call_id}")
            }
            Self::SplitPair { call_id } => {
                write!(f, "tool call without retained result: {call_id}")
            }
        }
    }
}

impl std::error::Error for CompactError {}

/// Retained history keeps every tool call/result pair together: a kept
/// result needs its call, a kept call needs its result.
pub fn check_compact_pairs(retained: &[CompactMessage]) -> Result<(), CompactError> {
    use std::collections::HashSet;
    let mut calls: HashSet<&str> = HashSet::new();
    let mut results: HashSet<&str> = HashSet::new();
    for m in retained {
        match &m.kind {
            CompactKind::ToolCall { call_id } => {
                calls.insert(call_id.as_str());
            }
            CompactKind::ToolResult { call_id } => {
                results.insert(call_id.as_str());
            }
            CompactKind::User | CompactKind::Assistant => {}
        }
    }
    for id in &results {
        if !calls.contains(id) {
            return Err(CompactError::OrphanResult {
                call_id: (*id).to_owned(),
            });
        }
    }
    for id in &calls {
        if !results.contains(id) {
            return Err(CompactError::SplitPair {
                call_id: (*id).to_owned(),
            });
        }
    }
    Ok(())
}

/// Import quotas: bounded incremental import from a read-only copy.
pub const MAX_IMPORT_SESSIONS: usize = 1_000;
pub const MAX_IMPORT_MESSAGES_PER_SESSION: usize = 100_000;
pub const MAX_IMPORT_BYTES: u64 = 512 * 1_048_576;
pub const IMPORT_BATCH_MESSAGES: usize = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportQuotaError {
    TooManySessions { max: usize, actual: usize },
    TooManyMessages { max: usize, actual: usize },
    TooManyBytes { max: u64, actual: u64 },
}

impl fmt::Display for ImportQuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySessions { max, actual } => {
                write!(f, "too many import sessions: max {max}, actual {actual}")
            }
            Self::TooManyMessages { max, actual } => {
                write!(f, "too many import messages: max {max}, actual {actual}")
            }
            Self::TooManyBytes { max, actual } => {
                write!(f, "import too large: max {max}, actual {actual}")
            }
        }
    }
}

impl std::error::Error for ImportQuotaError {}

/// Bound an incremental import from a read-only copy: session count, the
/// largest single-session message count, and total byte budget.
pub fn check_import_quota(
    session_count: usize,
    max_messages_in_any_session: usize,
    total_bytes: u64,
) -> Result<(), ImportQuotaError> {
    if session_count > MAX_IMPORT_SESSIONS {
        return Err(ImportQuotaError::TooManySessions {
            max: MAX_IMPORT_SESSIONS,
            actual: session_count,
        });
    }
    if max_messages_in_any_session > MAX_IMPORT_MESSAGES_PER_SESSION {
        return Err(ImportQuotaError::TooManyMessages {
            max: MAX_IMPORT_MESSAGES_PER_SESSION,
            actual: max_messages_in_any_session,
        });
    }
    if total_bytes > MAX_IMPORT_BYTES {
        return Err(ImportQuotaError::TooManyBytes {
            max: MAX_IMPORT_BYTES,
            actual: total_bytes,
        });
    }
    Ok(())
}

/// Number of fixed-size import batches for `total_messages` (ceil div).
#[must_use]
pub fn import_batches(total_messages: usize) -> usize {
    total_messages.div_ceil(IMPORT_BATCH_MESSAGES)
}

/// Crash-recovery marker: written before history mutation, replayed on start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrashRecoveryMarker {
    session_id: String,
    last_applied_seq: u64,
    epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkerError {
    BadEncoding,
    EmptySession,
}

impl fmt::Display for MarkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadEncoding => write!(f, "bad crash marker encoding"),
            Self::EmptySession => write!(f, "crash marker session id must be non-empty"),
        }
    }
}

impl std::error::Error for MarkerError {}

/// Magic prefix for the crash-marker line encoding.
const MARKER_MAGIC: &str = "apphist-crash1";

impl CrashRecoveryMarker {
    /// New pending-write marker; empty session ids are rejected.
    pub fn new(
        session_id: String,
        last_applied_seq: u64,
        epoch: u64,
    ) -> Result<Self, MarkerError> {
        if session_id.is_empty() || session_id.contains(['\t', '\n']) {
            return Err(MarkerError::EmptySession);
        }
        Ok(Self {
            session_id,
            last_applied_seq,
            epoch,
        })
    }

    /// Session this marker guards.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Last history seq known durable when the marker was written.
    #[must_use]
    pub fn last_applied_seq(&self) -> u64 {
        self.last_applied_seq
    }

    /// Writer epoch; stale epochs are ignored on replay.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Encode to one stable tab-separated line.
    #[must_use]
    pub fn encode(&self) -> String {
        format!(
            "{MARKER_MAGIC}\t{}\t{}\t{}",
            self.session_id, self.last_applied_seq, self.epoch
        )
    }

    /// Decode [`encode`](Self::encode) output; anything else is `BadEncoding`.
    pub fn decode(text: &str) -> Result<Self, MarkerError> {
        let mut parts = text.split('\t');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(m), Some(sid), Some(seq), Some(epoch)) if m == MARKER_MAGIC => {
                if parts.next().is_some() {
                    return Err(MarkerError::BadEncoding);
                }
                let last_applied_seq = seq.parse::<u64>().map_err(|_| MarkerError::BadEncoding)?;
                let epoch = epoch.parse::<u64>().map_err(|_| MarkerError::BadEncoding)?;
                Self::new(sid.to_owned(), last_applied_seq, epoch)
                    .map_err(|_| MarkerError::BadEncoding)
            }
            _ => Err(MarkerError::BadEncoding),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_preserves_provenance() {
        let p = ForkProvenance::new("parent-1".to_owned(), 41, Some("msg-7".to_owned())).unwrap();
        let q = p.clone();
        assert_eq!(q.parent_session_id(), "parent-1");
        assert_eq!(q.fork_message_seq(), 41);
        assert_eq!(q.boundary_message_id(), Some("msg-7"));
        assert_eq!(p, q);
    }

    #[test]
    fn rewind_distinguishes_workspace_effects() {
        let h = RewindCursor::history_only(9);
        let w = RewindCursor::with_workspace(9);
        assert_eq!(h.message_seq(), 9);
        assert!(!h.touches_workspace());
        assert!(w.touches_workspace());
        assert_ne!(h.scope(), w.scope());
    }

    #[test]
    fn compact_retains_pairs() {
        let kept = vec![
            CompactMessage {
                id: "c1".to_owned(),
                kind: CompactKind::ToolCall {
                    call_id: "k".to_owned(),
                },
            },
            CompactMessage {
                id: "r1".to_owned(),
                kind: CompactKind::ToolResult {
                    call_id: "k".to_owned(),
                },
            },
        ];
        assert!(check_compact_pairs(&kept).is_ok());
        let dropped_result = &kept[..1];
        assert!(matches!(
            check_compact_pairs(dropped_result),
            Err(CompactError::SplitPair { .. })
        ));
        let dropped_call = &kept[1..];
        assert!(matches!(
            check_compact_pairs(dropped_call),
            Err(CompactError::OrphanResult { .. })
        ));
    }

    #[test]
    fn import_quota_bounds() {
        assert!(check_import_quota(1, 10, 1024).is_ok());
        assert!(matches!(
            check_import_quota(MAX_IMPORT_SESSIONS + 1, 0, 0),
            Err(ImportQuotaError::TooManySessions { .. })
        ));
        assert!(matches!(
            check_import_quota(1, MAX_IMPORT_MESSAGES_PER_SESSION + 1, 0),
            Err(ImportQuotaError::TooManyMessages { .. })
        ));
        assert!(matches!(
            check_import_quota(1, 0, MAX_IMPORT_BYTES + 1),
            Err(ImportQuotaError::TooManyBytes { .. })
        ));
        assert_eq!(import_batches(0), 0);
        assert_eq!(import_batches(IMPORT_BATCH_MESSAGES), 1);
        assert_eq!(import_batches(IMPORT_BATCH_MESSAGES + 1), 2);
    }

    #[test]
    fn crash_marker_round_trip() {
        let m = CrashRecoveryMarker::new("s-1".to_owned(), 12, 3).unwrap();
        let enc = m.encode();
        assert_eq!(CrashRecoveryMarker::decode(&enc).unwrap(), m);
        assert!(matches!(
            CrashRecoveryMarker::decode("bogus"),
            Err(MarkerError::BadEncoding)
        ));
    }
}
