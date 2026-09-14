//! OpenCode current/legacy event compatibility contract.
//!
//! INT-008 keeps this boundary pure: wire validation and compatibility
//! projection only, with no event runtime, transport, persistence, or bus.

use std::collections::BTreeMap;

use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OpenCodeEventId(pub String);

impl<'de> Deserialize<'de> for OpenCodeEventId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.starts_with("evt_") {
            Ok(Self(value))
        } else {
            Err(de::Error::custom("OpenCode event id must start with evt_"))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DurableEventMeta {
    #[serde(rename = "aggregateID")]
    pub aggregate_id: String,
    pub seq: i64,
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenCodeEvent<T> {
    pub id: OpenCodeEventId,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable: Option<DurableEventMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct LegacyEventRef<'a, T> {
    pub id: &'a OpenCodeEventId,
    #[serde(rename = "type")]
    pub event_type: &'a str,
    pub properties: &'a T,
}

#[derive(Debug, Serialize)]
pub struct LegacySyncEventRef<'a, T> {
    pub id: &'a OpenCodeEventId,
    #[serde(rename = "type")]
    pub event_type: String,
    pub seq: i64,
    #[serde(rename = "aggregateID")]
    pub aggregate_id: &'a str,
    pub data: &'a T,
}

#[derive(Debug, Serialize)]
pub struct LegacySyncRef<'a, T> {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    #[serde(rename = "syncEvent")]
    pub sync_event: LegacySyncEventRef<'a, T>,
}

#[derive(Debug)]
pub struct LegacyProjection<'a, T> {
    pub primary: LegacyEventRef<'a, T>,
    pub sync: Option<LegacySyncRef<'a, T>>,
}

#[must_use]
pub fn versioned_type(event_type: &str, version: i64) -> String {
    format!("{event_type}.{version}")
}

#[must_use]
pub fn project_legacy<T>(event: &OpenCodeEvent<T>) -> LegacyProjection<'_, T> {
    let sync = event.durable.as_ref().map(|durable| LegacySyncRef {
        event_type: "sync",
        sync_event: LegacySyncEventRef {
            id: &event.id,
            event_type: versioned_type(&event.event_type, durable.version),
            seq: durable.seq,
            aggregate_id: &durable.aggregate_id,
            data: &event.data,
        },
    });

    LegacyProjection {
        primary: LegacyEventRef {
            id: &event.id,
            event_type: &event.event_type,
            properties: &event.data,
        },
        sync,
    }
}
