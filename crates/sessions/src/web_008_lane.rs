//! Branch/fork, retry and reload semantics (WEB-008, REQ-040): pure state.
//!
//! A fork copies parent history through the selected message **inclusive**
//! into exactly one child session, records parent-session plus fork-message
//! provenance, and never mutates the parent. Retry resolves the user request
//! text without appending a fake assistant record; cancel releases the held
//! turn slot. Snapshots round-trip sessions, active branch and provenance.
//!
//! No IO, no clock, no threads, no secrets. Caller owns the session buffer.

#![forbid(unsafe_code)]

use thiserror::Error;

/// Max fork ancestry chain length.
pub const MAX_FORK_DEPTH: usize = 8;
/// Max messages copied into one fork child (inclusive of the boundary).
pub const MAX_FORK_COPY_MESSAGES: usize = 500;
/// Max child title bytes (byte cap, cut at a char boundary).
pub const MAX_TITLE: usize = 512;
/// Max concurrent retry turns (one live provider turn at a time).
pub const MAX_ACTIVE_TURNS: usize = 1;

/// Visible branch boundary roles. Internal system/tool rows are not exposed
/// as user branch actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchRole {
    User,
    Assistant,
    System,
}

/// One history row, shaped like a persisted message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchMessage {
    pub id: String,
    pub seq: u64,
    pub role: BranchRole,
    pub body: String,
}

/// One session row with optional fork provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchSession {
    pub id: String,
    pub title: String,
    pub messages: Vec<BranchMessage>,
    pub parent_session_id: Option<String>,
    pub fork_message_id: Option<String>,
    pub fork_seq: Option<u64>,
}

/// Provenance stored on every fork child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForkProvenance {
    pub parent_session_id: String,
    pub fork_message_id: String,
    pub fork_seq: u64,
}

/// Retry turn lifecycle. A failed turn keeps history untouched and can be
/// retried from a deliberate action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnState {
    Idle,
    Running,
    Failed,
    Cancelled,
    Done,
}

/// One retry turn owned by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryTurn {
    pub state: TurnState,
}

impl RetryTurn {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: TurnState::Idle,
        }
    }
}

impl Default for RetryTurn {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyboard focus targets for fork/popover/navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchFocus {
    ForkButton,
    Popover,
    NewSession,
}

/// Navigation keys handled by [`move_focus`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchKey {
    Enter,
    Escape,
}

/// Reload record: sessions plus the active branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchSnapshot {
    pub sessions: Vec<BranchSession>,
    pub active_session_id: String,
}

/// Failure modes for branch/retry/snapshot operations.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BranchError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("message not found: {0}")]
    MessageNotFound(String),
    #[error("not a visible branch boundary")]
    InvalidBoundary,
    #[error("fork depth exceeded: max {max}")]
    DepthExceeded { max: usize },
    #[error("fork history too large: max {max}, actual {actual}")]
    HistoryTooLarge { max: usize, actual: usize },
    #[error("a retry turn is already running")]
    TurnBusy,
    #[error("no running turn to settle")]
    NoRunningTurn,
    #[error("bad branch encoding")]
    BadEncoding,
}

