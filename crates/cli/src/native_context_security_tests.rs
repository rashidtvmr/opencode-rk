use super::native_context::ContextView;
use serde_json::{json, Value};

fn snapshot(item: Value) -> String {
    json!({"scope":"session","items":[item],"next_before":null,
        "projection":{"status":"available"}}).to_string()
}

fn item() -> Value {
    json!({"id":"00000000-0000-0000-0000-000000000001","kind":"message",
        "role":"user","source":"durable_history","included":true,
        "selectable":true,"content":"fixture","bytes":7,"estimated_tokens":2})
}

#[test]
fn malformed_selection_authority_cannot_expose_mandatory_or_compacted_controls() {
    let mut view = ContextView::new();
    view.load_response(&snapshot(item())).unwrap();
    let original=view.render(120,20);
    let mut cases=Vec::new();
    let mut system=item();system["role"]=json!("system");cases.push(system);
    let mut memory=item();memory["id"]=json!("memory:/workspace/AGENTS.md");
    memory["kind"]=json!("memory");memory["role"]=json!("system");cases.push(memory);
    let mut compacted=item();compacted["represented_by_checkpoint"]=json!(true);cases.push(compacted);
    let mut reason=item();reason["reason"]=json!("checkpoint_is_indivisible");cases.push(reason);
    let mut mandatory=item();mandatory["mandatory"]=json!(true);cases.push(mandatory);
    for candidate in cases {
        assert!(view.load_response(&snapshot(candidate)).is_err(),"invalid selection authority must be rejected");
        assert_eq!(view.render(120,20),original,"malformed refresh must preserve last loaded state");
    }
}

#[test]
fn tool_record_payload_has_an_independent_four_kibibyte_preview_limit() {
    let mut candidate=item();candidate["kind"]=json!("tool_pair");candidate["role"]=json!("tool");
    candidate["records"]=json!([
        {"kind":"call","name":"read","call_id":"fixture","content":"x".repeat(4096)},
        {"kind":"output","name":"read","call_id":"fixture","content":"ok"}]);
    let mut view=ContextView::new();
    view.load_response(&snapshot(candidate.clone())).unwrap();
    let original=view.render(100,12);
    candidate["records"][0]["content"]=json!("x".repeat(4097));
    assert!(view.load_response(&snapshot(candidate)).is_err());
    assert_eq!(view.render(100,12),original);
}
