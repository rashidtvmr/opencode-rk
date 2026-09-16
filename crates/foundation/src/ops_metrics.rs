//! Ops metrics-sample slice (OPS-009): bounded push buffer.
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricSample {
    pub name: String,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum MetricError {
    #[error("metric name is empty")]
    EmptyName,
    #[error("too many metrics: max {max}, actual {actual}")]
    TooMany { max: usize, actual: usize },
}

pub const MAX_METRICS: usize = 512;

pub fn push_metric(buf: &mut Vec<MetricSample>, name: &str, value: u64) -> Result<(), MetricError> {
    if name.is_empty() {
        return Err(MetricError::EmptyName);
    }
    if buf.len() >= MAX_METRICS {
        return Err(MetricError::TooMany {
            max: MAX_METRICS,
            actual: buf.len(),
        });
    }
    buf.push(MetricSample {
        name: name.to_string(),
        value,
    });
    Ok(())
}
