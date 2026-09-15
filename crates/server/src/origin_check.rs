//! Pure origin predicate: exact allowlist match, empty origin rejected.

/// Return true iff `origin` exactly equals an entry in `allow`.
/// Empty (or whitespace-only) origins are always rejected.
pub fn origin_allowed(allow: &[String], origin: &str) -> bool {
    if origin.trim().is_empty() {
        return false;
    }
    allow.iter().any(|o| o == origin)
}
