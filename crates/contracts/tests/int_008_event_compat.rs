use opencode_rk_contracts::opencode_event::{project_legacy, versioned_type, OpenCodeEvent};
use serde_json::{json, Value};

fn decode(value: Value) -> OpenCodeEvent<Value> {
    serde_json::from_value(value).expect("source-grounded current event fixture should decode")
}

#[test]
fn int_008_t01_current_wire_roundtrips_core_fields_and_omits_absent_optionals() {
    let wire = json!({
        "id": "evt_connected",
        "type": "server.connected",
        "data": { "ready": true }
    });

    let event = decode(wire.clone());
    let encoded = serde_json::to_value(&event).expect("current event should encode");

    assert_eq!(encoded, wire);
    assert!(encoded.get("metadata").is_none());
    assert!(encoded.get("durable").is_none());
    assert!(encoded.get("location").is_none());
}

#[test]
fn int_008_t02_durable_wire_preserves_metadata_and_formats_versioned_type_exactly() {
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

    let event = decode(wire.clone());
    let encoded = serde_json::to_value(&event).expect("durable current event should encode");

    assert_eq!(encoded, wire);
    assert_eq!(
        encoded.get("durable"),
        Some(&json!({
            "aggregateID": "ses_test",
            "seq": 7,
            "version": 3
        }))
    );
    assert_eq!(
        versioned_type("session.next.model.switched", 3),
        "session.next.model.switched.3"
    );
}

#[test]
fn int_008_t03_decode_rejects_missing_or_non_evt_id_without_inventing_a_stronger_rule() {
    let missing = serde_json::from_value::<OpenCodeEvent<Value>>(json!({
        "type": "server.connected",
        "data": {}
    }));
    assert!(missing.is_err(), "a current event must carry an id");

    let wrong_prefix = serde_json::from_value::<OpenCodeEvent<Value>>(json!({
        "id": "event_connected",
        "type": "server.connected",
        "data": {}
    }));
    assert!(
        wrong_prefix.is_err(),
        "the canonical event id schema requires the evt_ prefix"
    );

    let prefix_only = serde_json::from_value::<OpenCodeEvent<Value>>(json!({
        "id": "evt_",
        "type": "server.connected",
        "data": {}
    }));
    assert!(
        prefix_only.is_ok(),
        "the pinned schema requires only the evt_ prefix"
    );
}

#[test]
fn int_008_t04_legacy_primary_projection_is_exact_and_borrows_event_data() {
    let event = decode(json!({
        "id": "evt_connected",
        "type": "server.connected",
        "data": {
            "nested": { "value": 42 }
        }
    }));

    let projection = project_legacy(&event);

    assert_eq!(
        serde_json::to_value(&projection.primary).expect("legacy primary projection should encode"),
        json!({
            "id": "evt_connected",
            "type": "server.connected",
            "properties": {
                "nested": { "value": 42 }
            }
        })
    );
    assert!(
        std::ptr::eq(projection.primary.properties, &event.data),
        "legacy projection should borrow caller-owned event data"
    );
}

#[test]
fn int_008_t05_durable_projection_adds_exact_sync_payload_and_non_durable_adds_none() {
    let durable = decode(json!({
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
    }));

    let durable_projection = project_legacy(&durable);
    let sync = durable_projection
        .sync
        .as_ref()
        .expect("durable current event should add one sync payload");
    assert_eq!(
        serde_json::to_value(sync).expect("legacy sync projection should encode"),
        json!({
            "type": "sync",
            "syncEvent": {
                "id": "evt_model_switched",
                "type": "session.next.model.switched.3",
                "seq": 7,
                "aggregateID": "ses_test",
                "data": {
                    "providerID": "openai",
                    "modelID": "gpt-test"
                }
            }
        })
    );
    assert!(
        std::ptr::eq(sync.sync_event.data, &durable.data),
        "durable sync projection should borrow caller-owned event data"
    );

    let non_durable = decode(json!({
        "id": "evt_connected",
        "type": "server.connected",
        "data": {}
    }));
    assert!(
        project_legacy(&non_durable).sync.is_none(),
        "non-durable current events must not produce a sync payload"
    );
}
