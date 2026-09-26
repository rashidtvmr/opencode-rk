#![forbid(unsafe_code)]

pub const MAX_MODELS: usize = 64;
pub const MAX_LEN: usize = 128;

#[derive(Debug, Default, Clone)]
pub struct ModelDialog {
    models: Vec<String>,
    cursor: usize,
}

impl ModelDialog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, model: &str) -> bool {
        if self.models.len() >= MAX_MODELS || model.is_empty() {
            return false;
        }
        let s: String = model.chars().take(MAX_LEN).collect();
        self.models.push(s);
        if self.models.len() == 1 {
            self.cursor = 0;
        }
        true
    }
    pub fn move_cursor(&mut self, delta: isize) {
        if self.models.is_empty() {
            return;
        }
        let n = self.models.len() as isize;
        let next = (self.cursor as isize + delta).rem_euclid(n);
        self.cursor = next as usize;
    }
    pub fn selected(&self) -> Option<&str> {
        self.models.get(self.cursor).map(String::as_str)
    }
    pub fn len(&self) -> usize {
        self.models.len()
    }
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_none() {
        let d = ModelDialog::new();
        assert!(d.selected().is_none());
        assert!(d.is_empty());
    }
    #[test]
    fn push_select() {
        let mut d = ModelDialog::new();
        assert!(d.push("gpt-4"));
        assert!(d.push("claude-3"));
        assert_eq!(d.selected(), Some("gpt-4"));
        assert_eq!(d.len(), 2);
    }
    #[test]
    fn move_wrap() {
        let mut d = ModelDialog::new();
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
    fn cap_64_reject() {
        let mut d = ModelDialog::new();
        for i in 0..MAX_MODELS {
            assert!(d.push(&format!("m{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.len(), 64);
    }
    #[test]
    fn trunc_128_empty_reject() {
        let mut d = ModelDialog::new();
        assert!(!d.push(""));
        let long = "x".repeat(200);
        assert!(d.push(&long));
        assert_eq!(d.selected().unwrap().len(), 128);
    }
    #[test]
    fn move_empty_no_panic() {
        let mut d = ModelDialog::new();
        d.move_cursor(5);
        assert!(d.selected().is_none());
    }
}
