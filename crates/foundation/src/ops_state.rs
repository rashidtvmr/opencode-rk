//! Ops-state slice plus OPS-009 pure in-memory replay nucleus.
//!
//! `OpsState` is the original named-revision state. The replay checker below is
//! the OPS-009 residual scope only: deterministic ordered matching over a
//! caller-supplied, caller-redacted in-memory cassette. No network, no
//! filesystem, no environment reads, no logging of frame bodies.

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpsState {
    pub name: String,
    pub rev: u64,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OpsStateError {
    #[error("name must not be empty")]
    EmptyName,
}

pub fn make_state(name: &str) -> Result<OpsState, OpsStateError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OpsStateError::EmptyName);
    }
    Ok(OpsState {
        name: trimmed.to_string(),
        rev: 0,
    })
}

pub fn bump(s: &mut OpsState) {
    s.rev = s.rev.saturating_add(1);
}

/// Maximum frames retained by one cassette.
pub const MAX_FRAMES: usize = 1024;
/// Maximum bytes per single request or response string.
pub const MAX_FRAME_BYTES: usize = 65536;

/// One caller-redacted request/response frame. Opaque strings; never scanned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub request: String,
    pub response: String,
}

/// Bounded ordered cassette. `Vec` never exceeds `MAX_FRAMES`; each frame is
/// capped at `MAX_FRAME_BYTES` per side at push time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cassette {
    pub frames: Vec<Frame>,
}

#[derive(Clone, Error, PartialEq, Eq)]
pub enum ReplayError {
    #[error("request is empty")]
    EmptyRequest,
    #[error("too many frames: max {max}, actual {actual}")]
    TooManyFrames { max: usize, actual: usize },
    #[error("mismatch at index {index}")]
    Mismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    #[error("cassette exhausted at index {index}")]
    Exhausted { index: usize },
    #[error("unused frames: {remaining} remaining")]
    UnusedFrames { remaining: usize },
}

// Redacted Debug: Mismatch reports index + lengths only, never frame bodies.
impl std::fmt::Debug for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRequest => write!(f, "EmptyRequest"),
            Self::TooManyFrames { max, actual } => {
                write!(f, "TooManyFrames {{ max: {max}, actual: {actual} }}")
            }
            Self::Mismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "Mismatch {{ index: {index}, expected_len: {}, actual_len: {} }}",
                expected.len(),
                actual.len()
            ),
            Self::Exhausted { index } => write!(f, "Exhausted {{ index: {index} }}"),
            Self::UnusedFrames { remaining } => {
                write!(f, "UnusedFrames {{ remaining: {remaining} }}")
            }
        }
    }
}

impl Cassette {
    #[must_use]
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Push one frame. Every refusal returns `Err` before any mutation, so a
    /// failed push leaves the cassette byte-identical (atomic transition).
    pub fn push_frame(&mut self, request: &str, response: &str) -> Result<(), ReplayError> {
        if request.is_empty() {
            return Err(ReplayError::EmptyRequest);
        }
        if self.frames.len() >= MAX_FRAMES {
            return Err(ReplayError::TooManyFrames {
                max: MAX_FRAMES,
                actual: self.frames.len(),
            });
        }
        if request.len() > MAX_FRAME_BYTES {
            return Err(ReplayError::TooManyFrames {
                max: MAX_FRAME_BYTES,
                actual: request.len(),
            });
        }
        if response.len() > MAX_FRAME_BYTES {
            return Err(ReplayError::TooManyFrames {
                max: MAX_FRAME_BYTES,
                actual: response.len(),
            });
        }
        self.frames.push(Frame {
            request: request.to_string(),
            response: response.to_string(),
        });
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
}

/// Ordered replay over `&Cassette`: returns count matched. Borrows only, so a
/// failed replay cannot mutate the cassette. `strict=true` requires exact
/// length; `strict=false` allows a prefix match. Pure: no clock, env, or I/O.
pub fn replay(cassette: &Cassette, requests: &[&str], strict: bool) -> Result<usize, ReplayError> {
    for (i, req) in requests.iter().enumerate() {
        let Some(frame) = cassette.frames.get(i) else {
            return Err(ReplayError::Exhausted { index: i });
        };
        if frame.request != *req {
            return Err(ReplayError::Mismatch {
                index: i,
                expected: frame.request.clone(),
                actual: (*req).to_string(),
            });
        }
    }
    if strict && cassette.frames.len() > requests.len() {
        return Err(ReplayError::UnusedFrames {
            remaining: cassette.frames.len() - requests.len(),
        });
    }
    Ok(requests.len())
}

#[cfg(test)]
mod tests {
    use super::{replay, Cassette, ReplayError, MAX_FRAMES, MAX_FRAME_BYTES};

