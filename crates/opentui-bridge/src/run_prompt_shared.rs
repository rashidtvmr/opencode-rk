//! Shared prompt queue for run prompts (TS: `prompt.shared.ts`).
#![forbid(unsafe_code)]

pub const MAX_PROMPT_ID_CHARS: usize = 64;
pub const MAX_PROMPT_TEXT_CHARS: usize = 4096;
pub const MAX_QUEUE_ENTRIES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptEntry {
    pub id: String,
    pub text: String,
}

impl PromptEntry {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Result<Self, String> {
        let id: String = id.into().chars().take(MAX_PROMPT_ID_CHARS + 1).collect();
        let text: String = text
            .into()
            .chars()
            .take(MAX_PROMPT_TEXT_CHARS + 1)
            .collect();
        if id.chars().count() > MAX_PROMPT_ID_CHARS {
            return Err("id exceeds 64 chars".to_string());
        }
        if text.chars().count() > MAX_PROMPT_TEXT_CHARS {
            return Err("text exceeds 4KiB chars".to_string());
        }
        Ok(Self { id, text })
    }
}

#[derive(Clone, Debug, Default)]
pub struct PromptQueue {
    items: Vec<PromptEntry>,
}

impl PromptQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn enqueue(&mut self, entry: PromptEntry) -> Result<(), String> {
        if self.items.len() >= MAX_QUEUE_ENTRIES {
            return Err("queue full".to_string());
        }
        self.items.push(entry);
        Ok(())
    }

    pub fn dequeue(&mut self) -> Option<PromptEntry> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    pub fn peek(&self) -> Option<&PromptEntry> {
        self.items.first()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, text: &str) -> PromptEntry {
        PromptEntry::new(id, text).unwrap()
    }

    #[test]
    fn enqueue_ok() {
        let mut q = PromptQueue::new();
        assert!(q.enqueue(entry("a", "hello")).is_ok());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn full_errs() {
        let mut q = PromptQueue::new();
        for i in 0..MAX_QUEUE_ENTRIES {
            q.enqueue(entry(&format!("id-{i}"), "x")).unwrap();
        }
        assert!(q.enqueue(entry("overflow", "x")).is_err());
    }

    #[test]
    fn fifo_order() {
        let mut q = PromptQueue::new();
        q.enqueue(entry("1", "first")).unwrap();
        q.enqueue(entry("2", "second")).unwrap();
        assert_eq!(q.dequeue().unwrap().id, "1");
        assert_eq!(q.dequeue().unwrap().id, "2");
    }

    #[test]
    fn peek_none_empty() {
        let q = PromptQueue::new();
        assert!(q.peek().is_none());
        assert!(q.is_empty());
    }

    #[test]
    fn dequeue_none_empty() {
        let mut q = PromptQueue::new();
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn peek_returns_head() {
        let mut q = PromptQueue::new();
        q.enqueue(entry("h", "head")).unwrap();
        q.enqueue(entry("t", "tail")).unwrap();
        assert_eq!(q.peek().unwrap().id, "h");
        assert_eq!(q.len(), 2);
    }
}
