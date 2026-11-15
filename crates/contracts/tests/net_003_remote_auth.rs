//! NET-003 RED: versioned remote control protocol + scoped authorization.
//! Targets: `crates/contracts/src/remote`, `crates/control-plane/src/authorization`
//! (both absent; this file freezes the contract before implementation).
//!
//! These are unimplemented `todo!()` placeholders. They are ignored so the
//! workspace suite reports a truthful result instead of a panic, and they stay
//! in the tree as the frozen RED contract for the missing remote-control work.

#[test]
#[ignore = "NET-003 remote control protocol unimplemented; frozen RED contract"]
fn net_003_t01_protocol_version_negotiates_supported_version() {
    todo!("NET-003: negotiate highest mutually supported remote protocol version");
}

#[test]
#[ignore = "NET-003 remote control protocol unimplemented; frozen RED contract"]
fn net_003_t02_out_of_scope_token_denied_without_side_effects() {
    todo!("NET-003: cross account/device/workspace/session scope must deny before routing");
}

#[test]
#[ignore = "NET-003 remote control protocol unimplemented; frozen RED contract"]
fn net_003_t03_version_mismatch_rejected_with_typed_error() {
    todo!("NET-003: unsupported protocol version must fail closed with typed error");
}
