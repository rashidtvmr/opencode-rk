pub fn next_seq(cur: u64) -> u64 {
    cur.saturating_add(1)
}

pub fn seq_label(seq: u64) -> String {
    format!("r{seq:04}")
}
