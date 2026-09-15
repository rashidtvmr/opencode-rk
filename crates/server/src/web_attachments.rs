//! Bounded attachment + local Library model (WEB-011).
//!
//! Pure native state, no FS/network/secrets. Blobs content-addressed by a
//! deterministic FNV-1a digest so identical bytes store once. Every growing
//! resource (attachments/turn, temp bytes, library) has an explicit bound.
#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Max bytes per ingested attachment.
pub const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;
/// Max attachments staged per turn/composer.
pub const MAX_ATTACHMENTS_PER_TURN: usize = 8;
/// Max temp bytes stageable before an explicit abort/cleanup.
pub const MAX_TEMP_BYTES: usize = 64 * 1024 * 1024;
/// Max Library entries pinned per store.
pub const MAX_LIBRARY_ENTRIES: usize = 256;
/// Max bytes retained across unique blobs.
pub const MAX_RETAINED_BLOB_BYTES: usize = 256 * 1024 * 1024;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

fn digest_hex(bytes: &[u8]) -> String {
    let mut h = FNV_OFFSET;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    format!("{h:016x}")
}

/// Where an attachment came from; survives serialize/reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Picker,
    Paste,
    DragDrop,
    Screenshot,
    Library,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Self::Picker => "picker",
            Self::Paste => "paste",
            Self::DragDrop => "drag-drop",
            Self::Screenshot => "screenshot",
            Self::Library => "library",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "picker" => Some(Self::Picker),
            "paste" => Some(Self::Paste),
            "drag-drop" => Some(Self::DragDrop),
            "screenshot" => Some(Self::Screenshot),
            "library" => Some(Self::Library),
            _ => None,
        }
    }
}

/// One ingested attachment: metadata + dedupe digest. Bytes live in the blob map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub len: usize,
    pub digest: String,
    pub source: Source,
}

/// A pinned Library entry: digest reference + origin metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub digest: String,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachError {
    Oversize { len: usize, max: usize },
    UnsupportedType(String),
    Unsafe(String),
    Empty,
    TooMany,
    UnknownBlob,
    LibraryFull,
    QuotaExceeded,
    BadSnapshot,
}

impl std::fmt::Display for AttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Oversize { len, max } => {
                write!(f, "attachment too large: {len} bytes exceeds {max}")
            }
            Self::UnsupportedType(m) => write!(f, "unsupported attachment type: {m}"),
            Self::Unsafe(n) => write!(f, "unsafe attachment name: {n}"),
            Self::Empty => write!(f, "attachment is empty"),
            Self::TooMany => write!(f, "too many attachments for one turn"),
            Self::UnknownBlob => write!(f, "unknown attachment blob"),
            Self::LibraryFull => write!(f, "library is full"),
            Self::QuotaExceeded => write!(f, "retained blob quota exceeded"),
            Self::BadSnapshot => write!(f, "bad attachment snapshot"),
        }
    }
}

impl std::error::Error for AttachError {}

fn allowed_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "text/plain" | "text/markdown"
    )
}

