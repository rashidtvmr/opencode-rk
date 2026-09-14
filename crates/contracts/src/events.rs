//! Typed domain event envelope and event schema.
use std::fmt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{AgentId, MessageRole, SessionId};
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub String);
impl EventId {
    #[must_use] pub fn new() -> Self { Self(Uuid::new_v4().to_string()) }
    #[must_use] pub fn as_str(&self) -> &str { &self.0 }
}
impl Default for EventId { fn default() -> Self { Self::new() } }
impl fmt::Display for EventId { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) } }
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource { System, Session(SessionId), Agent(AgentId), User }
impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::System => write!(f, "system"), Self::User => write!(f, "user"), Self::Session(id) => write!(f, "session:{id}"), Self::Agent(id) => write!(f, "agent:{id}") }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventEnvelope<T> { pub id: EventId, pub timestamp: DateTime<Utc>, pub source: EventSource, pub payload: T }
impl<T> EventEnvelope<T> {
    #[must_use] pub fn new(source: EventSource, payload: T) -> Self { Self { id: EventId::new(), timestamp: Utc::now(), source, payload } }
    #[must_use] pub const fn with_timestamp(id: EventId, timestamp: DateTime<Utc>, source: EventSource, payload: T) -> Self { Self { id, timestamp, source, payload } }
}
impl<T: PartialEq> PartialEq for EventEnvelope<T> { fn eq(&self, other: &Self) -> bool { self.id == other.id && self.timestamp == other.timestamp && self.source == other.source && self.payload == other.payload } }
impl<T: Eq> PartialOrd for EventEnvelope<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl<T: Eq> Ord for EventEnvelope<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.timestamp.cmp(&other.timestamp).then_with(|| self.id.cmp(&other.id)) }
}
impl<T: Eq + PartialEq> Eq for EventEnvelope<T> {}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainEvent { Session(SessionEvent), Message(MessageEvent), Tool(ToolEvent), Permission(PermissionEvent) }
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEvent { Created { id: SessionId, title: String }, Renamed { id: SessionId, title: String }, Archived(SessionId) }
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageEvent { Appended { session: SessionId, seq: u64, role: MessageRole }, Compacted { session: SessionId, before: u64, after: u64 } }
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEvent { Invoked { name: String, session: SessionId }, Completed { name: String, duration_ms: u64, success: bool } }
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionEvent { Requested { intent: String }, Granted { intent: String }, Denied { intent: String } }
#[cfg(test)] mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test] fn envelope_serde_roundtrip() {
        let env = EventEnvelope::new(EventSource::System, DomainEvent::Session(SessionEvent::Created { id: SessionId::new(), title: "t".into() }));
        let json = serde_json::to_string(&env).unwrap(); let back: EventEnvelope<DomainEvent> = serde_json::from_str(&json).unwrap(); assert_eq!(env.id, back.id); assert_eq!(env.timestamp, back.timestamp); assert_eq!(env.source, back.source); assert_eq!(env.payload, back.payload);
    }
    #[test] fn domain_event_variants() {
        let sid = SessionId::new();
        let cases: Vec<DomainEvent> = vec![
            DomainEvent::Session(SessionEvent::Created { id: sid, title: "a".into() }),
            DomainEvent::Session(SessionEvent::Renamed { id: sid, title: "b".into() }),
            DomainEvent::Session(SessionEvent::Archived(sid)),
            DomainEvent::Message(MessageEvent::Appended { session: sid, seq: 1, role: MessageRole::User }),
            DomainEvent::Message(MessageEvent::Compacted { session: sid, before: 10, after: 3 }),
            DomainEvent::Tool(ToolEvent::Invoked { name: "read".into(), session: sid }),
            DomainEvent::Tool(ToolEvent::Completed { name: "read".into(), duration_ms: 5, success: true }),
            DomainEvent::Permission(PermissionEvent::Requested { intent: "write".into() }),
            DomainEvent::Permission(PermissionEvent::Granted { intent: "write".into() }),
            DomainEvent::Permission(PermissionEvent::Denied { intent: "write".into() }),
        ];
        assert_eq!(cases.len(), 10);
        for ev in &cases { let json = serde_json::to_string(ev).unwrap(); let back: DomainEvent = serde_json::from_str(&json).unwrap(); assert_eq!(ev, &back); }
        assert!(matches!(cases[0], DomainEvent::Session(SessionEvent::Created { .. })));
        assert!(matches!(cases[1], DomainEvent::Session(SessionEvent::Renamed { .. })));
        assert!(matches!(cases[2], DomainEvent::Session(SessionEvent::Archived(_))));
        assert!(matches!(cases[3], DomainEvent::Message(MessageEvent::Appended { .. })));
        assert!(matches!(cases[4], DomainEvent::Message(MessageEvent::Compacted { .. })));
        assert!(matches!(cases[5], DomainEvent::Tool(ToolEvent::Invoked { .. })));
        assert!(matches!(cases[6], DomainEvent::Tool(ToolEvent::Completed { .. })));
        assert!(matches!(cases[7], DomainEvent::Permission(PermissionEvent::Requested { .. })));
        assert!(matches!(cases[8], DomainEvent::Permission(PermissionEvent::Granted { .. })));
        assert!(matches!(cases[9], DomainEvent::Permission(PermissionEvent::Denied { .. })));
    }
    #[test] fn event_id_uniqueness() {
        let mut ids: Vec<EventId> = (0..1000).map(|_| EventId::new()).collect(); let len = ids.len(); ids.sort(); ids.dedup(); assert_eq!(ids.len(), len);
    }
    #[test] fn source_display() {
        let sid = SessionId::new(); let aid = AgentId::new();
        assert_eq!(EventSource::System.to_string(), "system"); assert_eq!(EventSource::User.to_string(), "user");
        assert_eq!(EventSource::Session(sid).to_string(), format!("session:{sid}")); assert_eq!(EventSource::Agent(aid).to_string(), format!("agent:{aid}"));
    }
    #[test] fn event_ordering() {
        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(); let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 1).unwrap(); let t2 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap();
        let mk = |ts| EventEnvelope::with_timestamp(EventId::new(), ts, EventSource::System, 0u8);
        let mut evs = vec![mk(t2), mk(t0), mk(t1)]; evs.sort(); assert_eq!(evs[0].timestamp, t0); assert_eq!(evs[1].timestamp, t1); assert_eq!(evs[2].timestamp, t2);
    }
}
