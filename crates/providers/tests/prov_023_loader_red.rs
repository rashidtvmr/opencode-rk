//! PROV-023 loader RED tests.
//!
//! The public request-profile boundary must load the versioned provider catalog
//! from `OPENCODE_RK_PROVIDER_CATALOG_PATH`. An unset or empty override uses
//! the embedded catalog; a configured invalid override fails closed rather than
//! falling back to hardcoded provider entries.
//!
//! These tests perform no network access. The filesystem fixtures use stable
//! names, and the process environment is restored by `EnvGuard`.

use opencode_rk_providers::request_profile::{
    profile_for, AuthHeaderKind, EndpointKind, EndpointSelector, RequestError,
};
use serde_json::{json, Value};
use std::sync::Mutex;

const CATALOG_ENV_VAR: &str = "OPENCODE_RK_PROVIDER_CATALOG_PATH";
const MAX_PROVIDERS: usize = 32;
const MAX_ENDPOINTS_PER_PROVIDER: usize = 16;
const MAX_MODEL_ALIASES_PER_PROVIDER: usize = 32;
const MAX_AUTH_HEADERS_PER_ENDPOINT: usize = 8;
const MAX_CATALOG_BYTES: usize = 256 * 1024;

static ENV_GUARD_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    key: &'static str,
    old: Option<String>,
}

impl EnvGuard {
    fn new(key: &'static str) -> Self {
        let old = std::env::var(key).ok();
        std::env::remove_var(key);
        Self { key, old }
    }

    fn set(&self, value: &str) {
        std::env::set_var(self.key, value);
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn write_catalog(dir: &tempfile::TempDir, contents: &str) -> String {
    let path = dir.path().join("catalog.json");
    std::fs::write(&path, contents).expect("write disposable catalog fixture");
    path.to_string_lossy().into_owned()
}

fn endpoint(kind: &str, url_template: &str, auth_headers: Vec<String>) -> Value {
    json!({
        "kind": kind,
        "url_template": url_template,
        "auth_headers": auth_headers,
    })
}

fn provider(
    id: &str,
    endpoints: Vec<Value>,
    model_aliases: Vec<String>,
    limitations: Vec<String>,
) -> Value {
    json!({
        "id": id,
        "display_name": id,
        "doc_source": "https://docs.example.com/providers",
        "doc_date": "2026-09-15",
        "endpoints": endpoints,
        "model_aliases": model_aliases,
        "refresh": {"supported": false, "method": "not applicable"},
        "limitations": limitations,
    })
}

fn openai_provider(url_template: &str, auth_headers: Vec<String>) -> Value {
    provider(
        "openai",
        vec![endpoint("chat-completions", url_template, auth_headers)],
        vec!["gpt-5.6".to_owned()],
        Vec::new(),
    )
}

fn catalog(providers: Vec<Value>) -> String {
    json!({
        "catalog_version": "1",
        "providers": providers,
        "updated": "2026-09-15",
    })
    .to_string()
}

fn valid_catalog() -> String {
    catalog(vec![
        openai_provider(
            "https://api.openai.com/v1/chat/completions",
            vec!["Authorization".to_owned()],
        ),
        provider(
            "anthropic",
            vec![endpoint(
                "messages",
                "https://api.anthropic.com/v1/messages",
                vec!["x-api-key".to_owned(), "anthropic-version".to_owned()],
            )],
            vec!["claude-opus-4-5".to_owned()],
            Vec::new(),
        ),
        provider(
            "google",
            vec![endpoint(
                "generate-content",
                "https://generativelanguage.googleapis.com/v1beta/{model=models/*}:generateContent",
                vec!["x-goog-api-key".to_owned()],
            )],
            vec!["gemini-flash-latest".to_owned()],
            Vec::new(),
        ),
    ])
}

fn assert_configured_openai_rejected(contents: &str, label: &str) {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("create disposable fixture directory");
    let path = write_catalog(&dir, contents);
    let guard = EnvGuard::new(CATALOG_ENV_VAR);
    guard.set(&path);

    // Every invalid override queries the known hardcoded entry. An Ok result
    // therefore proves that the configured catalog was ignored.
    let result = profile_for("openai", EndpointKind::ChatCompletions);
    assert!(
        result.is_err(),
        "{label} override must fail closed for openai/chat-completions; got: {result:?}"
    );
}

#[test]
fn prov_023_loader_red_valid_override_drives_google() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("create disposable fixture directory");
    let path = write_catalog(&dir, &valid_catalog());
    let guard = EnvGuard::new(CATALOG_ENV_VAR);
    guard.set(&path);

    let result = profile_for("google", EndpointSelector::Name("generate-content"));
    assert!(
        result.is_ok(),
        "valid override must resolve Google from the catalog; got: {result:?}"
    );
    if let Ok(profile) = result {
        assert_eq!(
            profile.endpoint,
            "https://generativelanguage.googleapis.com/v1beta/{model=models/*}:generateContent"
        );
        assert!(
            profile
                .auth_headers
                .iter()
                .any(|header| *header == AuthHeaderKind::ApiKeyHeader),
            "Google catalog entry must map its API-key header"
        );
    }
}

#[test]
fn prov_023_loader_red_unset_and_empty_use_embedded_google() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let guard = EnvGuard::new(CATALOG_ENV_VAR);

