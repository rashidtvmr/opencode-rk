//! Pure connectivity probe planner. No IO, no network.

use thiserror::Error;

/// Planned connectivity probe description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbePlan {
    pub integration_id: String,
    pub endpoint: String,
    pub timeout_ms: u64,
}

/// Probe planning failure.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProbeError {
    #[error("integration id must not be empty")]
    EmptyId,
    #[error("endpoint must not be empty")]
    EmptyEndpoint,
    #[error("endpoint must start with https://")]
    BadEndpoint,
    #[error("timeout must be non-zero")]
    ZeroTimeout,
    #[error("too many probes: max {max}, got {actual}")]
    TooManyProbes { max: usize, actual: usize },
}

/// Maximum probes per plan call.
pub const MAX_PROBES: usize = 32;

/// Build probe plans in order. Pure, no IO/network.
pub fn plan_probes(probes: &[(&str, &str, u64)]) -> Result<Vec<ProbePlan>, ProbeError> {
    if probes.len() > MAX_PROBES {
        return Err(ProbeError::TooManyProbes {
            max: MAX_PROBES,
            actual: probes.len(),
        });
    }
    probes
        .iter()
        .map(|(id, endpoint, timeout_ms)| {
            if id.is_empty() {
                return Err(ProbeError::EmptyId);
            }
            if endpoint.is_empty() {
                return Err(ProbeError::EmptyEndpoint);
            }
            if !endpoint.starts_with("https://") {
                return Err(ProbeError::BadEndpoint);
            }
            if *timeout_ms == 0 {
                return Err(ProbeError::ZeroTimeout);
            }
            Ok(ProbePlan {
                integration_id: id.to_string(),
                endpoint: endpoint.to_string(),
                timeout_ms: *timeout_ms,
            })
        })
        .collect()
}
