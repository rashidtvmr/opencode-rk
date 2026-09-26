#![forbid(unsafe_code)]
//! Re-export kind names (mirrors component/prompt/{frecency,history,stash}.tsx).
#[must_use]
pub fn reexp_name(kind: &str) -> &'static str {
    match kind.trim() {
        "frecency" => "frecency",
        "history" => "history",
        "stash" => "stash",
        _ => "",
    }
}
#[must_use]
pub fn is_reexp(kind: &str) -> bool {
    !reexp_name(kind).is_empty()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(reexp_name("frecency"), "frecency");
        assert_eq!(reexp_name("history"), "history");
        assert_eq!(reexp_name("stash"), "stash");
    }
    #[test]
    fn unknown_empty() {
        assert_eq!(reexp_name("other"), "");
        assert!(!is_reexp("other"));
    }
    #[test]
    fn trims() {
        assert!(is_reexp(" history "));
        assert!(!is_reexp(""));
    }
}
