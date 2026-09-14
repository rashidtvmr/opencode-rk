//! Provider configuration module.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;

/// Configuration for an LLM provider.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderConfig {
    /// Unique identifier for the provider (e.g., "openai", "anthropic").
    pub provider_id: String,
    /// Base URL for the provider's API.
    pub base_url: String,
    /// Environment variable name for the API key.
    pub api_key_env: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum tokens for response generation.
    pub max_tokens: u32,
    /// Temperature for response generation.
    pub temperature: f32,
}

impl ProviderConfig {
    /// Returns default provider configuration for common LLM providers.
    pub fn load_default() -> Self {
        Self::load_default_for("openai")
    }

    /// Returns default provider configuration for the given provider id.
    pub fn load_default_for(provider_id: &str) -> Self {
        match provider_id {
            "openai" => Self {
                provider_id: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_env: "OPENAI_API_KEY".to_string(),
                timeout_secs: 60,
                max_tokens: 4096,
                temperature: 0.7,
            },
            "anthropic" => Self {
                provider_id: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
                timeout_secs: 60,
                max_tokens: 4096,
                temperature: 0.7,
            },
            "google" => Self {
                provider_id: "google".to_string(),
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                api_key_env: "GOOGLE_API_KEY".to_string(),
                timeout_secs: 60,
                max_tokens: 4096,
                temperature: 0.7,
            },
            "deepseek" => Self {
                provider_id: "deepseek".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                api_key_env: "DEEPSEEK_API_KEY".to_string(),
                timeout_secs: 60,
                max_tokens: 4096,
                temperature: 0.7,
            },
            _ => Self {
                provider_id: provider_id.to_string(),
                base_url: "https://api.example.com/v1".to_string(),
                api_key_env: format!("{}_API_KEY", provider_id.to_uppercase()),
                timeout_secs: 60,
                max_tokens: 4096,
                temperature: 0.7,
            },
        }
    }

