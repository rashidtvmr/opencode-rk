#![forbid(unsafe_code)]
//! Editor location arg + remote probe (editor.ts:26-54, :56-96).
//! ponytail: `path:line` only; upgrade: `file:line:col`.

/// Byte cap for the `path:line` arg.
pub const MAX_ARG_BYTES: usize = 512;

fn truncate(s: &str) -> &str {
    if s.len() <= MAX_ARG_BYTES {
        return s;
    }
    let mut end = MAX_ARG_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// `path:line` arg capped at [`MAX_ARG_BYTES`] bytes; line 0 = no suffix.
#[must_use]
pub fn editor_arg(path: &str, line: u32) -> String {
    let full = if line == 0 {
        path.to_string()
    } else {
        format!("{path}:{line}")
    };
    truncate(&full).to_string()
}

/// Remote probe: `ssh://`, `remote://`, `vscode-remote://` only.
#[must_use]
pub fn is_remote(path: &str) -> bool {
    path.starts_with("ssh://")
        || path.starts_with("remote://")
        || path.starts_with("vscode-remote://")
}

/// Static label for the editor slot.
#[must_use]
pub const fn editor_label() -> &'static str {
    "editor"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arg_line_suffix() {
        assert_eq!(editor_arg("src/main.rs", 12), "src/main.rs:12");
        assert_eq!(editor_arg("src/main.rs", 0), "src/main.rs");
    }
    #[test]
    fn arg_caps() {
        let out = editor_arg(&"x".repeat(600), 1);
        assert!(out.len() <= MAX_ARG_BYTES && out.is_char_boundary(out.len()));
    }
    #[test]
    fn remote_prefix() {
        assert!(is_remote("ssh://h/p") && is_remote("vscode-remote://h/p"));
        assert!(!is_remote("/tmp/l.md") && !is_remote("relative/p.md"));
    }
    #[test]
    fn label_static() {
        assert_eq!(editor_label(), "editor");
    }
}
