//! INT-005 frozen tests: refresh decision + single-flight gate (T01..T05).
//!
//! The implementation under test is included by path so this lane never edits
//! the shared `crates/providers/src/lib.rs` (integrator-owned).

#[path = "../src/refresh_gate.rs"]
mod refresh_gate;

use refresh_gate::{
    CredId, MAX_INFLIGHT, REFRESH_WINDOW_MS, RefreshError, RefreshFail, RefreshGate,
    RefreshGuard, RefreshState, needs_refresh,
};

const NOW: u64 = 1_700_000_000_000;

fn state(id: &str, expires_at_ms: u64) -> RefreshState {
    RefreshState {
        credential_id: CredId::new(id).expect("valid test cred"),
        expires_at_ms,
        refreshed_at_ms: NOW - 3_600_000,
    }
}

#[test]
fn int_005_t01_decision_and_success_path() {
    assert_eq!(REFRESH_WINDOW_MS, 300_000);
    assert_eq!(MAX_INFLIGHT, 128);
    let gate = RefreshGate::new(MAX_INFLIGHT);

    let due = state("cred-a", NOW + 60_000);
    assert!(needs_refresh(&due, NOW, 300_000));
    assert!(needs_refresh(&due, NOW, REFRESH_WINDOW_MS));
    // Exact boundary is due: now + window == expiry.
    let edge = state("cred-a", NOW + 300_000);
    assert!(needs_refresh(&edge, NOW, 300_000));

    let far = state("cred-a", NOW + 3_600_000);
    assert!(!needs_refresh(&far, NOW, 300_000));

    let guard = gate.try_begin("cred-a").expect("begin cred-a");
    let new_expiry = NOW + 3_600_000;
    let recorded = guard
        .complete(&gate, new_expiry, NOW)
        .expect("complete cred-a");
    assert_eq!(recorded.expires_at_ms, new_expiry);
    assert_eq!(recorded.credential_id.as_str(), "cred-a");
    assert_eq!(recorded.refreshed_at_ms, NOW);
    assert_eq!(gate.inflight(), 0);

    // Slot released: begin is Ok again.
    let again = gate.try_begin("cred-a").expect("re-begin after complete");
    again
        .complete(&gate, NOW + 7_200_000, NOW)
        .expect("second complete");
    assert_eq!(gate.inflight(), 0);
}

#[test]
fn int_005_t02_single_flight_coalescing() {
    let gate = RefreshGate::default();
    let guard_a = gate.try_begin("cred-a").expect("first begin A");
    assert_eq!(
        gate.try_begin("cred-a").expect_err("second begin A"),
        RefreshError::AlreadyRefreshing
    );
    let guard_b = gate.try_begin("cred-b").expect("independent begin B");
    assert_eq!(gate.inflight(), 2);

    guard_a
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete A");
    assert_eq!(gate.inflight(), 1);
    let guard_a2 = gate.try_begin("cred-a").expect("begin A again");
    guard_a2
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete A again");
    guard_b
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete B");
    assert_eq!(gate.inflight(), 0);
}

#[test]
fn int_005_t03_caps_hold() {
    assert_eq!(RefreshGate::default().max_inflight(), MAX_INFLIGHT);
    let gate = RefreshGate::new(4);
    assert_eq!(gate.max_inflight(), 4);
    let mut guards: Vec<RefreshGuard> = Vec::new();
    for i in 0..4 {
        guards.push(
            gate.try_begin(&format!("cred-{i}"))
                .expect("fill to cap"),
        );
    }
    assert_eq!(gate.inflight(), 4);
    assert_eq!(
        gate.try_begin("cred-4").expect_err("5th over cap"),
        RefreshError::GateFull
    );
    assert_eq!(gate.inflight(), 4);
    for (i, guard) in guards.into_iter().enumerate() {
        let recorded = guard
            .complete(&gate, NOW + 3_600_000 + i as u64, NOW)
            .expect("release");
        assert_eq!(recorded.expires_at_ms, NOW + 3_600_000 + i as u64);
    }
    assert_eq!(gate.inflight(), 0);

    // Fill/release cycle over 10k distinct creds: no leaked slots, no OOM.
    let big = RefreshGate::default();
    for i in 0..10_000 {
        let id = format!("cred-{i:05}");
        let guard = big.try_begin(&id).expect("cycle begin");
        guard
            .complete(&big, NOW + 3_600_000, NOW)
            .expect("cycle complete");
    }
    assert_eq!(big.inflight(), 0);
}

