//! NET-012: conflict-safe reconnection recovery for remote sessions.
//!
//! Pure recovery types for reconnect after mobile backgrounding, tunnel
//! restart, or PC reconnection. Covers the four recovery decisions:
//!
//! * Valid cursor → [`plan_reconnect`] returns [`ReconnectPlan::Rebuild`]
//!   with each replayable event exactly once (no duplicates).
//! * Expired cursor (behind retention, ahead of head, digest mismatch) →
//!   [`ReconnectPlan::Resync`], bounded to [`MAX_RESYNC_EVENTS`] events,
//!   with `invalidate_ui` set so stale UI state is discarded, not patched.
//! * Commands sent around a disconnect report [`CommandOutcome`]:
//!   [`CommandOutcome::Committed`], [`CommandOutcome::Rejected`], or
//!   [`CommandOutcome::Uncertain`]. [`CommandOutcome::Uncertain`] is never
//!   reported as committed: [`classify_command`] requires an ack proof
//!   (`ack_seq`) before returning [`CommandOutcome::Committed`], so the
//!   API never fakes exactly-once effects.
//! * Concurrent edits and control actions go through [`check_version`]:
//!   a stale `expected` version fails with [`VersionConflict`] instead of
//!   silently overwriting.
//! * Offline drafts stay local in [`DraftStore`]. [`can_auto_replay`]
//!   is false for every sensitive (privileged / side-effecting) draft —
//!   including after authority expiry — so sensitive work never
//!   auto-replays; it waits for explicit human re-authorization.
//!
//! Chain-digest semantics mirror `event_cursor::digest_of` at HEAD
//! 5af7884 (FNV-1a 64 over `seq` plus event-type bytes; integrity check
//! only, see `ponytail:` below). This module is self-contained (no crate
//! imports) so it compiles as a single `rustc --test` target.
//!
//! Bounds: at most [`MAX_RECOVERY_EVENTS`] retained events per window,
//! resync capped at [`MAX_RESYNC_EVENTS`], at most [`MAX_DRAFTS`] drafts
//! totalling [`MAX_DRAFT_BYTES_TOTAL`] bytes. No I/O, no clock, no
//! threads, no globals, no logging. The caller owns sockets, snapshots,
//! and authority grants.
//!
//! Digest note (`ponytail:`): [`digest_of`] is FNV-1a 64, a
//! non-cryptographic chain-integrity check only. Ceiling:
//! same-process/adjacent-task replay. Upgrade path: swap in a
//! cryptographic hash when cursors cross trust boundaries.

#![forbid(unsafe_code)]

use std::fmt;

/// Maximum retained events in one recovery window.
pub const MAX_RECOVERY_EVENTS: usize = 512;
/// Hard cap on events a bounded resync may carry.
pub const MAX_RESYNC_EVENTS: usize = 512;
/// Maximum offline drafts retained locally.
pub const MAX_DRAFTS: usize = 64;
/// Maximum bytes in one draft payload.
pub const MAX_DRAFT_BYTES: usize = 64 * 1024;
/// Maximum total draft payload bytes retained locally.
pub const MAX_DRAFT_BYTES_TOTAL: usize = 1024 * 1024;
/// Maximum id token length (command ids, draft ids).
pub const MAX_ID_LEN: usize = 128;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x1000_0000_01b3;

/// Deterministic chain digest over `seq` plus the event-type bytes.
#[must_use]
pub fn digest_of(seq: u64, event_type: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in seq.to_le_bytes().iter().chain(event_type.as_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Recovery position: `seq` is the last applied sequence (0 = genesis),
/// `digest` is [`digest_of`] that event (0 at genesis).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cursor {
    seq: u64,
    digest: u64,
}

impl Cursor {
    /// Genesis cursor: nothing applied yet.
    #[must_use]
    pub const fn genesis() -> Self {
        Self { seq: 0, digest: 0 }
    }

    /// Wrap a raw position. Validity is checked by [`plan_reconnect`].
    #[must_use]
    pub const fn new(seq: u64, digest: u64) -> Self {
        Self { seq, digest }
    }

    /// Position after applying `event`.
    #[must_use]
    pub fn after(event: &StoredEvent) -> Self {
        Self {
            seq: event.seq,
            digest: event.digest,
        }
    }

    /// Last applied sequence (0 at genesis).
    #[must_use]
    pub const fn seq(self) -> u64 {
        self.seq
    }

    /// Chain digest of the last applied event (0 at genesis).
    #[must_use]
    pub const fn digest(self) -> u64 {
        self.digest
    }
}

/// One retained event. `seq` starts at 1, never zero, never reused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEvent {
    /// 1-based sequence.
    pub seq: u64,
    /// Event-type token.
    pub event_type: String,
    /// [`digest_of`] (`seq`, `event_type`).
    pub digest: u64,
}

