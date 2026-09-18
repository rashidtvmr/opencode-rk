//! Device/token revocation and credential lifecycle (NET-013).
//!
//! Contract: an epoch-bump (revoke/rotate) invalidates open sockets and
//! queued ops; queued ops carry their epoch and are reauthorized at execution
//! time. Refresh-token reuse and cloned/expired credentials are rejected.
//! Re-enrollment after revocation requires a fresh pairing challenge.
//! Enforcement deadline after revocation is bounded by
//! [`MAX_REVOCATION_PROPAGATION_SECS`]. Std only, no I/O.
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;

/// Maximum delay between revocation and enforcement on every node (secs).
pub const MAX_REVOCATION_PROPAGATION_SECS: u64 = 30;
/// Lifetime of an enrolled credential from enrollment (secs).
pub const CREDENTIAL_TTL_SECS: u64 = 3600;
/// Largest queued operation payload retained (bytes).
pub const MAX_OP_PAYLOAD_BYTES: usize = 4096;
pub const MAX_DEVICES: usize = 64;
pub const MAX_STREAMS: usize = 256;
pub const MAX_QUEUED_OPS: usize = 256;
pub const MAX_CHALLENGES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationError {
    UnknownDevice { device: String },
    Revoked { device: String },
    StaleEpoch { device: String },
    NeedsFreshPairing { device: String },
    RefreshReuse { device: String },
    ClonedCredential { device: String },
    ExpiredCredential { device: String },
    EmptyDevice,
    EmptyCredential,
    TooManyDevices,
    TooManyStreams,
    TooManyChallenges,
    QueueFull,
    PayloadTooLarge { max: usize, actual: usize },
    UnknownOp { op: u64 },
}

impl fmt::Display for RevocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never renders credential material, only device names and bounds.
        match self {
            Self::UnknownDevice { device } => write!(f, "unknown device: {device}"),
            Self::Revoked { device } => write!(f, "device revoked: {device}"),
            Self::StaleEpoch { device } => {
                write!(f, "stale epoch, reauthorize: {device}")
            }
            Self::NeedsFreshPairing { device } => {
                write!(f, "re-enrollment needs fresh pairing: {device}")
            }
            Self::RefreshReuse { device } => write!(f, "refresh token reuse: {device}"),
            Self::ClonedCredential { device } => write!(f, "cloned credential: {device}"),
            Self::ExpiredCredential { device } => write!(f, "expired credential: {device}"),
            Self::EmptyDevice => write!(f, "empty device id"),
            Self::EmptyCredential => write!(f, "empty credential"),
            Self::TooManyDevices => write!(f, "too many devices"),
            Self::TooManyStreams => write!(f, "too many streams"),
            Self::TooManyChallenges => write!(f, "too many outstanding challenges"),
            Self::QueueFull => write!(f, "operation queue full"),
            Self::PayloadTooLarge { max, actual } => {
                write!(f, "payload too large: max {max}, actual {actual}")
            }
            Self::UnknownOp { op } => write!(f, "unknown queued op: {op}"),
        }
    }
}

impl std::error::Error for RevocationError {}

#[derive(Debug, Clone)]
struct DeviceState {
    epoch: u64,
    credential: String,
    prev_credential: Option<String>,
    enrolled: bool,
    revoked: bool,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct StreamRec {
    device: String,
    epoch: u64,
}

#[derive(Debug, Clone)]
struct QueuedOp {
    device: String,
    epoch: u64,
}

/// In-memory revocation registry. All bounds are enforced; no I/O.
#[derive(Debug, Default)]
pub struct RevocationRegistry {
    devices: HashMap<String, DeviceState>,
    streams: HashMap<u64, StreamRec>,
    queued: HashMap<u64, QueuedOp>,
    /// Outstanding challenge -> issuance sequence. Consumed challenges removed.
    issued: HashMap<String, u64>,
    revoked_at: HashMap<String, u64>,
    /// Per-device issuance watermark: challenges issued before this seq are stale.
    revoke_watermark: HashMap<String, u64>,
    next_op: u64,
    next_challenge: u64,
}

fn check_device(device: &str) -> Result<(), RevocationError> {
    if device.is_empty() {
        return Err(RevocationError::EmptyDevice);
    }
    Ok(())
}

impl RevocationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn live(&self, device: &str) -> Result<&DeviceState, RevocationError> {
        let d = self.devices.get(device).ok_or(RevocationError::UnknownDevice {
            device: device.to_string(),
        })?;
        if !d.enrolled {
            return Err(RevocationError::UnknownDevice {
                device: device.to_string(),
            });
        }
        if d.revoked {
            return Err(RevocationError::Revoked {
                device: device.to_string(),
            });
        }
        Ok(d)
    }

