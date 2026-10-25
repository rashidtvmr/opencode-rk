use super::native_context::ContextView;

fn body(items: serde_json::Value) -> String {
    serde_json::json!({"scope":"session","items": items, "next_before": null, "projection":{"status":"available","bytes":12,"estimated_tokens":3,"typed_tail_items":2,"provenance":"bytes / 4"},"input_limits":{"items":100,"bytes":524288}}).to_string()
}

fn item(id: &str, kind: &str) -> serde_json::Value {
    serde_json::json!({"id":id,"kind":kind,"role":"user","source":"durable_history","included":true,"selectable":true,"content":"hello","bytes":5,"estimated_tokens":2})
}

#[test]
fn loads_and_preserves_selected_identity_by_id() {
    let mut view = ContextView::new();
    view.load_response(&body(serde_json::json!([
        item("00000000-0000-0000-0000-000000000001", "message"),
        item("00000000-0000-0000-0000-000000000002", "message")
    ])))
    .unwrap();
    view.select_step(true);
    view.load_response(&body(serde_json::json!([
        item("00000000-0000-0000-0000-000000000002", "message"),
        item("00000000-0000-0000-0000-000000000003", "message")
    ])))
    .unwrap();
    assert_eq!(
        view.selected().unwrap().id,
        "00000000-0000-0000-0000-000000000002"
    );
}

#[test]
fn malformed_load_is_atomic_and_retains_old_state() {
    let mut view = ContextView::new();
    view.load_response(&body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )])))
    .unwrap();
    assert!(view
        .load_response(&body(serde_json::json!([item("bad", "message")])))
        .is_err());
    assert_eq!(
        view.selected().unwrap().id,
        "00000000-0000-0000-0000-000000000001"
    );
}

#[test]
fn rejects_unknown_kind_missing_required_flags_and_bad_next_before() {
    let mut view = ContextView::new();
    assert!(view
        .load_response(&body(serde_json::json!([item(
            "00000000-0000-0000-0000-000000000001",
            "unknown"
        )])))
        .is_err());
    let mut missing = item("00000000-0000-0000-0000-000000000001", "message");
    missing.as_object_mut().unwrap().remove("selectable");
    assert!(view
        .load_response(&body(serde_json::json!([missing])))
        .is_err());
}

#[test]
fn bounds_content_and_memory_identity() {
    let mut view = ContextView::new();
    let mut x = item("memory:/workspace/AGENTS.md", "memory");
    x["role"] = serde_json::json!("system");
    x["selectable"] = serde_json::json!(false);
    x["content"] = serde_json::json!("x".repeat(96 * 1024 + 1));
    assert!(view.load_response(&body(serde_json::json!([x]))).is_err());
}

#[test]
fn render_is_width_bounded_and_contains_metadata() {
    let mut view = ContextView::new();
    view.load_response(&body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )])))
    .unwrap();
    for line in view.render(12, 8) {
        assert!(crate::native_view_layout::visit_unpadded_cells(&line, 1000, |_, _| {}) <= 12);
    }
    assert!(view
        .render(80, 8)
        .join("\n")
        .contains("projection available"));
}

#[test]
fn navigation_and_detail_scroll_are_safe_when_empty_or_short() {
    let mut view = ContextView::new();
    view.select_step(true);
    view.scroll_detail(true);
    view.scroll_detail(false);
    assert!(view.selected().is_none());
    view.load_response(&body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )])))
    .unwrap();
    view.scroll_detail(true);
    assert!(view.selected().is_some());
}

#[test]
fn invalid_scope_duplicate_ids_and_mixed_prefixes_preserve_ack() {
    let mut view = ContextView::new();
    let good = body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )]));
    view.load_response(&good).unwrap();
    let mut wrong: serde_json::Value = serde_json::from_str(&good).unwrap();
    wrong["scope"] = serde_json::json!("global");
    assert!(view.load_response(&wrong.to_string()).is_err());
    let duplicate = body(serde_json::json!([
        item("00000000-0000-0000-0000-000000000001", "message"),
        item("00000000-0000-0000-0000-000000000001", "message")
    ]));
    assert!(view.load_response(&duplicate).is_err());
    let mixed = body(serde_json::json!([item("memory:/tmp/x", "message")]));
    assert!(view.load_response(&mixed).is_err());
    assert_eq!(
        view.selected().unwrap().id,
        "00000000-0000-0000-0000-000000000001"
    );
}

