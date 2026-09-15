//! Verification-verdict slice (REL-002 half).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub verdict: String,
    pub checks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    EmptySuite,
    ZeroChecks,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySuite => write!(f, "empty suite"),
            Self::ZeroChecks => write!(f, "zero checks"),
        }
    }
}

impl std::error::Error for VerifyError {}

pub fn build_report(suite: &str, checks: u64, failed: u64) -> Result<VerifyReport, VerifyError> {
    if suite.is_empty() {
        return Err(VerifyError::EmptySuite);
    }
    if checks == 0 {
        return Err(VerifyError::ZeroChecks);
    }
    let verdict = if failed == 0 { "pass" } else { "fail" };
    Ok(VerifyReport {
        verdict: verdict.to_string(),
        checks,
    })
}

pub fn parse_verdict(s: &str) -> Result<Verdict, VerifyError> {
    if s.eq_ignore_ascii_case("pass") {
        Ok(Verdict::Pass)
    } else if s.eq_ignore_ascii_case("fail") {
        Ok(Verdict::Fail)
    } else {
        Err(VerifyError::ZeroChecks)
    }
}