    let unset = profile_for("google", EndpointSelector::Name("generate-content"));
    assert!(
        unset.is_ok(),
        "unset override must resolve Google from embedded catalog; got: {unset:?}"
    );

    guard.set("");
    let empty = profile_for("google", EndpointSelector::Name("generate-content"));
    assert!(
        empty.is_ok(),
        "empty override must resolve Google from embedded catalog; got: {empty:?}"
    );
}

#[test]
fn prov_023_loader_red_missing_configured_file_fails_closed() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let dir = tempfile::tempdir().expect("create disposable fixture directory");
    let path = dir.path().join("missing-catalog.json");
    let guard = EnvGuard::new(CATALOG_ENV_VAR);
    guard.set(&path.to_string_lossy());

    let result = profile_for("openai", EndpointKind::ChatCompletions);
    assert!(
        result.is_err(),
        "configured missing catalog must fail closed; got: {result:?}"
    );
}

#[test]
fn prov_023_loader_red_malformed_json_fails_closed() {
    assert_configured_openai_rejected("{ this is deliberately not valid JSON", "malformed JSON");
}

#[test]
fn prov_023_loader_red_unsupported_version_fails_closed() {
    let contents = json!({
        "catalog_version": "2",
        "providers": [openai_provider(
            "https://api.openai.com/v1/chat/completions",
            vec!["Authorization".to_owned()],
        )],
        "updated": "2026-09-15",
    })
    .to_string();
    assert_configured_openai_rejected(&contents, "unsupported catalog version");
}

#[test]
fn prov_023_loader_red_missing_version_fails_closed() {
    let contents = json!({
        "providers": [openai_provider(
            "https://api.openai.com/v1/chat/completions",
            vec!["Authorization".to_owned()],
        )],
        "updated": "2026-09-15",
    })
    .to_string();
    assert_configured_openai_rejected(&contents, "missing catalog version");
}

#[test]
fn prov_023_loader_red_valid_json_wrong_shape_fails_closed() {
    let contents = json!({"some_other_json": true, "data": [1, 2, 3]}).to_string();
    assert_configured_openai_rejected(&contents, "valid JSON with wrong catalog shape");
}

#[test]
fn prov_023_loader_red_oversize_fails_closed() {
    let contents = catalog(vec![provider(
        "openai",
        vec![endpoint(
            "chat-completions",
            "https://api.openai.com/v1/chat/completions",
            vec!["Authorization".to_owned()],
        )],
        vec!["gpt-5.6".to_owned()],
        vec!["x".repeat(MAX_CATALOG_BYTES)],
    )]);
    assert!(
        contents.len() > MAX_CATALOG_BYTES,
        "oversize fixture must exceed 256 KiB: {} bytes",
        contents.len()
    );
    assert_configured_openai_rejected(&contents, "oversize catalog");
}

#[test]
fn prov_023_loader_red_duplicate_provider_fails_closed() {
    let duplicate = openai_provider(
        "https://api.openai.com/v1/chat/completions",
        vec!["Authorization".to_owned()],
    );
    let contents = catalog(vec![duplicate.clone(), duplicate]);
    assert_configured_openai_rejected(&contents, "duplicate provider id");
}

#[test]
fn prov_023_loader_red_duplicate_endpoint_kind_fails_closed() {
    let duplicate_endpoint = endpoint(
        "chat-completions",
        "https://api.openai.com/v1/chat/completions",
        vec!["Authorization".to_owned()],
    );
    let contents = catalog(vec![provider(
        "openai",
        vec![duplicate_endpoint.clone(), duplicate_endpoint],
        vec!["gpt-5.6".to_owned()],
        Vec::new(),
    )]);
    assert_configured_openai_rejected(&contents, "duplicate endpoint kind");
}

#[test]
fn prov_023_loader_red_provider_cap_fails_closed() {
    let mut providers = vec![openai_provider(
        "https://api.openai.com/v1/chat/completions",
        vec!["Authorization".to_owned()],
    )];
    for index in 1..=MAX_PROVIDERS {
        providers.push(provider(
            &format!("provider-{index}"),
            vec![endpoint(
                "chat-completions",
                "https://api.example.com/v1/chat/completions",
                vec!["Authorization".to_owned()],
            )],
            vec![format!("alias-{index}")],
            Vec::new(),
        ));
    }
    assert_eq!(providers.len(), MAX_PROVIDERS + 1);
    assert_configured_openai_rejected(&catalog(providers), "provider count cap");
}