    /// Issue a one-time pairing challenge token. Never reuses a live token.
    pub fn issue_pairing_challenge(&mut self, _nonce: &str) -> Result<String, RevocationError> {
        if self.issued.len() >= MAX_CHALLENGES {
            return Err(RevocationError::TooManyChallenges);
        }
        let seq = self.next_challenge;
        self.next_challenge = self.next_challenge.wrapping_add(1);
        let tok = format!("pair-{seq}");
        self.issued.insert(tok.clone(), seq);
        Ok(tok)
    }

    /// Enroll (or re-enroll) a device. The challenge must be outstanding
    /// (issued, unconsumed) and, for a revoked device, issued after the
    /// revocation instant. A consumed challenge cannot be reused.
    pub fn enroll(
        &mut self,
        device: &str,
        credential: &str,
        challenge: &str,
        now_secs: u64,
    ) -> Result<u64, RevocationError> {
        check_device(device)?;
        if credential.is_empty() {
            return Err(RevocationError::EmptyCredential);
        }
        let seq = self.issued.remove(challenge).ok_or(RevocationError::NeedsFreshPairing {
            device: device.to_string(),
        })?;
        if let Some(wm) = self.revoke_watermark.get(device) {
            if seq < *wm {
                return Err(RevocationError::NeedsFreshPairing {
                    device: device.to_string(),
                });
            }
        }
        if !self.devices.contains_key(device) && self.devices.len() >= MAX_DEVICES {
            return Err(RevocationError::TooManyDevices);
        }
        let epoch = self.devices.get(device).map(|d| d.epoch).unwrap_or(0);
        self.devices.insert(
            device.to_string(),
            DeviceState {
                epoch,
                credential: credential.to_string(),
                prev_credential: None,
                enrolled: true,
                revoked: false,
                expires_at: now_secs.saturating_add(CREDENTIAL_TTL_SECS),
            },
        );
        self.revoked_at.remove(device);
        Ok(epoch)
    }

    /// Open a control/subscription stream bound to the device's current epoch.
    pub fn open_stream(&mut self, device: &str, stream: u64) -> Result<(), RevocationError> {
        let epoch = self.live(device)?.epoch;
        if !self.streams.contains_key(&stream) && self.streams.len() >= MAX_STREAMS {
            return Err(RevocationError::TooManyStreams);
        }
        self.streams.insert(
            stream,
            StreamRec {
                device: device.to_string(),
                epoch,
            },
        );
        Ok(())
    }

    pub fn is_stream_open(&self, stream: u64) -> bool {
        self.streams.contains_key(&stream)
    }

    /// Authorize a command: device live, stream open, epochs match.
    pub fn authorize_command(&self, device: &str, stream: u64) -> Result<(), RevocationError> {
        let d = self.live(device)?;
        let s = self.streams.get(&stream).ok_or(RevocationError::StaleEpoch {
            device: device.to_string(),
        })?;
        if s.device != device || s.epoch != d.epoch {
            return Err(RevocationError::StaleEpoch {
                device: device.to_string(),
            });
        }
        Ok(())
    }

