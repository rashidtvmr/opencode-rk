use opencode_rk_tools::rel_tag::{TagError, qualify_tag};

#[test]
fn tag_t01_valid() {
    assert_eq!(qualify_tag("v1.2.3"), Ok("v1.2.3".to_string()));
}

#[test]
fn tag_t02_empty() {
    assert_eq!(qualify_tag(""), Err(TagError::EmptyTag));
}

#[test]
fn tag_t03_bad_prefix() {
    assert_eq!(qualify_tag("1.2.3"), Err(TagError::BadTag));
}

#[test]
fn tag_t04_bad_chars() {
    assert_eq!(qualify_tag("v1.2.3!"), Err(TagError::BadTag));
}

#[test]
fn tag_t05_needs_dot() {
    assert_eq!(qualify_tag("v123"), Err(TagError::BadTag));
}
