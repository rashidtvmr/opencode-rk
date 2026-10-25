//! Zero-expiry OAuth persistence for providers that issue permanent tokens.
//!
//! Upstream OpenCode V2 at REV450e7b92412c780a7b5a75226bb8d49aaca24e22
//! (packages/core/src/plugin/provider/github-copilot.ts line 129) writes
//! `expires: 0` for GitHub Copilot device-flow credentials. The schema at
//! packages/schema/src/credential.ts line 37 uses `NonNegativeInt` (>= 0),
//! so zero is a valid expiry sentinel, not an error.
//!
//! These tests use a disposable XDG data directory.  They do not use the
//! user's OpenCode state, network, database, or real credentials.

use opencode_rk_providers::persisted_auth::{
    api_key, load_oauth_credential, save_api_key, save_oauth_credential_with_metadata,
};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

// XDG_DATA_HOME is process-global.  Keep these tests mutually exclusive.
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct AuthFixture {
    _lock: std::sync::MutexGuard<'static, ()>,
    dir: TempDir,
}

impl AuthFixture {
    fn new() -> Self {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_DATA_HOME", dir.path());
        std::env::remove_var("OPENCODE_AUTH_CONTENT");
        Self { _lock: lock, dir }
    }

    fn path(&self) -> std::path::PathBuf {
        self.dir.path().join("opencode/auth.json")
    }

    fn read(&self) -> Vec<u8> {
        std::fs::read(self.path()).unwrap()
    }
}

impl Drop for AuthFixture {
    fn drop(&mut self) {
        std::env::remove_var("XDG_DATA_HOME");
        std::env::remove_var("OPENCODE_AUTH_CONTENT");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn zero_expiry_roundtrips_with_copilot_metadata() {
    // Upstream: Copilot device flow writes expires=0 with apiEndpoint/enterpriseUrl metadata.
    let fixture = AuthFixture::new();

    save_oauth_credential_with_metadata(
        "github-copilot".into(),
        "gh-token".into(),
        "gh-token".into(),
        0, // zero expiry: permanent token (matches upstream Copilot behavior)
        None,
        None,
    )
    .await
    .unwrap();

    let loaded = load_oauth_credential("github-copilot").await.unwrap();
    assert_eq!(loaded.access_token, "gh-token");
    assert_eq!(loaded.refresh_token, "gh-token");
    assert_eq!(loaded.expires_at, 0);
    assert_eq!(loaded.account_id, None);
    assert_eq!(loaded.enterprise_url, None);

    let root: Value = serde_json::from_slice(&fixture.read()).unwrap();
    assert_eq!(root["github-copilot"]["type"], "oauth");
    assert_eq!(root["github-copilot"]["expires"], 0);
}

#[tokio::test(flavor = "current_thread")]
async fn zero_expiry_preserves_existing_api_keys_and_oauth() {
    // Saving a zero-expiry credential must not touch other providers.
    let fixture = AuthFixture::new();

    save_api_key(
        "other-provider".into(),
        "api-key".into(),
        Some([("kind".into(), "fixture".into())].into_iter().collect()),
    )
    .await
    .unwrap();
    save_oauth_credential_with_metadata(
        "other-oauth".into(),
        "access".into(),
        "refresh".into(),
        100,
        Some("acct".into()),
        Some("https://enterprise".into()),
    )
    .await
    .unwrap();

    save_oauth_credential_with_metadata(
        "github-copilot".into(),
        "gh-token".into(),
        "gh-token".into(),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let loaded = load_oauth_credential("github-copilot").await.unwrap();
    assert_eq!(loaded.expires_at, 0);
    assert_eq!(loaded.access_token, "gh-token");

    // other provider untouched
    assert_eq!(
        api_key("other-provider", "UNSET_FIXTURE_KEY")
            .await
            .as_deref(),
        Some("api-key")
    );
    let other_oauth = load_oauth_credential("other-oauth").await.unwrap();
    assert_eq!(other_oauth.expires_at, 100);

    let root: Value = serde_json::from_slice(&fixture.read()).unwrap();
    assert_eq!(root["github-copilot"]["expires"], 0);
    assert_eq!(root["other-provider"]["metadata"]["kind"], "fixture");
    assert_eq!(root["other-oauth"]["expires"], 100);
}

#[tokio::test(flavor = "current_thread")]
async fn zero_expiry_update_replaces_only_target_preserving_metadata() {
    // Re-saving a zero-expiry credential updates only that provider's entry.
    let fixture = AuthFixture::new();

    save_oauth_credential_with_metadata(
        "github-copilot".into(),
        "old-token".into(),
        "old-refresh".into(),
        0,
        Some("acct".into()),
        Some("https://enterprise".into()),
    )
    .await
    .unwrap();

    save_oauth_credential_with_metadata(
        "github-copilot".into(),
        "new-token".into(),
        "new-refresh".into(),
        0,
        Some("acct2".into()),
        Some("https://enterprise2".into()),
    )
    .await
    .unwrap();

    let loaded = load_oauth_credential("github-copilot").await.unwrap();
    assert_eq!(loaded.access_token, "new-token");
    assert_eq!(loaded.refresh_token, "new-refresh");
    assert_eq!(loaded.expires_at, 0);
    assert_eq!(loaded.account_id.as_deref(), Some("acct2"));
    assert_eq!(loaded.enterprise_url.as_deref(), Some("https://enterprise2"));

    let root: Value = serde_json::from_slice(&fixture.read()).unwrap();
    assert_eq!(root["github-copilot"]["access"], "new-token");
    assert_eq!(root["github-copilot"]["expires"], 0);
}

#[tokio::test(flavor = "current_thread")]
async fn zero_expiry_file_mode_is_private() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let fixture = AuthFixture::new();
        save_oauth_credential_with_metadata(
            "github-copilot".into(),
            "gh-token".into(),
            "gh-token".into(),
            0,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::metadata(fixture.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[cfg(not(unix))]
    {
        // Unix-only assertion; trivially true on other platforms.
        assert!(true);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn non_zero_expiry_still_rejected_when_invalid() {
    // Ensure we did NOT weaken non-zero validation: empty tokens and
    // oversized tokens still fail.
    let fixture = AuthFixture::new();
    std::fs::create_dir_all(fixture.path().parent().unwrap()).unwrap();
    std::fs::write(fixture.path(), b"{}").unwrap();

    // empty access token -> error
    assert!(save_oauth_credential_with_metadata(
        "provider".into(),
        "".into(),
        "refresh".into(),
        1,
        None,
        None
    )
    .await
    .is_err());

    // empty refresh token -> error
    assert!(save_oauth_credential_with_metadata(
        "provider".into(),
        "access".into(),
        "".into(),
        1,
        None,
        None
    )
    .await
    .is_err());

    // invalid provider (control char) -> error
    assert!(save_oauth_credential_with_metadata(
        "bad\0provider".into(),
        "access".into(),
        "refresh".into(),
        1,
        None,
        None
    )
    .await
    .is_err());
}
