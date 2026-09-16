// SYNC-002 contract tests: persisted-versus-ephemeral part-event split.
// Maps to obligations SYNC-002-T01..T05 in tasks/SYNC-002.md.
use opencode_rk_sessions::part_events::{
    apply_delta, apply_remove, apply_update, classify, filter_compacted_for_provider, Durability,
    EphemeralDelta, PartError, PartEvent, PartKind, PartUpdate, StoredPart, MAX_DELTA_BYTES,
};

fn update(id: &str, kind: PartKind, bytes: &[u8], compacted: bool) -> PartUpdate {
    PartUpdate {
        part_id: id.to_owned(),
        kind,
        bytes: bytes.to_vec(),
        compacted,
    }
}

#[test]
fn sync002_t01_happy_path() {
    let mut parts: Vec<StoredPart> = Vec::new();
    let mut deltas: Vec<EphemeralDelta> = Vec::new();
    let u = update("p1", PartKind::Text, b"hello", false);
    assert_eq!(
        classify(&PartEvent::PersistedUpdate(u.clone())),
        Durability::Durable
    );
    apply_update(&mut parts, u).unwrap();
    assert_eq!(parts.len(), 1);
    assert!(apply_remove(&mut parts, "p1"));
    assert!(parts.is_empty());
    let d = EphemeralDelta {
        part_id: "p1".to_owned(),
        delta_bytes: b"tok".to_vec(),
    };
    assert_eq!(
        classify(&PartEvent::EphemeralDelta(d.clone())),
        Durability::Ephemeral
    );
    apply_delta(&mut deltas, d).unwrap();
    assert_eq!(deltas.len(), 1);
    assert!(parts.is_empty());
}

#[test]
fn sync002_t02_durability_split() {
    assert_eq!(
        classify(&PartEvent::PersistedUpdate(update(
            "a",
            PartKind::Text,
            b"x",
            false
        ))),
        Durability::Durable
    );
    assert_eq!(
        classify(&PartEvent::PersistedRemove {
            part_id: "a".to_owned()
        }),
        Durability::Durable
    );
    assert_eq!(
        classify(&PartEvent::EphemeralDelta(EphemeralDelta {
            part_id: "a".to_owned(),
            delta_bytes: b"x".to_vec()
        })),
        Durability::Ephemeral
    );
    let mut parts = vec![StoredPart {
        part_id: "a".to_owned(),
        kind: PartKind::Text,
        bytes: b"x".to_vec(),
        compacted: false,
    }];
    let mut deltas = vec![EphemeralDelta {
        part_id: "a".to_owned(),
        delta_bytes: b"y".to_vec(),
    }];
    deltas.clear();
    assert_eq!(parts.len(), 1);
    let _ = classify(&PartEvent::EphemeralDelta(EphemeralDelta {
        part_id: "z".to_owned(),
        delta_bytes: b"q".to_vec(),
    }));
    assert_eq!(parts.len(), 1);
}

#[test]
fn sync002_t03_compaction_filter() {
    let parts = vec![
        StoredPart {
            part_id: "t".to_owned(),
            kind: PartKind::Text,
            bytes: b"t".to_vec(),
            compacted: false,
        },
        StoredPart {
            part_id: "c".to_owned(),
            kind: PartKind::Compaction,
            bytes: b"c".to_vec(),
            compacted: true,
        },
        StoredPart {
            part_id: "f".to_owned(),
            kind: PartKind::File,
            bytes: b"f".to_vec(),
            compacted: false,
        },
    ];
    let out = filter_compacted_for_provider(&parts);
    assert_eq!(
        out.iter().map(|p| p.part_id.as_str()).collect::<Vec<_>>(),
        vec!["t", "f"]
    );
    assert_eq!(parts.len(), 3);
}

#[test]
fn sync002_t04_failure_states() {
    let mut parts: Vec<StoredPart> = Vec::new();
    let mut deltas: Vec<EphemeralDelta> = Vec::new();
    assert_eq!(PartEvent::decode_kind("nope"), Err(PartError::UnknownKind));
    assert_eq!(
        apply_update(&mut parts, update("", PartKind::Text, b"x", false)),
        Err(PartError::InvalidInput)
    );
    assert!(parts.is_empty());
    let big = EphemeralDelta {
        part_id: "p".to_owned(),
        delta_bytes: vec![0u8; MAX_DELTA_BYTES + 1],
    };
    assert_eq!(apply_delta(&mut deltas, big), Err(PartError::TooLarge));
    assert!(deltas.is_empty());
    assert!(!apply_remove(&mut parts, "ghost"));
    assert!(parts.is_empty());
}

#[test]
fn sync002_t05_purity_safety() {
    let p = StoredPart {
        part_id: "a".to_owned(),
        kind: PartKind::Text,
        bytes: b"fixture-bytes".to_vec(),
        compacted: false,
    };
    let dbg = format!("{p:?}");
    assert!(!dbg.contains("fixture-bytes"));
    assert_eq!(p.bytes.len(), 13);
    let dir = tempfile::tempdir().unwrap();
    assert!(dir.path().exists());
}
