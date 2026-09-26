#![forbid(unsafe_code)]
//! Missing plugin route line (mirrors
//! `packages/tui/src/component/plugin-route-missing.tsx:8`
//! `Unknown plugin route: {id}` + `:10` `go home`).

/// Max rendered line length.
pub const CAP: usize = 256;
/// Line prefix marking a missing-route line.
pub const PREFIX: &str = "Unknown plugin route: ";

/// `"Unknown plugin route: <route>"` trimmed, capped at [`CAP`].
pub fn route_missing_line(route: &str) -> String {
    let line = format!("{PREFIX}{}", route.trim());
    if line.len() <= CAP {
        return line;
    }
    let mut end = CAP;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_string()
}

/// Static hint label for the go-home action.
pub fn missing_hint() -> &'static str {
    "go home"
}

/// True when `line` carries the missing-route prefix.
pub fn is_missing(line: &str) -> bool {
    line.starts_with(PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_id() {
        assert_eq!(route_missing_line("demo"), "Unknown plugin route: demo");
    }

    #[test]
    fn caps_length() {
        assert!(route_missing_line(&"x".repeat(300)).len() <= CAP);
        assert!(is_missing(&route_missing_line(&"y".repeat(300))));
    }

    #[test]
    fn hint_and_detect() {
        assert_eq!(missing_hint(), "go home");
        assert!(is_missing("Unknown plugin route: z"));
        assert!(!is_missing("go home"));
    }
}
