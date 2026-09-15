use opencode_rk_foundation::ops_runtime::{next_tick, ticks_since, RuntimeError2, RuntimeTick};

#[test]
fn oprt_t01_next() {
    let t = RuntimeTick { n: 41 };
    let nxt = next_tick(&t).unwrap();
    assert_eq!(nxt.n, 42);
}

#[test]
fn oprt_t02_overflow() {
    let t = RuntimeTick { n: u64::MAX };
    assert!(matches!(next_tick(&t), Err(RuntimeError2::Overflow)));
}

#[test]
fn oprt_t03_since() {
    assert_eq!(ticks_since(3, 10), 7);
}

#[test]
fn oprt_t04_since_saturates() {
    assert_eq!(ticks_since(10, 3), 0);
}

#[test]
fn oprt_t05_zero() {
    assert_eq!(ticks_since(0, 0), 0);
    let t = RuntimeTick { n: 0 };
    assert_eq!(next_tick(&t).unwrap().n, 1);
}
