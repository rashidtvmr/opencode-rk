use super::native_compaction_settings::CompactionSettingsView;

fn response(model: serde_json::Value, availability: &str) -> String {
    serde_json::json!({
        "scope": "session",
        "capabilities": {
            "native": { "availability": availability, "model": model, "retention": "provider-owned" },
            "fallback": { "availability": "responses-summary", "retention": "keep_recent_turns" }
        },
        "configured": { "enabled": true, "threshold_tokens": 1000, "keep_recent_turns": 4, "strategy": "auto" },
        "effective": { "enabled": true, "threshold_tokens": 1000, "keep_recent_turns": 4, "strategy": "auto" },
        "overridden_by_environment": [],
        "checkpoint": null,
        "run_active": false
    }).to_string()
}

#[test]
fn model_provenance_is_strict_and_rejection_preserves_ack_and_draft() {
    let mut view = CompactionSettingsView::new();
    assert!(view.load_response(&response(serde_json::Value::String("openai/fixture".into()), "unverified")).is_err());
    assert!(view.load_response(&response(serde_json::Value::Null, "verified-for-checkpoint-model")).is_err());
    assert!(view.load_response(&response(serde_json::Value::String("openai/fixture".into()), "verified-for-checkpoint-model")).is_ok());

    // Make the unsaved draft observably differ from the acknowledged policy.
    view.key(b'j');
    view.key(b'\n');
    view.key(b' ');
    let before = view.render(160, 20).join("\n");
    assert!(view.load_response(&response(serde_json::Value::Null, "verified-for-checkpoint-model")).is_err());
    let after = view.render(160, 20).join("\n");
    assert_eq!(after, before, "malformed provenance ACK must preserve ACK and draft");
    assert!(after.contains("Configured"));
}
