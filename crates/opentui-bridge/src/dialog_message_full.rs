#![forbid(unsafe_code)]
//! Full message dialog: title plus wrapped body.
//!
//! Mirrors `packages/tui/src/routes/session/dialog-message.tsx:10`
//! `DialogMessage` (title "Message Actions" plus selectable text rows).

/// Max chars kept in title.
pub const MAX_TITLE_CHARS: usize = 128;
/// Max chars kept in body.
pub const MAX_BODY_CHARS: usize = 4096;
/// Max rows returned by [`MessageDialog::lines`].
pub const MAX_ROWS: usize = 32;

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let w = width.max(1);
    let mut rows: Vec<String> = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split(' ') {
            let need = cur.chars().count() + word.chars().count() + usize::from(!cur.is_empty());
            if need > w && !cur.is_empty() {
                rows.push(std::mem::take(&mut cur));
            }
            if word.chars().count() > w && cur.is_empty() {
                let mut chunk = String::new();
                for ch in word.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() == w {
                        rows.push(std::mem::take(&mut chunk));
                    }
                }
                cur = chunk;
            } else {
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(word);
            }
        }
        rows.push(std::mem::take(&mut cur));
    }
    rows
}

/// Title plus body message dialog with open flag.
#[derive(Debug, Clone, Default)]
pub struct MessageDialog {
    pub title: String,
    pub body: String,
    pub open: bool,
}

impl MessageDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_with(&mut self, title: &str, body: &str) {
        self.title = trunc(title, MAX_TITLE_CHARS);
        self.body = trunc(body, MAX_BODY_CHARS);
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn lines(&self, width: usize) -> Vec<String> {
        let mut rows = wrap(&self.title, width);
        rows.extend(wrap(&self.body, width));
        rows.truncate(MAX_ROWS);
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sets_caps_and_flag() {
        let mut d = MessageDialog::new();
        d.open_with(&"t".repeat(200), &"b".repeat(5000));
        assert!(d.open);
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(d.body.chars().count(), MAX_BODY_CHARS);
    }

    #[test]
    fn close_clears_flag() {
        let mut d = MessageDialog::new();
        d.open_with("t", "b");
        d.close();
        assert!(!d.open);
    }

    #[test]
    fn lines_caps_rows() {
        let mut d = MessageDialog::new();
        d.open_with("t", &"w ".repeat(500));
        assert!(d.lines(4).len() <= MAX_ROWS);
    }

    #[test]
    fn lines_char_safe_wrap() {
        let mut d = MessageDialog::new();
        d.open_with("héllo wörld", "body éè");
        for row in d.lines(4) {
            assert!(row.chars().count() <= 4 || row.is_empty());
        }
    }

    #[test]
    fn lines_empty_body_single_title_row() {
        let mut d = MessageDialog::new();
        d.open_with("hi", "");
        assert_eq!(d.lines(80), vec!["hi".to_string(), String::new()]);
    }

    #[test]
    fn width_zero_no_panic() {
        let d = MessageDialog::new();
        assert!(d.lines(0).len() <= MAX_ROWS);
    }
}
