use opencode_rk_contracts::opencode_event::OpenCodeEvent;
use serde_json::{json, Value};

fn event_with_location(location: Value) -> Value {
    json!({
        "id": "evt_location",
        "type": "server.connected",
        "data": {},
        "location": location
    })
}

#[test]
fn int_008_location_t01_directory_only_roundtrips_exactly() {
    let wire = event_with_location(json!({
        "directory": "relative/../workspace"
    }));

    let event: OpenCodeEvent<Value> =
        serde_json::from_value(wire.clone()).expect("directory-only Location.Ref should decode");
    let encoded = serde_json::to_value(&event).expect("current event should encode");

    assert_eq!(encoded, wire);
}

#[test]
fn int_008_location_t02_workspace_id_with_wrk_prefix_roundtrips_without_stronger_rule() {
    let wire = event_with_location(json!({
        "directory": "workspace",
        "workspaceID": "wrk"
    }));

    let event: OpenCodeEvent<Value> = serde_json::from_value(wire.clone())
        .expect("bare wrk prefix satisfies the pinned WorkspaceID contract");
    let encoded = serde_json::to_value(&event).expect("current event should encode");

    assert_eq!(encoded, wire);
}

#[test]
fn int_008_location_t03_rejects_null_scalar_array_and_empty_object_locations() {
    for location in [json!(null), json!("workspace"), json!([]), json!({})] {
        let decoded = serde_json::from_value::<OpenCodeEvent<Value>>(event_with_location(location));
        assert!(
            decoded.is_err(),
            "Location.Ref must be an object with directory"
        );
    }
}

#[test]
fn int_008_location_t04_rejects_non_string_directory() {
    for directory in [json!(null), json!(7), json!(true), json!([]), json!({})] {
        let decoded = serde_json::from_value::<OpenCodeEvent<Value>>(event_with_location(json!({
            "directory": directory
        })));
        assert!(decoded.is_err(), "Location.Ref directory must be a string");
    }
}

#[test]
fn int_008_location_t05_rejects_invalid_workspace_id_values() {
    for workspace_id in [
        json!(null),
        json!(7),
        json!(true),
        json!([]),
        json!({}),
        json!("workspace"),
    ] {
        let decoded = serde_json::from_value::<OpenCodeEvent<Value>>(event_with_location(json!({
            "directory": "workspace",
            "workspaceID": workspace_id
        })));
        assert!(
            decoded.is_err(),
            "present workspaceID must be a non-null string starting with wrk"
        );
    }
}
