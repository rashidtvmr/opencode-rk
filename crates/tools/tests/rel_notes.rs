use opencode_rk_tools::rel_notes::{MAX_NOTES, NotesError, qualify_notes};

#[test]
fn notes_t01_valid() {
    let out = qualify_notes(&[("1.2.3", "Fixed bug"), ("0.1.0", "Init")]).expect("valid notes");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].version, "1.2.3");
    assert_eq!(out[0].text, "Fixed bug");
    assert_eq!(out[1].version, "0.1.0");
    assert_eq!(out[1].text, "Init");
}

#[test]
fn notes_t02_bad_version() {
    let err = qualify_notes(&[("1.2", "oops")]).expect_err("bad version rejected");
    assert!(matches!(err, NotesError::BadVersion));
}

#[test]
fn notes_t03_empty_text() {
    let err = qualify_notes(&[("1.0.0", "")]).expect_err("empty text rejected");
    assert!(matches!(err, NotesError::EmptyText));
}

#[test]
fn notes_t04_empty_version() {
    let err = qualify_notes(&[("", "hi")]).expect_err("empty version rejected");
    assert!(matches!(err, NotesError::EmptyVersion));
}

#[test]
fn notes_t05_overflow_rejected() {
    let versions: Vec<String> = (0..MAX_NOTES + 1).map(|i| format!("1.0.{i}")).collect();
    let texts: Vec<String> = (0..MAX_NOTES + 1).map(|i| format!("note {i}")).collect();
    let refs: Vec<(&str, &str)> = versions
        .iter()
        .zip(texts.iter())
        .map(|(v, t)| (v.as_str(), t.as_str()))
        .collect();
    let err = qualify_notes(&refs).expect_err("overflow rejected");
    assert!(matches!(err, NotesError::TooManyNotes { .. }));
}
