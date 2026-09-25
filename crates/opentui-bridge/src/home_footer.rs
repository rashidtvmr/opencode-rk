#![forbid(unsafe_code)]
//! Home footer tip rotator (mirrors `home/footer.tsx` `home_footer` slot).
//! TS View lays out Directory/Mcp/spacer/Version; tips rotate in footer line.

/// Max tips retained.
pub const MAX_TIPS: usize = 16;
/// Max chars per tip.
pub const MAX_TIP_LEN: usize = 256;

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Rotating home footer tips.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HomeFooter {
    tips: Vec<String>,
    index: usize,
}

impl HomeFooter {
    /// New rotator; keeps first [`MAX_TIPS`], each [`MAX_TIP_LEN`] chars.
    #[must_use]
    pub fn new(tips: Vec<String>) -> Self {
        let kept: Vec<String> = tips
            .into_iter()
            .take(MAX_TIPS)
            .map(|t| trunc(&t, MAX_TIP_LEN))
            .collect();
        Self {
            tips: kept,
            index: 0,
        }
    }

    /// Advance; wraps. No-op when empty.
    pub fn next(&mut self) {
        if self.tips.is_empty() {
            return;
        }
        self.index = (self.index + 1) % self.tips.len();
    }

    /// Current tip.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.tips.get(self.index).map(String::as_str)
    }

    /// Current tip clipped to `width` chars; empty when no tips.
    #[must_use]
    pub fn render(&self, width: usize) -> String {
        match self.current() {
            None => String::new(),
            Some(t) => t.chars().take(width).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_none() {
        let f = HomeFooter::new(vec![]);
        assert_eq!(f.current(), None);
        assert_eq!(f.render(10), "");
    }

    #[test]
    fn wraps() {
        let mut f = HomeFooter::new(vec!["a".into(), "b".into()]);
        assert_eq!(f.current(), Some("a"));
        f.next();
        assert_eq!(f.current(), Some("b"));
        f.next();
        assert_eq!(f.current(), Some("a"));
    }

    #[test]
    fn render_clip() {
        let f = HomeFooter::new(vec!["abcdef".into()]);
        assert_eq!(f.render(3), "abc");
        assert_eq!(f.render(0), "");
        let g = HomeFooter::new(vec!["héllo✓world".into()]);
        assert_eq!(g.render(5), "héllo");
    }

    #[test]
    fn single_stable() {
        let mut f = HomeFooter::new(vec!["only".into()]);
        f.next();
        f.next();
        assert_eq!(f.current(), Some("only"));
        assert_eq!(f.render(80), "only");
    }

    #[test]
    fn cap_trunc_16() {
        let tips: Vec<String> = (0..20).map(|i| format!("t{i}")).collect();
        let f = HomeFooter::new(tips);
        assert_eq!(f.tips.len(), MAX_TIPS);
        assert_eq!(f.current(), Some("t0"));
    }

    #[test]
    fn tip_len_capped() {
        let f = HomeFooter::new(vec!["x".repeat(300)]);
        assert_eq!(f.current().unwrap().chars().count(), MAX_TIP_LEN);
    }
}
