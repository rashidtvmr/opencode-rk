//! Pure offline provider-interaction replay checker (OPS-009).
//!
//! In-memory ordered cassette replay only. No network, no filesystem,
//! no environment reads, no logging of frame bodies.

use std::fmt;
use thiserror::Error;

pub const MAX_FRAMES: usize = 1024;
pub const MAX_FRAME_BYTES: usize = 65536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub request: String,
    pub response: String,
}

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

// Redacted Debug: mismatch carries index + lengths only, never bodies.
impl fmt::Debug for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Ordered replay: returns count matched.
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
