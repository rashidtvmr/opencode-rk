use opencode_rk_providers::int_client::{client_tag, qualify_client, ClientError};

#[test]
fn intcl_t01_valid() {
    assert_eq!(
        qualify_client("abc-123_XY").expect("valid id should qualify"),
        "abc-123_xy"
    );
}

#[test]
fn intcl_t02_empty() {
    assert_eq!(qualify_client("").expect_err("empty"), ClientError::EmptyId);
    assert_eq!(client_tag("").expect_err("empty tag"), ClientError::EmptyId);
}

#[test]
fn intcl_t03_bad_chars() {
    for bad in ["a b", "a/b", "a!", "-abc", "_abc", ".abc"] {
        assert_eq!(
            qualify_client(bad).expect_err("bad id"),
            ClientError::BadId,
            "id {bad:?} must be rejected"
        );
        assert_eq!(
            client_tag(bad).expect_err("bad tag"),
            ClientError::BadId,
            "tag {bad:?} must be rejected"
        );
    }
}

#[test]
fn intcl_t04_tag() {
    assert_eq!(
        client_tag("AbC-1").expect("valid tag"),
        "client:abc-1"
    );
}

#[test]
fn intcl_t05_too_long() {
    let ok = "a".repeat(64);
    assert_eq!(qualify_client(&ok).expect("64 chars ok"), ok);
    let long = "a".repeat(65);
    assert_eq!(
        qualify_client(&long).expect_err("too long"),
        ClientError::BadId
    );
    assert_eq!(
        client_tag(&long).expect_err("too long tag"),
        ClientError::BadId
    );
}
