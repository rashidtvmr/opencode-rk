#![forbid(unsafe_code)]
//! Error card: `code` + `msg`, wrapped render capped at 8 rows.
//! TS truth `error-component.tsx:10,43`. `ponytail:` char-count width.
pub struct ErrorCard {
    msg: String,
    code: String,
}

impl ErrorCard {
    #[must_use]
    pub fn new(msg: &str, code: &str) -> Self {
        Self {
            msg: msg.chars().take(512).collect(),
            code: code.chars().take(32).collect(),
        }
    }

    #[must_use]
    pub fn code_of(&self) -> &str {
        &self.code
    }
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        let full = if self.code.is_empty() {
            self.msg.clone()
        } else if self.msg.is_empty() {
            self.code.clone()
        } else {
            format!("{}: {}", self.code, self.msg)
        };
        let mut out = Vec::new();
        for logical in full.split('\n') {
            wrap(logical, width, &mut out);
            if out.len() >= 8 {
                break;
            }
        }
        out.truncate(8);
        out
    }
}

fn wrap(line: &str, width: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let (mut cur, mut n) = (String::new(), 0usize);
    for w in line.split(' ') {
        if w.chars().count() > width {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                n = 0;
            }
            for c in w.chars().collect::<Vec<_>>().chunks(width) {
                out.push(c.iter().collect());
                if out.len() >= 8 {
                    return;
                }
            }
            continue;
        }
        let add = w.chars().count() + usize::from(n > 0);
        if n + add > width {
            out.push(std::mem::take(&mut cur));
            n = 0;
            if out.len() >= 8 {
                return;
            }
        }
        if n > 0 {
            cur.push(' ');
        }
        cur.push_str(w);
        n += add;
    }
    out.push(cur);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caps_msg_and_code() {
        let e = ErrorCard::new(&"a".repeat(600), &"b".repeat(40));
        assert_eq!(e.msg.chars().count(), 512);
        assert_eq!(e.code_of().chars().count(), 32);
    }
    #[test]
    fn code_of_roundtrip() {
        assert_eq!(ErrorCard::new("m", "E1").code_of(), "E1");
    }
    #[test]
    fn wraps_within_width() {
        for l in ErrorCard::new("aa bb cc", "E1").lines(5) {
            assert!(l.chars().count() <= 5);
        }
        assert!(ErrorCard::new("m", "C").lines(0).is_empty());
    }
    #[test]
    fn caps_eight_rows() {
        let e = ErrorCard::new(&"w ".repeat(200), "E");
        assert!(e.lines(4).len() <= 8);
    }
}
