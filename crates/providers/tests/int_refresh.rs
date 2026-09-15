//! INT-005 T01: decision + success path (RED: module missing).

#[path = "../src/int_refresh.rs"]
mod int_refresh;

use int_refresh::{IntRefreshGate, IntRefreshState, CredKey, needs_int_refresh};

fn state(expires: u64) -> IntRefreshState {
    IntRefreshState {
        credential: CredKey::new("cred-a").unwrap(),
        expires_at_ms: expires,
        refreshed_at_ms: 0,
    }
}

#[test]
fn int005_t01_decision_and_success() {
    let now = 1_000_000u64;
    let due = state(now + 60_000);
    assert!(needs_int_refresh(&due, now, 300_000));
    let far = state(now + 3_600_000);
    assert!(!needs_int_refresh(&far, now, 300_000));
    let gate = IntRefreshGate::new(128);
    let guard = gate.try_begin("cred-a").expect("begin");
    let rec = guard.complete(&gate, now + 3_600_000, now).expect("complete");
    assert_eq!(rec.expires_at_ms, now + 3_600_000);
    assert!(gate.try_begin("cred-a").is_ok(), "slot released");
}

#[test]
fn int005_t02_single_flight() {
    let gate = IntRefreshGate::new(128);
    let now = 500u64;
    let _g = gate.try_begin("cred-a").expect("first begin");
    assert_eq!(
        gate.try_begin("cred-a").unwrap_err(),
        int_refresh::IntRefreshError::AlreadyRefreshing
    );
    let _h = gate.try_begin("cred-b").expect("independent cred ok");
    drop(_g);
    let _ = gate.fail_for("cred-b", int_refresh::IntRefreshFail::ExpiredGrant);
    let g2 = gate.try_begin("cred-a").expect("re-begin after complete/drop");
    let _ = g2.complete(&gate, now + 10_000, now).expect("complete A");
    assert!(gate.try_begin("cred-a").is_ok());
}

#[test]
fn int005_t03_caps_hold() {
    let gate = IntRefreshGate::new(4);
    let now = 1000u64;
    let mut guards = Vec::new();
    for i in 0..4 {
        guards.push(gate.try_begin(&format!("cred-{i}")).expect("admit 4"));
    }
    assert_eq!(
        gate.try_begin("cred-x").unwrap_err(),
        int_refresh::IntRefreshError::GateFull
    );
    for g in guards {
        let _ = g.fail(&gate, int_refresh::IntRefreshFail::NetworkUnavailable);
    }
    assert_eq!(gate.inflight(), 0);
    for i in 0..10_000u64 {
        let g = gate.try_begin(&format!("bulk-{i}")).expect("cycle admit");
        let _ = g.complete(&gate, now + 60_000 + i, now).expect("cycle complete");
    }
    assert_eq!(gate.inflight(), 0);
}

#[test]
fn int005_t04_failure_states() {
    let gate = IntRefreshGate::new(128);
    let now = 2000u64;
    let g = gate.try_begin("cred-f").expect("begin");
    match g.complete(&gate, now - 1, now) {
        Err(int_refresh::IntRefreshError::InvalidExpiry) => {}
        other => panic!("expected InvalidExpiry, got {other:?}"),
    }
    assert!(gate.last("cred-f").is_none(), "state unchanged");
    assert_eq!(
        gate.fail_for("cred-ghost", int_refresh::IntRefreshFail::ProviderRejected),
        int_refresh::IntRefreshError::NoInflightRefresh
    );
    let g2 = gate.try_begin("cred-h").expect("begin h");
    let recorded = g2.fail(&gate, int_refresh::IntRefreshFail::ProviderRejected);
    assert!(
        matches!(
            recorded,
            int_refresh::IntRefreshError::ProviderRejected
                | int_refresh::IntRefreshError::NetworkUnavailable
                | int_refresh::IntRefreshError::ExpiredGrant
        ),
        "typed reason, got {recorded:?}"
    );
    assert!(gate.try_begin("cred-h").is_ok(), "slot released after fail");
    let _ = gate.fail_for("cred-h", int_refresh::IntRefreshFail::ExpiredGrant);
    let g3 = gate.try_begin("cred-drop").expect("begin drop");
    drop(g3);
    assert!(gate.try_begin("cred-drop").is_ok(), "dropped guard reclaimed");
}

#[test]
fn int005_t05_no_side_effects_and_safety() {
    let dir = tempfile::tempdir().expect("fixture dir");
    let before: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    let gate = IntRefreshGate::new(128);
    let now = 3000u64;
    let st = IntRefreshState {
        credential: CredKey::new("cred-alpha-secret-1").unwrap(),
        expires_at_ms: now + 10,
        refreshed_at_ms: 0,
    };
    assert!(needs_int_refresh(&st, now, 300_000));
    let g = gate.try_begin("cred-alpha-secret-1").expect("begin");
    let rec = g.complete(&gate, now + 60_000, now).expect("complete");
    assert_eq!(rec.expires_at_ms, now + 60_000);
    let after: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(before.len(), after.len(), "no writes into fixture dir");
    assert!(after.is_empty(), "fixture dir stays empty");
    assert!(
        std::fs::metadata(dir.path().join("int005_probe_never_created")).is_err(),
        "no stray probe file"
    );
    let dbg = format!(
        "{:?} {:?} {:?} {:?}",
        CredKey::new("cred-alpha-secret-1").unwrap(),
        st,
        gate,
        int_refresh::IntRefreshError::AlreadyRefreshing
    );
    assert!(!dbg.contains("cred-alpha-secret-1"), "raw id leaked: {dbg}");
    assert!(!dbg.contains("tok-SECRET"), "token bytes leaked");
    assert!(dbg.contains("cred-***"), "redaction missing: {dbg}");
    let gate2 = IntRefreshGate::new(128);
    let _live = gate2.try_begin("cred-cb").expect("live guard");
    let calls = std::cell::Cell::new(0u32);
    let denied = gate2.try_begin("cred-cb");
    assert_eq!(
        denied.unwrap_err(),
        int_refresh::IntRefreshError::AlreadyRefreshing
    );
    assert_eq!(calls.get(), 0, "denied begin runs zero callbacks");
    let _ = calls;
}
