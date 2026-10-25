use serde_json::Value;

use super::native_compaction_settings::CompactionSettingsView;

fn response(enabled: bool, threshold: u64, recent: u64, strategy: &str) -> String {
    serde_json::json!({
        "scope": "session",
        "capabilities": {
            "native": { "availability": "unverified", "model": null,
                "retention": "provider-owned" },
            "fallback": { "availability": "responses-summary",
                "retention": "keep_recent_turns" }
        },
        "configured": { "enabled": enabled, "threshold_tokens": threshold,
            "keep_recent_turns": recent, "strategy": strategy },
        "effective": { "enabled": true, "threshold_tokens": 1,
            "keep_recent_turns": 3, "strategy": "fallback" },
        "overridden_by_environment": ["enabled", "threshold_tokens"],
        "checkpoint": null,
        "run_active": false
    })
    .to_string()
}

fn text(view: &CompactionSettingsView, width: usize, height: usize) -> String {
    view.render(width, height).join("\n")
}

#[test]
fn new_view_is_unloaded_and_does_not_offer_apply_before_get() {
    let mut view = CompactionSettingsView::new();
    let rendered = text(&view, 120, 20);
    assert!(rendered.contains("unloaded"));
    assert!(!rendered.contains("configured: true"));
    for _ in 0..4 {
        view.key(b'j');
    }
    assert_eq!(view.key(b'\n'), None);
    assert_eq!(view.key(b'\r'), None);
}

#[test]
fn load_preserves_configured_and_effective_values_and_cycles_strategy() {
    let mut view = CompactionSettingsView::new();
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    let rendered = text(&view, 160, 20);
    assert!(rendered.contains("Enabled: false"));
    assert!(rendered.contains("Strategy: auto"));
    assert!(rendered.contains("Threshold: 100000"));
    assert!(rendered.contains("Keep recent turns: 20"));
    assert!(rendered.contains("unverified"));
    assert!(rendered.contains("provider-owned"));
    view.key(b'j');
    view.key(b'\n');
    assert!(text(&view, 160, 20).contains("Strategy: native"));
    view.key(b'\n');
    assert!(text(&view, 160, 20).contains("Strategy: fallback"));
    assert!(text(&view, 160, 20).contains("Effective: true"));
    assert!(text(&view, 160, 20).contains("Threshold: 1"));
    assert!(text(&view, 160, 20).contains("Keep recent turns: 3"));
}

#[test]
fn keys_edit_numeric_rows_and_apply_is_exactly_four_setting_keys() {
    let mut view = CompactionSettingsView::new();
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    view.key(b'j');
    view.key(b'j');
    view.key(b'\n');
    view.key(b'1');
    view.key(b'2');
    view.key(b'3');
    view.key(b'4');
    view.key(8);
    view.key(b'\n');
    view.key(b'j');
    view.key(b'\n');
    view.key(b'2');
    view.key(b'\n');
    view.key(b'j');
    let saved: Value = view.key(b'\n').expect("apply JSON").parse().unwrap();
    let object = saved.as_object().unwrap();
    assert_eq!(object.len(), 4);
    assert!(object.get("scope").is_none());
    assert_eq!(object["keep_recent_turns"], 2);
    assert_eq!(object["threshold_tokens"], 123);
    assert_eq!(object["strategy"], "auto");
    assert_eq!(object["enabled"], false);
    assert!(text(&view, 160, 20).contains("Threshold: 100000"));
    assert!(text(&view, 160, 20).contains("Keep recent turns: 20"));
    let after_apply = text(&view, 160, 20);
    assert!(after_apply.contains("Configured"));
    assert!(after_apply.contains("100000"));
    assert!(after_apply.contains("123"));
}

#[test]
fn discard_restores_ack_and_invalid_numbers_do_not_replace_valid_draft() {
    let mut view = CompactionSettingsView::new();
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    view.key(b'\r'); // toggle enabled with CR
    view.key(b'j');
    view.key(b' '); // strategy
    view.key(b'j');
    view.key(b'\n');
    view.key(b'0');
    view.key(b'\n'); // invalid threshold leaves 100000
    view.key(b'j');
    view.key(b'j');
    view.key(b'j'); // row 5, discard
    view.key(b'\n');
    view.key(b'k');
    view.key(b'k');
    view.key(b'k'); // explicitly return to row 2
    view.key(b'\n');
    view.key(b'0');
    view.key(b'\n'); // invalid edit still leaves valid draft
    view.key(b'j');
    view.key(b'j'); // row 4, Apply
    let saved: Value = view.key(b'\n').unwrap().parse().unwrap();
    assert_eq!(saved["threshold_tokens"], 100000);
    assert_eq!(saved["enabled"], false);
    assert!(text(&view, 160, 20).contains("Enabled: false"));
}

#[test]
fn malformed_server_error_and_invalid_metadata_preserve_acknowledged_state() {
    let mut view = CompactionSettingsView::new();
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    view.key(b' ');
    assert!(view.load_response("not json").is_err());
    assert!(view
        .load_response(
            &response(false, 100000, 20, "auto")
                .replace("\"scope\":\"session\"", "\"scope\":\"other\"")
        )
        .is_err());
    assert!(view
        .load_response(
            &response(false, 100000, 20, "auto")
                .replace("\"run_active\":false", "\"run_active\":\"no\"")
        )
        .is_err());
    assert!(view
        .load_response(
            &response(false, 100000, 20, "auto").replace("\"enabled\":false", "\"enabled\":\"no\"")
        )
        .is_err());
    assert!(view
        .load_response(&response(false, 100000, 20, "auto").replace(
            "\"strategy\":\"auto\"",
            "\"strategy\":\"auto\",\"unexpected\":true"
        ))
        .is_err());
    view.set_error("server error");
    assert!(text(&view, 160, 20).contains("Enabled: false"));
    assert!(text(&view, 160, 20).contains("server error"));
}

#[test]
fn invalidate_clears_stale_ack_and_render_is_bounded_with_selected_apply_visible() {
    let mut view = CompactionSettingsView::new();
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    view.invalidate();
    let unloaded = text(&view, 80, 20);
    assert!(unloaded.contains("unloaded"));
    assert!(!unloaded.contains("Apply"));
    view.load_response(&response(false, 100000, 20, "auto"))
        .unwrap();
    for _ in 0..4 {
        view.key(b'j');
    }
    assert!(view.key(b'\n').is_some());
    for width in 0..=20 {
        for height in 0..=12 {
            for line in view.render(width, height) {
                assert!(line.chars().count() <= width);
                assert!(line.chars().all(|c| !c.is_control() || c == '\n'));
            }
        }
    }
    let compact = text(&view, 20, 4);
    assert!(compact.contains("Apply"));
}