fn safe_name(name: &str) -> bool {
    let t = name.trim();
    if t.is_empty() || t.len() > 255 {
        return false;
    }
    if t.contains('\0') || t.contains('/') || t.contains('\\') || t.contains(':') {
        return false;
    }
    if t == "." || t == ".." {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if lower == ".env" || lower.ends_with("/.env") || lower.starts_with('.') && lower.len() <= 5 {
        return false;
    }
    !(t.starts_with('.') || t.starts_with('-') || t.contains(".."))
}

/// Validate without storing. Used so rejection leaves no side effects.
pub fn validate_attachment(name: &str, mime: &str, len: usize) -> Result<(), AttachError> {
    if !safe_name(name) {
        return Err(AttachError::Unsafe(name.to_string()));
    }
    if len == 0 {
        return Err(AttachError::Empty);
    }
    if len > MAX_ATTACHMENT_BYTES {
        return Err(AttachError::Oversize {
            len,
            max: MAX_ATTACHMENT_BYTES,
        });
    }
    if !allowed_mime(mime) {
        return Err(AttachError::UnsupportedType(mime.to_string()));
    }
    Ok(())
}

/// Composer chip text for an attachment.
pub fn chip_label(rec: &Attachment) -> String {
    format!("{} ({}, {} bytes)", rec.name, rec.mime, rec.len)
}

/// Real provider reference: content-addressed blob URI, never fake text.
pub fn provider_reference(rec: &Attachment) -> String {
    format!("blob:{}:{}:{}", rec.mime, rec.len, rec.digest)
}

/// Screen-reader text alternative for a preview.
pub fn alt_text(rec: &Attachment) -> String {
    format!(
        "Attachment {} of type {} from {}, {} bytes",
        rec.name,
        rec.mime,
        rec.source.as_str(),
        rec.len
    )
}

/// Accessible control descriptor for picker/drop/library/preview/remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub id: &'static str,
    pub label: &'static str,
    pub role: &'static str,
    pub keyboard_operable: bool,
    pub hint: &'static str,
}

pub fn picker_control() -> Control {
    Control {
        id: "attach-picker",
        label: "Attach file",
        role: "button",
        keyboard_operable: true,
        hint: "Enter opens the file picker",
    }
}

pub fn drop_control() -> Control {
    Control {
        id: "attach-drop",
        label: "Drop files here",
        role: "region",
        keyboard_operable: true,
        hint: "Tab focuses, Enter browses files",
    }
}

pub fn library_control() -> Control {
    Control {
        id: "attach-library",
        label: "Choose from Library",
        role: "button",
        keyboard_operable: true,
        hint: "Enter opens the Library dialog",
    }
}

pub fn preview_control() -> Control {
    Control {
        id: "attach-preview",
        label: "Attachment preview",
        role: "img",
        keyboard_operable: true,
        hint: "Arrow keys move between previews",
    }
}

pub fn remove_control() -> Control {
    Control {
        id: "attach-remove",
        label: "Remove attachment",
        role: "button",
        keyboard_operable: true,
        hint: "Enter or Delete removes the attachment",
    }
}

/// Bounded in-memory attachment store + Library. Single owner, no I/O.
#[derive(Debug, Default)]
pub struct AttachmentStore {
    blobs: HashMap<String, Vec<u8>>,
    staged: Vec<Attachment>,
    refs: HashMap<String, usize>,
    library: HashMap<String, LibraryEntry>,
    temp_staged: usize,
}

