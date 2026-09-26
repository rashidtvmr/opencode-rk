#![forbid(unsafe_code)]

pub const MAX_OPTS: usize = 32;
pub const MAX_LEN: usize = 256;

#[derive(Debug, Default, Clone)]
pub struct SelectDialog {
    opts: Vec<String>,
    cursor: usize,
}

impl SelectDialog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, opt: &str) -> bool {
        if self.opts.len() >= MAX_OPTS || opt.is_empty() {
            return false;
        }
        let s: String = opt.chars().take(MAX_LEN).collect();
        self.opts.push(s);
        if self.opts.len() == 1 {
            self.cursor = 0;
        }
        true
    }
    pub fn move_cursor(&mut self, delta: isize) {
        if self.opts.is_empty() {
            return;
        }
        let n = self.opts.len() as isize;
        let next = (self.cursor as isize + delta).rem_euclid(n);
        self.cursor = next as usize;
    }
    pub fn selected(&self) -> Option<&str> {
        self.opts.get(self.cursor).map(String::as_str)
    }
    pub fn confirm(&self) -> Option<String> {
        self.selected().map(str::to_owned)
    }
    pub fn len(&self) -> usize {
        self.opts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.opts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_none() {
        let d = SelectDialog::new();
        assert!(d.selected().is_none());
        assert!(d.confirm().is_none());
    }
    #[test]
    fn push_select_confirm() {
        let mut d = SelectDialog::new();
        assert!(d.push("a"));
        assert!(d.push("b"));
        assert_eq!(d.selected(), Some("a"));
        assert_eq!(d.confirm(), Some("a".to_owned()));
    }
    #[test]
    fn move_wrap() {
        let mut d = SelectDialog::new();
        d.push("a");
        d.push("b");
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("b"));
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("a"));
        d.move_cursor(-1);
        assert_eq!(d.selected(), Some("b"));
    }
    #[test]
    fn cap_32_reject() {
        let mut d = SelectDialog::new();
        for i in 0..MAX_OPTS {
            assert!(d.push(&format!("o{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.len(), 32);
    }
    #[test]
    fn trunc_256_empty_reject() {
        let mut d = SelectDialog::new();
        assert!(!d.push(""));
        let long = "x".repeat(300);
        assert!(d.push(&long));
        assert_eq!(d.selected().unwrap().len(), 256);
    }
    #[test]
    fn move_empty_no_panic() {
        let mut d = SelectDialog::new();
        d.move_cursor(5);
        assert!(d.selected().is_none());
    }
}
