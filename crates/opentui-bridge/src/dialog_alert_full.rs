#![forbid(unsafe_code)]
//! Alert dialog: capped title/body + seen (`dialog-alert.tsx:12` `DialogAlert`).
pub const MAX_TITLE_CHARS: usize = 128;
pub const MAX_BODY_CHARS: usize = 2048;
pub const MAX_LINES: usize = 16;
fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
fn wrap_into(out: &mut Vec<String>, text: &str, width: usize) {
    let w = width.max(1);
    let (mut cur, mut len) = (String::new(), 0usize);
    for word in text.split_whitespace() {
        let mut rest = word;
        while !rest.is_empty() {
            let space = usize::from(len > 0);
            if w.saturating_sub(len + space) == 0 {
                out.push(std::mem::take(&mut cur));
                len = 0;
                continue;
            }
            let take: String = rest.chars().take(w - len - space).collect();
            if len > 0 {
                cur.push(' ');
            }
            len += space + take.chars().count();
            cur.push_str(&take);
            rest = &rest[take.len()..];
            if len >= w {
                out.push(std::mem::take(&mut cur));
                len = 0;
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
}
#[derive(Debug, Clone, Default)]
pub struct AlertDialog {
    pub title: String,
    pub body: String,
    pub seen: bool,
}
impl AlertDialog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn show(&mut self, title: &str, body: &str) {
        self.title = trunc(title, MAX_TITLE_CHARS);
        self.body = trunc(body, MAX_BODY_CHARS);
        self.seen = false;
    }
    pub fn dismiss(&mut self) {
        self.seen = true;
    }
    pub fn lines(&self, width: usize) -> Vec<String> {
        let mut out = Vec::new();
        wrap_into(&mut out, &self.title, width);
        wrap_into(&mut out, &self.body, width);
        out.truncate(MAX_LINES);
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn show_caps_and_marks_unseen() {
        let mut d = AlertDialog::new();
        d.show(&"t".repeat(200), &"b".repeat(3000));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(d.body.chars().count(), MAX_BODY_CHARS);
        assert!(!d.seen);
    }
    #[test]
    fn dismiss_then_reshow_resets_seen() {
        let mut d = AlertDialog::new();
        d.show("a", "b");
        d.dismiss();
        assert!(d.seen);
        d.show("c", "d");
        assert!(!d.seen && d.title == "c");
    }
    #[test]
    fn lines_wrap_title_first() {
        let mut d = AlertDialog::new();
        d.show("hi", "one two three four");
        let rows = d.lines(9);
        assert_eq!(rows[0], "hi");
        assert!(rows.len() > 1 && rows.iter().all(|r| r.chars().count() <= 9));
    }
    #[test]
    fn lines_cap_and_zero_width() {
        let mut d = AlertDialog::new();
        d.show("t", &"w ".repeat(500));
        assert_eq!(d.lines(10).len(), MAX_LINES);
        assert!(d.lines(0).iter().all(|r| r.chars().count() <= 1));
    }
    #[test]
    fn unicode_caps_char_based() {
        let mut d = AlertDialog::new();
        d.show(&"é".repeat(200), &"é".repeat(3000));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(d.body.chars().count(), MAX_BODY_CHARS);
        assert!(d.lines(8).iter().all(|r| r.chars().count() <= 8));
    }
    #[test]
    fn default_unseen_blank() {
        assert!(!AlertDialog::new().seen && AlertDialog::new().lines(80).is_empty());
    }
}
