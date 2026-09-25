#![forbid(unsafe_code)]
//! Bounded stream session accumulator (TS `run/stream.ts` commit flow).
//!
//! `run_stream.rs` drains a byte-capped buffer into footer commits;
//! this session keeps the raw chunks (max 512 x 8 KiB) until `finish()`.
//! Fail-closed: pushes after `finish()`, oversize chunks, and over-cap
//! sessions are rejected whole via `Err`.

/// Max chunks retained per session.
pub const MAX_CHUNKS: usize = 512;
/// Max bytes per chunk (8 KiB).
pub const MAX_CHUNK_BYTES: usize = 8 * 1024;
/// Max chars returned by [`StreamSession::text`] (64 KiB chars).
pub const MAX_TEXT_CHARS: usize = 64 * 1024;

/// In-memory stream session: ordered chunks plus a terminal flag.
#[derive(Debug, Clone, Default)]
pub struct StreamSession {
    pub chunks: Vec<String>,
    pub done: bool,
}

impl StreamSession {
    /// Empty, unfinished session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a chunk; errs after `finish()`, when over 8 KiB, or past 512 chunks.
    pub fn push(&mut self, chunk: &str) -> Result<(), String> {
        if self.done {
            return Err("session finished".to_string());
        }
        if chunk.len() > MAX_CHUNK_BYTES {
            return Err(format!(
                "chunk {} bytes exceeds {MAX_CHUNK_BYTES}",
                chunk.len()
            ));
        }
        if self.chunks.len() >= MAX_CHUNKS {
            return Err(format!("session full at {MAX_CHUNKS} chunks"));
        }
        self.chunks.push(chunk.to_string());
        Ok(())
    }

    /// Mark the session done; idempotent.
    pub fn finish(&mut self) {
        self.done = true;
    }

    /// True once `finish()` ran.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Number of chunks pushed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// True when no chunks pushed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Concatenated chunks, truncated to 64 KiB chars.
    #[must_use]
    pub fn text(&self) -> String {
        let out = self.chunks.concat();
        if out.chars().count() > MAX_TEXT_CHARS {
            out.chars().take(MAX_TEXT_CHARS).collect()
        } else {
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut s = StreamSession::new();
        s.push("hello ").unwrap();
        s.push("world").unwrap();
        assert_eq!(s.len(), 2);
        assert!(!s.is_done());
    }

    #[test]
    fn done_errs() {
        let mut s = StreamSession::new();
        s.push("a").unwrap();
        s.finish();
        assert!(s.is_done());
        assert_eq!(s.push("b"), Err("session finished".to_string()));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn finish_idempotent() {
        let mut s = StreamSession::new();
        s.finish();
        s.finish();
        assert!(s.is_done());
        assert!(s.push("x").is_err());
    }

    #[test]
    fn text_concat() {
        let mut s = StreamSession::new();
        s.push("foo").unwrap();
        s.push("bar").unwrap();
        assert_eq!(s.text(), "foobar");
    }

    #[test]
    fn text_cap() {
        let mut s = StreamSession::new();
        let big = "a".repeat(MAX_CHUNK_BYTES);
        for _ in 0..9 {
            s.push(&big).unwrap();
        }
        let t = s.text();
        assert_eq!(t.chars().count(), MAX_TEXT_CHARS);
    }
}
