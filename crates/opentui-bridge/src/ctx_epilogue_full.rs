#![forbid(unsafe_code)]

/// Epilogue single text, mirrors `epilogue.tsx` setter.
/// ponytail: fixed 512-char cap; grow when upstream needs more.
#[derive(Debug, Default, Clone)]
pub struct Epilogue {
    text: String,
}

impl Epilogue {
    pub const MAX_LEN: usize = 512;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, value: &str) {
        self.text = value.chars().take(Self::MAX_LEN).collect();
    }

    pub fn text_of(&self) -> &str {
        &self.text
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_text_of() {
        let mut e = Epilogue::new();
        e.set("hello");
        assert_eq!(e.text_of(), "hello");
    }

    #[test]
    fn truncates_to_512() {
        let mut e = Epilogue::new();
        e.set(&"x".repeat(600));
        assert_eq!(e.text_of().chars().count(), 512);
    }

    #[test]
    fn clear_empties() {
        let mut e = Epilogue::new();
        e.set("hi");
        e.clear();
        assert_eq!(e.text_of(), "");
    }
}
