use opencode_rk_tools::ops_backup::{BackupError, backup_name};

#[test]
fn bak_t01_name() {
    assert_eq!(backup_name("daily", 3).unwrap(), "daily-0003");
}

#[test]
fn bak_t02_empty() {
    assert_eq!(backup_name("", 1).unwrap_err(), BackupError::EmptyName);
}

#[test]
fn bak_t03_bad_chars() {
    assert_eq!(
        backup_name("bad name!", 1).unwrap_err(),
        BackupError::BadName
    );
}

#[test]
fn bak_t04_padding() {
    assert_eq!(backup_name("db", 0).unwrap(), "db-0000");
    assert_eq!(backup_name("db", 42).unwrap(), "db-0042");
}

#[test]
fn bak_t05_too_long() {
    let long = "a".repeat(129);
    assert_eq!(
        backup_name(&long, 1).unwrap_err(),
        BackupError::BadName
    );
}