impl StoredEvent {
    /// Build one retained event at `seq`.
    #[must_use]
    pub fn at(seq: u64, event_type: &str) -> Self {
        Self {
            seq,
            event_type: event_type.to_string(),
            digest: digest_of(seq, event_type),
        }
    }

    /// True when this event rebuilds durable state on replay. Streaming
    /// deltas, progress, presence, and unknown future types never replay
    /// as durable state, so a rebuild can never invent messages.
    #[must_use]
    pub fn is_replayable(&self) -> bool {
        matches!(
            self.event_type.as_str(),
            "session.created"
                | "session.renamed"
                | "session.archived"
                | "message.appended"
                | "message.compacted"
                | "tool.completed"
                | "permission.granted"
                | "permission.denied"
        )
    }
}

/// Recovery failures. Variant names only; no payload carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    /// Malformed cursor (zero `seq` with nonzero digest).
    BadCursor,
    /// Draft payload exceeds [`MAX_DRAFT_BYTES`].
    DraftTooLarge,
    /// Draft store is full (count or byte bound).
    DraftStoreFull,
    /// Invalid id token (empty, too long, or bad charset).
    InvalidId,
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadCursor => write!(f, "bad cursor"),
            Self::DraftTooLarge => write!(f, "draft too large"),
            Self::DraftStoreFull => write!(f, "draft store full"),
            Self::InvalidId => write!(f, "invalid id"),
        }
    }
}

impl std::error::Error for RecoveryError {}

/// Reconnect decision for one cursor against the retained window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconnectPlan {
    /// Cursor is valid: apply these replayable events with `seq` greater
    /// than the cursor, each exactly once, in `seq` order.
    Rebuild { events: Vec<StoredEvent> },
    /// Cursor is expired (behind retention, ahead of head, or digest
    /// mismatch): discard stale UI state (`invalidate_ui`) and take a
    /// fresh snapshot, then apply at most `bound` recent events.
    Resync {
        from_seq: u64,
        bound: usize,
        invalidate_ui: bool,
    },
}

/// Decide rebuild vs bounded resync for `cursor` against the retained
/// `window` (oldest-first, each `seq` unique, `len <= MAX_RECOVERY_EVENTS`).
///
/// A valid cursor (genesis, or `seq`/`digest` matching a retained event)
/// rebuilds with no duplicates. Anything else resyncs within
/// [`MAX_RESYNC_EVENTS`]; a malformed cursor is [`RecoveryError::BadCursor`].
pub fn plan_reconnect(
    cursor: &Cursor,
    window: &[StoredEvent],
) -> Result<ReconnectPlan, RecoveryError> {
    if cursor.seq == 0 {
        if cursor.digest != 0 {
            return Err(RecoveryError::BadCursor);
        }
    } else if !window
        .iter()
        .any(|e| e.seq == cursor.seq && e.digest == cursor.digest)
    {
        return Ok(bounded_resync(window));
    }
    let mut events = Vec::new();
    for event in window {
        if event.seq > cursor.seq && event.is_replayable() {
            events.push(event.clone());
        }
    }
    Ok(ReconnectPlan::Rebuild { events })
}

fn bounded_resync(window: &[StoredEvent]) -> ReconnectPlan {
    let from_seq = window.first().map_or(0, |e| e.seq);
    ReconnectPlan::Resync {
        from_seq,
        bound: window.len().min(MAX_RESYNC_EVENTS),
        invalidate_ui: true,
    }
}

