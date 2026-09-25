#![forbid(unsafe_code)]
//! Simplified display-column width for terminal text.
//!
//! The TypeScript reference at commit `a0d9b6c7014dab2d6e94819d4ad1d3d2a2e4f737`
//! uses `Intl.Segmenter` plus `Bun.stringWidth` in
//! `packages/tui/src/prompt/display.ts`. This module deliberately uses a small
//! scalar range table instead: it does not reproduce Bun's full Unicode width
//! database, locale, or terminal detection. Ambiguous-width Latin is one
//! column, and only the ranges documented by [`char_width`] are two columns.
//! C0/C1 controls are zero columns here; the reference's prompt offset helper
//! treats a newline as one textarea position. Combining continuations and a
//! small ZWJ family are kept together when clipping.

fn is_c0_c1(cp: u32) -> bool {
    cp <= 0x001F || (0x007F..=0x009F).contains(&cp)
}

fn is_combining(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036F
            | 0x0B3E..=0x0B4F
            | 0x0B57
            | 0x0B82
            | 0x0BBE..=0x0BCD
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

fn is_variation_selector(cp: u32) -> bool {
    (0xFE00..=0xFE0F).contains(&cp) || (0xE0100..=0xE01EF).contains(&cp)
}

fn is_format_zero(cp: u32) -> bool {
    matches!(
        cp,
        0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0xFEFF
            | 0xFFF9..=0xFFFB
    )
}

fn is_tag(cp: u32) -> bool {
    (0xE0020..=0xE007F).contains(&cp)
}

fn is_emoji_modifier(cp: u32) -> bool {
    (0x1F3FB..=0x1F3FF).contains(&cp)
}

fn is_wide(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0x20000..=0x3FFFD
    )
}

fn is_emoji_block(cp: u32) -> bool {
    (0x1F300..=0x1FAFF).contains(&cp)
}

fn is_emoji_symbol(cp: u32) -> bool {
    (0x2600..=0x27BF).contains(&cp)
}

fn is_keycap_base(ch: char) -> bool {
    matches!(ch, '#' | '*' | '0'..='9')
}

fn is_zero_width_scalar(ch: char) -> bool {
    let cp = ch as u32;
    is_combining(cp) || is_variation_selector(cp) || is_format_zero(cp) || is_tag(cp)
}

fn is_zero_width_continuation(ch: char) -> bool {
    is_zero_width_scalar(ch) || is_emoji_modifier(ch as u32) || ch == '\u{200D}'
}

/// Return the display width of one Unicode scalar value.
///
/// The table is intentionally narrow and deterministic: ASCII, other
/// non-wide scalars, and ambiguous-width Latin are one; the specified
/// combining ranges and format scalars are zero; the specified wide and emoji
/// ranges are two. A 2600-27BF symbol becomes two only when followed by
/// U+FE0F, which is handled by [`line_width`] and [`clip_to_width`].
#[must_use]
pub fn char_width(ch: char) -> usize {
    let cp = ch as u32;
    if is_c0_c1(cp) || is_zero_width_scalar(ch) {
        0
    } else if is_wide(cp) || is_emoji_block(cp) {
        2
    } else {
        1
    }
}

/// Return the display width of a single line.
///
/// The implementation groups combining marks, variation selectors, emoji
/// modifiers, and ZWJ sequences approximately, matching the reference's
/// grapheme intent without pretending to implement UAX #29.
#[must_use]
pub fn line_width(s: &str) -> usize {
    let mut rest = s;
    let mut width = 0usize;

    while !rest.is_empty() {
        let Some((len, cluster_width)) = next_cluster(rest) else {
            break;
        };
        width = width.saturating_add(cluster_width);
        let Some(next) = rest.get(len..) else {
            break;
        };
        rest = next;
    }

    width
}

