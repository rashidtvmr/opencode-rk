//! Domain-suffix match helper.

pub fn host_matches_suffix(host: &str, suffix: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    let suffix = suffix.trim().to_ascii_lowercase();
    if host.is_empty() || suffix.is_empty() {
        return false;
    }
    if host == suffix {
        return true;
    }
    host.ends_with(&format!(".{suffix}"))
}
