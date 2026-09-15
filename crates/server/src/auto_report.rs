//! Auto-generated status report builder (AUTO report slice).

/// Maximum number of lines a single report may carry.
pub const MAX_REPORT_LINES: usize = 500;

/// Owned, validated report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoReport {
    pub title: String,
    pub lines: Vec<String>,
}

/// Failure modes for [`build_report`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportError {
    EmptyTitle,
    TooManyLines { max: usize, actual: usize },
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTitle => write!(f, "empty title"),
            Self::TooManyLines { max, actual } => {
                write!(f, "too many lines: max {max}, got {actual}")
            }
        }
    }
}

impl std::error::Error for ReportError {}

/// Validate `title` and copy `lines` into an owned report.
///
/// Trims surrounding whitespace from the title; a title that is empty
/// after trimming is rejected. More than [`MAX_REPORT_LINES`] lines is
/// rejected without copying.
pub fn build_report(title: &str, lines: &[&str]) -> Result<AutoReport, ReportError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(ReportError::EmptyTitle);
    }
    if lines.len() > MAX_REPORT_LINES {
        return Err(ReportError::TooManyLines {
            max: MAX_REPORT_LINES,
            actual: lines.len(),
        });
    }
    Ok(AutoReport {
        title: trimmed.to_string(),
        lines: lines.iter().map(|s| (*s).to_string()).collect(),
    })
}
