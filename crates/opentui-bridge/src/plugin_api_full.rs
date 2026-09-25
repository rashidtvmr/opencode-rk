#![forbid(unsafe_code)]

/// Minimal plugin API handle (mirrors TS `createTuiApi` input identity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginApi {
    name: String,
    calls: u32,
}

impl PluginApi {
    pub fn new(name: &str) -> Self {
        let name: String = name.chars().take(64).collect();
        Self { name, calls: 0 }
    }

    pub fn call(&mut self) -> bool {
        self.calls = self.calls.saturating_add(1);
        true
    }

    pub fn name_of(&self) -> &str {
        &self.name
    }

    pub fn calls(&self) -> u32 {
        self.calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_name_and_zero_calls() {
        let api = PluginApi::new("demo");
        assert_eq!(api.name_of(), "demo");
        assert_eq!(api.calls(), 0);
    }

    #[test]
    fn call_increments_and_returns_true() {
        let mut api = PluginApi::new("demo");
        assert!(api.call());
        assert!(api.call());
        assert_eq!(api.calls(), 2);
    }

    #[test]
    fn name_capped_at_64_chars() {
        let long = "x".repeat(100);
        let api = PluginApi::new(&long);
        assert_eq!(api.name_of().chars().count(), 64);
    }

    #[test]
    fn calls_saturate_at_max() {
        let mut api = PluginApi {
            name: String::from("s"),
            calls: u32::MAX,
        };
        assert!(api.call());
        assert_eq!(api.calls(), u32::MAX);
    }
}
