#![forbid(unsafe_code)]
//! Home session destination pick.
//! TS truth `packages/tui/src/routes/home/session-destination.tsx:13`
//! `HomeSessionDestination`: `{ type: "new" }` maps to `Destination::New`,
//! `{ type: "directory", directory }` maps to `Destination::Existing(id)` capped at 64.

/// Max stored id length; longer ids truncate on a char boundary.
pub const MAX_ID_LEN: usize = 64;

/// Session destination pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    New,
    Existing(String),
}

/// Pick a new session destination.
#[must_use]
pub fn pick_new() -> Destination {
    Destination::New
}

/// Pick an existing session destination; empty id errs; longer ids truncate.
pub fn pick_existing(id: &str) -> Result<Destination, String> {
    if id.is_empty() {
        return Err("session id must not be empty".to_string());
    }
    let mut s = id.to_string();
    if s.len() > MAX_ID_LEN {
        let mut end = MAX_ID_LEN;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    Ok(Destination::Existing(s))
}

/// `true` when the destination is new.
#[must_use]
pub fn is_new(d: &Destination) -> bool {
    matches!(d, Destination::New)
}

/// Existing id, or `None` for new.
#[must_use]
pub fn id_of(d: &Destination) -> Option<&str> {
    match d {
        Destination::New => None,
        Destination::Existing(s) => Some(s.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_new() {
        assert!(is_new(&pick_new()));
    }

    #[test]
    fn existing_id() {
        let d = pick_existing("abc").unwrap();
        assert!(!is_new(&d));
        assert_eq!(id_of(&d), Some("abc"));
    }

    #[test]
    fn empty_errs() {
        assert!(pick_existing("").is_err());
    }

    #[test]
    fn id_none_new() {
        assert_eq!(id_of(&Destination::New), None);
    }

    #[test]
    fn truncates() {
        let long = "x".repeat(MAX_ID_LEN + 36);
        let d = pick_existing(&long).unwrap();
        assert_eq!(id_of(&d).unwrap().len(), MAX_ID_LEN);
    }
}
