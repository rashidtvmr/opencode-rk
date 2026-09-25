#![forbid(unsafe_code)]

//! Bounded dirty-flag queue for render scheduling.
//!
//! ponytail: fixed caps (8 notes x 256 chars); add overflow
//! counter / oldest-evicts-newest policy when caller needs it.

/// Bounded queue: dirty flag plus up to [`RenderQueue::MAX_NOTES`] notes.
pub struct RenderQueue {
    dirty: bool,
    pending: Vec<String>,
}

impl RenderQueue {
    /// Max number of queued notes.
    pub const MAX_NOTES: usize = 8;
    /// Max chars stored per note.
    pub const MAX_NOTE_LEN: usize = 256;

    /// Empty queue, clean.
    pub fn new() -> Self {
        Self {
            dirty: false,
            pending: Vec::new(),
        }
    }

    /// Mark the view dirty.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Queue a note; `false` when full (note dropped).
    pub fn push_note(&mut self, note: &str) -> bool {
        if self.pending.len() >= Self::MAX_NOTES {
            return false;
        }
        if note.len() > Self::MAX_NOTE_LEN {
            self.pending
                .push(note.chars().take(Self::MAX_NOTE_LEN).collect());
        } else {
            self.pending.push(note.to_string());
        }
        true
    }

    /// Take `(dirty, notes)`, resetting both.
    pub fn take(&mut self) -> (bool, Vec<String>) {
        let dirty = self.dirty;
        self.dirty = false;
        (dirty, std::mem::take(&mut self.pending))
    }
}

impl Default for RenderQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_clean_and_empty() {
        let mut q = RenderQueue::new();
        assert_eq!(q.take(), (false, vec![]));
    }

    #[test]
    fn mark_dirty_take_resets() {
        let mut q = RenderQueue::new();
        q.mark_dirty();
        assert!(q.take().0);
        assert_eq!(q.take(), (false, vec![]));
    }

    #[test]
    fn push_note_caps_at_eight() {
        let mut q = RenderQueue::new();
        for i in 0..8 {
            assert!(q.push_note(&format!("n{i}")));
        }
        assert!(!q.push_note("overflow"));
        assert_eq!(q.take().1.len(), 8);
    }

    #[test]
    fn push_note_truncates_to_256_chars() {
        let mut q = RenderQueue::new();
        let long = "x".repeat(300);
        assert!(q.push_note(&long));
        let (_, notes) = q.take();
        assert_eq!(notes[0].chars().count(), RenderQueue::MAX_NOTE_LEN);
    }

    #[test]
    fn take_returns_notes_and_resets() {
        let mut q = RenderQueue::new();
        q.mark_dirty();
        q.push_note("a");
        let (dirty, notes) = q.take();
        assert!(dirty && notes == vec!["a".to_string()]);
        assert_eq!(q.take(), (false, vec![]));
    }
}
