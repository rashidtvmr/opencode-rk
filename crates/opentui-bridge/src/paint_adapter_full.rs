#![forbid(unsafe_code)]
//! Stateful paint counter over [`crate::paint_adapter::PaintAdapter`].
//! ponytail: wrapper only; styling lands with span/theme crate.

use crate::paint_adapter::PaintAdapter;

fn cap(s: &str) -> String {
    s.chars().take(256).collect()
}

/// Owned flow config with render count.
pub struct PaintFlow {
    /// Title row; capped to 256 chars.
    pub title: String,
    /// Status row; capped to 256 chars.
    pub status: String,
    /// Frames painted; saturates at u64::MAX.
    pub renders: u64,
}

impl PaintFlow {
    /// Build with 256-char caps applied.
    #[must_use]
    pub fn new(title: &str, status: &str) -> Self {
        Self {
            title: cap(title),
            status: cap(status),
            renders: 0,
        }
    }

    /// Paint one frame via `PaintAdapter::new` + `build_frame`; bumps `renders`.
    pub fn render(
        &mut self,
        transcript: &[String],
        draft: &str,
        w: usize,
        h: usize,
    ) -> Vec<String> {
        self.renders = self.renders.saturating_add(1);
        PaintAdapter::new(&self.title, &self.status, w, h).build_frame(transcript, draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_applied() {
        let f = PaintFlow::new(&"x".repeat(300), &"y".repeat(300));
        assert_eq!(f.title.chars().count(), 256);
        assert_eq!(f.status.chars().count(), 256);
        assert_eq!(f.renders, 0);
    }

    #[test]
    fn bumps_counter() {
        let mut f = PaintFlow::new("t", "s");
        f.render(&[], "", 40, 12);
        f.render(&[], "", 40, 12);
        assert_eq!(f.renders, 2);
    }

    #[test]
    fn delegates_frame() {
        let mut f = PaintFlow::new("t", "s");
        let out = f.render(&["hi".into()], "abc", 40, 12);
        assert!(out.iter().any(|l| l == "> abc"));
        assert!(out.iter().any(|l| l == "assistant> hi"));
    }

    #[test]
    fn empty_placeholder() {
        let mut f = PaintFlow::new("t", "s");
        let out = f.render(&[], "", 40, 12);
        assert!(out.iter().any(|l| l.contains("Start typing")));
    }

    #[test]
    fn saturates() {
        let mut f = PaintFlow::new("t", "s");
        f.renders = u64::MAX;
        f.render(&[], "", 40, 12);
        assert_eq!(f.renders, u64::MAX);
    }
}
