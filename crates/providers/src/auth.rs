//! Provider authentication module.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Authentication method for a provider.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// API Key authentication.
    ApiKey(String),
    /// Bearer token authentication.
    BearerToken(String),
    /// OAuth2 authentication with refresh support.
    OAuth2 {
        access_token: String,
        refresh_token: String,
        expires_at: Instant,
    },
}

/// Provider authentication container.
#[derive(Debug, Clone)]
pub struct ProviderAuth {
    /// Provider identifier.
    pub provider_id: String,
    /// Authentication method.
    pub method: AuthMethod,
}

impl ProviderAuth {
    /// Creates new provider authentication.
    pub fn new(provider_id: String, method: AuthMethod) -> Self {
        Self {
            provider_id,
            method,
        }
    }
}

/// Handler for managing provider authentications.
#[derive(Debug, Default)]
pub struct AuthHandler {
    auths: HashMap<String, ProviderAuth>,
}

impl AuthHandler {
    /// Creates a new empty auth handler.
    pub fn new() -> Self {
        Self {
            auths: HashMap::new(),
        }
    }

    /// Adds or updates authentication for a provider.
    pub fn add_auth(&mut self, provider_id: String, method: AuthMethod) {
        let auth = ProviderAuth::new(provider_id.clone(), method);
        self.auths.insert(provider_id, auth);
    }

    /// Retrieves the authentication method for a provider.
    pub fn get(&self, provider_id: &str) -> Option<&AuthMethod> {
        self.auths.get(provider_id).map(|a| &a.method)
    }

    /// Validates authentication for a provider.
    /// API keys and bearer tokens are always valid.
    /// OAuth2 tokens are valid only if not expired.
    pub fn validate(&self, provider_id: &str) -> bool {
        match self.get(provider_id) {
            Some(AuthMethod::ApiKey(_)) | Some(AuthMethod::BearerToken(_)) => true,
            Some(AuthMethod::OAuth2 { expires_at, .. }) => Instant::now() < *expires_at,
            None => false,
        }
    }

    /// Refreshes an OAuth2 token with new credentials.
    /// Returns true if the provider was found and updated.
    pub fn refresh(&mut self, provider_id: &str, new_token: String, expires_at: Instant) -> bool {
        let existing = self.auths.get(provider_id);
        if existing.is_none() {
            return false;
        }

        let refresh_token = match &existing.unwrap().method {
            AuthMethod::OAuth2 { refresh_token, .. } => refresh_token.clone(),
            _ => return false,
        };

        let new_method = AuthMethod::OAuth2 {
            access_token: new_token,
            refresh_token,
            expires_at,
        };

        self.auths.insert(
            provider_id.to_string(),
            ProviderAuth::new(provider_id.to_string(), new_method),
        );

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_get_apikey() {
        let mut handler = AuthHandler::new();
        handler.add_auth(
            "provider1".to_string(),
            AuthMethod::ApiKey("sk-test-key".to_string()),
        );

        let method = handler.get("provider1");
        assert!(method.is_some());

        if let Some(AuthMethod::ApiKey(key)) = method {
            assert_eq!(key, "sk-test-key");
        } else {
            panic!("Expected ApiKey method");
        }
    }

    #[test]
    fn bearer_token_valid() {
        let mut handler = AuthHandler::new();
        handler.add_auth(
            "provider2".to_string(),
            AuthMethod::BearerToken("bearer-token-123".to_string()),
        );

        assert!(handler.validate("provider2"));

        if let Some(AuthMethod::BearerToken(token)) = handler.get("provider2") {
            assert_eq!(token, "bearer-token-123");
        } else {
            panic!("Expected BearerToken method");
        }
    }

    #[test]
    fn oauth2_expired() {
        let mut handler = AuthHandler::new();
        let past = Instant::now() - Duration::from_secs(3600);
        handler.add_auth(
            "provider3".to_string(),
            AuthMethod::OAuth2 {
                access_token: "access123".to_string(),
                refresh_token: "refresh456".to_string(),
                expires_at: past,
            },
        );

        assert!(!handler.validate("provider3"));
    }

    #[test]
    fn refresh_updates_expiry() {
        let mut handler = AuthHandler::new();
        let past = Instant::now() - Duration::from_secs(3600);
        handler.add_auth(
            "provider4".to_string(),
            AuthMethod::OAuth2 {
                access_token: "old-access".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires_at: past,
            },
        );

        let future = Instant::now() + Duration::from_secs(3600);
        let result = handler.refresh("provider4", "new-access".to_string(), future);

        assert!(result);
        assert!(handler.validate("provider4"));

        if let Some(AuthMethod::OAuth2 { access_token, .. }) = handler.get("provider4") {
            assert_eq!(access_token, "new-access");
        } else {
            panic!("Expected OAuth2 method");
        }
    }

    #[test]
    fn token_format_bearer() {
        let mut handler = AuthHandler::new();
        handler.add_auth(
            "provider5".to_string(),
            AuthMethod::BearerToken("test-token".to_string()),
        );

        let method = handler.get("provider5");
        assert!(method.is_some());

        if let Some(AuthMethod::BearerToken(token)) = method {
            assert_eq!(token, "test-token");
        } else {
            panic!("Expected BearerToken method");
        }
    }
}
