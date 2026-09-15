use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutoBudget {
    pub limit: u64,
    pub spent: u64,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BudgetError {
    #[error("budget limit must not be zero")]
    ZeroLimit,
    #[error("over budget: limit {limit}, spent {spent}")]
    OverBudget { limit: u64, spent: u64 },
}

pub fn check_budget(b: &AutoBudget) -> Result<u64, BudgetError> {
    if b.limit == 0 {
        return Err(BudgetError::ZeroLimit);
    }
    if b.spent > b.limit {
        return Err(BudgetError::OverBudget {
            limit: b.limit,
            spent: b.spent,
        });
    }
    Ok(b.limit - b.spent)
}

pub fn spend(b: &mut AutoBudget, amount: u64) -> Result<(), BudgetError> {
    if b.limit == 0 {
        return Err(BudgetError::ZeroLimit);
    }
    let next = b.spent.saturating_add(amount);
    if next > b.limit {
        return Err(BudgetError::OverBudget {
            limit: b.limit,
            spent: next,
        });
    }
    b.spent = next;
    Ok(())
}