/// Fork the parent session through `boundary_id` **inclusive** into exactly
/// one child `child_id`. Atomic: every check runs before the child is
/// pushed, so any failure creates no partial child and leaves the source
/// untouched.
pub fn fork_from_message(
    sessions: &mut Vec<BranchSession>,
    parent_id: &str,
    boundary_id: &str,
    child_id: &str,
) -> Result<(), BranchError> {
    let parent = sessions
        .iter()
        .find(|s| s.id == parent_id)
        .ok_or_else(|| BranchError::SessionNotFound(parent_id.to_owned()))?;
    let boundary = parent
        .messages
        .iter()
        .find(|m| m.id == boundary_id)
        .ok_or_else(|| BranchError::MessageNotFound(boundary_id.to_owned()))?;
    if !matches!(boundary.role, BranchRole::User | BranchRole::Assistant) {
        return Err(BranchError::InvalidBoundary);
    }
    let boundary_seq = boundary.seq;
    let copied = parent
        .messages
        .iter()
        .filter(|m| m.seq <= boundary_seq)
        .count();
    if copied > MAX_FORK_COPY_MESSAGES {
        return Err(BranchError::HistoryTooLarge {
            max: MAX_FORK_COPY_MESSAGES,
            actual: copied,
        });
    }
    if fork_depth(sessions, parent_id) >= MAX_FORK_DEPTH {
        return Err(BranchError::DepthExceeded {
            max: MAX_FORK_DEPTH,
        });
    }
    let history: Vec<BranchMessage> = parent
        .messages
        .iter()
        .filter(|m| m.seq <= boundary_seq)
        .cloned()
        .collect();
    let title = branch_child_title(&parent.title);
    sessions.push(BranchSession {
        id: child_id.to_owned(),
        title,
        messages: history,
        parent_session_id: Some(parent_id.to_owned()),
        fork_message_id: Some(boundary_id.to_owned()),
        fork_seq: Some(boundary_seq),
    });
    Ok(())
}

/// Provenance stored on a fork child; `None` for root sessions.
#[must_use]
pub fn fork_provenance(session: &BranchSession) -> Option<ForkProvenance> {
    match (
        session.parent_session_id.clone(),
        session.fork_message_id.clone(),
        session.fork_seq,
    ) {
        (Some(parent_session_id), Some(fork_message_id), Some(fork_seq)) => Some(ForkProvenance {
            parent_session_id,
            fork_message_id,
            fork_seq,
        }),
        _ => None,
    }
}

/// Fork ancestry depth of one session (number of parent links).
#[must_use]
pub fn fork_depth(sessions: &[BranchSession], id: &str) -> usize {
    let mut depth = 0usize;
    let mut current = id.to_owned();
    loop {
        let parent = sessions
            .iter()
            .find(|s| s.id == current)
            .and_then(|s| s.parent_session_id.clone());
        let Some(parent) = parent else {
            break;
        };
        depth += 1;
        current = parent;
        if depth >= MAX_FORK_DEPTH {
            break;
        }
    }
    depth
}

/// Deterministic bounded child title: `Branch: <source title>` cut to
/// [`MAX_TITLE`] bytes at a char boundary.
#[must_use]
pub fn branch_child_title(source: &str) -> String {
    let full = format!("Branch: {source}");
    if full.len() <= MAX_TITLE {
        return full;
    }
    let mut end = MAX_TITLE;
    while end > 0 && !full.is_char_boundary(end) {
        end -= 1;
    }
    full[..end].to_owned()
}

/// Resolve the user request text for a retry without touching history.
/// A user target retries its own body; an assistant target retries the
/// nearest preceding user request. Holds the single turn slot.
pub fn begin_retry(
    active: &mut usize,
    turn: &mut RetryTurn,
    history: &[BranchMessage],
    target_id: &str,
) -> Result<String, BranchError> {
    let target = history
        .iter()
        .find(|m| m.id == target_id)
        .ok_or_else(|| BranchError::MessageNotFound(target_id.to_owned()))?;
    let request = match target.role {
        BranchRole::User => target.body.clone(),
        BranchRole::Assistant => history
            .iter()
            .filter(|m| m.role == BranchRole::User && m.seq < target.seq)
            .max_by_key(|m| m.seq)
            .map(|m| m.body.clone())
            .ok_or(BranchError::InvalidBoundary)?,
        BranchRole::System => return Err(BranchError::InvalidBoundary),
    };
    if *active >= MAX_ACTIVE_TURNS || turn.state == TurnState::Running {
        return Err(BranchError::TurnBusy);
    }
    *active += 1;
    turn.state = TurnState::Running;
    Ok(request)
}

/// Record a provider failure: history untouched, no assistant record
/// appended, turn slot released for a deliberate retry.
pub fn fail_retry(active: &mut usize, turn: &mut RetryTurn) -> Result<(), BranchError> {
    if turn.state != TurnState::Running {
        return Err(BranchError::NoRunningTurn);
    }
    turn.state = TurnState::Failed;
    *active = active.saturating_sub(1);
    Ok(())
}

