//! Bounded append-only operation receipt log (evidence collector).

use thiserror::Error;

/// Maximum receipts retained in one [`ReceiptLog`].
pub const MAX_RECEIPTS: usize = 256;
/// Maximum `detail` length in chars (not bytes).
pub const MAX_DETAIL_CHARS: usize = 512;

/// Single operation outcome record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpReceipt {
    pub op: String,
    pub ok: bool,
    pub detail: String,
}

/// Rejection reasons for [`ReceiptLog::push`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReceiptError {
    #[error("operation name is empty")]
    EmptyOp,
    #[error("detail too long: max {max} chars, got {actual}")]
    DetailTooLong { max: usize, actual: usize },
    #[error("too many receipts: max {max}, have {actual}")]
    TooManyReceipts { max: usize, actual: usize },
}

/// Bounded append-only receipt collector.
#[derive(Debug, Default, Clone)]
pub struct ReceiptLog {
    receipts: Vec<OpReceipt>,
}

impl ReceiptLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, r: OpReceipt) -> Result<(), ReceiptError> {
        if r.op.is_empty() {
            return Err(ReceiptError::EmptyOp);
        }
        let actual = r.detail.chars().count();
        if actual > MAX_DETAIL_CHARS {
            return Err(ReceiptError::DetailTooLong {
                max: MAX_DETAIL_CHARS,
                actual,
            });
        }
        if self.receipts.len() >= MAX_RECEIPTS {
            return Err(ReceiptError::TooManyReceipts {
                max: MAX_RECEIPTS,
                actual: self.receipts.len(),
            });
        }
        self.receipts.push(r);
        Ok(())
    }

    pub fn list(&self) -> &[OpReceipt] {
        &self.receipts
    }

    pub fn failures(&self) -> Vec<&OpReceipt> {
        self.receipts.iter().filter(|r| !r.ok).collect()
    }
}
