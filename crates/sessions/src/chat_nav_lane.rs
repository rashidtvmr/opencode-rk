//! WEB-014: chat navigation, search and long-history UX (REQ-040).
//!
//! Pure sidebar/search/history model over caller-owned rows shaped like
//! persisted sessions/messages. No IO, no clock, no network, no threads.
//! Caller owns buffers; every growing result is capped for virtualized
//! rendering. Secrets never enter this module (ids/titles/counts only,
//! message bodies only as caller-supplied search haystacks).
//!
//! Paging starts at the oldest message (`seq` ascending) and advances via
//! an opaque last-seen-`seq` cursor, so the recent provider-context window
//! never reuses the legacy oldest-page shape.
#![forbid(unsafe_code)]

use thiserror::Error;

/// Max sidebar rows retained in the caller-owned buffer.
pub const MAX_NAV_SESSIONS: usize = 200;
/// Max messages returned by one [`page_messages`] call.
pub const MAX_PAGE_ITEMS: usize = 100;
/// Max hits returned by one [`search_chats`] call (virtualized render cap).
pub const MAX_SEARCH_RESULTS: usize = 100;
/// Max search query bytes (byte cap, checked before any scan).
pub const MAX_QUERY_BYTES: usize = 256;
/// Max snippet chars per search hit.
pub const MAX_SNIPPET_CHARS: usize = 80;

/// One sidebar row, shaped like a persisted session summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatEntry {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub temporary: bool,
    pub updated_us: i64,
}

/// One history row, shaped like a persisted message (oldest-first by `seq`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageRow {
    pub id: String,
    pub seq: u64,
    pub body: String,
}

/// One bounded search hit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub session_id: String,
    pub title: String,
    pub snippet: String,
}

/// One bounded message page plus the cursor for the next page, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessagePage {
    pub items: Vec<MessageRow>,
    pub next_cursor: Option<u64>,
}

/// Explicit share availability; never silently downgraded to ordinary chat.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShareState {
    Available,
    UnavailableTemporary,
    UnavailableArchived,
}

/// Keyboard focus targets for sidebar/search/list navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusTarget {
    Sidebar,
    SearchBox,
    MessageList,
}

/// Navigation keys handled by [`move_focus`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavKey {
    FocusSearch,
    Enter,
    Escape,
    Up,
    Down,
}

/// Cooperative cancellation flag for [`search_chats`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchCancel {
    cancelled: bool,
}

impl SearchCancel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ChatNavError {
    #[error("chat id must not be empty")]
    EmptyId,
    #[error("chat not found")]
    NotFound,
    #[error("too many sessions: max {max}, actual {actual}")]
    TooManySessions { max: usize, actual: usize },
    #[error("query too long: max {max}, actual {actual}")]
    QueryTooLong { max: usize, actual: usize },
    #[error("search cancelled")]
    Cancelled,
    #[error("bad nav encoding")]
    BadEncoding,
}

/// Insert or replace a sidebar row. Same id replaces in place (flags and
/// title refresh); new ids append. Empty id rejected; new id at/over the
/// bound rejected.
pub fn upsert_chat(rows: &mut Vec<ChatEntry>, entry: ChatEntry) -> Result<(), ChatNavError> {
    if entry.id.is_empty() {
        return Err(ChatNavError::EmptyId);
    }
    if let Some(slot) = rows.iter_mut().find(|e| e.id == entry.id) {
        *slot = entry;
        return Ok(());
    }
    if rows.len() >= MAX_NAV_SESSIONS {
        return Err(ChatNavError::TooManySessions {
            max: MAX_NAV_SESSIONS,
            actual: rows.len(),
        });
    }
    rows.push(entry);
    Ok(())
}

/// Set the pin flag on an existing row.
pub fn set_pin(rows: &mut [ChatEntry], id: &str, pinned: bool) -> Result<(), ChatNavError> {
    rows.iter_mut()
        .find(|e| e.id == id)
        .map(|e| e.pinned = pinned)
        .ok_or(ChatNavError::NotFound)
}

/// Set the archive flag on an existing row.
pub fn set_archived(rows: &mut [ChatEntry], id: &str, archived: bool) -> Result<(), ChatNavError> {
    rows.iter_mut()
        .find(|e| e.id == id)
        .map(|e| e.archived = archived)
        .ok_or(ChatNavError::NotFound)
}

/// Sidebar order: archived rows excluded unless `include_archived`; pins
/// first, then `updated_us` descending, then id ascending (stable reload).
#[must_use]
pub fn sidebar_order(rows: &[ChatEntry], include_archived: bool) -> Vec<ChatEntry> {
    let mut out: Vec<ChatEntry> = rows
        .iter()
        .filter(|e| include_archived || !e.archived)
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_us.cmp(&a.updated_us))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// Explicit share availability for one row. Temporary and archived chats
/// report their reason instead of degrading to ordinary persisted chat.
#[must_use]
pub fn share_state(entry: &ChatEntry) -> ShareState {
    if entry.temporary {
        ShareState::UnavailableTemporary
    } else if entry.archived {
        ShareState::UnavailableArchived
    } else {
        ShareState::Available
    }
}

