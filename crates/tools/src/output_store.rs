//! Bounded output store: retains tool outputs per tool id with a byte cap.

use std::time::{SystemTime, UNIX_EPOCH};

/// A single tool execution result retained by the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    /// Tool instance id that produced this output.
    pub tool_id: String,
    /// Captured output text.
    pub output: String,
    /// Monotonic-ish millisecond timestamp of push (UNIX epoch millis).
    pub timestamp: u64,
    /// Whether the tool call succeeded.
    pub success: bool,
}

impl ToolOutput {
    /// Convenience constructor; timestamp is set from the system clock.
    pub fn new(tool_id: impl Into<String>, output: impl Into<String>, success: bool) -> Self {
        Self {
            tool_id: tool_id.into(),
            output: output.into(),
            timestamp: now_millis(),
            success,
        }
    }

    /// Bytes attributed to this entry for size accounting.
    fn size_bytes(&self) -> usize {
        self.tool_id.len() + self.output.len()
    }
}

/// Ring-like bounded store of tool outputs.
///
/// Entries are appended in push order; when `current_size` would exceed
/// `max_size`, the oldest entries are evicted until the budget fits again.
#[derive(Debug, Clone)]
pub struct OutputStore {
    entries: Vec<ToolOutput>,
    max_size: usize,
    current_size: usize,
}

impl OutputStore {
    /// Create an empty store with the given maximum retained byte budget.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Append an output, evicting oldest entries while over budget.
    pub fn push(&mut self, output: ToolOutput) {
        self.current_size += output.size_bytes();
        self.entries.push(output);
        self.evict();
    }

    /// All outputs for a tool id, in push order.
    pub fn get(&self, tool_id: &str) -> Vec<&ToolOutput> {
        self.entries
            .iter()
            .filter(|e| e.tool_id == tool_id)
            .collect()
    }

    /// The last `n` outputs overall, in push order.
    pub fn recent(&self, n: usize) -> Vec<&ToolOutput> {
        let skip = self.entries.len().saturating_sub(n);
        self.entries.iter().skip(skip).collect()
    }

    /// Remove all retained outputs and reset size accounting.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }

    /// (entry count, total retained bytes).
    pub fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.current_size)
    }

    /// Evict oldest entries until size is within budget.
    fn evict(&mut self) {
        while self.current_size > self.max_size && !self.entries.is_empty() {
            let removed = self.entries.remove(0);
            self.current_size -= removed.size_bytes();
        }
    }
}

/// Current UNIX epoch time in milliseconds.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_get() {
        let mut store = OutputStore::new(1024);
        store.push(ToolOutput::new("read", "alpha", true));
        store.push(ToolOutput::new("grep", "beta", true));
        store.push(ToolOutput::new("read", "gamma", false));

        let reads = store.get("read");
        assert_eq!(reads.len(), 2);
        assert_eq!(reads[0].output, "alpha");
        assert_eq!(reads[1].output, "gamma");
        assert_eq!(reads[1].success, false);

        let none = store.get("absent");
        assert!(none.is_empty());
    }

    #[test]
    fn recent_returns_last_n() {
        let mut store = OutputStore::new(4096);
        for i in 0..5 {
            store.push(ToolOutput::new("t", format!("out-{i}"), true));
        }
        let last2 = store.recent(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].output, "out-3");
        assert_eq!(last2[1].output, "out-4");

        // n larger than the store returns everything.
        assert_eq!(store.recent(99).len(), 5);
    }

    #[test]
    fn clear_empties() {
        let mut store = OutputStore::new(1024);
        store.push(ToolOutput::new("a", "x", true));
        store.push(ToolOutput::new("b", "y", false));
        store.clear();

        assert!(store.get("a").is_empty());
        assert!(store.recent(10).is_empty());
        assert_eq!(store.stats(), (0, 0));
    }

    #[test]
    fn stats_correct() {
        let mut store = OutputStore::new(4096);
        let entries = [
            ToolOutput::new("read", "abc", true),
            ToolOutput::new("grep", "déjà", false),
            ToolOutput::new("read", "xyz", true),
        ];
        let mut expected_bytes = 0;
        for e in &entries {
            expected_bytes += e.size_bytes();
            store.push(e.clone());
        }
        let (total, size) = store.stats();
        assert_eq!(total, 3);
        assert_eq!(size, expected_bytes);
    }

    #[test]
    fn max_size_enforced() {
        // Budget smaller than a single entry evicts everything.
        let mut store = OutputStore::new(4);
        store.push(ToolOutput::new("a", "aaaa", true));
        assert_eq!(store.stats(), (0, 0));

        // Budget fits two entries (42 bytes) but not three (63 bytes).
        let mut store = OutputStore::new(60);
        store.push(ToolOutput::new("a", "01234567890123456789", true)); // 21 bytes
        store.push(ToolOutput::new("a", "01234567890123456789", true)); // 42 total
        assert_eq!(store.stats(), (2, 42));

        store.push(ToolOutput::new("a", "01234567890123456789", true)); // 63 total
        let (total, size) = store.stats();
        assert!(total <= 2);
        assert!(size <= 60);
        assert_eq!(store.get("a").len(), total);
    }
}
