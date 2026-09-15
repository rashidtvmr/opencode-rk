use thiserror::Error;

pub const MAX_STAMPS: usize = 1000;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TsError {
    #[error("timestamp must not be negative")]
    NegativeTs,
    #[error("too many timestamps: max {max}, actual {actual}")]
    TooManyStamps { max: usize, actual: usize },
}

pub fn format_ts(micros: i64) -> Result<String, TsError> {
    if micros < 0 {
        return Err(TsError::NegativeTs);
    }
    Ok(format!("{}.{:03}s", micros / 1_000_000, (micros % 1_000_000) / 1000))
}

pub fn format_batch(ts: &[i64]) -> Result<Vec<String>, TsError> {
    if ts.len() > MAX_STAMPS {
        return Err(TsError::TooManyStamps {
            max: MAX_STAMPS,
            actual: ts.len(),
        });
    }
    ts.iter().map(|t| format_ts(*t)).collect()
}