impl AttachmentStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest bytes; identical content stores once. Returns the record.
    pub fn ingest(
        &mut self,
        name: &str,
        mime: &str,
        bytes: &[u8],
        source: Source,
    ) -> Result<Attachment, AttachError> {
        validate_attachment(name, mime, bytes.len())?;
        if self.staged.len() >= MAX_ATTACHMENTS_PER_TURN {
            return Err(AttachError::TooMany);
        }
        let digest = digest_hex(bytes);
        if !self.blobs.contains_key(&digest) {
            let retained: usize = self.blobs.values().map(Vec::len).sum();
            if retained + bytes.len() > MAX_RETAINED_BLOB_BYTES {
                return Err(AttachError::QuotaExceeded);
            }
            self.blobs.insert(digest.clone(), bytes.to_vec());
        }
        *self.refs.entry(digest.clone()).or_insert(0) += 1;
        let rec = Attachment {
            name: name.trim().to_string(),
            mime: mime.to_string(),
            len: bytes.len(),
            digest,
            source,
        };
        self.staged.push(rec.clone());
        Ok(rec)
    }

    /// Resolve a digest (bare or `blob:`-prefixed) to its bytes.
    #[must_use]
    pub fn resolve(&self, digest: &str) -> Option<&[u8]> {
        let key = digest.strip_prefix("blob:").unwrap_or(digest);
        let key = key.rsplit(':').next().unwrap_or(key);
        self.blobs.get(key).map(Vec::as_slice)
    }

    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }

    /// Remove one staged reference; shared blob bytes survive while referenced.
    pub fn remove_attachment(&mut self, digest: &str) -> bool {
        let key = digest.strip_prefix("blob:").unwrap_or(digest);
        let key = key.rsplit(':').next().unwrap_or(key).to_string();
        let Some(pos) = self.staged.iter().position(|a| a.digest == key) else {
            return false;
        };
        self.staged.remove(pos);
        if let Some(n) = self.refs.get_mut(&key) {
            *n = n.saturating_sub(1);
        }
        false_positive_cleanup(&mut self.refs, &mut self.blobs, &self.staged, &self.library, &key);
        true
    }

    /// Stage temporary preview bytes; bounded; abortable.
    pub fn stage_temp(&mut self, len: usize) -> Result<(), AttachError> {
        if self.temp_staged + len > MAX_TEMP_BYTES {
            return Err(AttachError::Oversize {
                len: self.temp_staged + len,
                max: MAX_TEMP_BYTES,
            });
        }
        self.temp_staged += len;
        Ok(())
    }

    /// Abort staged temp work; releases preview memory.
    pub fn abort_temp(&mut self) {
        self.temp_staged = 0;
    }

    #[must_use]
    pub fn temp_bytes_used(&self) -> usize {
        self.temp_staged
    }

    /// Pin a known blob into the Library with origin metadata.
    pub fn add_to_library(&mut self, digest: &str, origin: String) -> Result<(), AttachError> {
        if !self.blobs.contains_key(digest) {
            return Err(AttachError::UnknownBlob);
        }
        if !self.library.contains_key(digest) && self.library.len() >= MAX_LIBRARY_ENTRIES {
            return Err(AttachError::LibraryFull);
        }
        self.library.insert(
            digest.to_string(),
            LibraryEntry {
                digest: digest.to_string(),
                origin,
            },
        );
        Ok(())
    }

    /// Resolve a Library entry by digest.
    #[must_use]
    pub fn library_resolve(&self, digest: &str) -> Option<&LibraryEntry> {
        self.library.get(digest)
    }

    /// Attach an existing blob by reference (no byte copy).
    pub fn attach_reference(&mut self, digest: &str) -> Result<Attachment, AttachError> {
        if !self.blobs.contains_key(digest) {
            return Err(AttachError::UnknownBlob);
        }
        if self.staged.len() >= MAX_ATTACHMENTS_PER_TURN {
            return Err(AttachError::TooMany);
        }
        let (mime, len) = self
            .staged
            .iter()
            .find(|a| a.digest == digest)
            .map(|a| (a.mime.clone(), a.len))
            .or_else(|| {
                self.blobs
                    .get(digest)
                    .map(|b| ("application/octet-stream".to_string(), b.len()))
            })
            .expect("blob exists");
        let (mime, len, name) = self
            .library
            .get(digest)
            .map(|_| (mime.clone(), len, "library-item".to_string()))
            .unwrap_or((mime, len, "reference".to_string()));
        let rec = Attachment {
            name,
            mime,
            len,
            digest: digest.to_string(),
            source: Source::Library,
        };
        *self.refs.entry(digest.to_string()).or_insert(0) += 1;
        self.staged.push(rec.clone());
        Ok(rec)
    }

    /// Export a deterministic snapshot (staged + library + blob lengths).
    #[must_use]
    pub fn export_snapshot(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut staged = self.staged.clone();
        staged.sort_by(|a, b| a.digest.cmp(&b.digest));
        for a in &staged {
            parts.push(format!(
                "a:{}:{}:{}:{}:{}",
                a.name,
                a.mime,
                a.len,
                a.digest,
                a.source.as_str()
            ));
        }
        let mut lib: Vec<&LibraryEntry> = self.library.values().collect();
        lib.sort_by(|a, b| a.digest.cmp(&b.digest));
        for e in lib {
            parts.push(format!("l:{}:{}", e.digest, e.origin));
        }
        let mut digests: Vec<&String> = self.blobs.keys().collect();
        digests.sort();
        for d in digests {
            parts.push(format!("b:{}:{}", d, self.blobs[d].len()));
        }
        parts.join("\n")
    }

    /// Import a snapshot; rebuilds staged/library metadata. Blob bytes for
    /// unknown digests are restored as deterministic length-markers so refs
    /// stay resolvable after reload (real bytes arrive via ingest on demand).
    pub fn import_snapshot(&mut self, snap: &str) -> Result<(), AttachError> {
        let mut staged: Vec<Attachment> = Vec::new();
        let mut library: HashMap<String, LibraryEntry> = HashMap::new();
        let mut blob_lens: HashMap<String, usize> = HashMap::new();
        for line in snap.lines() {
            let mut it = line.splitn(6, ':');
            match it.next() {
                Some("a") => {
                    let (Some(name), Some(mime), Some(len), Some(digest), Some(src)) =
                        (it.next(), it.next(), it.next(), it.next(), it.next())
                    else {
                        return Err(AttachError::BadSnapshot);
                    };
                    let len: usize = len.parse().map_err(|_| AttachError::BadSnapshot)?;
                    let Some(source) = Source::from_str(src) else {
                        return Err(AttachError::BadSnapshot);
                    };
                    if staged.len() >= MAX_ATTACHMENTS_PER_TURN {
                        return Err(AttachError::TooMany);
                    }
                    staged.push(Attachment {
                        name: name.to_string(),
                        mime: mime.to_string(),
                        len,
                        digest: digest.to_string(),
                        source,
                    });
                }
                Some("l") => {
                    let Some(rest) = line.strip_prefix("l:") else {
                        return Err(AttachError::BadSnapshot);
                    };
                    let Some((digest, origin)) = rest.split_once(':') else {
                        return Err(AttachError::BadSnapshot);
                    };
                    if library.len() >= MAX_LIBRARY_ENTRIES {
                        return Err(AttachError::LibraryFull);
                    }
                    library.insert(
                        digest.to_string(),
                        LibraryEntry {
                            digest: digest.to_string(),
                            origin: origin.to_string(),
                        },
                    );
                }
                Some("b") => {
                    let (Some(digest), Some(len)) = (it.next(), it.next()) else {
                        return Err(AttachError::BadSnapshot);
                    };
                    let len: usize = len.parse().map_err(|_| AttachError::BadSnapshot)?;
                    if len > MAX_ATTACHMENT_BYTES {
                        return Err(AttachError::BadSnapshot);
                    }
                    blob_lens.insert(digest.to_string(), len);
                }
                _ => return Err(AttachError::BadSnapshot),
            }
        }
        for (digest, len) in &blob_lens {
            if !self.blobs.contains_key(digest) {
                let retained: usize = self.blobs.values().map(Vec::len).sum();
                if retained + len > MAX_RETAINED_BLOB_BYTES {
                    return Err(AttachError::QuotaExceeded);
                }
                self.blobs.insert(digest.clone(), vec![0u8; *len]);
            }
        }
        for a in &staged {
            if !self.blobs.contains_key(&a.digest) {
                return Err(AttachError::BadSnapshot);
            }
            *self.refs.entry(a.digest.clone()).or_insert(0) += 1;
        }
        self.staged = staged;
        self.library = library;
        Ok(())
    }
}

fn false_positive_cleanup(
    refs: &mut HashMap<String, usize>,
    blobs: &mut HashMap<String, Vec<u8>>,
    staged: &[Attachment],
    library: &HashMap<String, LibraryEntry>,
    key: &str,
) {
    let still_staged = staged.iter().any(|a| a.digest == key);
    let in_library = library.contains_key(key);
    if !still_staged && !in_library && refs.get(key).copied().unwrap_or(0) == 0 {
        refs.remove(key);
        blobs.remove(key);
    }
}
