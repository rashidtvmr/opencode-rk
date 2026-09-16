//! Bounded persisted-versus-ephemeral part-event split (SYNC-002 slice).
//!
//! Pure caller-owned classification. No I/O, no clock, no threads.

#![forbid(unsafe_code)]

/// Maximum bytes per stored part or ephemeral delta.
pub const MAX_DELTA_BYTES: usize = 65_536;
/// Alias for the stored-part byte bound.
pub const MAX_PART_BYTES: usize = MAX_DELTA_BYTES;

/// Closed part-kind enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Text,
    Reasoning,
    File,
    Tool,
    Compaction,
}

/// Durability classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    Durable,
    Ephemeral,
}

/// Part-event failures; never carry part content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartError {
    UnknownKind,
    InvalidInput,
    TooLarge,
}

impl std::fmt::Display for PartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKind => f.write_str("unknown part kind"),
            Self::InvalidInput => f.write_str("invalid part input"),
            Self::TooLarge => f.write_str("part exceeds its byte bound"),
        }
    }
}

impl std::error::Error for PartError {}

/// Durable part update supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartUpdate {
    pub part_id: String,
    pub kind: PartKind,
    pub bytes: Vec<u8>,
    pub compacted: bool,
}

/// Stored durable part.
#[derive(Clone, PartialEq, Eq)]
pub struct StoredPart {
    pub part_id: String,
    pub kind: PartKind,
    pub bytes: Vec<u8>,
    pub compacted: bool,
}

impl std::fmt::Debug for StoredPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredPart")
            .field("part_id", &self.part_id)
            .field("kind", &self.kind)
            .field("len", &self.bytes.len())
            .field("compacted", &self.compacted)
            .finish()
    }
}

/// Ephemeral streaming delta; droppable at any time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EphemeralDelta {
    pub part_id: String,
    pub delta_bytes: Vec<u8>,
}

/// Part event: durable update/remove or ephemeral delta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartEvent {
    PersistedUpdate(PartUpdate),
    PersistedRemove { part_id: String },
    EphemeralDelta(EphemeralDelta),
}

impl PartEvent {
    /// Decode a wire kind string without defaulting.
    pub fn decode_kind(kind: &str) -> Result<PartKind, PartError> {
        match kind {
            "text" => Ok(PartKind::Text),
            "reasoning" => Ok(PartKind::Reasoning),
            "file" => Ok(PartKind::File),
            "tool" => Ok(PartKind::Tool),
            "compaction" => Ok(PartKind::Compaction),
            _ => Err(PartError::UnknownKind),
        }
    }
}

/// Classify durability by type.
#[must_use]
pub fn classify(event: &PartEvent) -> Durability {
    match event {
        PartEvent::PersistedUpdate(_) | PartEvent::PersistedRemove { .. } => Durability::Durable,
        PartEvent::EphemeralDelta(_) => Durability::Ephemeral,
    }
}

fn valid_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Upsert a durable part by id.
pub fn apply_update(parts: &mut Vec<StoredPart>, update: PartUpdate) -> Result<(), PartError> {
    if !valid_id(&update.part_id) {
        return Err(PartError::InvalidInput);
    }
    if update.bytes.len() > MAX_PART_BYTES {
        return Err(PartError::TooLarge);
    }
    match parts.iter_mut().find(|p| p.part_id == update.part_id) {
        Some(slot) => {
            slot.kind = update.kind;
            slot.bytes = update.bytes;
            slot.compacted = update.compacted;
        }
        None => parts.push(StoredPart {
            part_id: update.part_id,
            kind: update.kind,
            bytes: update.bytes,
            compacted: update.compacted,
        }),
    }
    Ok(())
}

/// Remove one durable part; unknown id is harmless.
pub fn apply_remove(parts: &mut Vec<StoredPart>, part_id: &str) -> bool {
    match parts.iter().position(|p| p.part_id == part_id) {
        Some(i) => {
            parts.remove(i);
            true
        }
        None => false,
    }
}

/// Buffer an ephemeral delta; never touches stored parts.
pub fn apply_delta(
    deltas: &mut Vec<EphemeralDelta>,
    delta: EphemeralDelta,
) -> Result<(), PartError> {
    if !valid_id(&delta.part_id) {
        return Err(PartError::InvalidInput);
    }
    if delta.delta_bytes.len() > MAX_DELTA_BYTES {
        return Err(PartError::TooLarge);
    }
    deltas.push(delta);
    Ok(())
}

/// Project non-compacted parts in stored order without mutating input.
#[must_use]
pub fn filter_compacted_for_provider(parts: &[StoredPart]) -> Vec<&StoredPart> {
    parts.iter().filter(|p| !p.compacted).collect()
}
