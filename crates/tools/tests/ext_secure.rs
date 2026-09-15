use opencode_rk_tools::ext_secure::{MAX_SECURE_PERMS, SecureError, grant_all};

#[test]
fn sec_t01_grants() {
    let out = grant_all(&["read", "write"]).unwrap();
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|p| p.granted));
    assert_eq!(out[0].name, "read");
    assert_eq!(out[1].name, "write");
}

#[test]
fn sec_t02_empty() {
    assert_eq!(grant_all(&[""]), Err(SecureError::EmptyName));
    assert_eq!(grant_all(&["read", ""]), Err(SecureError::EmptyName));
}

#[test]
fn sec_t03_overflow() {
    let names: Vec<String> = (0..MAX_SECURE_PERMS + 1)
        .map(|i| format!("perm{i}"))
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(
        grant_all(&refs),
        Err(SecureError::TooManyPerms {
            max: MAX_SECURE_PERMS,
            actual: MAX_SECURE_PERMS + 1
        })
    );
}

#[test]
fn sec_t04_order_kept() {
    let out = grant_all(&["c", "a", "b"]).unwrap();
    let names: Vec<&str> = out.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["c", "a", "b"]);
}

#[test]
fn sec_t05_single() {
    let out = grant_all(&["solo"]).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "solo");
    assert!(out[0].granted);
}
