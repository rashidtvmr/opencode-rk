use opencode_rk_tools::slash_defs::{MAX_SLASH, SlashError, qualify_slash};

#[test]
fn sld_t01_valid() {
    let out = qualify_slash(&[("/help", "help text"), ("/init", "")]).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].name, "/help");
    assert_eq!(out[0].desc, "help text");
    assert_eq!(out[1].name, "/init");
    assert_eq!(out[1].desc, "");
}

#[test]
fn sld_t02_empty() {
    assert_eq!(qualify_slash(&[("", "x")]).unwrap_err(), SlashError::EmptyName);
}

#[test]
fn sld_t03_bad() {
    for bad in ["help", "/", "/has space", " /x"] {
        assert_eq!(
            qualify_slash(&[(bad, "d")]).unwrap_err(),
            SlashError::BadName,
            "input: {bad:?}"
        );
    }
}

#[test]
fn sld_t04_overflow() {
    let names: Vec<String> = (0..MAX_SLASH + 1).map(|i| format!("/c{i}")).collect();
    let items: Vec<(&str, &str)> = names.iter().map(|s| (s.as_str(), "d")).collect();
    assert_eq!(
        qualify_slash(&items).unwrap_err(),
        SlashError::TooMany {
            max: MAX_SLASH,
            actual: MAX_SLASH + 1
        }
    );
}

#[test]
fn sld_t05_order() {
    let out = qualify_slash(&[("/b", "1"), ("/a", "2"), ("/c", "3")]).unwrap();
    let names: Vec<&str> = out.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["/b", "/a", "/c"]);
}