/// Outcome of one command sent around a disconnect. There is no
/// exactly-once variant: without an ack proof the outcome stays
/// [`CommandOutcome::Uncertain`] until explicitly reconciled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Server acknowledged the command at `seq`. Requires ack proof;
    /// never inferred from a send alone.
    Committed { seq: u64 },
    /// Server explicitly rejected the command.
    Rejected { reason: String },
    /// Fate unknown (sent before disconnect, no ack, no reject). The
    /// caller must reconcile before retrying or claiming effect.
    Uncertain { command_id: String },
}

impl CommandOutcome {
    /// True only for [`CommandOutcome::Committed`]. In particular,
    /// [`CommandOutcome::Uncertain`] is never committed.
    #[must_use]
    pub const fn is_committed(&self) -> bool {
        matches!(self, Self::Committed { .. })
    }

    /// True for [`CommandOutcome::Committed`] and
    /// [`CommandOutcome::Rejected`]; [`CommandOutcome::Uncertain`]
    /// still needs reconciliation.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        !matches!(self, Self::Uncertain { .. })
    }
}

impl fmt::Display for CommandOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Committed { seq } => write!(f, "committed at seq {seq}"),
            Self::Rejected { reason } => write!(f, "rejected: {reason}"),
            Self::Uncertain { command_id } => {
                write!(f, "uncertain: {command_id} needs reconciliation")
            }
        }
    }
}

/// Classify one pending command. `ack_seq` is the server ack proof;
/// `rejected` is an explicit server rejection. A command that was merely
/// sent — or whose ack was lost in the disconnect — stays
/// [`CommandOutcome::Uncertain`]; it is never promoted to
/// [`CommandOutcome::Committed`] without proof.
#[must_use]
pub fn classify_command(command_id: &str, ack_seq: Option<u64>, rejected: bool) -> CommandOutcome {
    if rejected {
        return CommandOutcome::Rejected {
            reason: "rejected around disconnect".to_string(),
        };
    }
    match ack_seq {
        Some(seq) if seq > 0 => CommandOutcome::Committed { seq },
        _ => CommandOutcome::Uncertain {
            command_id: command_id.to_string(),
        },
    }
}

/// Stale-write guard failure: `expected` no longer matches `actual`.
/// Carry versions only; no content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionConflict {
    /// Version the writer based its edit on.
    pub expected: u64,
    /// Current version at the authority.
    pub actual: u64,
}

impl fmt::Display for VersionConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "version conflict: expected {}, actual {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for VersionConflict {}

/// Conflict check for concurrent edits and control actions: the write
/// proceeds only when `expected` equals `actual`. Returns the next
/// version (`actual + 1`, saturating) on success.
pub fn check_version(expected: u64, actual: u64) -> Result<u64, VersionConflict> {
    if expected == actual {
        Ok(actual.saturating_add(1).max(1))
    } else {
        Err(VersionConflict { expected, actual })
    }
}

/// One offline draft. `sensitive` marks privileged or side-effecting
/// work (control actions, sends, grants) that must never auto-replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineDraft {
    /// Stable draft id (nonempty, `<= MAX_ID_LEN`, charset-checked).
    pub id: String,
    /// Local payload bytes (`<= MAX_DRAFT_BYTES`).
    pub bytes: Vec<u8>,
    /// True for privileged / side-effecting drafts.
    pub sensitive: bool,
}

/// Replay disposition of one offline draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftDisposition {
    /// Retained locally only; replay needs explicit human re-authorization.
    KeepLocalOnly,
    /// Safe to replay automatically once reconnected.
    MayReplay,
}

/// Decide whether `draft` may auto-replay. Sensitive drafts always stay
/// [`DraftDisposition::KeepLocalOnly`] — including when `authority_valid`
/// is false after authority expiry — so privileged side effects never
/// auto-replay. Non-sensitive drafts may replay only while the authority
/// grant is still valid.
#[must_use]
pub const fn draft_disposition(draft: &OfflineDraft, authority_valid: bool) -> DraftDisposition {
    if draft.sensitive || !authority_valid {
        DraftDisposition::KeepLocalOnly
    } else {
        DraftDisposition::MayReplay
    }
}

