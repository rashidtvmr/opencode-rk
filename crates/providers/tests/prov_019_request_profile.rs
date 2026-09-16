//! PROV-019 frozen tests: documented provider request profiles (T01..T05).

use opencode_rk_providers::request_profile::{
    headers_for, profile_for, profile_for_with_bounds, profile_for_with_headers, with_diagnostics,
    AuthHeaderKind, EndpointKind, RedactedMarker, RequestError,
};

#[test]
fn prov_019_t01_openai_happy_path() {
    let profile = profile_for("openai", EndpointKind::ChatCompletions).expect("openai profile");
    assert_eq!(
        profile.endpoint,
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(profile.auth_headers, vec![AuthHeaderKind::Bearer]);
    assert!((1_000..=120_000).contains(&profile.timeout_ms));
    assert!(profile.max_retries <= 3);

    let headers = headers_for(&profile);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0, "Authorization");
    assert_eq!(headers[0].1, RedactedMarker::Redacted);
    let rendered = format!("{profile:?}")
        + &serde_json::to_string(&profile).unwrap()
        + &serde_json::to_string(&headers).unwrap();
    assert!(!rendered.contains("sk-"), "no secret bytes");
}

#[test]
fn prov_019_t02_anthropic_happy_path() {
    let profile = profile_for("anthropic", EndpointKind::Messages).expect("anthropic profile");
    assert_eq!(profile.endpoint, "https://api.anthropic.com/v1/messages");
    assert!(profile.auth_headers.contains(&AuthHeaderKind::ApiKeyHeader));

    let diag = with_diagnostics(&profile, 1);
    assert_eq!(diag.provider_id, "anthropic");
    assert_eq!(diag.endpoint, profile.endpoint);
    assert_eq!(diag.attempt, 1);
    let rendered = format!("{diag:?}") + &serde_json::to_string(&diag).unwrap();
    assert!(!rendered.contains("sk-") && !rendered.contains("key-"));
}

#[test]
fn prov_019_t03_prohibitions() {
    assert_eq!(
        profile_for("openai", EndpointKind::Messages).unwrap_err(),
        RequestError::UndocumentedEndpoint
    );
    assert_eq!(
        profile_for_with_headers(
            "openai",
            EndpointKind::ChatCompletions,
            vec![("X-Custom".to_string(), "value".to_string())]
        )
        .unwrap_err(),
        RequestError::HeaderNotAllowed
    );
    assert_eq!(
        profile_for("openai", "http://api.openai.com/v1/chat/completions").unwrap_err(),
        RequestError::BadEndpoint
    );
}

#[test]
fn prov_019_t04_bounds() {
    assert_eq!(
        profile_for_with_bounds("openai", EndpointKind::ChatCompletions, 0, 1).unwrap_err(),
        RequestError::BadBounds
    );
    assert_eq!(
        profile_for_with_bounds("openai", EndpointKind::ChatCompletions, 30_000, 99).unwrap_err(),
        RequestError::BadBounds
    );
    assert_eq!(
        profile_for("unknown-provider-xyz", EndpointKind::ChatCompletions).unwrap_err(),
        RequestError::UnknownProvider
    );
}

#[test]
fn prov_019_t05_determinism_and_redaction() {
    let a = serde_json::to_string(&profile_for("openai", EndpointKind::ChatCompletions).unwrap())
        .unwrap();
    let b = serde_json::to_string(&profile_for("openai", EndpointKind::ChatCompletions).unwrap())
        .unwrap();
    assert_eq!(a, b, "byte-identical profile");
    let diag = with_diagnostics(
        &profile_for("anthropic", EndpointKind::Messages).unwrap(),
        2,
    )
    .with_error_code(RequestError::BadBounds);
    let rendered = format!("{diag:?}") + &serde_json::to_string(&diag).unwrap();
    assert!(!rendered.contains("sk-"));
}
