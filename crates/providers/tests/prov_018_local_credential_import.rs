//! PROV-018 frozen tests: consent-gated local credential import (T01..T05).

use opencode_rk_providers::local_credential_import::{
    check_permissions, plan_import, validate_schema, CredentialKind, ImportError, ImportRequest,
    ImportSource, PathMetadata, UserConsent,
};
use std::path::PathBuf;

const CODEX_FIXTURE: &str = r#"{"api_key":"fixture-codex-key-0001"}"#;
const CLAUDE_FIXTURE: &str = r#"{"access_token":"fixture-claude-access-0001","refresh_token":"fixture-claude-refresh-0001"}"#;

fn codex_req() -> ImportRequest {
    ImportRequest {
        source: ImportSource::CodexDefault,
        consent: UserConsent::Granted,
    }
}

fn claude_req() -> ImportRequest {
    ImportRequest {
        source: ImportSource::ClaudeDefault,
        consent: UserConsent::Granted,
    }
}

#[test]
fn prov_018_t01_codex_happy_path() {
    let kind = validate_schema(CODEX_FIXTURE.as_bytes()).expect("valid codex schema");
    assert_eq!(kind, CredentialKind::ApiKey);
    let plan = plan_import(codex_req(), kind, PathMetadata::new(0o600)).expect("consented plan");
    assert_eq!(plan.provider, "codex");
    assert_eq!(plan.kind, CredentialKind::ApiKey);
    let rendered = format!("{plan:?}") + &serde_json::to_string(&plan).unwrap();
    assert!(
        !rendered.contains("fixture-codex-key-0001"),
        "secret bytes in plan"
    );
}

#[test]
fn prov_018_t02_claude_happy_path() {
    let kind = validate_schema(CLAUDE_FIXTURE.as_bytes()).expect("valid claude schema");
    assert_eq!(kind, CredentialKind::OAuth);
    let plan = plan_import(claude_req(), kind, 0o600u32).expect("consented plan");
    assert_eq!(plan.provider, "claude-code");
    assert_eq!(plan.kind, CredentialKind::OAuth);
    let rendered = format!("{plan:?}") + &serde_json::to_string(&plan).unwrap();
    assert!(!rendered.contains("fixture-claude-access-0001"));
    assert!(!rendered.contains("fixture-claude-refresh-0001"));
}

#[test]
fn prov_018_t03_consent_and_path_gating() {
    let no_consent = ImportRequest {
        source: ImportSource::CodexDefault,
        consent: UserConsent::NotGranted,
    };
    assert_eq!(
        plan_import(no_consent, CredentialKind::ApiKey, 0o600u32).unwrap_err(),
        ImportError::ConsentRequired
    );
    let bad_path = ImportRequest {
        source: ImportSource::ConfigDir {
            path: PathBuf::from("/etc/passwd"),
        },
        consent: UserConsent::Granted,
    };
    assert_eq!(
        plan_import(bad_path, CredentialKind::ApiKey, 0o600u32).unwrap_err(),
        ImportError::PathNotAllowed
    );
}

#[test]
fn prov_018_t04_schema_and_permission_failures() {
    assert_eq!(
        validate_schema(b"{not json").unwrap_err(),
        ImportError::BadSchema
    );
    assert_eq!(validate_schema(b"{}").unwrap_err(), ImportError::BadSchema);
    assert_eq!(
        check_permissions(0o644u32).unwrap_err(),
        ImportError::TooPermissive
    );
    assert_eq!(
        check_permissions(0o600u32),
        Ok(()),
        "owner-only mode accepted"
    );
    let oversize = vec![b'a'; 64 * 1024 + 1];
    assert_eq!(
        validate_schema(&oversize).unwrap_err(),
        ImportError::TooLarge
    );
}

#[test]
fn prov_018_t05_redaction_and_isolation() {
    let plan = plan_import(codex_req(), CredentialKind::ApiKey, true).expect("bool meta ok");
    let text = format!("{plan:?}")
        + &serde_json::to_string(&plan).unwrap()
        + &format!("{:?}", ImportError::BadSchema)
        + &serde_json::to_string(&ImportError::TooPermissive).unwrap();
    assert!(!text.contains("fixture-codex-key-0001"));
    assert!(!text.contains("api_key"));
    assert!(
        !text.contains("/root") && !text.contains("/home/"),
        "live home paths"
    );
    assert!(
        plan.dest_label.starts_with("protected://"),
        "redacted dest label"
    );
}
