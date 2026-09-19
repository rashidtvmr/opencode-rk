//! NET-001 RED: self-hosted account register → login → revoke lifecycle.
//! No `crates/control-plane` exists yet (workspace members: agents, contracts,
//! foundation, storage, security, sessions, catalog, server, cli, providers,
//! tools, opentui-bridge). Closest existing auth-adjacent patterns:
//! `crates/security/src/credentials.rs:34` (CredentialStore lifecycle),
//! `crates/security/src/platform_matrix.rs:177` (Decision allow/deny).
//! These 3 tests define the owned-service account contract; they FAIL by
//! design until the control-plane account module lands.

/// NET-001-T01: register creates an account retrievable by identity.
#[test]
fn net001_register_creates_account() {
    todo!("RED: register new account, expect stored account record");
}

/// NET-001-T02: login returns a scoped token bound to account/device.
#[test]
fn net001_login_returns_scoped_token() {
    todo!("RED: login with valid credentials, expect scoped token");
}

/// NET-001-T03: revoked token is rejected on subsequent use.
#[test]
fn net001_revoked_token_rejected() {
    todo!("RED: revoke token, expect authorization deny");
}
