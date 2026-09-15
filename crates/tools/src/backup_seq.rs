pub fn backup_seq_name(base: &str, seq: u32) -> String {
    format!("{base}-{seq:04}")
}
