use opencode_rk_providers::integration::{
    IntegrationError, IntegrationInfo, IntegrationMethod, IntegrationRegistry, OAuthAttemptStatus,
    OAuthAttempts, OAuthCompletion, MAX_INTEGRATION_METHODS, MAX_INTEGRATION_SCOPES,
    MAX_OAUTH_ATTEMPTS, OAUTH_ATTEMPT_LIFETIME_MS,
};

fn integration(id: &str, name: &str) -> IntegrationInfo {
    IntegrationInfo {
        id: id.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn int_004_t01_scoped_registration_is_visible_then_removed_when_scope_closes() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register(10, integration("openai", "OpenAI"))
        .expect("first scoped registration should fit");

    assert_eq!(
        registry.get("openai"),
        Some(integration("openai", "OpenAI"))
    );
    assert_eq!(registry.list(), vec![integration("openai", "OpenAI")]);

    assert!(registry.close_scope(10));
    assert_eq!(registry.get("openai"), None);
    assert!(registry.list().is_empty());
}

#[test]
fn int_004_t02_later_scope_overrides_and_closing_it_reveals_prior_registration() {
    let mut registry = IntegrationRegistry::new();

    registry
        .register(1, integration("openai", "OpenAI"))
        .expect("base scope should register");
    registry
        .register(2, integration("openai", "OpenAI Override"))
        .expect("later scope should register an override");

    assert_eq!(
        registry.get("openai"),
        Some(integration("openai", "OpenAI Override"))
    );
    assert!(registry.close_scope(2));
    assert_eq!(
        registry.get("openai"),
        Some(integration("openai", "OpenAI"))
    );
    assert_eq!(registry.list(), vec![integration("openai", "OpenAI")]);
}

#[test]
fn int_004_t03_method_metadata_replacement_is_bounded_deterministic_and_secret_free() {
    assert_eq!(MAX_INTEGRATION_METHODS, 8);

    let mut registry = IntegrationRegistry::new();
    registry
        .register(1, integration("openai", "OpenAI"))
        .expect("integration should register");

    registry
        .set_method(
            1,
            "openai",
            IntegrationMethod::Key {
                label: "API key".to_owned(),
            },
        )
        .expect("key method should fit");
    registry
        .set_method(
            1,
            "openai",
            IntegrationMethod::OAuth {
                id: "chatgpt".to_owned(),
                label: "ChatGPT".to_owned(),
            },
        )
        .expect("oauth method should fit");
    registry
        .set_method(
            1,
            "openai",
            IntegrationMethod::Key {
                label: "Replacement key label".to_owned(),
            },
        )
        .expect("key metadata should replace the single key slot");
    registry
        .set_method(
            1,
            "openai",
            IntegrationMethod::OAuth {
                id: "chatgpt".to_owned(),
                label: "ChatGPT Override".to_owned(),
            },
        )
        .expect("same oauth id should replace metadata in place");

    assert_eq!(
        registry.methods("openai"),
        vec![
            IntegrationMethod::Key {
                label: "Replacement key label".to_owned(),
            },
            IntegrationMethod::OAuth {
                id: "chatgpt".to_owned(),
                label: "ChatGPT Override".to_owned(),
            },
        ]
    );

    for index in 0..(MAX_INTEGRATION_METHODS - 2) {
        registry
            .set_method(
                1,
                "openai",
                IntegrationMethod::OAuth {
                    id: format!("oauth-{index}"),
                    label: format!("OAuth {index}"),
                },
            )
            .expect("exact method bound should succeed");
    }
    assert_eq!(registry.methods("openai").len(), MAX_INTEGRATION_METHODS);

    let overflow = registry
        .set_method(
            1,
            "openai",
            IntegrationMethod::OAuth {
                id: "overflow".to_owned(),
                label: "Overflow".to_owned(),
            },
        )
        .expect_err("method metadata must be bounded");
    assert_eq!(
        overflow,
        IntegrationError::TooManyMethods {
            max: MAX_INTEGRATION_METHODS,
            actual: MAX_INTEGRATION_METHODS + 1,
        }
    );
}

#[test]
fn int_004_t04_code_oauth_missing_code_is_non_mutating_and_completion_returns_metadata_only() {
    assert_eq!(OAUTH_ATTEMPT_LIFETIME_MS, 10 * 60 * 1_000);

    let mut attempts = OAuthAttempts::new();
    attempts
        .start_code(
            "attempt-1",
            "openai",
            "chatgpt",
            Some("Personal"),
            1_000,
        )
        .expect("code oauth attempt should start");

    let before = attempts
        .get("attempt-1")
        .expect("attempt should be retained")
        .clone();
    assert_eq!(before.status, OAuthAttemptStatus::Pending);
    assert_eq!(before.created_at_ms, 1_000);
    assert_eq!(before.expires_at_ms, 1_000 + OAUTH_ATTEMPT_LIFETIME_MS);

    let missing = attempts
        .complete_code("attempt-1", None)
        .expect_err("code mode completion without code must be typed");
    assert_eq!(
        missing,
        IntegrationError::CodeRequired {
            attempt_id: "attempt-1".to_owned(),
        }
    );
    assert_eq!(attempts.get("attempt-1"), Some(&before));

    let completion = attempts
        .complete_code("attempt-1", Some("1234"))
        .expect("code completion should settle the attempt");
    assert_eq!(
        completion,
        OAuthCompletion {
            integration_id: "openai".to_owned(),
            method_id: "chatgpt".to_owned(),
            label: Some("Personal".to_owned()),
        }
    );
    assert_eq!(
        attempts.get("attempt-1").map(|attempt| attempt.status),
        Some(OAuthAttemptStatus::Complete)
    );
}

#[test]
fn int_004_t05_expiry_cancel_duplicates_and_public_caps_are_explicit_and_typed() {
    assert_eq!(MAX_INTEGRATION_SCOPES, 8);
    assert_eq!(MAX_INTEGRATION_METHODS, 8);
    assert_eq!(MAX_OAUTH_ATTEMPTS, 8);
    assert_eq!(OAUTH_ATTEMPT_LIFETIME_MS, 600_000);

    let mut registry = IntegrationRegistry::new();
    for scope in 0..MAX_INTEGRATION_SCOPES {
        registry
            .register(scope as u64, integration(&format!("i-{scope}"), "Integration"))
            .expect("exact active-scope bound should succeed");
    }
    assert_eq!(
        registry
            .register(999, integration("scope-overflow", "Overflow"))
            .expect_err("active scopes must be bounded"),
        IntegrationError::TooManyScopes {
            max: MAX_INTEGRATION_SCOPES,
            actual: MAX_INTEGRATION_SCOPES + 1,
        }
    );

    let mut attempts = OAuthAttempts::new();
    attempts
        .start_code("expire", "openai", "chatgpt", None, 50)
        .expect("expiring fixture should start");
    assert_eq!(attempts.expire(50 + OAUTH_ATTEMPT_LIFETIME_MS - 1), 0);
    assert_eq!(attempts.expire(50 + OAUTH_ATTEMPT_LIFETIME_MS), 1);
    assert_eq!(
        attempts.get("expire").map(|attempt| attempt.status),
        Some(OAuthAttemptStatus::Expired)
    );

    attempts
        .start_code("cancel", "openai", "chatgpt", None, 100)
        .expect("cancel fixture should start");
    attempts
        .cancel("cancel")
        .expect("pending attempt should cancel explicitly");
    assert!(attempts.get("cancel").is_none());

    attempts
        .start_code("duplicate", "openai", "chatgpt", None, 200)
        .expect("duplicate fixture should start once");
    assert_eq!(
        attempts
            .start_code("duplicate", "openai", "chatgpt", None, 201)
            .expect_err("caller-supplied attempt ids must be unique"),
        IntegrationError::DuplicateAttempt {
            attempt_id: "duplicate".to_owned(),
        }
    );

    let mut capped = OAuthAttempts::new();
    for index in 0..MAX_OAUTH_ATTEMPTS {
        capped
            .start_code(
                &format!("attempt-{index}"),
                "openai",
                "chatgpt",
                None,
                index as u64,
            )
            .expect("exact attempt bound should succeed");
    }
    assert_eq!(
        capped
            .start_code("overflow", "openai", "chatgpt", None, 999)
            .expect_err("attempt registry must reject overflow"),
        IntegrationError::TooManyAttempts {
            max: MAX_OAUTH_ATTEMPTS,
            actual: MAX_OAUTH_ATTEMPTS + 1,
        }
    );
}