/// True only when the draft may auto-replay; sensitive drafts never do.
#[must_use]
pub const fn can_auto_replay(draft: &OfflineDraft, authority_valid: bool) -> bool {
    matches!(
        draft_disposition(draft, authority_valid),
        DraftDisposition::MayReplay
    )
}

fn valid_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_ID_LEN {
        return false;
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Bounded local-only store for offline drafts. Drafts are preserved
/// locally (never silently dropped); overflow and oversize payloads fail
/// closed instead. Never auto-replays: replay goes through
/// [`can_auto_replay`] per draft.
#[derive(Clone, Debug, Default)]
pub struct DraftStore {
    drafts: Vec<OfflineDraft>,
    bytes: usize,
}

impl DraftStore {
    /// Empty store; no allocation beyond the vec.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drafts: Vec::new(),
            bytes: 0,
        }
    }

    /// Preserve one draft locally. Same `id` replaces in place (upsert,
    /// no dup); anything else fails closed without dropping history.
    pub fn store(&mut self, draft: OfflineDraft) -> Result<(), RecoveryError> {
        if !valid_id(&draft.id) {
            return Err(RecoveryError::InvalidId);
        }
        if draft.bytes.len() > MAX_DRAFT_BYTES {
            return Err(RecoveryError::DraftTooLarge);
        }
        if let Some(slot) = self.drafts.iter_mut().find(|d| d.id == draft.id) {
            self.bytes = self.bytes.saturating_sub(slot.bytes.len());
            let additional = draft.bytes.len();
            if self.bytes.saturating_add(additional) > MAX_DRAFT_BYTES_TOTAL {
                self.bytes = self.bytes.saturating_add(slot.bytes.len());
                return Err(RecoveryError::DraftStoreFull);
            }
            self.bytes = self.bytes.saturating_add(additional);
            *slot = draft;
            return Ok(());
        }
        if self.drafts.len() >= MAX_DRAFTS
            || self.bytes.saturating_add(draft.bytes.len()) > MAX_DRAFT_BYTES_TOTAL
        {
            return Err(RecoveryError::DraftStoreFull);
        }
        self.bytes = self.bytes.saturating_add(draft.bytes.len());
        self.drafts.push(draft);
        Ok(())
    }

    /// Look up a preserved draft by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&OfflineDraft> {
        self.drafts.iter().find(|d| d.id == id)
    }

    /// Draft count (`<= MAX_DRAFTS`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.drafts.len()
    }

    /// True when nothing is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }

    /// Retained payload bytes (`<= MAX_DRAFT_BYTES_TOTAL`).
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(n: u64) -> Vec<StoredEvent> {
        (1..=n).map(|s| StoredEvent::at(s, "message.appended")).collect()
    }

    #[test]
    fn valid_cursor_rebuilds_same_session_without_duplicates() {
        let log = window(3);
        let mut cursor = Cursor::genesis();
        let first = plan_reconnect(&cursor, &log).unwrap();
        let ReconnectPlan::Rebuild { events } = first else {
            panic!("valid cursor must rebuild");
        };
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[2].seq, 3);
        cursor = Cursor::after(events.last().unwrap());
        let second = plan_reconnect(&cursor, &log).unwrap();
        let ReconnectPlan::Rebuild { events } = second else {
            panic!("caught-up cursor must still rebuild");
        };
        assert!(events.is_empty(), "replay after catch-up must not duplicate");
        let mid = Cursor::after(&log[0]);
        let tail = plan_reconnect(&mid, &log).unwrap();
        let ReconnectPlan::Rebuild { events } = tail else {
            panic!("mid cursor must rebuild");
        };
        assert_eq!(vec![events[0].seq, events[1].seq], vec![2, 3]);
    }

    #[test]
    fn expired_cursor_triggers_bounded_resync_and_invalidates_ui() {
        let full = window(MAX_RECOVERY_EVENTS as u64 + 1);
        let retained: Vec<StoredEvent> = full[1..].to_vec();
        assert_eq!(retained.len(), MAX_RECOVERY_EVENTS);
        let stale = Cursor::new(full[0].seq, full[0].digest);
        let plan = plan_reconnect(&stale, &retained).unwrap();
        let ReconnectPlan::Resync {
            from_seq,
            bound,
            invalidate_ui,
        } = plan
        else {
            panic!("expired cursor must resync");
        };
        assert!(invalidate_ui, "resync must invalidate stale UI state");
        assert!(bound <= MAX_RESYNC_EVENTS);
        assert_eq!(from_seq, retained[0].seq);
        let ahead = Cursor::new(u64::MAX, 0x9e37);
        assert!(matches!(
            plan_reconnect(&ahead, &retained).unwrap(),
            ReconnectPlan::Resync { .. }
        ));
        let tampered = Cursor::new(retained[0].seq, retained[0].digest ^ 1);
        assert!(matches!(
            plan_reconnect(&tampered, &retained).unwrap(),
            ReconnectPlan::Resync { .. }
        ));
        assert_eq!(
            plan_reconnect(&Cursor::new(0, 7), &retained),
            Err(RecoveryError::BadCursor)
        );
    }

    #[test]
    fn uncertain_outcome_is_never_claimed_committed() {
        let lost_ack = classify_command("cmd-1", None, false);
        assert_eq!(
            lost_ack,
            CommandOutcome::Uncertain {
                command_id: "cmd-1".to_string()
            }
        );
        assert!(!lost_ack.is_committed());
        assert!(!lost_ack.is_settled());
        assert!(!classify_command("cmd-2", Some(0), false).is_committed());
        let rejected = classify_command("cmd-3", Some(9), true);
        assert!(matches!(rejected, CommandOutcome::Rejected { .. }));
        assert!(!rejected.is_committed());
        assert!(!format!("{lost_ack}").contains("committed"));
    }

    #[test]
    fn committed_requires_ack_proof() {
        let outcome = classify_command("cmd-9", Some(42), false);
        assert_eq!(outcome, CommandOutcome::Committed { seq: 42 });
        assert!(outcome.is_committed());
        assert!(outcome.is_settled());
    }

    #[test]
    fn version_conflict_detects_concurrent_edits() {
        assert_eq!(check_version(7, 7), Ok(8));
        assert_eq!(
            check_version(7, 9),
            Err(VersionConflict {
                expected: 7,
                actual: 9
            })
        );
        assert_eq!(
            format!("{}", VersionConflict { expected: 7, actual: 9 }),
            "version conflict: expected 7, actual 9"
        );
    }

    #[test]
    fn stale_privileged_draft_never_auto_replays() {
        let privileged = OfflineDraft {
            id: "draft-1".to_string(),
            bytes: b"grant access".to_vec(),
            sensitive: true,
        };
        assert!(!can_auto_replay(&privileged, true));
        assert!(!can_auto_replay(&privileged, false));
        assert_eq!(
            draft_disposition(&privileged, false),
            DraftDisposition::KeepLocalOnly
        );
        let plain = OfflineDraft {
            id: "draft-2".to_string(),
            bytes: b"note text".to_vec(),
            sensitive: false,
        };
        assert!(can_auto_replay(&plain, true));
        assert!(!can_auto_replay(&plain, false));
    }

    #[test]
    fn offline_drafts_preserved_locally_within_bounds() {
        let mut store = DraftStore::new();
        let draft = OfflineDraft {
            id: "draft-1".to_string(),
            bytes: b"unsent note".to_vec(),
            sensitive: true,
        };
        store.store(draft.clone()).unwrap();
        assert_eq!(store.get("draft-1"), Some(&draft));
        assert_eq!(store.len(), 1);
        store.store(draft).unwrap();
        assert_eq!(store.len(), 1, "same id must upsert, not duplicate");
        assert_eq!(
            store.store(OfflineDraft {
                id: String::new(),
                bytes: vec![],
                sensitive: false
            }),
            Err(RecoveryError::InvalidId)
        );
        assert_eq!(
            store.store(OfflineDraft {
                id: "big".to_string(),
                bytes: vec![0u8; MAX_DRAFT_BYTES + 1],
                sensitive: false
            }),
            Err(RecoveryError::DraftTooLarge)
        );
        assert_eq!(store.len(), 1, "failed stores must not drop history");
    }
}
