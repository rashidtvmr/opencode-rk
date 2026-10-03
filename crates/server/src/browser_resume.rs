//! Short-lived, origin-bound browser capabilities. Leases exist only in memory
//! and are scoped to one daemon-auth generation.

#![forbid(unsafe_code)]

use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::daemon_auth::{random_bytes, TOKEN_BYTES};

pub(crate) const MAX_LEASE_MS: u64 = 15 * 60 * 1000;
const MAX_LIVE_LEASES: usize = 128;

#[derive(Default)]
pub(crate) struct BrowserResumeRegistry {
    leases: Mutex<Vec<Lease>>,
}

struct Lease {
    capability: String,
    origin: String,
    expires_at_unix_ms: u64,
}

pub(crate) enum IssueError {
    InvalidTtl,
    Capacity,
    Randomness(String),
    Clock,
}

// Manual Debug equivalent to the former derive. Reads the `Randomness` detail
// (keeps the OS error text for diagnostics) so the field is not dead code while
// the variant and error response stay exactly as before.
impl std::fmt::Debug for IssueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTtl => f.write_str("InvalidTtl"),
            Self::Capacity => f.write_str("Capacity"),
            Self::Randomness(detail) => f.debug_tuple("Randomness").field(detail).finish(),
            Self::Clock => f.write_str("Clock"),
        }
    }
}

impl BrowserResumeRegistry {
    pub(crate) fn issue(&self, origin: &str, ttl_ms: u64) -> Result<(String, u64), IssueError> {
        if !(1..=MAX_LEASE_MS).contains(&ttl_ms) {
            return Err(IssueError::InvalidTtl);
        }
        let now = now_ms().ok_or(IssueError::Clock)?;
        let expires_at_unix_ms = now.checked_add(ttl_ms).ok_or(IssueError::Clock)?;
        let mut leases = self.leases.lock().map_err(|_| IssueError::Capacity)?;
        leases.retain(|lease| lease.expires_at_unix_ms > now);
        if leases.len() >= MAX_LIVE_LEASES {
            return Err(IssueError::Capacity);
        }
        let mut capability = None;
        for _ in 0..4 {
            let bytes =
                random_bytes().map_err(|error| IssueError::Randomness(error.to_string()))?;
            let candidate = base64url_encode(&bytes);
            if !leases
                .iter()
                .any(|lease| constant_time_eq(lease.capability.as_bytes(), candidate.as_bytes()))
            {
                capability = Some(candidate);
                break;
            }
        }
        let capability =
            capability.ok_or(IssueError::Randomness("credential collision".to_owned()))?;
        leases.push(Lease {
            capability: capability.clone(),
            origin: origin.to_owned(),
            expires_at_unix_ms,
        });
        Ok((capability, expires_at_unix_ms))
    }

    pub(crate) fn exchange(&self, presented: &str, origin: &str) -> Option<(String, u64)> {
        let now = now_ms()?;
        let mut leases = self.leases.lock().ok()?;
        leases.retain(|lease| lease.expires_at_unix_ms > now);
        let index = leases.iter().position(|lease| {
            constant_time_eq(lease.capability.as_bytes(), presented.as_bytes())
                && lease.origin == origin
        })?;
        let expires_at_unix_ms = leases[index].expires_at_unix_ms;
        let mut replacement = None;
        for _ in 0..4 {
            let bytes = random_bytes().ok()?;
            let candidate = base64url_encode(&bytes);
            if !leases.iter().enumerate().any(|(i, lease)| {
                i != index && constant_time_eq(lease.capability.as_bytes(), candidate.as_bytes())
            }) {
                replacement = Some(candidate);
                break;
            }
        }
        let replacement = replacement?;
        leases.remove(index);
        leases.push(Lease {
            capability: replacement.clone(),
            origin: origin.to_owned(),
            expires_at_unix_ms,
        });
        Some((replacement, expires_at_unix_ms))
    }

    pub(crate) fn verify(&self, presented: &str, request_origin: &str) -> bool {
        let Some(now) = now_ms() else {
            return false;
        };
        let Ok(mut leases) = self.leases.lock() else {
            return false;
        };
        leases.retain(|lease| lease.expires_at_unix_ms > now);
        leases.iter().any(|lease| {
            constant_time_eq(lease.capability.as_bytes(), presented.as_bytes())
                && request_origin == lease.origin
        })
    }

    pub(crate) fn revoke(&self, presented: &str, origin: &str) -> bool {
        let Some(now) = now_ms() else {
            return false;
        };
        let Ok(mut leases) = self.leases.lock() else {
            return false;
        };
        leases.retain(|lease| lease.expires_at_unix_ms > now);
        let Some(index) = leases.iter().position(|lease| {
            constant_time_eq(lease.capability.as_bytes(), presented.as_bytes())
                && lease.origin == origin
        }) else {
            return false;
        };
        leases.remove(index);
        true
    }
}

fn now_ms() -> Option<u64> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()
}

fn base64url_encode(bytes: &[u8; TOKEN_BYTES]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(43);
    for chunk in bytes.chunks_exact(3) {
        let value = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(value & 0x3f) as usize] as char);
    }
    let remainder = bytes.chunks_exact(3).remainder();
    if let [last] = remainder {
        out.push(ALPHABET[(last >> 2) as usize] as char);
        out.push(ALPHABET[((last & 0x03) << 4) as usize] as char);
    } else if let [first, last] = remainder {
        out.push(ALPHABET[(first >> 2) as usize] as char);
        out.push(ALPHABET[(((first & 0x03) << 4) | (last >> 4)) as usize] as char);
        out.push(ALPHABET[((last & 0x0f) << 2) as usize] as char);
    }
    debug_assert_eq!(out.len(), 43);
    out
}

fn constant_time_eq(expected: &[u8], presented: &[u8]) -> bool {
    let mut diff = (expected.len() ^ presented.len()) as u8;
    for i in 0..expected.len().max(presented.len()) {
        diff |= expected.get(i).copied().unwrap_or(0) ^ presented.get(i).copied().unwrap_or(0);
    }
    diff == 0
}
