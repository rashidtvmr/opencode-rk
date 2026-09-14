//! Provider registry module for LLM provider routing.

use std::collections::HashMap;

/// Represents an LLM provider configuration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Provider {
    /// Unique identifier for the provider (e.g., "openai", "anthropic").
    pub id: String,
    /// Human-readable name (e.g., "OpenAI", "Anthropic").
    pub name: String,
    /// Base URL for the provider's API.
    pub base_url: String,
    /// Environment variable name for the API key.
    pub api_key_env: String,
    /// Priority for routing (lower = higher priority).
    pub priority: u32,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Provider {
    /// Creates a new Provider with the given configuration.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key_env: impl Into<String>,
        priority: u32,
        timeout_secs: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            api_key_env: api_key_env.into(),
            priority,
            timeout_secs,
        }
    }
}

/// Registry for LLM providers with priority-based ordering.
#[derive(Clone, Debug, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Provider>,
    by_priority: Vec<String>,
}

impl ProviderRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            by_priority: Vec::new(),
        }
    }

    /// Registers a provider. Returns true if the provider was newly added,
    /// false if it replaced an existing one.
    pub fn register(&mut self, provider: Provider) -> bool {
        let is_new = !self.providers.contains_key(&provider.id);
        self.providers.insert(provider.id.clone(), provider);

        // Rebuild priority ordering
        self.rebuild_priority_order();

        is_new
    }

    /// Retrieves a provider by id. Returns None if not found.
    pub fn get(&self, id: &str) -> Option<&Provider> {
        self.providers.get(id)
    }

    /// Lists all providers sorted by priority (lowest priority value first).
    pub fn list_by_priority(&self) -> Vec<&Provider> {
        self.by_priority
            .iter()
            .filter_map(|id| self.providers.get(id))
            .collect()
    }

    /// Removes a provider by id. Returns true if the provider existed and was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let removed = self.providers.remove(id).is_some();
        if removed {
            self.by_priority.retain(|existing_id| existing_id != id);
        }
        removed
    }

    /// Returns the number of registered providers.
    pub fn count(&self) -> usize {
        self.providers.len()
    }

    fn rebuild_priority_order(&mut self) {
        self.by_priority = self.providers.values().map(|p| p.id.clone()).collect();
        self.by_priority.sort_by(|a, b| {
            let pa = self.providers.get(a).unwrap();
            let pb = self.providers.get(b).unwrap();
            pa.priority.cmp(&pb.priority)
        });
    }
}

/// Provider configuration with default provider settings.
pub struct ProviderConfig;

impl ProviderConfig {
    /// Returns default provider configurations for common LLM providers.
    pub fn default_providers() -> Vec<Provider> {
        vec![
            Provider::new(
                "openai",
                "OpenAI",
                "https://api.openai.com/v1",
                "OPENAI_API_KEY",
                1,
                60,
            ),
            Provider::new(
                "anthropic",
                "Anthropic",
                "https://api.anthropic.com",
                "ANTHROPIC_API_KEY",
                2,
                60,
            ),
            Provider::new(
                "google",
                "Google",
                "https://generativelanguage.googleapis.com/v1beta",
                "GOOGLE_API_KEY",
                3,
                60,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut registry = ProviderRegistry::new();
        let provider = Provider::new(
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
            "OPENAI_API_KEY",
            1,
            60,
        );

        assert!(registry.register(provider.clone()));
        assert_eq!(registry.count(), 1);

        let retrieved = registry.get("openai");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &provider);
    }

    #[test]
    fn list_sorted_by_priority() {
        let mut registry = ProviderRegistry::new();

        registry.register(Provider::new(
            "google",
            "Google",
            "https://google.com",
            "GOOGLE_API_KEY",
            3,
            60,
        ));
        registry.register(Provider::new(
            "anthropic",
            "Anthropic",
            "https://anthropic.com",
            "ANTHROPIC_API_KEY",
            2,
            60,
        ));
        registry.register(Provider::new(
            "openai",
            "OpenAI",
            "https://openai.com",
            "OPENAI_API_KEY",
            1,
            60,
        ));

        let list = registry.list_by_priority();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, "openai");
        assert_eq!(list[1].id, "anthropic");
        assert_eq!(list[2].id, "google");
    }

    #[test]
    fn remove_existing() {
        let mut registry = ProviderRegistry::new();
        registry.register(Provider::new(
            "openai",
            "OpenAI",
            "https://openai.com",
            "OPENAI_API_KEY",
            1,
            60,
        ));

        assert!(registry.remove("openai"));
        assert_eq!(registry.count(), 0);
        assert!(registry.get("openai").is_none());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut registry = ProviderRegistry::new();
        registry.register(Provider::new(
            "openai",
            "OpenAI",
            "https://openai.com",
            "OPENAI_API_KEY",
            1,
            60,
        ));

        assert!(!registry.remove("anthropic"));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn count_correct() {
        let mut registry = ProviderRegistry::new();

        assert_eq!(registry.count(), 0);

        registry.register(Provider::new(
            "openai",
            "OpenAI",
            "https://openai.com",
            "OPENAI_API_KEY",
            1,
            60,
        ));
        assert_eq!(registry.count(), 1);

        registry.register(Provider::new(
            "anthropic",
            "Anthropic",
            "https://anthropic.com",
            "ANTHROPIC_API_KEY",
            2,
            60,
        ));
        assert_eq!(registry.count(), 2);

        // Replacing an existing provider should not increase count
        registry.register(Provider::new(
            "openai",
            "OpenAI",
            "https://new.openai.com",
            "NEW_KEY",
            1,
            30,
        ));
        assert_eq!(registry.count(), 2);
    }
}
