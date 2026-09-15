//! Integration event buffer.
//!
//! Bounded append-only log of integration lifecycle events. Events keep
//! insertion order; each event carries a 1-based sequence number derived
//! from its position in the buffer.

/// Maximum number of events retained in a buffer.
pub const MAX_INT_EVENTS: usize = 512;

/// A single integration event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntEvent {
    /// Event kind label (non-empty).
    pub kind: String,
    /// 1-based position in the buffer at push time.
    pub seq: u64,
}

/// Error returned by [`push_event`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntEventsError {
    /// The `kind` label was empty.
    #[error("event kind must not be empty")]
    EmptyKind,
    /// The buffer already holds `max` events.
    #[error("too many events: max {max}, actual {actual}")]
    TooManyEvents { max: usize, actual: usize },
}

/// Pushes an event onto the buffer.
///
/// - Empty `kind` yields [`IntEventsError::EmptyKind`].
/// - A full buffer (`len >= MAX_INT_EVENTS`) yields
///   [`IntEventsError::TooManyEvents`].
/// - On success the event's `seq` is `buf.len() + 1` (1-based) and order
///   is preserved.
///
/// # Errors
///
/// - [`IntEventsError::EmptyKind`] if `kind` is empty.
/// - [`IntEventsError::TooManyEvents`] if the buffer is full.
pub fn push_event(buf: &mut Vec<IntEvent>, kind: &str) -> Result<(), IntEventsError> {
    if kind.is_empty() {
        return Err(IntEventsError::EmptyKind);
    }
    if buf.len() >= MAX_INT_EVENTS {
        return Err(IntEventsError::TooManyEvents {
            max: MAX_INT_EVENTS,
            actual: buf.len(),
        });
    }
    let seq = buf.len() as u64 + 1;
    buf.push(IntEvent {
        kind: kind.to_owned(),
        seq,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_push() {
        let mut buf = Vec::new();
        push_event(&mut buf, "x").unwrap();
        assert_eq!(buf[0].seq, 1);
    }
}
