//! OPS-007 frozen tests T01..T05. Self-contained via #[path] include;
//! integrator wires `pub mod ops_budget` into lib.rs later.
#[path = "../src/ops_budget.rs"]
mod ops_budget;

use ops_budget::{Admission, BudgetError, OpsBudget};

#[test]
fn ops_budget_t01_defaults_happy_path() {
    let b = OpsBudget::default();
    assert_eq!(b.max_live_tasks, 4);
    assert_eq!(b.max_queued, 256);
    assert_eq!(b.max_input_bytes, 1_048_576);
    assert_eq!(b.max_preview_bytes, 65_536);
    assert!(b.validate().is_ok());
    assert!(b.admit(0, 0, 0).is_ok());
    assert!(b.admit(3, 255, 1_048_576).is_ok());
    assert_eq!(b.reserve_preview(65_536), Ok(65_536));
}

#[test]
fn ops_budget_t02_deterministic() {
    let b = OpsBudget::default();
    let first = b.admit(2, 100, 512);
    for _ in 0..16 {
        assert_eq!(b.admit(2, 100, 512), first);
    }
    let p_first = b.reserve_preview(1024);
    for _ in 0..16 {
        assert_eq!(b.reserve_preview(1024), p_first);
    }
    // Verdict depends only on inputs: a fresh budget agrees, order irrelevant.
    let fresh = OpsBudget::default();
    assert_eq!(fresh.reserve_preview(1024), p_first);
    assert_eq!(fresh.admit(2, 100, 512), first);
    assert_ne!(b.admit(2, 100, 512), b.admit(4, 100, 512));
}

#[test]
fn ops_budget_t03_caps_refuse() {
    let b = OpsBudget::default();
    assert_eq!(
        b.admit(4, 0, 0),
        Err(BudgetError::OverCap {
            requested: 5,
            available: 4
        })
    );
    assert_eq!(
        b.admit(0, 256, 0),
        Err(BudgetError::OverCap {
            requested: 257,
            available: 256
        })
    );
    assert_eq!(
        b.admit(0, 0, 1_048_577),
        Err(BudgetError::OverCap {
            requested: 1_048_577,
            available: 1_048_576
        })
    );
    assert_eq!(
        b.reserve_preview(65_537),
        Err(BudgetError::OverCap {
            requested: 65_537,
            available: 65_536
        })
    );
}

#[test]
fn ops_budget_t04_invalid_limits() {
    let base = OpsBudget::default();
    let cases: [(OpsBudget, &'static str); 4] = [
        (
            OpsBudget {
                max_live_tasks: 0,
                ..base
            },
            "max_live_tasks",
        ),
        (
            OpsBudget {
                max_queued: 0,
                ..base
            },
            "max_queued",
        ),
        (
            OpsBudget {
                max_input_bytes: 0,
                ..base
            },
            "max_input_bytes",
        ),
        (
            OpsBudget {
                max_preview_bytes: 0,
                ..base
            },
            "max_preview_bytes",
        ),
    ];
    for (bad, name) in cases {
        assert_eq!(bad.validate(), Err(BudgetError::InvalidLimit(name)));
        // Policy entry points surface the same verdict, never admit.
        assert_eq!(bad.admit(0, 0, 0), Err(BudgetError::InvalidLimit(name)));
        assert_eq!(bad.reserve_preview(0), Err(BudgetError::InvalidLimit(name)));
    }
    // u64::MAX byte request: OverCap, never admit, never panic.
    let b = OpsBudget::default();
    assert!(matches!(
        b.admit(0, 0, u64::MAX),
        Err(BudgetError::OverCap { .. })
    ));
    assert!(matches!(
        b.reserve_preview(u64::MAX),
        Err(BudgetError::OverCap { .. })
    ));
}

#[test]
fn ops_budget_t05_no_side_effects_purity() {
    let b = OpsBudget::default();
    // Caller-owned counters are byte-identical before/after a refusal.
    let (live, queued, bytes) = (4u32, 10u32, 128u64);
    let before = (live, queued, bytes);
    let err = b.admit(live, queued, bytes).unwrap_err();
    assert_eq!((live, queued, bytes), before);
    // Error carries counters only: no paths, bodies, or byte content.
    let rendered = format!("{err}");
    assert!(!rendered.contains('/'));
    assert!(!rendered.contains('\n'));
    // Success echoes inputs without mutation or retention.
    let admission: Admission = b.admit(1, 2, 3).unwrap();
    assert_eq!(
        admission,
        Admission {
            live_tasks: 1,
            queued: 2,
            reserved_bytes: 3,
        }
    );
    assert_eq!((live, queued, bytes), before);
}
