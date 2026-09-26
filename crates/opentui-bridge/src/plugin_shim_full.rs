#![forbid(unsafe_code)]
//! Single legacy `api.command` shim (TS `command-shim.ts`); `ponytail:` one cmd only.
pub const MAX_CMD_LEN: usize = 128;
/// One shimmed command plus run count.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShimCmd {
    cmd: String,
    ran: u32,
}
impl ShimCmd {
    /// New shim; overlong truncated.
    #[must_use]
    pub fn new(cmd: &str) -> Self {
        Self {
            cmd: cmd.chars().take(MAX_CMD_LEN).collect(),
            ran: 0,
        }
    }
    /// Run once; false when empty, else bumps count.
    pub fn run(&mut self) -> bool {
        if self.cmd.is_empty() {
            return false;
        }
        self.ran = self.ran.saturating_add(1);
        true
    }
    /// Command id.
    #[must_use]
    pub fn cmd_of(&self) -> &str {
        &self.cmd
    }
    /// Successful runs.
    #[must_use]
    pub fn ran(&self) -> u32 {
        self.ran
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn run_bumps() {
        let mut s = ShimCmd::new("palette.show");
        assert_eq!(s.cmd_of(), "palette.show");
        assert!(s.run());
        assert_eq!(s.ran(), 1);
    }
    #[test]
    fn run_empty_false() {
        let mut s = ShimCmd::new("");
        assert!(!s.run());
        assert_eq!(s.ran(), 0);
    }
    #[test]
    fn truncates() {
        let s = ShimCmd::new(&"c".repeat(200));
        assert_eq!(s.cmd_of().chars().count(), MAX_CMD_LEN);
    }
}
