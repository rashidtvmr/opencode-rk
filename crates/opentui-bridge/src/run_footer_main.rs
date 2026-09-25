#![forbid(unsafe_code)]
//! Footer bar assembly (TS `run/footer.ts` footer view ref).
//!
//! Splits available width between left/right status segments.
//! Char-boundary safe; `assemble` caps each side at half width
//! (and 256 chars); `render` pads the middle so output is
//! exactly `width` chars (shorter only when width is 0).

/// Per-side char cap.
const CAP: usize = 256;

/// Assembled footer bar: left/right segments plus total width in chars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterBar {
    pub left: String,
    pub right: String,
    pub width: u32,
}

/// Char-safe prefix of `s` capped at `max` chars.
fn take(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Char count of `s`.
fn len(s: &str) -> usize {
    s.chars().count()
}

/// Build a bar, truncating each side to `min(256, width/2)` chars.
#[must_use]
pub fn assemble(left: &str, right: &str, width: u32) -> FooterBar {
    let half = (width as usize) / 2;
    let cap = half.min(CAP);
    FooterBar {
        left: take(left, cap),
        right: take(right, cap),
        width,
    }
}

impl FooterBar {
    /// Render `left + pad + right`; total `<= width` chars, char-safe.
    #[must_use]
    pub fn render(&self) -> String {
        let w = self.width as usize;
        if w == 0 {
            return String::new();
        }
        let r = len(&self.right);
        if r >= w {
            return take(&self.right, w);
        }
        let budget = w - r;
        let left = if len(&self.left) > budget {
            take(&self.left, budget)
        } else {
            self.left.clone()
        };
        let gap = w - len(&left) - r;
        let mut out = String::new();
        out.push_str(&left);
        out.push_str(&" ".repeat(gap));
        out.push_str(&self.right);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_truncates_to_half_width() {
        let b = assemble("abcdefgh", "12345678", 6);
        assert_eq!(b.left, "abc");
        assert_eq!(b.right, "123");
    }

    #[test]
    fn assemble_caps_at_256_chars() {
        let long = "x".repeat(300);
        let b = assemble(&long, &long, 2000);
        assert_eq!(len(&b.left), 256);
        assert_eq!(len(&b.right), 256);
    }

    #[test]
    fn render_width_exact_with_padding() {
        let b = assemble("hi", "yo", 10);
        let s = b.render();
        assert_eq!(s.chars().count(), 10);
        assert_eq!(s, "hi      yo");
    }

    #[test]
    fn short_passthrough_pads_middle() {
        let b = FooterBar {
            left: "a".into(),
            right: "b".into(),
            width: 5,
        };
        assert_eq!(b.render(), "a   b");
    }

    #[test]
    fn zero_width_renders_empty() {
        let b = assemble("left", "right", 0);
        assert!(b.left.is_empty());
        assert!(b.right.is_empty());
        assert!(b.render().is_empty());
    }

    #[test]
    fn multibyte_safe_truncate_and_render() {
        let b = assemble("héllo🌍🌍🌍", "世界世界世界", 6);
        assert!(b.render().chars().count() <= 6);
        assert!(b.left.is_char_boundary(b.left.len()));
        assert!(b.right.is_char_boundary(b.right.len()));
        let _ = b.render();
    }
}
