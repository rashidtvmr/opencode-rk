//! Integration tests for ProviderConfig::from_env
//! These tests require environment variables and are in a separate crate
//! to avoid the forbid(unsafe_code) restriction in the lib.

use opencode_rk_providers::config::ProviderConfig;

#[test]
fn from_env_loads() {
    let provider_id = "test_provider";
    unsafe {
        std::env::set_var("TEST_PROVIDER_BASE_URL", "https://test.example.com/v1");
        std::env::set_var("TEST_PROVIDER_API_KEY_ENV", "TEST_KEY");
        std::env::set_var("TEST_PROVIDER_TIMEOUT_SECS", "120");
        std::env::set_var("TEST_PROVIDER_MAX_TOKENS", "8192");
        std::env::set_var("TEST_PROVIDER_TEMPERATURE", "0.5");
    }

    let config = ProviderConfig::from_env(provider_id);

    assert_eq!(config.provider_id, "test_provider");
    assert_eq!(config.base_url, "https://test.example.com/v1");
    assert_eq!(config.api_key_env, "TEST_KEY");
    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_tokens, 8192);
    assert!((config.temperature - 0.5).abs() < f32::EPSILON);

    unsafe {
        std::env::remove_var("TEST_PROVIDER_BASE_URL");
        std::env::remove_var("TEST_PROVIDER_API_KEY_ENV");
        std::env::remove_var("TEST_PROVIDER_TIMEOUT_SECS");
        std::env::remove_var("TEST_PROVIDER_MAX_TOKENS");
        std::env::remove_var("TEST_PROVIDER_TEMPERATURE");
    }
}