/// Cancel a live turn and release its provider/runtime slot.
pub fn cancel_retry(active: &mut usize, turn: &mut RetryTurn) -> Result<(), BranchError> {
    if !matches!(turn.state, TurnState::Running | TurnState::Failed) {
        return Err(BranchError::NoRunningTurn);
    }
    turn.state = TurnState::Cancelled;
    *active = active.saturating_sub(1);
    Ok(())
}

/// Finish a live turn by appending the real assistant answer; releases the
/// slot. Only called with provider output, never synthesized.
pub fn complete_retry(
    active: &mut usize,
    turn: &mut RetryTurn,
    history: &mut Vec<BranchMessage>,
    id: &str,
    body: &str,
) -> Result<(), BranchError> {
    if turn.state != TurnState::Running {
        return Err(BranchError::NoRunningTurn);
    }
    let seq = history.iter().map(|m| m.seq).max().unwrap_or(0) + 1;
    history.push(BranchMessage {
        id: id.to_owned(),
        seq,
        role: BranchRole::Assistant,
        body: body.to_owned(),
    });
    turn.state = TurnState::Done;
    *active = active.saturating_sub(1);
    Ok(())
}

/// Screen-reader name of the Fork trigger.
#[must_use]
pub fn fork_accessible_name() -> &'static str {
    "Fork"
}

/// Popover announcement for the branch action.
#[must_use]
pub fn popover_announcement() -> &'static str {
    "Branch in new chat"
}

/// Keyboard-reachable actions per message role. Every boundary keeps
/// edit/retry/fork reachable; no pointer-only control.
#[must_use]
pub fn keyboard_actions(role: BranchRole) -> Vec<&'static str> {
    match role {
        BranchRole::User => vec!["edit", "retry", "fork", "branch-in-new-chat"],
        BranchRole::Assistant => vec!["retry", "fork", "branch-in-new-chat"],
        BranchRole::System => vec![],
    }
}

/// Focus movement for fork/popover/navigation. `Enter` on the Fork button
/// opens the popover; `Enter` in the popover navigates to the new session;
/// `Escape` restores focus to the Fork button.
#[must_use]
pub const fn move_focus(current: BranchFocus, key: BranchKey) -> BranchFocus {
    match (current, key) {
        (BranchFocus::ForkButton, BranchKey::Enter) => BranchFocus::Popover,
        (BranchFocus::Popover, BranchKey::Escape) => BranchFocus::ForkButton,
        (BranchFocus::Popover, BranchKey::Enter) => BranchFocus::NewSession,
        (here, _) => here,
    }
}

/// Navigate to the newly created branch session without focus loss: the
/// active session switches and focus lands in the new session.
pub fn open_branch(
    active: &mut String,
    focus: &mut BranchFocus,
    child_id: &str,
) -> Result<(), BranchError> {
    if child_id.is_empty() {
        return Err(BranchError::BadEncoding);
    }
    *active = child_id.to_owned();
    *focus = BranchFocus::NewSession;
    Ok(())
}

const SNAP_MAGIC: &str = "branch1";

/// Encode sessions plus the active branch to a stable line format.
/// Fields are backslash-escaped, so tabs/newlines/backslashes round-trip.
#[must_use]
pub fn encode_snapshot(sessions: &[BranchSession], active_session_id: &str) -> String {
    let mut out = String::from(SNAP_MAGIC);
    out.push('\n');
    out.push('A');
    out.push('\t');
    out.push_str(&escape(active_session_id));
    for s in sessions {
        out.push('\n');
        out.push('S');
        out.push('\t');
        out.push_str(&escape(&s.id));
        out.push('\t');
        out.push_str(&escape(&s.title));
        out.push('\t');
        out.push_str(&escape(s.parent_session_id.as_deref().unwrap_or("-")));
        out.push('\t');
        out.push_str(&escape(s.fork_message_id.as_deref().unwrap_or("-")));
        out.push('\t');
        out.push_str(&s.fork_seq.map(|q| q.to_string()).unwrap_or("-".to_owned()));
        for m in &s.messages {
            out.push('\n');
            out.push('M');
            out.push('\t');
            out.push_str(&escape(&m.id));
            out.push('\t');
            out.push_str(&m.seq.to_string());
            out.push('\t');
            out.push(match m.role {
                BranchRole::User => 'U',
                BranchRole::Assistant => 'A',
                BranchRole::System => 'S',
            });
            out.push('\t');
            out.push_str(&escape(&m.body));
        }
    }
    out
}

