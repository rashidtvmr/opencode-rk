//! Pure redaction-safe provider call recording (PROV-012).

/// One recorded provider call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    pub provider_id: String,
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_ms: u64,
    pub failed: bool,
}

/// Aggregate over a slice of recorded calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingSummary {
    pub calls: u64,
    pub total_input: u64,
    pub total_output: u64,
    pub failures: u64,
}

/// Validation failure for [`summarize_calls`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RecordingError {
    #[error("empty provider id")]
    EmptyProviderId,
    #[error("too many calls: max {max}, actual {actual}")]
    TooManyCalls { max: usize, actual: usize },
}

/// Maximum number of calls accepted by [`summarize_calls`].
pub const MAX_RECORDED_CALLS: usize = 1024;

/// Summarize recorded calls. Pure, no IO/clock.
///
/// # Errors
/// - [`RecordingError::EmptyProviderId`] if any call has an empty `provider_id`.
/// - [`RecordingError::TooManyCalls`] if `calls.len()` exceeds [`MAX_RECORDED_CALLS`].
pub fn summarize_calls(calls: &[RecordedCall]) -> Result<RecordingSummary, RecordingError> {
    if calls.len() > MAX_RECORDED_CALLS {
        return Err(RecordingError::TooManyCalls {
            max: MAX_RECORDED_CALLS,
            actual: calls.len(),
        });
    }
    let mut summary = RecordingSummary {
        calls: 0,
        total_input: 0,
        total_output: 0,
        failures: 0,
    };
    for call in calls {
        if call.provider_id.is_empty() {
            return Err(RecordingError::EmptyProviderId);
        }
        summary.calls = summary.calls.saturating_add(1);
        summary.total_input = summary.total_input.saturating_add(call.input_tokens);
        summary.total_output = summary.total_output.saturating_add(call.output_tokens);
        if call.failed {
            summary.failures = summary.failures.saturating_add(1);
        }
    }
    Ok(summary)
}
