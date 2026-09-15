//! AUTO steps planner: name list -> ordered steps.
use std::collections::HashSet;

use thiserror::Error;

/// Maximum number of steps accepted by [`plan_steps`].
pub const MAX_STEPS: usize = 128;

/// A planned automation step.
#[derive(Debug, PartialEq, Eq)]
pub struct AutoStep {
    /// Step name as given.
    pub name: String,
    /// Positional index of the step.
    pub order: u32,
}

/// Step planning failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StepsError {
    /// A step name is empty.
    #[error("empty step name")]
    EmptyName,
    /// A step name appears more than once.
    #[error("duplicate step name: {name}")]
    DuplicateName {
        /// The duplicated name.
        name: String,
    },
    /// The step list exceeds [`MAX_STEPS`].
    #[error("too many steps: max {max}, got {actual}")]
    TooManySteps { max: usize, actual: usize },
}

/// Validate step names into ordered [`AutoStep`]s.
///
/// Length is checked first; entries validate in order: empty name, then
/// duplicate name. Order is the positional index; input order is preserved.
pub fn plan_steps(names: &[&str]) -> Result<Vec<AutoStep>, StepsError> {
    if names.len() > MAX_STEPS {
        return Err(StepsError::TooManySteps {
            max: MAX_STEPS,
            actual: names.len(),
        });
    }
    let mut seen = HashSet::with_capacity(names.len());
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        if name.is_empty() {
            return Err(StepsError::EmptyName);
        }
        if !seen.insert(*name) {
            return Err(StepsError::DuplicateName {
                name: (*name).to_string(),
            });
        }
        out.push(AutoStep {
            name: (*name).to_string(),
            order: i as u32,
        });
    }
    Ok(out)
}
