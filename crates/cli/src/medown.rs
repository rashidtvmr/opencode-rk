#![forbid(unsafe_code)]
//! Minimal markdown renderer state (pure, no IO).
//!
//! Pattern: `native_status.rs` pure state. Parses `**bold**` and `` `code` ``
//! into [`Span`]s, strips `#` heading prefixes, exposes pure
//! [`render_plain`] that strips markers. Output capped at [`MAX_OUT`].

/// Max rendered/plain output bytes (char-boundary truncated).
pub const MAX_OUT: usize = 16384;

/// One inline run with style flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub code: bool,
}

fn strip_heading(line: &str) -> &str {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i] == b'#' {
        i += 1;
    }
    if i == 0 {
        return line;
    }
    if i < b.len() && b[i] == b' ' {
        i += 1;
    }
    line.get(i..).unwrap_or("")
}

fn truncate_floor(s: &mut String, max: usize) {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
}

/// Parse one line of inline markdown into styled spans.
/// Strips a leading `#` heading prefix; `**` toggles bold, `` ` `` toggles
/// code. Total text capped at [`MAX_OUT`] bytes.
#[must_use]
pub fn parse_inline(input: &str) -> Vec<Span> {
    let text = strip_heading(input);
    let mut spans: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut bold = false;
    let mut code = false;
    let mut flush = |buf: &mut String, spans: &mut Vec<Span>, bold: bool, code: bool| {
        if !buf.is_empty() {
            spans.push(Span {
                text: std::mem::take(buf),
                bold,
                code,
            });
        }
    };
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            flush(&mut buf, &mut spans, bold, code);
            code = !code;
            continue;
        }
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            flush(&mut buf, &mut spans, bold, code);
            bold = !bold;
            continue;
        }
        buf.push(c);
    }
    flush(&mut buf, &mut spans, bold, code);
    let mut budget = MAX_OUT;
    for s in spans.iter_mut() {
        if budget == 0 {
            s.text.clear();
        } else if s.text.len() > budget {
            truncate_floor(&mut s.text, budget);
            budget = 0;
        } else {
            budget -= s.text.len();
        }
    }
    spans.retain(|s| !s.text.is_empty());
    spans
}

/// Strip markdown markers (`**`, `` ` ``, `#` heading prefixes) to plain
/// text, line by line. Capped at [`MAX_OUT`] bytes.
#[must_use]
pub fn render_plain(input: &str) -> String {
    let mut out = String::new();
    for (i, line) in input.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if out.len() >= MAX_OUT {
                break;
            }
        }
        let stripped = strip_heading(line).replace("**", "").replace('`', "");
        // Respect byte budget on char boundary per line push.
        let room = MAX_OUT.saturating_sub(out.len());
        if stripped.len() > room {
            let mut cut = room;
            while !stripped.is_char_boundary(cut) {
                cut = cut.saturating_sub(1);
            }
            out.push_str(&stripped[..cut]);
            break;
        }
        out.push_str(&stripped);
    }
    truncate_floor(&mut out, MAX_OUT);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_markers_stripped() {
        assert_eq!(render_plain("a **b** c"), "a b c");
        let spans = parse_inline("a **b** c");
        assert_eq!(spans.len(), 3);
        assert!(spans[1].bold);
        assert_eq!(spans[1].text, "b");
    }

    #[test]
    fn code_markers_stripped() {
        assert_eq!(render_plain("a `b` c"), "a b c");
        let spans = parse_inline("a `b` c");
        assert!(spans[1].code);
        assert_eq!(spans[1].text, "b");
    }

    #[test]
    fn heading_prefix_stripped() {
        assert_eq!(render_plain("# hello"), "hello");
        assert_eq!(render_plain("## hi"), "hi");
        assert_eq!(parse_inline("# hello")[0].text, "hello");
    }

    #[test]
    fn output_capped() {
        let big = "x".repeat(MAX_OUT + 100);
        assert_eq!(render_plain(&big).len(), MAX_OUT);
        let total: usize = parse_inline(&big).iter().map(|s| s.text.len()).sum();
        assert_eq!(total, MAX_OUT);
    }
}
