use opencode_rk_sessions::share_invite::{InviteError, make_invite};

#[test]
fn inv_t01_valid() {
    let inv = make_invite("Ada@Example.COM", "editor").unwrap();
    assert_eq!(inv.email, "ada@example.com");
    assert_eq!(inv.role, "editor");
}

#[test]
fn inv_t02_empty() {
    assert!(matches!(make_invite("", "editor"), Err(InviteError::EmptyEmail)));
    assert!(matches!(
        make_invite("   ", "editor"),
        Err(InviteError::EmptyEmail)
    ));
}

#[test]
fn inv_t03_bad() {
    assert!(matches!(
        make_invite("no-at-sign", "editor"),
        Err(InviteError::BadEmail)
    ));
    assert!(matches!(
        make_invite("a@b", "editor"),
        Err(InviteError::BadEmail)
    ));
    assert!(matches!(
        make_invite("a@bcd", "editor"),
        Err(InviteError::BadEmail)
    ));
}

#[test]
fn inv_t04_empty_role() {
    assert!(matches!(
        make_invite("a@b.co", ""),
        Err(InviteError::EmptyRole)
    ));
    assert!(matches!(
        make_invite("a@b.co", "   "),
        Err(InviteError::EmptyRole)
    ));
}

#[test]
fn inv_t05_normalizes() {
    let inv = make_invite("  BOB@Example.Org  ", "viewer").unwrap();
    assert_eq!(inv.email, "bob@example.org");
    assert_eq!(inv.role, "viewer");
}
