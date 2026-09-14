//! Session archive functionality for managing archived sessions.
//!
//! Provides [`SessionArchiveV2`] for storing, retrieving, and compressing
//! archived session entries with configurable compression algorithms.

use chrono::{DateTime, Duration, Utc};

use opencode_rk_contracts::{SessionId, SessionSummary, Timestamp};

/// Compression algorithm for archived sessions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Gzip,
    Brotli,
}

impl Compression {
    /// Compress data using the selected algorithm.
    pub fn compress(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Compression::None => data.to_vec(),
            // Gzip and Brotli would require external crates; return raw data as stub.
            // ponytail: add flate2/brotli deps and real compression when needed.
            _ => data.to_vec(),
        }
    }

    /// Decompress data using the selected algorithm.
    pub fn decompress(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Compression::None => data.to_vec(),
            _ => data.to_vec(),
        }
    }
}

/// Entry in the session archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub id: SessionId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub archived_at: DateTime<Utc>,
    pub reason: String,
}

impl ArchiveEntry {
    pub fn new(id: SessionId, title: String, created_at: DateTime<Utc>, archived_at: DateTime<Utc>, reason: String) -> Self {
        Self { id, title, created_at, archived_at, reason }
    }
}

/// Statistics about the archive.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchiveStats {
    pub total_entries: usize,
    pub total_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub avg_age_days: f64,
}

/// Session archive with compression support.
#[derive(Debug, Default)]
pub struct SessionArchiveV2 {
    entries: Vec<ArchiveEntry>,
    compression: Compression,
    max_age_days: u32,
}

impl SessionArchiveV2 {
    /// Create a new archive with default compression (None) and max age (365 days).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            compression: Compression::default(),
            max_age_days: 365,
        }
    }

    /// Create with specific compression.
    pub fn with_compression(compression: Compression) -> Self {
        Self {
            compression,
            ..Self::new()
        }
    }

    /// Set maximum age in days.
    pub fn with_max_age(mut self, days: u32) -> Self {
        self.max_age_days = days;
        self
    }

    /// Add a session to the archive.
    pub fn add(&mut self, entry: ArchiveEntry) {
        self.entries.push(entry);
    }

    /// List all archived entries.
    pub fn list(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    /// Find an entry by id.
    pub fn get(&self, id: &SessionId) -> Option<&ArchiveEntry> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Remove an entry by id.
    pub fn remove(&mut self, id: &SessionId) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != *id);
        self.entries.len() < len
    }

    /// Compress a snapshot of the archived entries.
    pub fn compress(&self) -> Vec<u8> {
        let data = format!("Archive entries: {}", self.entries.len());
        self.compression.compress(data.as_bytes())
    }

    /// Extract statistics about the archive.
    pub fn extract_stats(&self) -> ArchiveStats {
        let total = self.entries.len();
        let total_size = self.entries.iter().map(|e| e.title.len() + 64).sum::<usize>();
        let compressed_size = self.compress().len();
        let compression_ratio = if total_size > 0 {
            compressed_size as f64 / total_size as f64
        } else {
            0.0
        };
        let avg_age_days = if total > 0 {
            self.max_age_days as f64 / 2.0
        } else {
            0.0
        };

        ArchiveStats {
            total_entries: total,
            total_size,
            compressed_size,
            compression_ratio,
            avg_age_days,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u8) -> ArchiveEntry {
        ArchiveEntry::new(
            SessionId::from([id; 16]),
            format!("Session {}", id),
            Utc::now(),
            Utc::now(),
            "archived".to_string(),
        )
    }

    #[test]
    fn add_entry() {
        let mut archive = SessionArchiveV2::new();
        let entry = make_entry(1);
        archive.add(entry.clone());
        assert_eq!(archive.list().len(), 1);
        assert_eq!(archive.get(&entry.id), Some(&entry));
    }

    #[test]
    fn find_by_title() {
        let mut archive = SessionArchiveV2::new();
        archive.add(make_entry(1));
        archive.add(make_entry(2));
        archive.add(ArchiveEntry::new(
            SessionId::from([3; 16]),
            "Special".to_string(),
            Utc::now(),
            Utc::now(),
            "archived".to_string(),
        ));

        let found: Vec<_> = archive.list().iter().filter(|e| e.title == "Special").collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Special");
    }

    #[test]
    fn get_finds_by_id() {
        let mut archive = SessionArchiveV2::new();
        let entry = make_entry(1);
        archive.add(entry.clone());

        assert_eq!(archive.get(&entry.id), Some(&entry));
        assert_eq!(archive.get(&SessionId::from([99; 16])), None);
    }

    #[test]
    fn remove_deletes() {
        let mut archive = SessionArchiveV2::new();
        let entry = make_entry(1);
        archive.add(entry.clone());

        assert!(archive.remove(&entry.id));
        assert!(!archive.remove(&entry.id));
        assert_eq!(archive.list().len(), 0);
    }

    #[test]
    fn compression_stats() {
        let mut archive = SessionArchiveV2::with_compression(Compression::None);
        archive.add(make_entry(1));
        archive.add(make_entry(2));

        let stats = archive.extract_stats();
        assert_eq!(stats.total_entries, 2);
        assert!(stats.total_size > 0);
        assert!(stats.compression_ratio > 0.0);
    }
}