    /// Queue an op stamped with the device's current epoch (reauthorize-at-execution).
    pub fn enqueue(&mut self, device: &str, payload: &str) -> Result<u64, RevocationError> {
        let epoch = self.live(device)?.epoch;
        if payload.len() > MAX_OP_PAYLOAD_BYTES {
            return Err(RevocationError::PayloadTooLarge {
                max: MAX_OP_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }
        if self.queued.len() >= MAX_QUEUED_OPS {
            return Err(RevocationError::QueueFull);
        }
        let op = self.next_op;
        self.next_op = self.next_op.wrapping_add(1);
        self.queued.insert(
            op,
            QueuedOp {
                device: device.to_string(),
                epoch,
            },
        );
        Ok(op)
    }

    /// Execute a queued op: reauthorize against the live epoch; stale or
    /// revoked devices are denied and the op is dropped (no replay).
    pub fn execute_queued(&mut self, op: u64) -> Result<(), RevocationError> {
        let q = self.queued.remove(&op).ok_or(RevocationError::UnknownOp { op })?;
        let d = self.devices.get(&q.device).ok_or(RevocationError::UnknownDevice {
            device: q.device.clone(),
        })?;
        if !d.enrolled || d.revoked {
            return Err(RevocationError::Revoked {
                device: q.device.clone(),
            });
        }
        if d.epoch != q.epoch {
            return Err(RevocationError::StaleEpoch {
                device: q.device.clone(),
            });
        }
        Ok(())
    }

    /// Revoke a device: epoch-bump and close its streams. Queued ops are
    /// retained but carry a stale epoch, so execution-time reauthorization
    /// denies them (no silent replay, no silent drop ambiguity for callers
    /// holding the op id). Unrelated devices untouched.
    pub fn revoke(&mut self, device: &str, now_secs: u64) -> Result<usize, RevocationError> {
        let d = self.devices.get_mut(device).ok_or(RevocationError::UnknownDevice {
            device: device.to_string(),
        })?;
        if !d.enrolled {
            return Err(RevocationError::UnknownDevice {
                device: device.to_string(),
            });
        }
        d.revoked = true;
        d.epoch = d.epoch.wrapping_add(1);
        d.prev_credential = None;
        self.revoked_at.insert(device.to_string(), now_secs);
        // Watermark: only challenges issued at/after this seq are fresh.
        self.revoke_watermark.insert(device.to_string(), self.next_challenge);
        let before = self.streams.len();
        self.streams.retain(|_, s| s.device != device);
        let closed = before - self.streams.len();
        Ok(closed)
    }

    /// Rotate credentials: epoch-bump with the same semantics: streams closed,
    /// queued ops retained-but-stale so `execute_queued` denies them.
    pub fn rotate(&mut self, device: &str, new_cred: &str) -> Result<u64, RevocationError> {
        if new_cred.is_empty() {
            return Err(RevocationError::EmptyCredential);
        }
        let d = self.devices.get_mut(device).ok_or(RevocationError::UnknownDevice {
            device: device.to_string(),
        })?;
        if !d.enrolled || d.revoked {
            return Err(RevocationError::Revoked {
                device: device.to_string(),
            });
        }
        d.epoch = d.epoch.wrapping_add(1);
        d.credential = new_cred.to_string();
        d.prev_credential = None;
        self.streams.retain(|_, s| s.device != device);
        Ok(d.epoch)
    }

    /// Refresh rotation: `presented` must equal the current credential exactly
    /// (replay of the previous one => reuse). Cloned/unknown material rejected.
    pub fn refresh(
        &mut self,
        device: &str,
        presented: &str,
        next: &str,
    ) -> Result<(), RevocationError> {
        if next.is_empty() {
            return Err(RevocationError::EmptyCredential);
        }
        let d = self.devices.get_mut(device).ok_or(RevocationError::UnknownDevice {
            device: device.to_string(),
        })?;
        if !d.enrolled || d.revoked {
            return Err(RevocationError::Revoked {
                device: device.to_string(),
            });
        }
        if presented == d.credential {
            d.prev_credential = Some(d.credential.clone());
            d.credential = next.to_string();
            return Ok(());
        }
        if d.prev_credential.as_deref() == Some(presented) {
            return Err(RevocationError::RefreshReuse {
                device: device.to_string(),
            });
        }
        Err(RevocationError::ClonedCredential {
            device: device.to_string(),
        })
    }

    /// Authenticate a credential: must match current, be unexpired, device live.
    pub fn authenticate(
        &self,
        device: &str,
        credential: &str,
        now_secs: u64,
    ) -> Result<(), RevocationError> {
        let d = self.live(device)?;
        if credential != d.credential {
            return Err(RevocationError::ClonedCredential {
                device: device.to_string(),
            });
        }
        if now_secs > d.expires_at {
            return Err(RevocationError::ExpiredCredential {
                device: device.to_string(),
            });
        }
        Ok(())
    }

    pub fn revoked_at(&self, device: &str) -> Option<u64> {
        self.revoked_at.get(device).copied()
    }

    pub fn propagation_deadline(revoked_at: u64) -> u64 {
        revoked_at.saturating_add(MAX_REVOCATION_PROPAGATION_SECS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enrolled() -> (RevocationRegistry, String, String) {
        let mut r = RevocationRegistry::new();
        let c = r.issue_pairing_challenge("n1").unwrap();
        let d = r.enroll("phone", "cred-a", &c, 100).unwrap();
        assert_eq!(d, 0);
        (r, "phone".to_string(), "cred-a".to_string())
    }

    #[test]
    fn revoked_closes_streams_and_rejects_new_commands() {
        let (mut r, dev, _) = enrolled();
        r.open_stream(&dev, 1).unwrap();
        assert!(r.is_stream_open(1));
        let closed = r.revoke(&dev, 200).unwrap();
        assert_eq!(closed, 1);
        assert!(!r.is_stream_open(1), "revocation must close streams");
        assert!(r.authorize_command(&dev, 1).is_err());
        assert!(r.open_stream(&dev, 2).is_err());
    }

    #[test]
    fn queued_ops_reauthorized_post_epoch() {
        let mut r = RevocationRegistry::new();
        let c = r.issue_pairing_challenge("n1").unwrap();
        r.enroll("pc", "cred-a", &c, 100).unwrap();
        let op = r.enqueue("pc", "write").unwrap();
        r.rotate("pc", "cred-b").unwrap();
        assert_eq!(
            r.execute_queued(op),
            Err(RevocationError::StaleEpoch {
                device: "pc".to_string()
            }),
            "pre-epoch queued op must be denied at execution"
        );
        let op2 = r.enqueue("pc", "write2").unwrap();
        assert!(r.execute_queued(op2).is_ok());
    }

    #[test]
    fn refresh_reuse_cloned_and_expired_rejected() {
        let (mut r, dev, _) = enrolled();
        r.refresh(&dev, "cred-a", "cred-b").unwrap();
        assert_eq!(
            r.refresh(&dev, "cred-a", "cred-c"),
            Err(RevocationError::RefreshReuse {
                device: dev.clone()
            })
        );
        assert_eq!(
            r.authenticate(&dev, "clone-x", 150),
            Err(RevocationError::ClonedCredential {
                device: dev.clone()
            })
        );
        assert_eq!(
            r.authenticate(&dev, "cred-b", 100 + CREDENTIAL_TTL_SECS + 1),
            Err(RevocationError::ExpiredCredential { device: dev })
        );
    }

    #[test]
    fn reenroll_needs_fresh_challenge() {
        let mut r = RevocationRegistry::new();
        let c = r.issue_pairing_challenge("n1").unwrap();
        r.enroll("phone", "cred-a", &c, 100).unwrap();
        r.revoke("phone", 200).unwrap();
        assert_eq!(
            r.enroll("phone", "cred-b", &c, 300),
            Err(RevocationError::NeedsFreshPairing {
                device: "phone".to_string()
            })
        );
        let c2 = r.issue_pairing_challenge("n2").unwrap();
        assert!(r.enroll("phone", "cred-b", &c2, 300).is_ok());
    }

    #[test]
    fn propagation_bound_documented() {
        assert!(MAX_REVOCATION_PROPAGATION_SECS > 0 && MAX_REVOCATION_PROPAGATION_SECS <= 300);
        assert_eq!(
            RevocationRegistry::propagation_deadline(1000),
            1000 + MAX_REVOCATION_PROPAGATION_SECS
        );
        let (mut r, dev, _) = enrolled();
        r.revoke(&dev, 200).unwrap();
        assert_eq!(r.revoked_at(&dev), Some(200));
        assert_eq!(
            RevocationRegistry::propagation_deadline(200),
            200 + MAX_REVOCATION_PROPAGATION_SECS
        );
    }

    #[test]
    fn revoke_isolated_to_device() {
        let mut r = RevocationRegistry::new();
        let a = r.issue_pairing_challenge("na").unwrap();
        let b = r.issue_pairing_challenge("nb").unwrap();
        r.enroll("phone", "ca", &a, 100).unwrap();
        r.enroll("pc", "cb", &b, 100).unwrap();
        r.open_stream("phone", 1).unwrap();
        r.open_stream("pc", 2).unwrap();
        let pc_op = r.enqueue("pc", "sync").unwrap();
        r.revoke("phone", 200).unwrap();
        assert!(r.is_stream_open(2));
        assert!(r.authorize_command("pc", 2).is_ok());
        assert!(r.execute_queued(pc_op).is_ok());
    }
}
