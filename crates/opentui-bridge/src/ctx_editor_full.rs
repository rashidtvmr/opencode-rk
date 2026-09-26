#![forbid(unsafe_code)]

//! Editor context protocol marker.
//! TS truth: packages/tui/src/context/editor.ts:9 (`MCP_PROTOCOL_VERSION`).

/// Protocol version holder, `proto` capped at 32 chars.
pub struct CtxEditor {
    pub proto: String,
}

impl CtxEditor {
    /// Current protocol version (`2025-11-25`).
    pub fn new() -> Self {
        Self {
            proto: String::from("2025-11-25"),
        }
    }

    /// True when `other` matches the current protocol.
    pub fn is_current(&self, other: &str) -> bool {
        self.proto == other
    }
}

impl Default for CtxEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_protocol() {
        assert_eq!(CtxEditor::new().proto, "2025-11-25");
    }

    #[test]
    fn proto_within_cap() {
        assert!(CtxEditor::new().proto.len() <= 32);
    }

    #[test]
    fn is_current_accepts_match() {
        assert!(CtxEditor::new().is_current("2025-11-25"));
    }

    #[test]
    fn is_current_rejects_mismatch() {
        assert!(!CtxEditor::new().is_current("2024-01-01"));
    }
}
