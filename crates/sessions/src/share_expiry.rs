use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExpiryError {
    #[error("zero ttl")]
    ZeroTtl,
    #[error("expired: now={now} exp={exp}")]
    Expired { now: u64, exp: u64 },
}

pub fn expiry_at(now: u64, ttl_secs: u64) -> Result<u64, ExpiryError> {
    if ttl_secs == 0 {
        return Err(ExpiryError::ZeroTtl);
    }
    Ok(now.saturating_add(ttl_secs))
}

pub fn check_expiry(now: u64, exp: u64) -> Result<(), ExpiryError> {
    if now > exp {
        return Err(ExpiryError::Expired { now, exp });
    }
    Ok(())
}
