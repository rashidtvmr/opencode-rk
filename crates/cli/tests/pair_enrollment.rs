#![forbid(unsafe_code)]

//! NET-002 RED: explicit PC enrollment + single-use pairing.
//! Frozen: do not edit to make green; fix implementation instead.
//! Pair module is binary-private (`mod pair` in src/main.rs), so include it
//! via path like tests/ci_ext.rs does for ci_output.

#[path = "../src/pair.rs"]
mod pair;

#[test]
fn enrollment_issues_single_use_code() {
    todo!(
        "NET-002: enrollment must issue single-use account-bound code with visible fingerprint"
    );
}

#[test]
fn pairing_code_reuse_rejected() {
    todo!("NET-002: second redeem of same code must fail AlreadyUsed without enrollment");
}

#[test]
fn expired_pairing_code_rejected() {
    todo!("NET-002: redeem past expires_at_secs must fail Expired without enrollment");
}
