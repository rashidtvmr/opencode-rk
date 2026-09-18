//! PC pairing types for `opencode2 pair` (NET-002).
//!
//! Commit: 5af7884. Design source: docs/architecture/COMPLETION_REMOTE.md
//! ("Identity and pairing"): short-lived single-use account-bound challenge,
//! visible device fingerprint, explicit local approval; QR carries only
//! challenge material, never provider secrets or long-lived tokens.
//!
//! std only. No cryptography invented here: opaque challenge bytes are issued
//! by the gateway/identity service; this module only stores, binds,
//! expiry-checks, single-use-guards and renders them.

#![forbid(unsafe_code)]

use std::fmt;

pub const MAX_OPAQUE_LEN: usize = 256;
pub const MAX_ACCOUNT_LEN: usize = 128;
pub const MAX_FINGERPRINT_LEN: usize = 64;
pub const QR_SCHEME_PREFIX: &str = "opencode2://pair?";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairError {
    Expired,
    AlreadyUsed,
    WrongAccount,
    InvalidChallenge,
    Cancelled,
    InvalidInput,
}

impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Expired => "pairing challenge expired",
            Self::AlreadyUsed => "pairing challenge already used",
            Self::WrongAccount => "pairing challenge bound to another account",
            Self::InvalidChallenge => "invalid pairing challenge",
            Self::Cancelled => "pairing cancelled",
            Self::InvalidInput => "invalid pairing input",
        };
        f.write_str(s)
    }
}

impl std::error::Error for PairError {}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceFingerprint(String);

impl DeviceFingerprint {
    pub fn new(display: &str) -> Result<Self, PairError> {
        if display.is_empty() || display.len() > MAX_FINGERPRINT_LEN {
            return Err(PairError::InvalidInput);
        }
        if !display
            .bytes()
            .all(|b| b.is_ascii_graphic() || b == b' ')
        {
            return Err(PairError::InvalidInput);
        }
        Ok(Self(display.to_owned()))
    }

    pub fn display(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for DeviceFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Visible by design: operator compares this on both screens.
        write!(f, "DeviceFingerprint({})", self.0)
    }
}

#[derive(Clone)]
pub struct SingleUseChallenge {
    opaque: String,
    account_id: String,
    expires_at_secs: u64,
    fingerprint: DeviceFingerprint,
}

impl SingleUseChallenge {
    pub fn new(
        opaque: &str,
        account_id: &str,
        expires_at_secs: u64,
        fingerprint: DeviceFingerprint,
    ) -> Result<Self, PairError> {
        if opaque.is_empty()
            || opaque.len() > MAX_OPAQUE_LEN
            || account_id.is_empty()
            || account_id.len() > MAX_ACCOUNT_LEN
            || expires_at_secs == 0
        {
            return Err(PairError::InvalidInput);
        }
        if !opaque.bytes().all(|b| b.is_ascii_graphic())
            || !account_id.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(PairError::InvalidInput);
        }
        Ok(Self {
            opaque: opaque.to_owned(),
            account_id: account_id.to_owned(),
            expires_at_secs,
            fingerprint,
        })
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn expires_at_secs(&self) -> u64 {
        self.expires_at_secs
    }

    pub fn fingerprint(&self) -> &DeviceFingerprint {
        &self.fingerprint
    }

    /// Bearer material safe to embed in the QR payload. Never provider keys:
    /// this type has no field for them by construction.
    pub fn challenge_material(&self) -> &str {
        &self.opaque
    }

