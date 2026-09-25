#![forbid(unsafe_code)]
use std::fmt;
pub const MAX_SYS_PLUGINS: usize = 32;
pub const MAX_SYS_NAME: usize = 64;
#[derive(Debug, Default, Clone)]
pub struct SysPlugins {
    names: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SysPluginError {
    Empty,
    TooLong,
    Full,
}
impl fmt::Display for SysPluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty name"),
            Self::TooLong => write!(f, "name too long"),
            Self::Full => write!(f, "registry full"),
        }
    }
}
impl SysPlugins {
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }
    pub fn register(&mut self, n: &str) -> Result<(), SysPluginError> {
        if n.is_empty() {
            return Err(SysPluginError::Empty);
        }
        if n.len() > MAX_SYS_NAME {
            return Err(SysPluginError::TooLong);
        }
        if self.names.len() >= MAX_SYS_PLUGINS {
            return Err(SysPluginError::Full);
        }
        if !self.has(n) {
            self.names.push(n.to_string());
        }
        Ok(())
    }
    pub fn has(&self, n: &str) -> bool {
        self.names.iter().any(|x| x == n)
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn register_has_len() {
        let mut s = SysPlugins::new();
        s.register("internal:plugin-manager").unwrap();
        assert!(s.has("internal:plugin-manager"));
        assert!(!s.has("other"));
        assert_eq!(s.len(), 1);
    }
    #[test]
    fn rejects_bad_names() {
        let mut s = SysPlugins::new();
        assert_eq!(s.register("").unwrap_err(), SysPluginError::Empty);
        assert_eq!(
            s.register(&"x".repeat(65)).unwrap_err(),
            SysPluginError::TooLong
        );
    }
    #[test]
    fn caps_at_32() {
        let mut s = SysPlugins::new();
        for i in 0..MAX_SYS_PLUGINS {
            s.register(&format!("p{i}")).unwrap();
        }
        assert_eq!(s.len(), 32);
        assert_eq!(s.register("overflow").unwrap_err(), SysPluginError::Full);
    }
}
