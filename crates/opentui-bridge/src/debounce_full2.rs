#![forbid(unsafe_code)]
//! Tick-driven debounce: last-push-wins String stage, explicit `poll`.
//! ponytail: no timers/threads; upgrade: drive `poll` from a clock loop.

const MAX_PENDING: usize = 4096;

/// Last-push-wins staged text; `poll` fires after `wait` ticks.
#[derive(Debug, Clone)]
pub struct DebounceFull {
    pending: Option<String>,
    ticks: u32,
    wait: u32,
}

impl DebounceFull {
    #[must_use]
    pub fn new(wait: u32) -> Self {
        Self {
            pending: None,
            ticks: 0,
            wait,
        }
    }
    pub fn push(&mut self, next: &str) {
        let mut end = next.len().min(MAX_PENDING);
        while end > 0 && !next.is_char_boundary(end) {
            end -= 1;
        }
        self.pending = Some(next[..end].to_owned());
        self.ticks = 0;
    }
    pub fn poll(&mut self) -> Option<String> {
        if self.pending.is_none() {
            return None;
        }
        self.ticks = self.ticks.saturating_add(1);
        if self.ticks >= self.wait {
            self.ticks = 0;
            self.pending.take()
        } else {
            None
        }
    }
    pub fn set_wait(&mut self, wait: u32) -> bool {
        if self.wait == wait {
            return false;
        }
        self.wait = wait;
        true
    }
    #[must_use]
    pub fn wait(&self) -> u32 {
        self.wait
    }
    #[must_use]
    pub fn ticks(&self) -> u32 {
        self.ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_after_wait() {
        let mut d = DebounceFull::new(2);
        d.push("hi");
        assert_eq!(d.poll(), None);
        assert_eq!(d.poll(), Some("hi".to_owned()));
    }

    #[test]
    fn none_when_empty() {
        let mut d = DebounceFull::new(1);
        assert_eq!(d.poll(), None);
        assert_eq!(d.poll(), None);
    }

    #[test]
    fn last_push_wins_resets_ticks() {
        let mut d = DebounceFull::new(2);
        d.push("a");
        assert_eq!(d.poll(), None);
        d.push("b");
        assert_eq!(d.poll(), None);
        assert_eq!(d.poll(), Some("b".to_owned()));
        assert_eq!(d.poll(), None);
    }

    #[test]
    fn caps_at_4kib_char_boundary() {
        let mut d = DebounceFull::new(1);
        let big = "e\u{301}".repeat(3000);
        d.push(&big);
        let out = d.poll().unwrap();
        assert!(out.len() <= 4096);
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn set_wait_reports_change() {
        let mut d = DebounceFull::new(2);
        assert!(!d.set_wait(2));
        assert!(d.set_wait(3));
        assert_eq!(d.wait(), 3);
    }

    #[test]
    fn wait_zero_fires_immediately() {
        let mut d = DebounceFull::new(0);
        d.push("x");
        assert_eq!(d.poll(), Some("x".to_owned()));
    }
}
