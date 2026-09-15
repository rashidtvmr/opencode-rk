use opencode_rk_sessions::auto_lease::{
    normalize_owner, LeaseAsk, LeaseError, LeaseTable, MAX_LEASES,
};

#[test]
fn lease_t01_add_list() {
    let mut table = LeaseTable::new();
    assert_eq!(table.count(), 0);
    assert!(table.list().is_empty());
    table
        .add(LeaseAsk {
            owner: "Alice".to_string(),
            ttl_secs: 60,
        })
        .unwrap();
    table
        .add(LeaseAsk {
            owner: "bob".to_string(),
            ttl_secs: 30,
        })
        .unwrap();
    assert_eq!(table.count(), 2);
    let owners: Vec<&str> = table.list().iter().map(|l| l.owner.as_str()).collect();
    assert_eq!(owners, vec!["alice", "bob"]);
}

#[test]
fn lease_t02_empty_rejected() {
    let mut table = LeaseTable::new();
    for bad in ["", "   ", "\t\n "] {
        let err = table
            .add(LeaseAsk {
                owner: bad.to_string(),
                ttl_secs: 10,
            })
            .unwrap_err();
        assert!(matches!(err, LeaseError::EmptyOwner));
    }
    assert_eq!(table.count(), 0);
}

#[test]
fn lease_t03_zero_ttl() {
    let mut table = LeaseTable::new();
    let err = table
        .add(LeaseAsk {
            owner: "alice".to_string(),
            ttl_secs: 0,
        })
        .unwrap_err();
    assert!(matches!(err, LeaseError::ZeroTtl));
    assert_eq!(table.count(), 0);
}

#[test]
fn lease_t04_normalize() {
    assert_eq!(normalize_owner("  Alice ").unwrap(), "alice");
    assert_eq!(normalize_owner("BOB").unwrap(), "bob");
    assert!(matches!(normalize_owner(""), Err(LeaseError::EmptyOwner)));
    assert!(matches!(
        normalize_owner("   "),
        Err(LeaseError::EmptyOwner)
    ));
}

#[test]
fn lease_t05_overflow() {
    let mut table = LeaseTable::new();
    for i in 0..MAX_LEASES {
        table
            .add(LeaseAsk {
                owner: format!("owner-{i}"),
                ttl_secs: 10,
            })
            .unwrap();
    }
    assert_eq!(table.count(), MAX_LEASES);
    let err = table
        .add(LeaseAsk {
            owner: "one-more".to_string(),
            ttl_secs: 10,
        })
        .unwrap_err();
    match err {
        LeaseError::TooManyLeases { max, actual } => {
            assert_eq!(max, MAX_LEASES);
            assert_eq!(actual, MAX_LEASES);
        }
        other => panic!("expected TooManyLeases, got {other:?}"),
    }
    assert_eq!(table.count(), MAX_LEASES);
}
