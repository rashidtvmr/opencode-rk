#![forbid(unsafe_code)]

//! NET-002 acceptance: explicit PC enrollment + single-use pairing.
//! Frozen contract: single-use account-bound code with a visible fingerprint,
//! `AlreadyUsed` on the second redeem, and `Expired` past `expires_at_secs`
//! without enrolling the device. Do not relax these assertions to make green.
//! Pair module is binary-private (`mod pair` in src/main.rs), so include it
//! via path like tests/ci_ext.rs does for ci_output.

#[path = "../src/pair.rs"]
mod pair;

use pair::{DeviceFingerprint, PairError, Pairing, SingleUseChallenge, QR_SCHEME_PREFIX};

const ACCT: &str = "acct-net-002";
const OPAQUE: &str = "opaque-net-002-single-use";
const NOW: u64 = 1_700_000_000;
const LIFETIME_SECS: u64 = 300;

fn issued_challenge() -> SingleUseChallenge {
    let fingerprint = DeviceFingerprint::new("PC-AB12-CD34").expect("fingerprint");
    SingleUseChallenge::new(OPAQUE, ACCT, NOW + LIFETIME_SECS, fingerprint).expect("challenge")
}

#[test]
fn enrollment_issues_single_use_code() {
    let challenge = issued_challenge();
    assert_eq!(challenge.account_id(), ACCT, "code must bind the account");
    assert_eq!(
        challenge.fingerprint().display(),
        "PC-AB12-CD34",
        "the device fingerprint must be visible for local comparison"
    );
    let payload = pair::qr_payload(&challenge);
    assert!(payload.starts_with(QR_SCHEME_PREFIX), "payload: {payload}");
    assert!(payload.contains("fp=PC-AB12-CD34"), "payload: {payload}");

    let mut pairing = Pairing::begin(challenge);
    assert!(pairing.is_pending());
    let bound = pairing
        .redeem(ACCT, OPAQUE, NOW)
        .expect("first redeem inside the window succeeds");
    assert_eq!(bound.display(), "PC-AB12-CD34");
    assert!(!pairing.is_pending(), "a redeemed code is single-use");
}

#[test]
fn pairing_code_reuse_rejected() {
    let mut pairing = Pairing::begin(issued_challenge());
    pairing
        .redeem(ACCT, OPAQUE, NOW)
        .expect("first redeem binds the device");
    assert_eq!(
        pairing.redeem(ACCT, OPAQUE, NOW),
        Err(PairError::AlreadyUsed),
        "a second redeem must not enroll another device"
    );
}

#[test]
fn expired_pairing_code_rejected() {
    // `expires_at_secs` is exclusive: the deadline instant is already expired.
    let mut pairing = Pairing::begin(issued_challenge());
    assert_eq!(
        pairing.redeem(ACCT, OPAQUE, NOW + LIFETIME_SECS),
        Err(PairError::Expired)
    );
    assert!(pairing.is_pending(), "an expired code must not enroll a device");

    // One second inside the window still redeems, so the rejection above is
    // about time rather than a permanently unusable code.
    let mut fresh = Pairing::begin(issued_challenge());
    assert!(fresh.redeem(ACCT, OPAQUE, NOW + LIFETIME_SECS - 1).is_ok());
}