    /// Loads provider configuration from environment variables.
    ///
    /// Expected environment variables (all prefixed with uppercase provider_id):
    /// - {PROVIDER_ID}_BASE_URL
    /// - {PROVIDER_ID}_API_KEY_ENV
    /// - {PROVIDER_ID}_TIMEOUT_SECS
    /// - {PROVIDER_ID}_MAX_TOKENS
    /// - {PROVIDER_ID}_TEMPERATURE
    pub fn from_env(provider_id: &str) -> Self {
        let prefix = provider_id.to_uppercase();
        let base_url = env::var(format!("{}_BASE_URL", prefix))
            .unwrap_or_else(|_| Self::load_default_for(provider_id).base_url);

        let api_key_env = env::var(format!("{}_API_KEY_ENV", prefix))
            .unwrap_or_else(|_| Self::load_default_for(provider_id).api_key_env);

        let timeout_secs = env::var(format!("{}_TIMEOUT_SECS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let max_tokens = env::var(format!("{}_MAX_TOKENS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4096);

        let temperature = env::var(format!("{}_TEMPERATURE", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.7);

        Self {
            provider_id: provider_id.to_string(),
            base_url,
            api_key_env,
            timeout_secs,
            max_tokens,
            temperature,
        }
    }

    /// Validates the provider configuration.
    ///
    /// Returns Ok(()) if valid, Err(String) with an error message otherwise.
    pub fn validate(&self) -> Result<(), String> {
        if self.provider_id.is_empty() {
            return Err("provider_id cannot be empty".to_string());
        }

        if self.base_url.is_empty() {
            return Err("base_url cannot be empty".to_string());
        }

        if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            return Err(format!("base_url must start with http:// or https://, got: {}", self.base_url));
        }

        if self.api_key_env.is_empty() {
            return Err("api_key_env cannot be empty".to_string());
        }

        if self.timeout_secs == 0 {
            return Err("timeout_secs must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Returns the API key from the environment variable specified in api_key_env.
    pub fn get_api_key(&self) -> Option<String> {
        env::var(&self.api_key_env).ok()
    }
}

/// A set of provider configurations.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfigSet {
    configs: HashMap<String, ProviderConfig>,
}

impl ProviderConfigSet {
    /// Creates a new empty ProviderConfigSet.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// Adds a provider configuration to the set.
    /// Overwrites any existing config for the same provider_id.
    pub fn add(&mut self, config: ProviderConfig) -> Option<ProviderConfig> {
        self.configs.insert(config.provider_id.clone(), config)
    }

    /// Retrieves a provider configuration by provider_id.
    pub fn get(&self, provider_id: &str) -> Option<&ProviderConfig> {
        self.configs.get(provider_id)
    }

    /// Merges another config set into this one.
    /// Configs from `other` overwrite conflicting ones in `self`.
    pub fn merge(&mut self, other: ProviderConfigSet) {
        for (id, config) in other.configs {
            self.configs.insert(id, config);
        }
    }

    /// Converts the config set to a JSON representation.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        for (id, config) in &self.configs {
            let mut config_map = Map::new();
            config_map.insert("provider_id".to_string(), Value::String(config.provider_id.clone()));
            config_map.insert("base_url".to_string(), Value::String(config.base_url.clone()));
            config_map.insert("api_key_env".to_string(), Value::String(config.api_key_env.clone()));
            config_map.insert("timeout_secs".to_string(), Value::Number(config.timeout_secs.into()));
            config_map.insert("max_tokens".to_string(), Value::Number(config.max_tokens.into()));
            config_map.insert("temperature".to_string(), Value::Number(serde_json::Number::from_f64(config.temperature as f64).unwrap_or(serde_json::Number::from_f64(0.7).unwrap())));
            map.insert(id.clone(), Value::Object(config_map));
        }
        Value::Object(map)
    }

    /// Returns the number of configs in the set.
    pub fn len(&self) -> usize {
        self.configs.len()
    }

    /// Returns true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ProviderConfig::load_default();
        assert_eq!(config.provider_id, "openai");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key_env, "OPENAI_API_KEY");
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.max_tokens, 4096);
        assert!((config.temperature - 0.7).abs() < f32::EPSILON);

        let validate_result = config.validate();
        assert!(validate_result.is_ok());
    }

    #[test]
    fn from_env_loads_defaults() {
        // Test that from_env falls back to defaults when env vars are not set
        let config = ProviderConfig::from_env("nonexistent_test_provider_xyz");

        assert_eq!(config.provider_id, "nonexistent_test_provider_xyz");
        assert_eq!(config.base_url, "https://api.example.com/v1");
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.max_tokens, 4096);
        assert!((config.temperature - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn validate_valid() {
        let config = ProviderConfig {
            provider_id: "my_provider".to_string(),
            base_url: "https://api.example.com/v1".to_string(),
            api_key_env: "MY_API_KEY".to_string(),
            timeout_secs: 30,
            max_tokens: 2048,
            temperature: 0.5,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_invalid_url() {
        let config = ProviderConfig {
            provider_id: "bad_provider".to_string(),
            base_url: "not-a-valid-url".to_string(),
            api_key_env: "BAD_API_KEY".to_string(),
            timeout_secs: 30,
            max_tokens: 2048,
            temperature: 0.5,
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("base_url must start with http:// or https://"));
    }

    #[test]
    fn merge_combines() {
        let mut set1 = ProviderConfigSet::new();
        set1.add(ProviderConfig {
            provider_id: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            timeout_secs: 60,
            max_tokens: 4096,
            temperature: 0.7,
        });

        let mut set2 = ProviderConfigSet::new();
        set2.add(ProviderConfig {
            provider_id: "anthropic".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            timeout_secs: 30,
            max_tokens: 8192,
            temperature: 0.3,
        });

        set1.merge(set2);

        assert_eq!(set1.len(), 2);
        assert!(set1.get("openai").is_some());
        assert!(set1.get("anthropic").is_some());

        let openai = set1.get("openai").unwrap();
        assert_eq!(openai.base_url, "https://api.openai.com/v1");
        let anthropic = set1.get("anthropic").unwrap();
        assert_eq!(anthropic.timeout_secs, 30);

        let json = set1.to_json();
        assert!(json.is_object());
        let json_map = json.as_object().unwrap();
        assert_eq!(json_map.len(), 2);
        assert!(json_map.contains_key("openai"));
        assert!(json_map.contains_key("anthropic"));
    }
}