/// Unified title/content search over sidebar rows plus caller-supplied
/// `(session_id, body)` haystacks. Temporary and archived rows are never
/// searched (explicit exclusion, not silent conversion). Bounded by
/// [`MAX_QUERY_BYTES`] (checked before scanning) and
/// [`MAX_SEARCH_RESULTS`]; honours [`SearchCancel`].
pub fn search_chats(
    rows: &[ChatEntry],
    bodies: &[(&str, &str)],
    query: &str,
    cancel: &SearchCancel,
) -> Result<Vec<SearchHit>, ChatNavError> {
    if cancel.is_cancelled() {
        return Err(ChatNavError::Cancelled);
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err(ChatNavError::QueryTooLong {
            max: MAX_QUERY_BYTES,
            actual: query.len(),
        });
    }
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    for entry in sidebar_order(rows, false) {
        if out.len() >= MAX_SEARCH_RESULTS {
            break;
        }
        if entry.temporary {
            continue;
        }
        if cancel.is_cancelled() {
            return Err(ChatNavError::Cancelled);
        }
        let body = bodies
            .iter()
            .find(|(id, _)| *id == entry.id)
            .map(|(_, b)| *b)
            .unwrap_or("");
        let title_hit = entry.title.to_lowercase().contains(&needle);
        let body_hit = body.to_lowercase().contains(&needle);
        if title_hit || body_hit {
            let source = if title_hit { entry.title.clone() } else { body.to_owned() };
            out.push(SearchHit {
                session_id: entry.id.clone(),
                title: entry.title.clone(),
                snippet: snippet_of(&source),
            });
        }
    }
    Ok(out)
}

fn snippet_of(text: &str) -> String {
    text.chars().take(MAX_SNIPPET_CHARS).collect()
}

/// Oldest-first message paging. `cursor` is the last seen `seq`
/// (exclusive); `None` starts at the oldest message. `limit` is clamped
/// to `1..=MAX_PAGE_ITEMS`. `next_cursor` is the last returned `seq` when
/// more rows remain, else `None`.
#[must_use]
pub fn page_messages(
    history: &[MessageRow],
    cursor: Option<u64>,
    limit: usize,
) -> MessagePage {
    let limit = limit.clamp(1, MAX_PAGE_ITEMS);
    let start = match cursor {
        None => 0,
        Some(seq) => history.iter().position(|m| m.seq > seq).unwrap_or(history.len()),
    };
    let end = (start + limit).min(history.len());
    let items = history[start..end].to_vec();
    let next_cursor = if end < history.len() {
        items.last().map(|m| m.seq)
    } else {
        None
    };
    MessagePage { items, next_cursor }
}

/// Focus movement for sidebar/search/list keyboard navigation. `Enter`
/// from sidebar or search lands in the message list; `Escape` returns to
/// the sidebar; in-list up/down keys keep focus in place.
#[must_use]
pub fn move_focus(current: FocusTarget, key: NavKey) -> FocusTarget {
    match (current, key) {
        (_, NavKey::FocusSearch) => FocusTarget::SearchBox,
        (FocusTarget::Sidebar, NavKey::Enter) => FocusTarget::MessageList,
        (FocusTarget::SearchBox, NavKey::Enter) => FocusTarget::MessageList,
        (_, NavKey::Escape) => FocusTarget::Sidebar,
        (here, _) => here,
    }
}

/// Discoverable keyboard shortcuts as `(key, label)` pairs.
#[must_use]
pub fn shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/", "Focus chat search"),
        ("Ctrl+K", "Focus chat search"),
        ("Enter", "Open selected chat"),
        ("Escape", "Back to sidebar"),
        ("Up", "Previous chat or message"),
        ("Down", "Next chat or message"),
    ]
}

const NAV_MAGIC: &str = "nav1";

/// Encode sidebar rows to a stable line format for reload persistence.
/// Fields are backslash-escaped, so tabs/newlines/backslashes in ids and
/// titles round-trip exactly.
#[must_use]
pub fn encode_nav(rows: &[ChatEntry]) -> String {
    let mut out = String::from(NAV_MAGIC);
    for e in rows {
        out.push('\n');
        out.push_str(&escape(&e.id));
        out.push('\t');
        out.push_str(&escape(&e.title));
        out.push('\t');
        out.push(if e.pinned { '1' } else { '0' });
        out.push(if e.archived { '1' } else { '0' });
        out.push(if e.temporary { '1' } else { '0' });
        out.push('\t');
        out.push_str(&e.updated_us.to_string());
    }
    out
}

/// Decode [`encode_nav`] output. Any structural deviation is
/// [`ChatNavError::BadEncoding`], never a partial row set.
pub fn decode_nav(text: &str) -> Result<Vec<ChatEntry>, ChatNavError> {
    let mut lines = text.split('\n');
    if lines.next() != Some(NAV_MAGIC) {
        return Err(ChatNavError::BadEncoding);
    }
    let mut out = Vec::new();
    for line in lines {
        let mut cols = split_unescaped(line);
        if cols.len() != 4 {
            return Err(ChatNavError::BadEncoding);
        }
        let updated_us: i64 = cols.pop().unwrap().parse().map_err(|_| ChatNavError::BadEncoding)?;
        let flags = cols.pop().unwrap();
        let mut flag_chars = flags.chars();
        let bit = |c: Option<char>| match c {
            Some('0') => Ok(false),
            Some('1') => Ok(true),
            _ => Err(ChatNavError::BadEncoding),
        };
        let (pinned, archived, temporary) = (bit(flag_chars.next())?, bit(flag_chars.next())?, bit(flag_chars.next())?);
        if flag_chars.next().is_some() {
            return Err(ChatNavError::BadEncoding);
        }
        let title = unescape(&cols[1])?;
        let id = unescape(&cols[0])?;
        if id.is_empty() {
            return Err(ChatNavError::BadEncoding);
        }
        out.push(ChatEntry {
            id,
            title,
            pinned,
            archived,
            temporary,
            updated_us,
        });
    }
    Ok(out)
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

fn unescape(text: &str) -> Result<String, ChatNavError> {
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
            _ => return Err(ChatNavError::BadEncoding),
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
