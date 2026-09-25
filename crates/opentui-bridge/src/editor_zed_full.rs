#![forbid(unsafe_code)]
//! Zed open command (full).
//!
//! Mirrors `packages/tui/src/editor-zed.ts:197-199` (`isZedTerminal`) and
//! `packages/tui/src/editor.ts:26-35` (`openEditor` argv). Plan only; host spawns.
//! ponytail: line as separate arg; upgrade: `path:line` single arg when needed.

/// Build `zed path line` argv; capacity 3.
#[must_use]
pub fn zed_cmd(path: &str, line: u32) -> Vec<String> {
    let mut v = Vec::with_capacity(3);
    v.push("zed".to_string());
    v.push(path.to_string());
    v.push(line.to_string());
    v
}

/// Stub: always false. No spawn/exec in lib; host probes `PATH`/env.
#[must_use]
pub fn is_zed_available() -> bool {
    false
}

/// Binary label for UI/messages.
#[must_use]
pub fn zed_label() -> &'static str {
    "zed"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_shape() {
        let v = zed_cmd("src/main.rs", 12);
        assert_eq!(v, vec!["zed", "src/main.rs", "12"]);
        assert_eq!(v.capacity(), 3);
    }

    #[test]
    fn cmd_line_passthrough() {
        let v = zed_cmd("a.rs", 1);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], "zed");
    }

    #[test]
    fn label_and_stub() {
        assert_eq!(zed_label(), "zed");
        assert!(!is_zed_available());
    }
}
