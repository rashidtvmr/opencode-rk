//! PROV-023 frozen tests: versioned provider compatibility catalog (T01..T05).

use serde_json::Value;

fn catalog_path() -> String {
    format!(
        "{}/../../docs/provider-compatibility.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn catalog() -> (String, Value) {
    let raw = std::fs::read_to_string(catalog_path()).expect("catalog file exists");
    let value: Value = serde_json::from_str(&raw).expect("catalog parses");
    (raw, value)
}

fn lookup<'a>(catalog: &'a Value, provider: &str, kind: &str) -> Option<&'a Value> {
    catalog
        .get("providers")?
        .as_array()?
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(provider))?
        .get("endpoints")?
        .as_array()?
        .iter()
        .find(|e| e.get("kind").and_then(Value::as_str) == Some(kind))
}

#[test]
fn prov_023_t01_schema_valid() {
    let (_, value) = catalog();
    assert_eq!(
        value.get("catalog_version").and_then(Value::as_str),
        Some("1")
    );
    let updated = value
        .get("updated")
        .and_then(Value::as_str)
        .expect("updated date");
    assert_eq!(updated.len(), 10, "YYYY-MM-DD");
    assert!(updated.chars().nth(4) == Some('-') && updated.chars().nth(7) == Some('-'));
    for provider in ["openai", "anthropic"] {
        let entry = value
            .get("providers")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p.get("id").and_then(Value::as_str) == Some(provider))
            .unwrap_or_else(|| panic!("{provider} present"));
        for endpoint in entry.get("endpoints").unwrap().as_array().unwrap() {
            let url = endpoint
                .get("url_template")
                .and_then(Value::as_str)
                .unwrap();
            assert!(url.starts_with("https://"), "https-only endpoint");
            assert!(!endpoint
                .get("auth_headers")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty());
        }
    }
}

#[test]
fn prov_023_t02_model_aliases_and_lookup() {
    let (_, value) = catalog();
    for entry in value.get("providers").unwrap().as_array().unwrap() {
        let aliases = entry.get("model_aliases").unwrap().as_array().unwrap();
        assert!(!aliases.is_empty(), "each provider lists an alias");
    }
    let openai = lookup(&value, "openai", "chat-completions").expect("openai lookup");
    assert_eq!(
        openai.get("url_template").and_then(Value::as_str),
        Some("https://api.openai.com/v1/chat/completions")
    );
    let anthropic = lookup(&value, "anthropic", "messages").expect("anthropic lookup");
    assert_eq!(
        anthropic.get("url_template").and_then(Value::as_str),
        Some("https://api.anthropic.com/v1/messages")
    );
}

#[test]
fn prov_023_t03_unknown_lookup() {
    let (_, value) = catalog();
    assert!(lookup(&value, "no-such-provider", "chat-completions").is_none());
    assert!(lookup(&value, "openai", "no-such-endpoint").is_none());
}

#[test]
fn prov_023_t04_prohibitions_and_caps() {
    let (raw, value) = catalog();
    assert!(!raw.contains("undocumented"), "no undocumented entries");
    for banned in ["User-Agent", "user-agent", "X-Forwarded", "sk-", "sk-ant-"] {
        assert!(!raw.contains(banned), "banned string present: {banned}");
    }
    assert!(!raw.contains("Bearer "), "no bearer token values");
    let providers = value.get("providers").unwrap().as_array().unwrap();
    assert!((1..=32).contains(&providers.len()));
    for entry in providers {
        let endpoints = entry.get("endpoints").unwrap().as_array().unwrap();
        assert!((1..=16).contains(&endpoints.len()));
        assert!(
            entry
                .get("model_aliases")
                .unwrap()
                .as_array()
                .unwrap()
                .len()
                <= 32
        );
    }
    assert!(raw.len() <= 256 * 1024, "file within 256 KiB");
}

#[test]
fn prov_023_t05_determinism_offline() {
    let (raw, value) = catalog();
    let canonical = serde_json::to_string_pretty(&value).unwrap() + "\n";
    assert_eq!(raw, canonical, "canonical re-serialization byte-identical");
}
