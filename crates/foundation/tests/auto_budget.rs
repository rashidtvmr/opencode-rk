use opencode_rk_foundation::auto_budget::{check_budget, spend, AutoBudget, BudgetError};

#[test]
fn ab_t01_remaining() {
    let b = AutoBudget { limit: 100, spent: 30 };
    assert_eq!(check_budget(&b), Ok(70));
}

#[test]
fn ab_t02_spend() {
    let mut b = AutoBudget { limit: 100, spent: 30 };
    assert_eq!(spend(&mut b, 20), Ok(()));
    assert_eq!(b.spent, 50);
    assert_eq!(check_budget(&b), Ok(50));
}

#[test]
fn ab_t03_over() {
    let mut b = AutoBudget { limit: 100, spent: 90 };
    assert_eq!(
        spend(&mut b, 20),
        Err(BudgetError::OverBudget { limit: 100, spent: 110 })
    );
    assert_eq!(b.spent, 90);
    let over = AutoBudget { limit: 100, spent: 150 };
    assert_eq!(
        check_budget(&over),
        Err(BudgetError::OverBudget { limit: 100, spent: 150 })
    );
}

#[test]
fn ab_t04_zero() {
    let b = AutoBudget { limit: 0, spent: 0 };
    assert_eq!(check_budget(&b), Err(BudgetError::ZeroLimit));
    let mut z = AutoBudget { limit: 0, spent: 0 };
    assert_eq!(spend(&mut z, 1), Err(BudgetError::ZeroLimit));
    assert_eq!(spend(&mut z, 0), Err(BudgetError::ZeroLimit));
}

#[test]
fn ab_t05_saturating() {
    let mut b = AutoBudget { limit: 100, spent: u64::MAX - 1 };
    assert_eq!(
        spend(&mut b, 20),
        Err(BudgetError::OverBudget { limit: 100, spent: u64::MAX })
    );
    assert_eq!(b.spent, u64::MAX - 1);
    let mut c = AutoBudget { limit: u64::MAX, spent: u64::MAX - 1 };
    assert_eq!(spend(&mut c, u64::MAX), Ok(()));
    assert_eq!(c.spent, u64::MAX);
    assert_eq!(check_budget(&c), Ok(0));
}
