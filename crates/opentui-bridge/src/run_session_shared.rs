#![forbid(unsafe_code)]
//! Session registry shared by run views (std-only).
//!
//! Evidence (TS `packages/opencode/src/cli/cmd/run/session.shared.ts`):
//! - `createSession`/`resolveSession` build per-session turn state keyed by
//!   session id; `sessionHistory`/`sessionVariant` read it back.
//! - This port keeps the shared registry half: id/title refs with bounded
//!   caps so list views cannot grow without bound.

/// Max chars kept for a session id.
pub const MAX_ID_LEN: usize = 64;
/// Max chars kept for a session title.
pub const MAX_TITLE_LEN: usize = 128;
/// Max refs held in one table.
pub const MAX_SESSIONS: usize = 256;

/// Bounded reference to one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRef {
    /// Session id, truncated to [`MAX_ID_LEN`] chars.
    pub id: String,
    /// Display title, truncated to [`MAX_TITLE_LEN`] chars.
    pub title: String,
}

impl SessionRef {
    /// Build a ref, truncating over-long fields at a char boundary.
    #[must_use]
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: truncate(id, MAX_ID_LEN),
            title: truncate(title, MAX_TITLE_LEN),
        }
    }
}

/// Ordered registry of session refs; insertion order is recency order.
#[derive(Debug, Default, Clone)]
pub struct SessionTable {
    items: Vec<SessionRef>,
}

impl SessionTable {
    /// Empty table.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Number of refs held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when no refs are held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Insert or update `session`. True on insert/update, false when a new
    /// id is rejected because the table holds [`MAX_SESSIONS`] refs.
    pub fn upsert(&mut self, session: SessionRef) -> bool {
        if let Some(slot) = self.items.iter_mut().find(|s| s.id == session.id) {
            slot.title = session.title;
            return true;
        }
        if self.items.len() >= MAX_SESSIONS {
            return false;
        }
        self.items.push(session);
        true
    }

    /// Drop the ref with `id`. True when something was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        match self.items.iter().position(|s| s.id == id) {
            Some(i) => {
                self.items.remove(i);
                true
            }
            None => false,
        }
    }

    /// Borrow the ref with `id`, if present.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&SessionRef> {
        self.items.iter().find(|s| s.id == id)
    }

    /// Most recently inserted ref, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&SessionRef> {
        self.items.last()
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_adds() {
        let mut t = SessionTable::new();
        assert!(t.upsert(SessionRef::new("a", "A")));
        assert_eq!(t.len(), 1);
        assert_eq!(t.get("a").unwrap().title, "A");
    }

    #[test]
    fn upsert_updates_title() {
        let mut t = SessionTable::new();
        t.upsert(SessionRef::new("a", "A"));
        assert!(t.upsert(SessionRef::new("a", "B")));
        assert_eq!(t.len(), 1);
        assert_eq!(t.get("a").unwrap().title, "B");
    }

    #[test]
    fn remove_missing_false() {
        let mut t = SessionTable::new();
        assert!(!t.remove("nope"));
        t.upsert(SessionRef::new("a", "A"));
        assert!(t.remove("a"));
        assert!(!t.remove("a"));
    }

    #[test]
    fn get_none() {
        let t = SessionTable::new();
        assert!(t.get("missing").is_none());
        assert!(t.latest().is_none());
    }

    #[test]
    fn cap_256() {
        let mut t = SessionTable::new();
        for i in 0..MAX_SESSIONS {
            assert!(t.upsert(SessionRef::new(&format!("s{i}"), "T")));
        }
        assert_eq!(t.len(), 256);
        assert!(!t.upsert(SessionRef::new("overflow", "T")));
        assert!(t.get("overflow").is_none());
    }

    #[test]
    fn latest_is_last() {
        let mut t = SessionTable::new();
        t.upsert(SessionRef::new("a", "A"));
        t.upsert(SessionRef::new("b", "B"));
        assert_eq!(t.latest().unwrap().id, "b");
    }
}