#[test]
fn invalidation_discards_projection_metadata_and_checkpoint() {
    let mut view = ContextView::new();
    let mut value: serde_json::Value = serde_json::from_str(&body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )])))
    .unwrap();
    value["checkpoint"] = serde_json::json!({"strategy":"native","included":true,"boundary_message_id":"00000000-0000-0000-0000-000000000001","provider_id":"p","model_id":"m"});
    view.load_response(&value.to_string()).unwrap();
    view.invalidate();
    assert!(view.selected().is_none());
    assert!(view.render(80, 8).join("\n").contains("unloaded"));
}

#[test]
fn small_viewport_keeps_selected_summary_and_shows_actual_estimates() {
    let mut view = ContextView::new();
    let mut value: serde_json::Value = serde_json::from_str(&body(serde_json::json!([item(
        "00000000-0000-0000-0000-000000000001",
        "message"
    )])))
    .unwrap();
    value["items"][0]["bytes"] = serde_json::json!(77);
    value["items"][0]["estimated_tokens"] = serde_json::json!(19);
    view.load_response(&value.to_string()).unwrap();
    let rows = view.render(20, 2).join("\n");
    assert!(rows.contains("message") && rows.contains("77 bytes / 19 tokens"));
}

#[test]
fn tool_pair_records_share_budget_and_require_call_output_pair() {
    let mut view = ContextView::new();
    let mut tool = item("00000000-0000-0000-0000-000000000001", "tool_pair");
    tool["role"] = serde_json::json!("tool");
    tool["records"] = serde_json::json!([
        {"kind":"call","name":"search","call_id":"c1","content":"call"},
        {"kind":"output","name":"search","call_id":"c1","content":"output"}
    ]);
    view.load_response(&body(serde_json::json!([tool])))
        .unwrap();
    let mut bad = item("00000000-0000-0000-0000-000000000002", "tool_pair");
    bad["role"] = serde_json::json!("tool");
    bad["records"] = serde_json::json!([{"kind":"output","content":"x"}]);
    assert!(view.load_response(&body(serde_json::json!([bad]))).is_err());
}

#[test]
fn long_source_and_checkpoint_metadata_remain_reachable_by_detail_scrolling() {
    let mut view = ContextView::new();
    let source = format!("/workspace/{}/SOURCE_SUFFIX", "nested/".repeat(35));
    let mut entry = item(&format!("memory:{source}"), "memory");
    entry["source"] = serde_json::json!(source);
    entry["role"] = serde_json::json!("system");
    entry["selectable"] = serde_json::json!(false);
    let mut snapshot: serde_json::Value = serde_json::from_str(&body(serde_json::json!([entry]))).unwrap();
    snapshot["checkpoint"] = serde_json::json!({
        "strategy":"native","included":true,
        "boundary_message_id":"00000000-0000-0000-0000-000000000001",
        "provider_id":"p".repeat(180),
        "model_id":format!("{}MODEL_SUFFIX","m".repeat(180))
    });
    view.load_response(&snapshot.to_string()).unwrap();
    let mut seen = String::new();
    for _ in 0..100 {
        seen.push_str(&view.render(40, 8).join("\n"));
        view.scroll_detail(true);
    }
    assert!(seen.contains("SOURCE_SUFFIX"), "source attribution must be fully inspectable");
    assert!(seen.contains("MODEL_SUFFIX"), "checkpoint identity must be fully inspectable");
}
#[test]
fn typed_checkpoint_representation_authority_and_legacy_fallback() {
    let mut explicit_false = item("00000000-0000-0000-0000-000000000001", "message");
    explicit_false["represented_by_checkpoint"] = serde_json::json!(false);
    explicit_false["reason"] = serde_json::json!("checkpoint_is_indivisible");
    let mut view = ContextView::new();
    view.load_response(&body(serde_json::json!([explicit_false])))
        .unwrap();
    assert!(!view.items()[0].represented_by_checkpoint);
    assert!(view.items()[0].selectable);

    let mut legacy = item("00000000-0000-0000-0000-000000000002", "message");
    legacy.as_object_mut().unwrap().remove("represented_by_checkpoint");
    legacy["selectable"] = serde_json::json!(false);
    legacy["reason"] = serde_json::json!("checkpoint_is_indivisible");
    let mut legacy_view = ContextView::new();
    legacy_view.load_response(&body(serde_json::json!([legacy]))).unwrap();
    assert!(legacy_view.items()[0].represented_by_checkpoint);

    let mut explicit_true = item("00000000-0000-0000-0000-000000000003", "message");
    explicit_true["selectable"] = serde_json::json!(false);
    explicit_true["represented_by_checkpoint"] = serde_json::json!(true);
    let mut true_view = ContextView::new();
    true_view.load_response(&body(serde_json::json!([explicit_true]))).unwrap();
    assert!(true_view.items()[0].represented_by_checkpoint);
}
