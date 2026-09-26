#![forbid(unsafe_code)]
//! Widget kind map (no render): canonical `shapes.rs` vs `widget_paint.rs` dupe.
//! `shapes::Bar/Sparkline/Menu/Card` canonical; `widget_paint::Bar` dupe (paint-only).

/// Map-only widget discriminator (no payload, no render).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    Bar,
    Sparkline,
    Menu,
    Card,
}

impl WidgetKind {
    /// Canonical source; Bar notes the `widget_paint::Bar` dupe.
    #[must_use]
    pub fn canonical_of(&self) -> &'static str {
        match self {
            Self::Bar => "shapes::Bar (canonical; widget_paint::Bar dupe)",
            Self::Sparkline => "shapes::Sparkline (canonical)",
            Self::Menu => "shapes::Menu (canonical)",
            Self::Card => "shapes::Card (canonical)",
        }
    }
}

/// 8-level block for a `u8` value: `(v * 7) / 255` into `SPARK_BLOCKS`.
#[must_use]
pub fn spark_char(v: u8) -> char {
    const B: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '█'];
    B[(v as usize * 7) / 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_marks_dupe() {
        assert!(WidgetKind::Bar.canonical_of().contains("dupe"));
    }

    #[test]
    fn rest_canonical_shapes() {
        for k in [WidgetKind::Sparkline, WidgetKind::Menu, WidgetKind::Card] {
            assert!(k.canonical_of().starts_with("shapes::"));
        }
    }

    #[test]
    fn spark_char_edges() {
        assert_eq!((spark_char(0), spark_char(255)), (' ', '█'));
    }

    #[test]
    fn spark_char_monotonic() {
        let (mut last, mut n) = (0usize, 0usize);
        for v in 0..=255u8 {
            let blocks = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '█'];
            let i = blocks.iter().position(|&c| c == spark_char(v)).unwrap();
            assert!(i >= last);
            last = i;
            n += 1;
        }
        assert_eq!(n, 256);
    }
}
