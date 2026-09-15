//! Pure runtime-report summarizer (OPS-009).
//!
//! Aggregates bounded [`RuntimeSample`] slices into a [`RuntimeReport`].
//! No IO, no threads, no allocation beyond the returned report.

use thiserror::Error;

pub const MAX_SAMPLES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSample {
    pub tasks: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReport {
    pub total_tasks: u64,
    pub total_errors: u64,
    pub healthy: bool,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReportError {
    #[error("too many samples: max {max}, actual {actual}")]
    TooManySamples { max: usize, actual: usize },
}

pub fn summarize(samples: &[RuntimeSample]) -> Result<RuntimeReport, ReportError> {
    if samples.len() > MAX_SAMPLES {
        return Err(ReportError::TooManySamples {
            max: MAX_SAMPLES,
            actual: samples.len(),
        });
    }
    let mut total_tasks: u64 = 0;
    let mut total_errors: u64 = 0;
    for s in samples {
        total_tasks = total_tasks.saturating_add(s.tasks);
        total_errors = total_errors.saturating_add(s.errors);
    }
    Ok(RuntimeReport {
        total_tasks,
        total_errors,
        healthy: total_errors == 0,
    })
}