#[test]
fn int_005_t04_failure_states() {
    let gate = RefreshGate::default();

    // Invalid credential ids never admitted.
    assert_eq!(
        gate.try_begin("").expect_err("empty cred"),
        RefreshError::InvalidCred
    );
    assert_eq!(
        gate.try_begin(&"c".repeat(129)).expect_err("oversize cred"),
        RefreshError::InvalidCred
    );
    assert_eq!(gate.inflight(), 0);

    // Complete with past expiry: InvalidExpiry, recorded state unchanged,
    // slot released (no leak).
    assert!(gate.last("cred-a").is_none());
    let guard = gate.try_begin("cred-a").expect("begin cred-a");
    assert_eq!(
        guard
            .complete(&gate, NOW - 1, NOW)
            .expect_err("past expiry"),
        RefreshError::InvalidExpiry
    );
    assert!(gate.last("cred-a").is_none());
    // Equal expiry (new_expiry <= now) is also invalid.
    let guard = gate.try_begin("cred-a").expect("begin cred-a again");
    assert_eq!(
        guard
            .complete(&gate, NOW, NOW)
            .expect_err("equal expiry"),
        RefreshError::InvalidExpiry
    );
    assert!(gate.last("cred-a").is_none());
    assert_eq!(gate.inflight(), 0);

    // Complete/fail without a live guard: typed error, never a panic.
    assert_eq!(
        gate.complete_for("cred-never", NOW + 1_000, NOW)
            .expect_err("complete without guard"),
        RefreshError::NoInflightRefresh
    );
    assert_eq!(
        gate.fail_for("cred-never", RefreshFail::ProviderRejected),
        RefreshError::NoInflightRefresh
    );
    // Guard from another gate is unknown here.
    let other = RefreshGate::default();
    let foreign = other.try_begin("cred-x").expect("foreign begin");
    assert_eq!(
        foreign
            .complete(&gate, NOW + 1_000, NOW)
            .expect_err("foreign guard"),
        RefreshError::NoInflightRefresh
    );

    // Fail with a live guard records each typed reason and releases the slot.
    for reason in [
        RefreshFail::ProviderRejected,
        RefreshFail::NetworkUnavailable,
        RefreshFail::ExpiredGrant,
    ] {
        let guard = gate.try_begin("cred-f").expect("begin cred-f");
        let recorded = guard.fail(&gate, reason);
        assert!(
            matches!(
                recorded,
                RefreshError::ProviderRejected
                    | RefreshError::NetworkUnavailable
                    | RefreshError::ExpiredGrant
            ),
            "typed failure, got {recorded:?}"
        );
        assert_eq!(recorded, reason.as_error());
        assert_eq!(gate.inflight(), 0);
    }

    // Dropped guard without completion reclaims the slot.
    {
        let _dangling = gate.try_begin("cred-drop").expect("begin cred-drop");
    }
    let reclaimed = gate.try_begin("cred-drop").expect("reclaimed after drop");
    reclaimed
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete reclaimed");
    assert_eq!(gate.inflight(), 0);
}

#[test]
fn int_005_t05_no_side_effects_safety() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let snapshot = || -> Vec<String> {
        std::fs::read_dir(dir.path())
            .expect("read fixture dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    };
    let before = snapshot();

    let secret_id = "secret-cred-9-TOKEN-abc123";
    let token_bytes = "tok-secret-body-xyz";
    let gate = RefreshGate::default();
    let mut callbacks = 0u32;

    // Denied double-begin executes zero provider callbacks.
    let guard = gate.try_begin(secret_id).expect("first begin");
    match gate.try_begin(secret_id) {
        Err(e) => assert_eq!(e, RefreshError::AlreadyRefreshing),
        Ok(_) => {
            callbacks += 1;
            panic!("denied begin must not admit");
        }
    }
    assert_eq!(callbacks, 0);

    // Independent credential still admitted (exactly one callback).
    let other = gate.try_begin("other-cred").expect("independent begin");
    callbacks += 1;
    assert_eq!(callbacks, 1);

    // Captured logs carry zero token bytes and zero raw credential ids;
    // every id renders as the fixed redaction.
    let mut logs = String::new();
    logs.push_str(&format!("{:?}", gate));
    logs.push_str(&format!("{:?}", guard));
    logs.push_str(&format!("{:?}", other));
    logs.push_str(&format!("{:?}", state(secret_id, NOW + 60_000)));
    logs.push_str(&format!("{:?}", RefreshError::AlreadyRefreshing));
    assert!(!logs.contains(secret_id), "raw cred id leaked: {logs}");
    assert!(!logs.contains("other-cred"), "raw cred id leaked: {logs}");
    assert!(!logs.contains(token_bytes), "token bytes leaked");
    assert!(logs.contains("cred-***"), "missing fixed redaction: {logs}");

    guard
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete secret");
    other
        .complete(&gate, NOW + 3_600_000, NOW)
        .expect("complete other");
    assert_eq!(gate.inflight(), 0);

    // No I/O: fixture dir unchanged, no DB files.
    assert_eq!(snapshot(), before);
    assert!(std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .all(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            !n.ends_with(".db") && !n.ends_with(".sqlite")
        }));
}
