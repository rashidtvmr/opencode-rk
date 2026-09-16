//! OPS-009 frozen tests: pure in-memory replay checker.
//! Single-file lane: includes the module under test via path so the
//! shared `lib.rs` stays untouched.

#[path = "../src/ops_replay.rs"]
mod ops_replay;

use ops_replay::{replay, Cassette, ReplayError, MAX_FRAMES, MAX_FRAME_BYTES};

fn three_frame() -> Cassette {
    let mut c = Cassette::new();
    c.push_frame("req-a", "res-a").unwrap();
    c.push_frame("req-b", "res-b").unwrap();
    c.push_frame("req-c", "res-c").unwrap();
    c
}

#[test]
fn ops009_t01_happy_path() {
    let c = three_frame();
    assert_eq!(c.len(), 3);
    assert!(!c.is_empty());
    let reqs = ["req-a", "req-b", "req-c"];
    assert_eq!(replay(&c, &reqs, false), Ok(3));
    assert_eq!(replay(&c, &reqs, true), Ok(3));
}

#[test]
fn ops009_t02_determinism_and_order() {
    let c = three_frame();
    let reqs = ["req-a", "req-b", "req-c"];
    assert_eq!(replay(&c, &reqs, false), replay(&c, &reqs, false));
    assert_eq!(replay(&c, &reqs, true), replay(&c, &reqs, true));
    let shuffled = ["req-a", "req-c", "req-b"];
    assert!(matches!(
        replay(&c, &shuffled, false),
        Err(ReplayError::Mismatch { index: 1, .. })
    ));
    let prefix = ["req-a", "req-b"];
    assert_eq!(replay(&c, &prefix, false), Ok(2));
    assert_eq!(
        replay(&c, &prefix, true),
        Err(ReplayError::UnusedFrames { remaining: 1 })
    );
}

#[test]
fn ops009_t03_caps_refuse() {
    let mut c = Cassette::new();
    for i in 0..MAX_FRAMES {
        c.push_frame(&format!("req-{i}"), &format!("res-{i}"))
            .unwrap();
    }
    assert_eq!(c.len(), MAX_FRAMES);
    assert_eq!(
        c.push_frame("req-overflow", "res-overflow"),
        Err(ReplayError::TooManyFrames {
            max: MAX_FRAMES,
            actual: MAX_FRAMES
        })
    );
    assert_eq!(c.len(), MAX_FRAMES);
    let big = "x".repeat(MAX_FRAME_BYTES + 1);
    let mut c2 = Cassette::new();
    assert!(matches!(
        c2.push_frame(&big, "res"),
        Err(ReplayError::TooManyFrames { .. })
    ));
    assert!(matches!(
        c2.push_frame("req", &big),
        Err(ReplayError::TooManyFrames { .. })
    ));
    assert!(c2.is_empty());
    let reqs: Vec<String> = (0..MAX_FRAMES).map(|i| format!("req-{i}")).collect();
    let refs: Vec<&str> = reqs.iter().map(String::as_str).collect();
    assert_eq!(replay(&c, &refs, true), Ok(MAX_FRAMES));
}

#[test]
fn ops009_t04_failure_states() {
    let mut c = Cassette::new();
    assert_eq!(c.push_frame("", "res"), Err(ReplayError::EmptyRequest));
    assert!(c.is_empty());
    let c = three_frame();
    let past_end = ["req-a", "req-b", "req-c", "req-d"];
    assert_eq!(
        replay(&c, &past_end, false),
        Err(ReplayError::Exhausted { index: 3 })
    );
    let wrong = ["req-a", "WRONG", "req-c"];
    match replay(&c, &wrong, false) {
        Err(ReplayError::Mismatch {
            index,
            expected,
            actual,
        }) => {
            assert_eq!(index, 1);
            assert_eq!(expected, "req-b");
            assert_eq!(actual, "WRONG");
        }
        other => panic!("expected mismatch, got {other:?}"),
    }
    let before = c.clone();
    let _ = replay(&c, &wrong, true);
    let _ = replay(&c, &past_end, true);
    assert_eq!(c, before);
}

#[test]
fn ops009_t05_redaction_and_isolation() {
    let decoy = "decoy-sk-live-9f8e7d6c5b4a3939";
    let frame0_req = format!("login {decoy}");
    let mut c = Cassette::new();
    c.push_frame(&frame0_req, "ok").unwrap();
    c.push_frame("req-b", "res-b").unwrap();
    let wrong_actual = "probe-with-decoy-sk-live-0000000000";
    let reqs = [frame0_req.as_str(), wrong_actual];
    let err = replay(&c, &reqs, false).unwrap_err();
    let dbg = format!("{err:?}");
    assert!(!dbg.contains(decoy), "debug leaks decoy: {dbg}");
    assert!(!dbg.contains(wrong_actual), "debug leaks actual: {dbg}");
    assert!(!dbg.contains("req-b"), "debug leaks expected: {dbg}");
    assert!(!dbg.contains("login"), "debug leaks frame body: {dbg}");
    assert!(dbg.contains('1'), "debug missing index: {dbg}");
    assert!(
        dbg.contains(&"req-b".len().to_string()),
        "debug missing expected len: {dbg}"
    );
    assert!(
        dbg.contains(&wrong_actual.len().to_string()),
        "debug missing actual len: {dbg}"
    );
    let disp = format!("{err}");
    assert!(!disp.contains(decoy), "display leaks decoy: {disp}");
    assert!(!disp.contains(wrong_actual), "display leaks actual: {disp}");
}
