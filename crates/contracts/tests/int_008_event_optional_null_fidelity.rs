use opencode_rk_contracts::opencode_event::OpenCodeEvent;
use serde_json::{json, Value};

fn base_event() -> Value {
    json!({
        "id": "evt_connected",
        "type": "server.connected",
        "data": {}
    })
}

#[test]
fn int_008_optional_null_t01_explicit_metadata_null_is_rejected() {
    let mut wire = base_event();
    wire["metadata"] = Value::Null;

    let decoded = serde_json::from_value::<OpenCodeEvent<Value>>(wire);

    assert!(
        decoded.is_err(),
        "metadata is optional by key presence; an explicitly present null is invalid"
    );
}

#[test]
fn int_008_optional_null_t02_explicit_durable_null_is_rejected() {
    let mut wire = base_event();
    wire["durable"] = Value::Null;

    let decoded = serde_json::from_value::<OpenCodeEvent<Value>>(wire);

    assert!(
        decoded.is_err(),
        "durable is optional by key presence; an explicitly present null is invalid"
    );
}

#[test]
fn int_008_optional_null_t03_absent_metadata_and_durable_remain_accepted_and_omitted() {
    let wire = base_event();

    let event: OpenCodeEvent<Value> =
        serde_json::from_value(wire.clone()).expect("absent optional fields should decode");
    let encoded = serde_json::to_value(event).expect("decoded event should re-encode");

    assert_eq!(encoded, wire);
    assert!(encoded.get("metadata").is_none());
    assert!(encoded.get("durable").is_none());
}

#[test]
fn int_008_optional_null_t04_metadata_object_may_contain_null_values_and_roundtrips() {
    let wire = json!({
        "id": "evt_connected",
        "type": "server.connected",
        "metadata": {
            "trace": null,
            "attempt": 2
        },
        "data": {}
    });

    let event: OpenCodeEvent<Value> = serde_json::from_value(wire.clone())
        .expect("null metadata values are valid unknown values");
    let encoded = serde_json::to_value(event).expect("metadata object should re-encode");

    assert_eq!(encoded, wire);
}

#[test]
fn int_008_optional_null_t05_valid_durable_object_roundtrips_unchanged() {
    let wire = json!({
        "id": "evt_model_switched",
        "type": "session.next.model.switched",
        "durable": {
            "aggregateID": "ses_test",
            "seq": 7,
            "version": 3
        },
        "data": {
            "providerID": "openai",
            "modelID": "gpt-test"
        }
    });

    let event: OpenCodeEvent<Value> =
        serde_json::from_value(wire.clone()).expect("valid durable metadata should decode");
    let encoded = serde_json::to_value(event).expect("durable event should re-encode");

    assert_eq!(encoded, wire);
}