/// Decode [`encode_snapshot`] output. Any structural deviation is
/// [`BranchError::BadEncoding`], never a partial session set.
pub fn decode_snapshot(text: &str) -> Result<BranchSnapshot, BranchError> {
    let mut lines = text.split('\n');
    if lines.next() != Some(SNAP_MAGIC) {
        return Err(BranchError::BadEncoding);
    }
    let active_line = lines.next().ok_or(BranchError::BadEncoding)?;
    let active_session_id = match split_unescaped(active_line) {
        cols if cols.len() == 2 && cols[0] == "A" => unescape(&cols[1])?,
        _ => return Err(BranchError::BadEncoding),
    };
    if active_session_id.is_empty() {
        return Err(BranchError::BadEncoding);
    }
    let mut sessions: Vec<BranchSession> = Vec::new();
    for line in lines {
        let cols = split_unescaped(line);
        match cols.first().map(String::as_str) {
            Some("S") => {
                if cols.len() != 6 {
                    return Err(BranchError::BadEncoding);
                }
                let id = unescape(&cols[1])?;
                let title = unescape(&cols[2])?;
                let parent_raw = unescape(&cols[3])?;
                let fork_msg_raw = unescape(&cols[4])?;
                if id.is_empty() {
                    return Err(BranchError::BadEncoding);
                }
                let fork_seq = if cols[5] == "-" {
                    None
                } else {
                    Some(
                        cols[5]
                            .parse::<u64>()
                            .map_err(|_| BranchError::BadEncoding)?,
                    )
                };
                sessions.push(BranchSession {
                    id,
                    title,
                    messages: Vec::new(),
                    parent_session_id: if parent_raw == "-" {
                        None
                    } else {
                        Some(parent_raw)
                    },
                    fork_message_id: if fork_msg_raw == "-" {
                        None
                    } else {
                        Some(fork_msg_raw)
                    },
                    fork_seq,
                });
            }
            Some("M") => {
                if cols.len() != 5 {
                    return Err(BranchError::BadEncoding);
                }
                let session = sessions.last_mut().ok_or(BranchError::BadEncoding)?;
                let id = unescape(&cols[1])?;
                if id.is_empty() || session.messages.iter().any(|m| m.id == id) {
                    return Err(BranchError::BadEncoding);
                }
                let seq: u64 = cols[2].parse().map_err(|_| BranchError::BadEncoding)?;
                let role = match cols[3].as_str() {
                    "U" => BranchRole::User,
                    "A" => BranchRole::Assistant,
                    "S" => BranchRole::System,
                    _ => return Err(BranchError::BadEncoding),
                };
                let body = unescape(&cols[4])?;
                session.messages.push(BranchMessage {
                    id,
                    seq,
                    role,
                    body,
                });
            }
            _ => return Err(BranchError::BadEncoding),
        }
    }
    if sessions.is_empty() || !sessions.iter().any(|s| s.id == active_session_id) {
        return Err(BranchError::BadEncoding);
    }
    Ok(BranchSnapshot {
        sessions,
        active_session_id,
    })
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> Result<String, BranchError> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            _ => return Err(BranchError::BadEncoding),
        }
    }
    Ok(out)
}

/// Split on unescaped tabs only; escaped sequences stay intact for
/// [`unescape`].
fn split_unescaped(line: &str) -> Vec<String> {
    let mut cols = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            cur.push(c);
            if let Some(next) = chars.next() {
                cur.push(next);
            }
            continue;
        }
        if c == '\t' {
            cols.push(std::mem::take(&mut cur));
            continue;
        }
        cur.push(c);
    }
    cols.push(cur);
    cols
}
