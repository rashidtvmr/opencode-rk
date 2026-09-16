//! PROV-021 frozen tests: usage and limits telemetry (T01..T05).

use opencode_rk_providers::usage_status::{
    record, reset, snapshot, UsageDelta, UsageError, UsageKey, UsageStore,
};

fn key(provider: &str, model: &str) -> UsageKey {
    UsageKey::new(provider, model, "api-key").expect("valid key")
}

fn delta(requests: u64, input: u64, output: u64) -> UsageDelta {
    UsageDelta {
        requests,
        input_tokens: input,
        output_tokens: output,
        cost_micros: Some(1500),
        rate_limit_reset_ms: Some(9_999_000),
    }
}

#[test]
fn prov_021_t01_record_happy_path() {
    let mut store = UsageStore::with_providers(["openai"]).expect("register");
    record(&mut store, key("openai", "gpt-x"), delta(3, 100, 50)).unwrap();
    let snap = snapshot(&store);
    assert_eq!(snap.entries.len(), 1);
    let entry = &snap.entries[0];
    assert_eq!(entry.counters.requests, 3);
    assert_eq!(entry.counters.input_tokens, 100);
    assert_eq!(entry.counters.output_tokens, 50);
    assert_eq!(entry.counters.cost_micros, Some(1500));

    record(&mut store, key("openai", "gpt-x"), delta(2, 10, 5)).unwrap();
    let snap = snapshot(&store);
    assert_eq!(snap.entries[0].counters.requests, 5);
    assert_eq!(snap.entries[0].counters.cost_micros, Some(3000));
}

#[test]
fn prov_021_t02_snapshot_order_and_reset() {
    let mut store = UsageStore::with_providers(["openai", "anthropic"]).expect("register");
    record(&mut store, key("openai", "gpt-x"), delta(1, 1, 1)).unwrap();
    record(&mut store, key("anthropic", "claude-x"), delta(1, 1, 1)).unwrap();
    let snap = snapshot(&store);
    assert_eq!(snap.entries.len(), 2);
    assert_eq!(snap.entries[0].key.provider_id, "anthropic");
    assert_eq!(snap.entries[1].key.provider_id, "openai");
    assert!(!snap.truncated);

    reset(&mut store, key("anthropic", "claude-x"));
    let snap = snapshot(&store);
    assert_eq!(snap.entries.len(), 1);
    assert_eq!(snap.entries[0].key.provider_id, "openai");
}

#[test]
fn prov_021_t03_overflow_and_delta_caps() {
    let providers: Vec<String> = (0..256).map(|i| format!("p-{i:03}")).collect();
    let mut store = UsageStore::with_providers(providers).expect("256 providers");
    for i in 0..256 {
        record(
            &mut store,
            UsageKey::new(format!("p-{i:03}"), "m", "api-key").unwrap(),
            delta(1, 1, 1),
        )
        .unwrap();
    }
    let before = snapshot(&store).entries.len();
    assert_eq!(
        record(
            &mut store,
            UsageKey::new("p-000", "m2", "api-key").unwrap(),
            delta(1, 1, 1)
        )
        .unwrap_err(),
        UsageError::Overflow
    );
    assert_eq!(snapshot(&store).entries.len(), before, "store unchanged");

    let mut store = UsageStore::with_providers(["openai"]).expect("register");
    assert_eq!(
        record(&mut store, key("openai", "m"), delta(1, 10_000_001, 0)).unwrap_err(),
        UsageError::DeltaTooLarge
    );

    record(
        &mut store,
        key("openai", "m"),
        UsageDelta {
            requests: u64::MAX,
            ..Default::default()
        },
    )
    .unwrap();
    record(
        &mut store,
        key("openai", "m"),
        UsageDelta {
            requests: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        store.get(key("openai", "m")).unwrap().requests,
        u64::MAX,
        "saturates without wrap"
    );
}

#[test]
fn prov_021_t04_validation() {
    assert_eq!(
        UsageKey::new("", "m", "s").unwrap_err(),
        UsageError::EmptyField
    );
    assert_eq!(
        UsageKey::new("p", "", "s").unwrap_err(),
        UsageError::EmptyField
    );
    assert_eq!(
        UsageKey::new("p", "m", "").unwrap_err(),
        UsageError::EmptyField
    );
    let mut store = UsageStore::with_providers(["openai"]).expect("register");
    assert_eq!(
        record(&mut store, key("ghost", "m"), delta(1, 1, 1)).unwrap_err(),
        UsageError::UnknownProvider
    );
    reset(&mut store, key("openai", "never-recorded"));
}

#[test]
fn prov_021_t05_determinism_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let run = || {
        let mut store = UsageStore::with_providers(["openai"]).expect("register");
        record(&mut store, key("openai", "gpt-x"), delta(3, 100, 50)).unwrap();
        serde_json::to_string(&snapshot(&store)).unwrap()
    };
    assert_eq!(run(), run(), "byte-identical snapshot");

    let mut store = UsageStore::with_providers(["openai"]).expect("register");
    record(&mut store, key("openai", "gpt-x"), delta(1, 1, 1)).unwrap();
    let text = format!("{:?}", snapshot(&store))
        + &serde_json::to_string(&snapshot(&store)).unwrap()
        + &serde_json::to_string(&UsageError::Overflow).unwrap();
    assert!(!text.contains("sk-") && !text.contains("Bearer"));
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "no files or db writes");
}
