#![forbid(unsafe_code)]

//! Bounded permission request queue (mirrors `permission.shared.ts` queue side).
//!
//! TS truth holds the stage machine; this keeps pending rows bounded.

pub const MAX_TOOL_CHARS: usize = 64;
pub const MAX_DETAIL_CHARS: usize = 512;
pub const MAX_QUEUE: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermRequest {
    pub tool: String,
    pub detail: String,
    pub allow: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PermQueue {
    pub items: Vec<PermRequest>,
}

impl PermQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn request(
        &mut self,
        tool: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<(), String> {
        let tool = tool.into();
        let detail = detail.into();
        if tool.trim().is_empty() {
            return Err("tool must not be empty".to_string());
        }
        if tool.chars().count() > MAX_TOOL_CHARS {
            return Err("tool over 64 chars".to_string());
        }
        if detail.chars().count() > MAX_DETAIL_CHARS {
            return Err("detail over 512 chars".to_string());
        }
        if self.items.len() >= MAX_QUEUE {
            return Err("queue full (32)".to_string());
        }
        self.items.push(PermRequest {
            tool,
            detail,
            allow: None,
        });
        Ok(())
    }

    pub fn resolve(&mut self, idx: usize, allow: bool) -> bool {
        match self.items.get_mut(idx) {
            Some(row) => {
                row.allow = Some(allow);
                true
            }
            None => false,
        }
    }

    pub fn pending(&self) -> usize {
        self.items.iter().filter(|row| row.allow.is_none()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ok_pushes_unresolved_row() {
        let mut queue = PermQueue::new();
        assert!(queue.request("bash", "run ls").is_ok());
        assert_eq!(queue.items.len(), 1);
        assert_eq!(queue.items[0].allow, None);
    }

    #[test]
    fn request_rejects_tool_over_64_chars() {
        let mut queue = PermQueue::new();
        assert!(queue.request("x".repeat(65), "detail").is_err());
    }

    #[test]
    fn request_rejects_detail_over_512_chars() {
        let mut queue = PermQueue::new();
        assert!(queue.request("bash", "y".repeat(513)).is_err());
    }

    #[test]
    fn request_rejects_full_queue() {
        let mut queue = PermQueue::new();
        for _ in 0..MAX_QUEUE {
            queue.request("bash", "detail").unwrap();
        }
        assert!(queue.request("bash", "detail").is_err());
    }

    #[test]
    fn resolve_sets_allow_flag() {
        let mut queue = PermQueue::new();
        queue.request("bash", "detail").unwrap();
        assert!(queue.resolve(0, true));
        assert_eq!(queue.items[0].allow, Some(true));
    }

    #[test]
    fn resolve_oob_returns_false() {
        let mut queue = PermQueue::new();
        assert!(!queue.resolve(0, true));
        queue.request("bash", "detail").unwrap();
        assert!(!queue.resolve(7, false));
    }

    #[test]
    fn pending_counts_only_unresolved() {
        let mut queue = PermQueue::new();
        queue.request("a", "d").unwrap();
        queue.request("b", "d").unwrap();
        assert_eq!(queue.pending(), 2);
        queue.resolve(0, true);
        assert_eq!(queue.pending(), 1);
    }
}