#[test]
fn prov_023_loader_red_endpoint_cap_fails_closed() {
    let mut endpoints = vec![endpoint(
        "chat-completions",
        "https://api.openai.com/v1/chat/completions",
        vec!["Authorization".to_owned()],
    )];
    for index in 0..MAX_ENDPOINTS_PER_PROVIDER {
        endpoints.push(endpoint(
            &format!("endpoint-{index}"),
            &format!("https://api.openai.com/v1/endpoint-{index}"),
            vec!["Authorization".to_owned()],
        ));
    }
    assert_eq!(endpoints.len(), MAX_ENDPOINTS_PER_PROVIDER + 1);
    assert_configured_openai_rejected(
        &catalog(vec![provider(
            "openai",
            endpoints,
            vec!["gpt-5.6".to_owned()],
            Vec::new(),
        )]),
        "endpoint count cap",
    );
}

#[test]
fn prov_023_loader_red_model_alias_cap_fails_closed() {
    let aliases = (0..=MAX_MODEL_ALIASES_PER_PROVIDER)
        .map(|index| format!("alias-{index}"))
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), MAX_MODEL_ALIASES_PER_PROVIDER + 1);
    assert_configured_openai_rejected(
        &catalog(vec![provider(
            "openai",
            vec![endpoint(
                "chat-completions",
                "https://api.openai.com/v1/chat/completions",
                vec!["Authorization".to_owned()],
            )],
            aliases,
            Vec::new(),
        )]),
        "model alias count cap",
    );
}

#[test]
fn prov_023_loader_red_auth_header_cap_fails_closed() {
    let headers = (0..=MAX_AUTH_HEADERS_PER_ENDPOINT)
        .map(|index| {
            if index == 0 {
                "Authorization".to_owned()
            } else {
                format!("X-Auth-{index}")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(headers.len(), MAX_AUTH_HEADERS_PER_ENDPOINT + 1);
    assert_configured_openai_rejected(
        &catalog(vec![openai_provider(
            "https://api.openai.com/v1/chat/completions",
            headers,
        )]),
        "authentication-header count cap",
    );
}

#[test]
fn prov_023_loader_red_non_https_url_fails_closed() {
    let contents = catalog(vec![openai_provider(
        "http://api.openai.com/v1/chat/completions",
        vec!["Authorization".to_owned()],
    )]);
    assert_configured_openai_rejected(&contents, "non-HTTPS endpoint");
}

#[test]
fn prov_023_loader_red_credential_bearing_url_fails_closed() {
    let contents = catalog(vec![openai_provider(
        "https://user:password@api.openai.com/v1/chat/completions?api_key=abc123",
        vec!["Authorization".to_owned()],
    )]);
    assert_configured_openai_rejected(&contents, "credential-bearing endpoint URL");
}

#[test]
fn prov_023_loader_red_disallowed_placeholder_fails_closed() {
    let contents = catalog(vec![openai_provider(
        "https://api.openai.com/v1/{model}",
        vec!["Authorization".to_owned()],
    )]);
    assert_configured_openai_rejected(&contents, "disallowed endpoint placeholder");
}

#[test]
fn prov_023_loader_red_secret_like_fixture_fails_closed() {
    let contents = catalog(vec![provider(
        "openai",
        vec![endpoint(
            "chat-completions",
            "https://api.openai.com/v1/chat/completions",
            vec!["Authorization".to_owned()],
        )],
        vec!["gpt-5.6".to_owned()],
        vec!["diagnostic value sk-test-secret must be rejected".to_owned()],
    )]);
    assert_configured_openai_rejected(&contents, "secret-like catalog value");
}

#[test]
fn prov_023_loader_green_unknown_provider_fails_closed_without_guessing() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _guard = EnvGuard::new(CATALOG_ENV_VAR);
    let result = profile_for(
        "provider-that-is-not-documented",
        EndpointKind::ChatCompletions,
    );
    assert!(matches!(result, Err(RequestError::UnknownProvider)));
}

#[test]
fn prov_023_loader_green_unknown_endpoint_fails_closed_without_guessing() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _guard = EnvGuard::new(CATALOG_ENV_VAR);
    let result = profile_for(
        "openai",
        EndpointSelector::Name("endpoint-that-is-not-documented"),
    );
    assert!(matches!(result, Err(RequestError::UndocumentedEndpoint)));
}

#[test]
fn prov_023_loader_green_debug_contains_no_secret_bytes() {
    let _lock = ENV_GUARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _guard = EnvGuard::new(CATALOG_ENV_VAR);
    let result = profile_for("openai", EndpointKind::ChatCompletions);
    assert!(
        result.is_ok(),
        "embedded OpenAI profile must remain available; got: {result:?}"
    );
    if let Ok(profile) = result {
        let rendered =
            format!("{profile:?}") + &serde_json::to_string(&profile).expect("profile serializes");
        assert!(
            !rendered.contains("sk-"),
            "profile debug must not contain secret bytes"
        );
        assert!(
            !rendered.contains("Bearer "),
            "profile debug must not contain bearer values"
        );
    }
}