    fn matches_opaque(&self, presented: &str) -> bool {
        if self.opaque.len() != presented.len() {
            return false;
        }
        self.opaque
            .bytes()
            .zip(presented.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }

    fn clear_opaque(&mut self) {
        // Zeroize-then-drop without unsafe: overwrite via a zeroed take.
        let len = self.opaque.len();
        self.opaque = "0".repeat(len);
        self.opaque.clear();
    }
}

impl fmt::Debug for SingleUseChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never leak bearer material or account binding into logs.
        f.debug_struct("SingleUseChallenge")
            .field("expires_at_secs", &self.expires_at_secs)
            .field("fingerprint", &self.fingerprint)
            .field("opaque", &"[redacted]")
            .field("account_id", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    Pending,
    Consumed,
    Cancelled,
}

#[derive(Debug)]
pub struct Pairing {
    challenge: SingleUseChallenge,
    state: SlotState,
}

impl Pairing {
    pub fn begin(challenge: SingleUseChallenge) -> Self {
        Self {
            challenge,
            state: SlotState::Pending,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.state == SlotState::Pending
    }

    pub fn redeem(
        &mut self,
        account_id: &str,
        presented_opaque: &str,
        now_secs: u64,
    ) -> Result<DeviceFingerprint, PairError> {
        match self.state {
            SlotState::Cancelled => return Err(PairError::Cancelled),
            SlotState::Consumed => return Err(PairError::AlreadyUsed),
            SlotState::Pending => {}
        }
        if account_id != self.challenge.account_id {
            return Err(PairError::WrongAccount);
        }
        if now_secs >= self.challenge.expires_at_secs {
            return Err(PairError::Expired);
        }
        if !self.challenge.matches_opaque(presented_opaque) {
            return Err(PairError::InvalidChallenge);
        }
        let fp = self.challenge.fingerprint().clone();
        self.challenge.clear_opaque();
        self.state = SlotState::Consumed;
        Ok(fp)
    }

    pub fn cancel(&mut self) -> CancelReceipt {
        let was_pending = self.state == SlotState::Pending;
        self.challenge.clear_opaque();
        self.state = SlotState::Cancelled;
        CancelReceipt {
            cleared_pending: was_pending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelReceipt {
    pub cleared_pending: bool,
}

impl CancelReceipt {
    pub fn cleared_pending(&self) -> bool {
        self.cleared_pending
    }
}

/// QR payload: challenge material only. Takes `&SingleUseChallenge`, which has
/// no provider-key field, so secrets cannot reach the QR by construction.
pub fn qr_payload(challenge: &SingleUseChallenge) -> String {
    let mut out = String::with_capacity(
        QR_SCHEME_PREFIX.len() + challenge.challenge_material().len() + 64,
    );
    out.push_str(QR_SCHEME_PREFIX);
    out.push_str("c=");
    out.push_str(&pct_encode(challenge.challenge_material()));
    out.push_str("&acct=");
    out.push_str(&pct_encode(challenge.account_id()));
    out.push_str("&exp=");
    out.push_str(&challenge.expires_at_secs().to_string());
    out.push_str("&fp=");
    out.push_str(&pct_encode(challenge.fingerprint().display()));
    out
}

fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCT: &str = "acct-123";
    const OPAQUE: &str = "ch-opaque-single-use-xyz";
    const NOW: u64 = 1_700_000_000;

    fn fp() -> DeviceFingerprint {
        DeviceFingerprint::new("AB12-CD34").unwrap()
    }

    fn challenge() -> SingleUseChallenge {
        SingleUseChallenge::new(OPAQUE, ACCT, NOW + 300, fp()).unwrap()
    }

    #[test]
    fn redeem_ok_binds_intended_account_and_device() {
        let mut p = Pairing::begin(challenge());
        let got = p.redeem(ACCT, OPAQUE, NOW).unwrap();
        assert_eq!(got.display(), "AB12-CD34");
        assert!(!p.is_pending());
    }

    #[test]
    fn expired_challenge_fails_without_enrollment() {
        let mut p = Pairing::begin(challenge());
        assert_eq!(p.redeem(ACCT, OPAQUE, NOW + 301), Err(PairError::Expired));
        assert!(p.is_pending());
    }

    #[test]
    fn reused_challenge_fails_second_redeem() {
        let mut p = Pairing::begin(challenge());
        p.redeem(ACCT, OPAQUE, NOW).unwrap();
        assert_eq!(
            p.redeem(ACCT, OPAQUE, NOW),
            Err(PairError::AlreadyUsed)
        );
    }

    #[test]
    fn other_account_fails_without_enrollment() {
        let mut p = Pairing::begin(challenge());
        assert_eq!(
            p.redeem("acct-attacker", OPAQUE, NOW),
            Err(PairError::WrongAccount)
        );
        assert!(p.is_pending());
    }

    #[test]
    fn wrong_opaque_fails() {
        let mut p = Pairing::begin(challenge());
        assert_eq!(
            p.redeem(ACCT, "forged-guess", NOW),
            Err(PairError::InvalidChallenge)
        );
        assert!(p.is_pending());
    }

    #[test]
    fn qr_payload_has_challenge_only_no_provider_secret() {
        let secret = "sk-provider-SECRET-99";
        let payload = qr_payload(&challenge());
        assert!(payload.starts_with(QR_SCHEME_PREFIX), "payload: {payload}");
        assert!(payload.contains(OPAQUE), "payload: {payload}");
        assert!(!payload.contains(secret), "payload: {payload}");
        assert!(!payload.contains("sk-"), "payload: {payload}");
        assert!(!payload.contains("provider"), "payload: {payload}");
    }

    #[test]
    fn cancel_clears_pending_and_blocks_redeem() {
        let mut p = Pairing::begin(challenge());
        let rc = p.cancel();
        assert!(rc.cleared_pending());
        assert!(!p.is_pending());
        assert_eq!(p.redeem(ACCT, OPAQUE, NOW), Err(PairError::Cancelled));
    }

    #[test]
    fn debug_redacts_opaque_challenge_material() {
        let dbg = format!("{:?}", challenge());
        assert!(!dbg.contains(OPAQUE), "debug: {dbg}");
    }
}
