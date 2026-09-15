use opencode_rk_tools::ext_perms::{MAX_EXT_PERMS, ExtPerm, PermError, grant_perm};

#[test]
fn exp_t01_grant() {
    let mut buf: Vec<ExtPerm> = Vec::new();
    grant_perm(&mut buf, "ext-a", "read").expect("grant ok");
    assert_eq!(buf.len(), 1);
    assert_eq!(buf[0].ext, "ext-a");
    assert_eq!(buf[0].perm, "read");
}

#[test]
fn t02_empty_ext() {
    let mut buf: Vec<ExtPerm> = Vec::new();
    assert!(matches!(
        grant_perm(&mut buf, "", "read"),
        Err(PermError::EmptyExt)
    ));
    assert!(buf.is_empty());
}

#[test]
fn t03_empty_perm() {
    let mut buf: Vec<ExtPerm> = Vec::new();
    assert!(matches!(
        grant_perm(&mut buf, "ext-a", ""),
        Err(PermError::EmptyPerm)
    ));
    assert!(buf.is_empty());
}

#[test]
fn t04_dup_ok() {
    let mut buf: Vec<ExtPerm> = Vec::new();
    grant_perm(&mut buf, "ext-a", "read").expect("first ok");
    grant_perm(&mut buf, "ext-a", "read").expect("dup ok idempotent");
    assert_eq!(buf.len(), 1);
}

#[test]
fn t05_overflow() {
    let mut buf: Vec<ExtPerm> = Vec::new();
    for i in 0..MAX_EXT_PERMS {
        grant_perm(&mut buf, &format!("ext-{i}"), "read").expect("fill ok");
    }
    assert_eq!(buf.len(), MAX_EXT_PERMS);
    match grant_perm(&mut buf, "ext-over", "read") {
        Err(PermError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_EXT_PERMS);
            assert_eq!(actual, MAX_EXT_PERMS);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
    assert_eq!(buf.len(), MAX_EXT_PERMS);
}
