use opencode_rk_tools::backup_seq::backup_seq_name;

#[test]
fn bsq_t01_pad() {
    assert_eq!(backup_seq_name("db", 1), "db-0001");
}

#[test]
fn bsq_t02_zero() {
    assert_eq!(backup_seq_name("db", 0), "db-0000");
}

#[test]
fn bsq_t03_wide() {
    assert_eq!(backup_seq_name("db", 12345), "db-12345");
}

#[test]
fn bsq_t04_base_kept() {
    assert_eq!(backup_seq_name("my-backup", 42), "my-backup-0042");
}

#[test]
fn bsq_t05_dash() {
    let out = backup_seq_name("a", 7);
    assert_eq!(out, "a-0007");
    assert!(out.starts_with("a-"));
}
