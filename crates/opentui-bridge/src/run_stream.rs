//! Buffered run-stream writer (TS `run/stream.ts` writeSessionOutput commit-to-footer).
//!
//! Upstream `run/stream.ts` is not vendored in this repo; the migration index
//! (`BRIDGE_MIGRATION_DETAIL.md`) cites only the `FooterApi`/`StreamCommit`
//! shape from the run/* footer family. This is new boundary logic: callers
//! `push` output chunks, then `commit` drains the buffer into one footer
//! commit. Fail-closed: an oversize chunk is rejected whole, never partially
//! appended.

/// Byte cap for the pending buffer (64 KiB).
pub const STREAM_CAP_BYTES: usize = 64 * 1024;

/// One committed footer entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCommit {
    /// 1-based commit id, formatted `c-<n>`.
    pub id: String,
    /// Drained buffer text.
    pub text: String,
}

/// Rejection reason for [`StreamBuf::push`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// `pending.len() + chunk.len()` would exceed [`STREAM_CAP_BYTES`].
    /// Nothing was appended.
    Oversize { pending: usize, chunk: usize },
}

/// Pending run-stream output with a byte cap and commit counter.
#[derive(Debug, Clone, Default)]
pub struct StreamBuf {
    pending: String,
    commits: u32,
}

impl StreamBuf {
    /// Empty buffer, zero commits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a chunk; rejects the whole chunk when it would exceed the cap.
    pub fn push(&mut self, chunk: &str) -> Result<(), StreamError> {
        if self.pending.len() + chunk.len() > STREAM_CAP_BYTES {
            return Err(StreamError::Oversize {
                pending: self.pending.len(),
                chunk: chunk.len(),
            });
        }
        self.pending.push_str(chunk);
        Ok(())
    }

    /// Drain pending text into a commit; `None` when the buffer is empty.
    pub fn commit(&mut self) -> Option<StreamCommit> {
        if self.pending.is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.pending);
        self.commits += 1;
        Some(StreamCommit {
            id: format!("c-{}", self.commits),
            text,
        })
    }

    /// Pending byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// True when no text is buffered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Number of commits issued so far.
    #[must_use]
    pub fn commit_count(&self) -> u32 {
        self.commits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_returns_buffered_text() {
        let mut b = StreamBuf::new();
        b.push("hello ").unwrap();
        b.push("world").unwrap();
        let c = b.commit().expect("pending text commits");
        assert_eq!(c.text, "hello world");
        assert!(b.is_empty());
    }

    #[test]
    fn commit_empty_yields_none() {
        let mut b = StreamBuf::new();
        assert_eq!(b.commit(), None);
        assert_eq!(b.commit_count(), 0);
    }

    #[test]
    fn oversize_chunk_is_rejected() {
        let mut b = StreamBuf::new();
        let big = "x".repeat(STREAM_CAP_BYTES + 1);
        let err = b.push(&big).expect_err("over-cap push must fail");
        assert_eq!(
            err,
            StreamError::Oversize {
                pending: 0,
                chunk: STREAM_CAP_BYTES + 1,
            }
        );
    }

    #[test]
    fn rejected_push_appends_nothing() {
        let mut b = StreamBuf::new();
        b.push("keep").unwrap();
        let big = "y".repeat(STREAM_CAP_BYTES);
        assert!(b.push(&big).is_err());
        assert_eq!(b.len(), 4);
        let c = b.commit().expect("original text intact");
        assert_eq!(c.text, "keep");
    }

    #[test]
    fn commit_ids_increment() {
        let mut b = StreamBuf::new();
        b.push("a").unwrap();
        assert_eq!(b.commit().unwrap().id, "c-1");
        b.push("b").unwrap();
        assert_eq!(b.commit().unwrap().id, "c-2");
        assert_eq!(b.commit_count(), 2);
    }

    #[test]
    fn exact_cap_fits_but_one_more_byte_fails() {
        let mut b = StreamBuf::new();
        b.push(&"z".repeat(STREAM_CAP_BYTES)).unwrap();
        assert_eq!(b.len(), STREAM_CAP_BYTES);
        assert!(b.push("!").is_err());
    }
}