    fn three_frame() -> Cassette {
        let mut c = Cassette::new();
        c.push_frame("req-a", "res-a").unwrap();
        c.push_frame("req-b", "res-b").unwrap();
        c.push_frame("req-c", "res-c").unwrap();
        c
    }

    #[test]
    fn t01_happy_path() {
        let c = three_frame();
        assert_eq!(c.len(), 3);
        assert!(!c.is_empty());
        let reqs = ["req-a", "req-b", "req-c"];
        assert_eq!(replay(&c, &reqs, false), Ok(3));
        assert_eq!(replay(&c, &reqs, true), Ok(3));
    }

    #[test]
    fn t02_determinism_and_order() {
        let c = three_frame();
        let reqs = ["req-a", "req-b", "req-c"];
        assert_eq!(replay(&c, &reqs, false), replay(&c, &reqs, false));
        assert_eq!(replay(&c, &reqs, true), replay(&c, &reqs, true));
        let shuffled = ["req-a", "req-c", "req-b"];
        assert!(matches!(
            replay(&c, &shuffled, false),
            Err(ReplayError::Mismatch { index: 1, .. })
        ));
        let prefix = ["req-a", "req-b"];
        assert_eq!(replay(&c, &prefix, false), Ok(2));
        assert_eq!(
            replay(&c, &prefix, true),
            Err(ReplayError::UnusedFrames { remaining: 1 })
        );
    }

    #[test]
    fn t03_caps_refuse() {
        let mut c = Cassette::new();
        for i in 0..MAX_FRAMES {
            c.push_frame(&format!("req-{i}"), &format!("res-{i}"))
                .unwrap();
        }
        assert_eq!(
            c.push_frame("req-overflow", "res-overflow"),
            Err(ReplayError::TooManyFrames {
                max: MAX_FRAMES,
                actual: MAX_FRAMES
            })
        );
        assert_eq!(c.len(), MAX_FRAMES);
        let big = "x".repeat(MAX_FRAME_BYTES + 1);
        let mut c2 = Cassette::new();
        assert!(matches!(
            c2.push_frame(&big, "res"),
            Err(ReplayError::TooManyFrames { .. })
        ));
        assert!(matches!(
            c2.push_frame("req", &big),
            Err(ReplayError::TooManyFrames { .. })
        ));
        assert!(c2.is_empty());
        let reqs: Vec<String> = (0..MAX_FRAMES).map(|i| format!("req-{i}")).collect();
        let refs: Vec<&str> = reqs.iter().map(String::as_str).collect();
        assert_eq!(replay(&c, &refs, true), Ok(MAX_FRAMES));
    }

    #[test]
    fn t04_failure_states_atomic() {
        let mut c = Cassette::new();
        assert_eq!(c.push_frame("", "res"), Err(ReplayError::EmptyRequest));
        assert!(c.is_empty());
        let c = three_frame();
        let past_end = ["req-a", "req-b", "req-c", "req-d"];
        assert_eq!(
            replay(&c, &past_end, false),
            Err(ReplayError::Exhausted { index: 3 })
        );
        let wrong = ["req-a", "WRONG", "req-c"];
        match replay(&c, &wrong, false) {
            Err(ReplayError::Mismatch {
                index,
                expected,
                actual,
            }) => {
                assert_eq!(index, 1);
                assert_eq!(expected, "req-b");
                assert_eq!(actual, "WRONG");
            }
            other => panic!("expected mismatch, got {other:?}"),
        }
        let before = c.clone();
        let _ = replay(&c, &wrong, true);
        let _ = replay(&c, &past_end, true);
        assert_eq!(c, before);
    }

    #[test]
    fn t05_redaction_and_isolation() {
        let decoy = "decoy-sk-live-9f8e7d6c5b4a3939";
        let frame0_req = format!("login {decoy}");
        let mut c = Cassette::new();
        c.push_frame(&frame0_req, "ok").unwrap();
        c.push_frame("req-b", "res-b").unwrap();
        let wrong_actual = "probe-with-decoy-sk-live-0000000000";
        let reqs = [frame0_req.as_str(), wrong_actual];
        let err = replay(&c, &reqs, false).unwrap_err();
        let dbg = format!("{err:?}");
        assert!(!dbg.contains(decoy));
        assert!(!dbg.contains(wrong_actual));
        assert!(!dbg.contains("req-b"));
        assert!(!dbg.contains("login"));
        assert!(dbg.contains('1'));
        assert!(dbg.contains(&"req-b".len().to_string()));
        assert!(dbg.contains(&wrong_actual.len().to_string()));
        let disp = format!("{err}");
        assert!(!disp.contains(decoy));
        assert!(!disp.contains(wrong_actual));
    }
}
