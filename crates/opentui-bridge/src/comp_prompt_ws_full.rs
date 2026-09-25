#![forbid(unsafe_code)]
//! Prompt workspace path (`packages/tui/src/component/prompt/workspace.tsx`).
pub const MAX_PATH: usize = 512;
pub const MAX_LABEL: usize = 128;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptWs {
    path: String,
}
fn floor(s: &str, max: usize) -> usize {
    let mut e = max.min(s.len());
    while e > 0 && !s.is_char_boundary(e) {
        e -= 1;
    }
    e
}
impl PromptWs {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, p: &str) {
        self.path = p[..floor(p, MAX_PATH)].to_string();
    }
    #[must_use]
    pub fn path_of(&self) -> &str {
        &self.path
    }
    #[must_use]
    pub fn label(&self, home: &str) -> String {
        let s = match self.path.strip_prefix(home) {
            Some(r) if !home.is_empty() => {
                if r.is_empty() || r.starts_with('/') {
                    format!("~{r}")
                } else {
                    self.path.clone()
                }
            }
            _ => self.path.clone(),
        };
        s[..floor(&s, MAX_LABEL)].to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_caps_at_512() {
        let mut w = PromptWs::new();
        w.set(&"a".repeat(600));
        assert_eq!(w.path_of().len(), MAX_PATH);
    }
    #[test]
    fn label_tildes_home_prefix() {
        let mut w = PromptWs::new();
        w.set("/home/u/proj");
        assert_eq!(w.label("/home/u"), "~/proj");
    }
    #[test]
    fn label_caps_at_128_no_home() {
        let mut w = PromptWs::new();
        w.set(&"x".repeat(200));
        assert_eq!(w.label("/nope"), "x".repeat(MAX_LABEL));
    }
}