/// Return a display-width-safe prefix and its actual consumed width.
///
/// A cluster wider than the remaining budget is not split. Zero-width
/// continuations are retained with the preceding cluster.
#[must_use]
pub fn clip_to_width(s: &str, max: usize) -> (String, usize) {
    let mut out = String::new();
    let mut rest = s;
    let mut consumed = 0usize;

    while !rest.is_empty() {
        let Some((len, cluster_width)) = next_cluster(rest) else {
            break;
        };
        if consumed > max || cluster_width > max - consumed {
            break;
        }
        let Some(cluster) = rest.get(..len) else {
            break;
        };
        out.push_str(cluster);
        consumed = consumed.saturating_add(cluster_width);
        let Some(next) = rest.get(len..) else {
            break;
        };
        rest = next;
    }

    (out, consumed)
}

/// Return `(utf8_length, display_width)` for one simplified grapheme cluster.
fn next_cluster(s: &str) -> Option<(usize, usize)> {
    let mut chars = s.char_indices().peekable();
    let (_, first) = chars.next()?;
    let mut end = first.len_utf8();
    let mut width = char_width(first);
    let mut has_emoji_base = is_emoji_symbol(first as u32) || is_emoji_block(first as u32);
    let mut saw_vs16 = false;
    let mut saw_keycap = is_keycap_base(first);

    // A leading ZWJ is unusual but should not make a valid suffix unparsable.
    if first == '\u{200D}' {
        if let Some((index, joined)) = chars.next() {
            end = index + joined.len_utf8();
            width = char_width(joined);
            has_emoji_base = is_emoji_symbol(joined as u32) || is_emoji_block(joined as u32);
            saw_keycap = is_keycap_base(joined);
        }
    }

    loop {
        let Some((index, ch)) = chars.peek().copied() else {
            break;
        };

        if ch == '\u{200D}' {
            chars.next();
            end = index + ch.len_utf8();
            let Some((joined_index, joined)) = chars.next() else {
                break;
            };
            end = joined_index + joined.len_utf8();
            if is_emoji_symbol(joined as u32) || is_emoji_block(joined as u32) {
                has_emoji_base = true;
                width = width.max(char_width(joined));
            }
            if joined == '\u{FE0F}' {
                saw_vs16 = true;
            }
            saw_keycap |= is_keycap_base(joined);
            continue;
        }

        if is_zero_width_continuation(ch) {
            chars.next();
            end = index + ch.len_utf8();
            if ch == '\u{FE0F}' {
                saw_vs16 = true;
            }
            if is_emoji_symbol(ch as u32) || is_emoji_block(ch as u32) {
                has_emoji_base = true;
                width = width.max(char_width(ch));
            }
            continue;
        }

        break;
    }

    if saw_vs16 && (has_emoji_base || saw_keycap) {
        width = width.max(2);
    }
    Some((end, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(line_width("abc"), 3);
    }

    #[test]
    fn cjk_double_width() {
        assert_eq!(char_width('中'), 2);
        assert_eq!(line_width("中文"), 4);
    }

    #[test]
    fn combining_zero_width() {
        assert_eq!(char_width('\u{0301}'), 0);
        assert_eq!(line_width("a\u{0301}b"), 2);
    }

    #[test]
    fn emoji_width() {
        assert_eq!(line_width("🙂"), 2);
        assert_eq!(line_width("❤️"), 2);
        assert_eq!(line_width("👨‍👩‍👧‍👦"), 2);
    }

    #[test]
    fn tamil_base_and_combining_mark() {
        assert_eq!(line_width("கா"), 1);
    }

    #[test]
    fn clip_boundary() {
        assert_eq!(clip_to_width("a中b", 3), ("a中".into(), 3));
        assert_eq!(clip_to_width("a中b", 1), ("a".into(), 1));
        assert_eq!(clip_to_width("a\u{0301}中", 2), ("a\u{0301}".into(), 1));
    }

    #[test]
    fn empty() {
        assert_eq!(line_width(""), 0);
        assert_eq!(clip_to_width("", 0), (String::new(), 0));
    }

    #[test]
    fn controls_and_max_budget() {
        assert_eq!(line_width("\0\u{007f}\u{0085}"), 0);
        assert_eq!(clip_to_width("a🙂", usize::MAX), ("a🙂".into(), 3));
    }
}
