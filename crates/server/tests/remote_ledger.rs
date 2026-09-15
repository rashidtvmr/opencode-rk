use opencode_rk_server::remote_ledger::{
    LedgerError, RemoteEntry, RemoteLedger, MAX_LEDGER_ENTRIES,
};

fn entry(r: &str, hash: &str, at: u64) -> RemoteEntry {
    RemoteEntry {
        ref_name: r.to_string(),
        hash: hash.to_string(),
        pushed_at: at,
    }
}

fn good_hash(seed: u8) -> String {
    // 40 lowercase hex chars, deterministic
    let mut s = String::with_capacity(40);
    for i in 0..40 {
        let v = (seed.wrapping_add(i as u8) % 16) as u8;
        s.push(char::from_digit(v as u32, 16).unwrap());
    }
    s
}

#[test]
fn ledger_t01_record_and_get() {
    let mut l = RemoteLedger::new();
    let h = good_hash(1);
    l.record(entry("refs/heads/main", &h, 7)).unwrap();
    let got = l.get("refs/heads/main").unwrap();
    assert_eq!(got.ref_name, "refs/heads/main");
    assert_eq!(got.hash, h);
    assert_eq!(got.pushed_at, 7);
    assert_eq!(l.list().len(), 1);
    assert_eq!(l.list()[0].ref_name, "refs/heads/main");
}

#[test]
fn ledger_t02_upsert_replaces() {
    let mut l = RemoteLedger::new();
    l.record(entry("refs/heads/main", &good_hash(1), 1))
        .unwrap();
    l.record(entry("refs/heads/dev", &good_hash(2), 2)).unwrap();
    // upsert same ref: replace in place, no dup, insertion order kept
    l.record(entry("refs/heads/main", &good_hash(9), 99))
        .unwrap();
    assert_eq!(l.list().len(), 2);
    assert_eq!(l.list()[0].ref_name, "refs/heads/main");
    assert_eq!(l.list()[0].hash, good_hash(9));
    assert_eq!(l.list()[0].pushed_at, 99);
    assert_eq!(l.list()[1].ref_name, "refs/heads/dev");
    let got = l.get("refs/heads/main").unwrap();
    assert_eq!(got.hash, good_hash(9));
}

#[test]
fn ledger_t03_bad_hash_rejected() {
    let mut l = RemoteLedger::new();
    // empty ref
    let r = l.record(entry("", &good_hash(1), 1));
    assert!(matches!(r, Err(LedgerError::EmptyRef)));
    // short hash
    let r = l.record(entry("refs/heads/main", "abc", 1));
    assert!(matches!(r, Err(LedgerError::BadHash)));
    // uppercase hex rejected (must be lowercase)
    let upper = good_hash(1).to_uppercase();
    // guard: only run if actually different (seed 1 yields hex with letters)
    if upper != good_hash(1) {
        let r = l.record(entry("refs/heads/main", &upper, 1));
        assert!(matches!(r, Err(LedgerError::BadHash)));
    }
    // non-hex char
    let mut bad = good_hash(1);
    bad.replace_range(0..1, "z");
    let r = l.record(entry("refs/heads/main", &bad, 1));
    assert!(matches!(r, Err(LedgerError::BadHash)));
    // wrong length (41)
    let long = format!("{}0", good_hash(1));
    assert_eq!(long.len(), 41);
    let r = l.record(entry("refs/heads/main", &long, 1));
    assert!(matches!(r, Err(LedgerError::BadHash)));
    assert!(l.list().is_empty());
}

#[test]
fn ledger_t04_unknown_rejected() {
    let l = RemoteLedger::new();
    let r = l.get("refs/heads/nope");
    match r {
        Err(LedgerError::UnknownRef { name }) => assert_eq!(name, "refs/heads/nope"),
        _ => panic!("expected UnknownRef"),
    }
}

#[test]
fn ledger_t05_overflow_rejected() {
    let mut l = RemoteLedger::new();
    for i in 0..MAX_LEDGER_ENTRIES {
        let seed = (i % 250) as u8 + 1;
        l.record(entry(
            &format!("refs/heads/b{i}"),
            &good_hash(seed),
            i as u64,
        ))
        .unwrap();
    }
    assert_eq!(l.list().len(), MAX_LEDGER_ENTRIES);
    let r = l.record(entry("refs/heads/overflow", &good_hash(7), 999));
    match r {
        Err(LedgerError::TooManyEntries { max, actual }) => {
            assert_eq!(max, MAX_LEDGER_ENTRIES);
            assert_eq!(actual, MAX_LEDGER_ENTRIES);
        }
        _ => panic!("expected TooManyEntries"),
    }
    assert_eq!(l.list().len(), MAX_LEDGER_ENTRIES);
}
